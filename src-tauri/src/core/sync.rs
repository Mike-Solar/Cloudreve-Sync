use crate::core::cloudreve::{CloudreveClient, MetadataPatch, RemoteFile};
use crate::core::config::ApiPaths;
use crate::core::db::{
    insert_conflict, insert_tombstone, list_entries_by_task, list_tombstones, now_ms, upsert_entry,
    ConflictRow, EntryRow, TaskRow, TombstoneRow,
};
use crate::core::error::CloudreveError;
use crate::core::logging::{LogEntry, LogLevel, LogStore};
use chrono::{DateTime, Local, Utc};
use filetime::FileTime;
use rayon::prelude::*;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

const META_DEVICE_ID: &str = "customize:sync_device_id";
const META_MTIME: &str = "customize:sync_mtime_ms";
const META_SHA256: &str = "customize:sync_sha256";
const META_DELETED_AT: &str = "customize:sync_deleted_at_ms";
const META_CONFLICT_OF: &str = "customize:sync_conflict_of";
const META_CONFLICT_TS: &str = "customize:sync_conflict_ts";

#[derive(Debug, Clone)]
pub struct LocalFileInfo {
    pub relpath: String,
    pub abs_path: PathBuf,
    pub size: u64,
    pub mtime_ms: i64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct RemoteFileInfo {
    pub file_id: String,
    pub uri: String,
    pub relpath: String,
    pub size: u64,
    pub mtime_ms: i64,
    pub sha256: String,
    pub deleted_at_ms: Option<i64>,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone)]
pub struct SyncEngine {
    task: TaskRow,
    client: CloudreveClient,
    db_path: PathBuf,
    log_store: LogStore,
    progress_notifier: Option<Arc<dyn Fn(SyncStats) + Send + Sync>>,
    status_notifier: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub uploaded_bytes: u64,
    pub downloaded_bytes: u64,
    pub operations: u32,
}

impl SyncEngine {
    pub fn new(
        task: TaskRow,
        api_paths: ApiPaths,
        access_token: Option<String>,
        db_path: PathBuf,
        progress_notifier: Option<Arc<dyn Fn(SyncStats) + Send + Sync>>,
        status_notifier: Option<Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Self {
        let client = CloudreveClient::new(task.base_url.clone(), access_token, api_paths);
        let log_store = LogStore::new(db_path.clone());
        Self {
            task,
            client,
            db_path,
            log_store,
            progress_notifier,
            status_notifier,
        }
    }

    pub async fn sync_once(&self) -> Result<SyncStats, Box<dyn Error>> {
        let mut conn = Connection::open(&self.db_path)?;
        let mut stats = SyncStats::default();
        let entries = list_entries_by_task(&conn, &self.task.task_id)?;
        let tombstones = list_tombstones(&conn, &self.task.task_id)?;

        self.notify_status("Hashing");
        let local_files = scan_local(&self.task.local_root)?;
        self.notify_status("ListingRemote");
        let remote_files = self
            .client
            .list_all_files(&self.task.remote_root_uri)
            .await?;
        self.notify_status("Syncing");
        let local_map = to_local_map(local_files);
        let remote_map = to_remote_map(remote_files, &self.task.remote_root_uri)?;
        let entry_map = entries
            .into_iter()
            .map(|entry| (entry.local_relpath.clone(), entry))
            .collect::<HashMap<_, _>>();
        let tombstone_map = tombstones
            .into_iter()
            .map(|item| (item.local_relpath.clone(), item))
            .collect::<HashMap<_, _>>();

        let mut all_paths = Vec::new();
        all_paths.extend(local_map.keys().cloned());
        all_paths.extend(remote_map.keys().cloned());
        all_paths.extend(entry_map.keys().cloned());
        all_paths.sort();
        all_paths.dedup();

        for relpath in all_paths {
            let relpath_for_log = relpath.clone();
            let local = local_map.get(&relpath);
            let remote = remote_map.get(&relpath);
            let entry = entry_map.get(&relpath);
            let tombstone = tombstone_map.get(&relpath);
            let result: Result<(), Box<dyn Error>> = async {
                if let Some(remote) = remote {
                    if remote.deleted_at_ms.is_some() {
                        if let Some(local) = local {
                            remove_local_file(local)?;
                            self.log_db(
                                &mut conn,
                                LogLevel::Warn,
                                "delete",
                                &format!("本地删除: {} (远端标记删除)", local.relpath),
                            )?;
                        }
                        if tombstone.is_none() {
                            insert_tombstone(
                                &conn,
                                &TombstoneRow {
                                    task_id: self.task.task_id.clone(),
                                    cloud_file_id: remote.file_id.clone(),
                                    local_relpath: relpath.clone(),
                                    deleted_at_ms: remote.deleted_at_ms.unwrap_or_else(now_ms),
                                    origin: "remote".to_string(),
                                },
                            )?;
                        }
                        return Ok(());
                    }
                }

                if local.is_none() && entry.is_some() && tombstone.is_none() {
                    if let Some(remote) = remote {
                        let deleted_at = now_ms();
                        self.set_remote_deleted(&remote.uri, deleted_at).await?;
                        insert_tombstone(
                            &conn,
                            &TombstoneRow {
                                task_id: self.task.task_id.clone(),
                                cloud_file_id: remote.file_id.clone(),
                                local_relpath: relpath.clone(),
                                deleted_at_ms: deleted_at,
                                origin: "local".to_string(),
                            },
                        )?;
                        self.log_db(
                            &mut conn,
                            LogLevel::Warn,
                            "delete",
                            &format!("远端标记删除: {}", relpath),
                        )?;
                    }
                    return Ok(());
                }

                match (local, remote) {
                    (Some(local), Some(remote)) => {
                        let local_changed = entry
                            .map(|e| {
                                e.last_local_sha256 != local.sha256
                                    || e.last_local_mtime_ms != local.mtime_ms
                            })
                            .unwrap_or(true);
                        let remote_changed = entry
                            .map(|e| {
                                e.last_remote_sha256 != remote.sha256
                                    || e.last_remote_mtime_ms != remote.mtime_ms
                            })
                            .unwrap_or(true);

                        if entry.is_some()
                            && local_changed
                            && remote_changed
                            && local.sha256 != remote.sha256
                        {
                            self.handle_conflict(&mut conn, local, remote).await?;
                            return Ok(());
                        }

                        let prefer_local = local_changed
                            && (!remote_changed
                                || entry.is_none()
                                || local.mtime_ms >= remote.mtime_ms);
                        if prefer_local {
                            self.upload_local(&mut conn, local, remote, &mut stats)
                                .await?;
                        } else if remote_changed {
                            self.download_remote(&mut conn, local, remote, &mut stats)
                                .await?;
                        }
                    }
                    (Some(local), None) => {
                        self.upload_new_local(&mut conn, local, &mut stats).await?;
                    }
                    (None, Some(remote)) => {
                        self.download_new_remote(&mut conn, remote, &mut stats)
                            .await?;
                    }
                    (None, None) => {}
                }
                Ok(())
            }
            .await;

            if let Err(err) = result {
                self.log_db(
                    &mut conn,
                    LogLevel::Error,
                    "sync",
                    &format!("文件同步失败: {} ({})", relpath_for_log, err),
                )?;
            }
        }

        Ok(stats)
    }

    async fn upload_new_local(
        &self,
        conn: &mut Connection,
        local: &LocalFileInfo,
        stats: &mut SyncStats,
    ) -> Result<(), Box<dyn Error>> {
        let uri = build_remote_uri(&self.task.remote_root_uri, &local.relpath);
        let content = fs::read(&local.abs_path)?;
        self.upload_content(&uri, &content, &local.relpath, Some(stats))
            .await?;
        self.patch_sync_metadata(&uri, local, None).await?;
        upsert_entry(
            conn,
            &EntryRow {
                task_id: self.task.task_id.clone(),
                local_relpath: local.relpath.clone(),
                cloud_file_id: "".to_string(),
                cloud_uri: uri.clone(),
                last_local_mtime_ms: local.mtime_ms,
                last_local_sha256: local.sha256.clone(),
                last_remote_mtime_ms: local.mtime_ms,
                last_remote_sha256: local.sha256.clone(),
                last_sync_ts_ms: now_ms(),
                state: "ok".to_string(),
            },
        )?;
        self.log_db(
            conn,
            LogLevel::Info,
            "upload",
            &format!("上传新文件: {}", local.relpath),
        )?;
        Ok(())
    }

    async fn upload_local(
        &self,
        conn: &mut Connection,
        local: &LocalFileInfo,
        remote: &RemoteFileInfo,
        stats: &mut SyncStats,
    ) -> Result<(), Box<dyn Error>> {
        let content = fs::read(&local.abs_path)?;
        self.upload_content(&remote.uri, &content, &local.relpath, Some(stats))
            .await?;
        self.patch_sync_metadata(&remote.uri, local, Some(remote))
            .await?;
        upsert_entry(
            conn,
            &EntryRow {
                task_id: self.task.task_id.clone(),
                local_relpath: local.relpath.clone(),
                cloud_file_id: remote.file_id.clone(),
                cloud_uri: remote.uri.clone(),
                last_local_mtime_ms: local.mtime_ms,
                last_local_sha256: local.sha256.clone(),
                last_remote_mtime_ms: local.mtime_ms,
                last_remote_sha256: local.sha256.clone(),
                last_sync_ts_ms: now_ms(),
                state: "ok".to_string(),
            },
        )?;
        self.log_db(
            conn,
            LogLevel::Info,
            "upload",
            &format!("上传更新: {}", local.relpath),
        )?;
        Ok(())
    }

    async fn download_new_remote(
        &self,
        conn: &mut Connection,
        remote: &RemoteFileInfo,
        stats: &mut SyncStats,
    ) -> Result<(), Box<dyn Error>> {
        let target = Path::new(&self.task.local_root).join(&remote.relpath);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = self
            .client
            .download_file(&remote.uri)
            .await
            .map_err(|err| format!("下载失败: {} ({})", remote.relpath, err))?;
        fs::write(&target, &bytes)?;
        set_local_mtime(&target, remote.mtime_ms)?;
        upsert_entry(
            conn,
            &EntryRow {
                task_id: self.task.task_id.clone(),
                local_relpath: remote.relpath.clone(),
                cloud_file_id: remote.file_id.clone(),
                cloud_uri: remote.uri.clone(),
                last_local_mtime_ms: remote.mtime_ms,
                last_local_sha256: remote.sha256.clone(),
                last_remote_mtime_ms: remote.mtime_ms,
                last_remote_sha256: remote.sha256.clone(),
                last_sync_ts_ms: now_ms(),
                state: "ok".to_string(),
            },
        )?;
        self.log_db(
            conn,
            LogLevel::Info,
            "download",
            &format!("下载新文件: {}", remote.relpath),
        )?;
        stats.downloaded_bytes = stats.downloaded_bytes.saturating_add(bytes.len() as u64);
        stats.operations = stats.operations.saturating_add(1);
        self.notify_progress(stats);
        Ok(())
    }

    async fn download_remote(
        &self,
        conn: &mut Connection,
        local: &LocalFileInfo,
        remote: &RemoteFileInfo,
        stats: &mut SyncStats,
    ) -> Result<(), Box<dyn Error>> {
        let bytes = self
            .client
            .download_file(&remote.uri)
            .await
            .map_err(|err| format!("下载失败: {} ({})", local.relpath, err))?;
        fs::write(&local.abs_path, &bytes)?;
        set_local_mtime(&local.abs_path, remote.mtime_ms)?;
        upsert_entry(
            conn,
            &EntryRow {
                task_id: self.task.task_id.clone(),
                local_relpath: local.relpath.clone(),
                cloud_file_id: remote.file_id.clone(),
                cloud_uri: remote.uri.clone(),
                last_local_mtime_ms: remote.mtime_ms,
                last_local_sha256: remote.sha256.clone(),
                last_remote_mtime_ms: remote.mtime_ms,
                last_remote_sha256: remote.sha256.clone(),
                last_sync_ts_ms: now_ms(),
                state: "ok".to_string(),
            },
        )?;
        self.log_db(
            conn,
            LogLevel::Info,
            "download",
            &format!("下载更新: {}", local.relpath),
        )?;
        stats.downloaded_bytes = stats.downloaded_bytes.saturating_add(bytes.len() as u64);
        stats.operations = stats.operations.saturating_add(1);
        self.notify_progress(stats);
        Ok(())
    }

    async fn handle_conflict(
        &self,
        conn: &mut Connection,
        local: &LocalFileInfo,
        remote: &RemoteFileInfo,
    ) -> Result<(), Box<dyn Error>> {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let conflict_name = format!(
            "{} (conflict-{}-{})",
            file_stem(&local.relpath),
            self.task.device_id,
            timestamp
        );
        let conflict_relpath = match file_extension(&local.relpath) {
            Some(ext) => format!("{}.{}", conflict_name, ext),
            None => conflict_name,
        };
        let conflict_abs = Path::new(&self.task.local_root).join(&conflict_relpath);
        if let Some(parent) = conflict_abs.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&local.abs_path, &conflict_abs)?;

        let conflict_uri = build_remote_uri(&self.task.remote_root_uri, &conflict_relpath);
        self.upload_content(
            &conflict_uri,
            &fs::read(&conflict_abs)?,
            &conflict_relpath,
            None,
        )
        .await?;
        self.patch_conflict_metadata(&conflict_uri, local, remote)
            .await?;

        insert_conflict(
            conn,
            &ConflictRow {
                task_id: self.task.task_id.clone(),
                original_relpath: local.relpath.clone(),
                conflict_relpath: conflict_relpath.clone(),
                created_at_ms: now_ms(),
                reason: "both_modified".to_string(),
            },
        )?;

        self.log_db(
            conn,
            LogLevel::Warn,
            "conflict",
            &format!("冲突生成: {} -> {}", local.relpath, conflict_relpath),
        )?;
        Ok(())
    }

    async fn set_remote_deleted(
        &self,
        uri: &str,
        deleted_at_ms: i64,
    ) -> Result<(), Box<dyn Error>> {
        let patches = vec![MetadataPatch {
            key: META_DELETED_AT.to_string(),
            value: Some(deleted_at_ms.to_string()),
            remove: Some(false),
        }];
        self.client
            .patch_metadata(vec![uri.to_string()], patches)
            .await
    }

    async fn patch_sync_metadata(
        &self,
        uri: &str,
        local: &LocalFileInfo,
        remote: Option<&RemoteFileInfo>,
    ) -> Result<(), Box<dyn Error>> {
        let mut patches = vec![
            MetadataPatch {
                key: META_DEVICE_ID.to_string(),
                value: Some(self.task.device_id.clone()),
                remove: Some(false),
            },
            MetadataPatch {
                key: META_MTIME.to_string(),
                value: Some(local.mtime_ms.to_string()),
                remove: Some(false),
            },
            MetadataPatch {
                key: META_SHA256.to_string(),
                value: Some(local.sha256.clone()),
                remove: Some(false),
            },
        ];
        if remote.is_some() {
            patches.push(MetadataPatch {
                key: META_DELETED_AT.to_string(),
                value: None,
                remove: Some(true),
            });
        }
        self.client
            .patch_metadata(vec![uri.to_string()], patches)
            .await
    }

    async fn patch_conflict_metadata(
        &self,
        uri: &str,
        local: &LocalFileInfo,
        remote: &RemoteFileInfo,
    ) -> Result<(), Box<dyn Error>> {
        let patches = vec![
            MetadataPatch {
                key: META_DEVICE_ID.to_string(),
                value: Some(self.task.device_id.clone()),
                remove: Some(false),
            },
            MetadataPatch {
                key: META_MTIME.to_string(),
                value: Some(local.mtime_ms.to_string()),
                remove: Some(false),
            },
            MetadataPatch {
                key: META_SHA256.to_string(),
                value: Some(local.sha256.clone()),
                remove: Some(false),
            },
            MetadataPatch {
                key: META_CONFLICT_OF.to_string(),
                value: Some(remote.file_id.clone()),
                remove: Some(false),
            },
            MetadataPatch {
                key: META_CONFLICT_TS.to_string(),
                value: Some(now_ms().to_string()),
                remove: Some(false),
            },
        ];
        self.client
            .patch_metadata(vec![uri.to_string()], patches)
            .await
    }

    fn log_db(
        &self,
        conn: &mut Connection,
        level: LogLevel,
        event: &str,
        detail: &str,
    ) -> Result<(), Box<dyn Error>> {
        let entry = LogEntry::new(&self.task.task_id, level, event, detail);
        self.log_store.append(conn, &entry)?;
        Ok(())
    }

    fn notify_progress(&self, stats: &SyncStats) {
        if let Some(notifier) = &self.progress_notifier {
            notifier(stats.clone());
        }
    }

    fn notify_status(&self, status: &str) {
        if let Some(notifier) = &self.status_notifier {
            notifier(status.to_string());
        }
    }

    async fn upload_content(
        &self,
        uri: &str,
        content: &[u8],
        relpath: &str,
        stats: Option<&mut SyncStats>,
    ) -> Result<(), Box<dyn Error>> {
        let mut stats = stats;
        match self.client.update_file_content(uri, content).await {
            Ok(()) => {
                if let Some(stats) = stats.as_deref_mut() {
                    stats.uploaded_bytes =
                        stats.uploaded_bytes.saturating_add(content.len() as u64);
                    stats.operations = stats.operations.saturating_add(1);
                    self.notify_progress(stats);
                }
                Ok(())
            }
            Err(err) => {
                if is_file_too_large(&*err) {
                    self.upload_with_session(uri, content, stats.as_deref_mut())
                        .await
                        .map(|()| {
                            if let Some(stats) = stats.as_deref_mut() {
                                stats.operations = stats.operations.saturating_add(1);
                                self.notify_progress(stats);
                            }
                        })
                        .map_err(|upload_err| {
                            if is_file_too_large(&*upload_err) {
                                format!(
                                    "上传失败: {} (存储策略限制，文件过大: {})",
                                    relpath, upload_err
                                )
                                .into()
                            } else {
                                format!("上传失败: {} (分片上传失败: {})", relpath, upload_err)
                                    .into()
                            }
                        })
                } else {
                    Err(format!("上传失败: {} ({})", relpath, err).into())
                }
            }
        }
    }

    async fn upload_with_session(
        &self,
        uri: &str,
        content: &[u8],
        stats: Option<&mut SyncStats>,
    ) -> Result<(), Box<dyn Error>> {
        let mut stats = stats;
        let session = self
            .client
            .create_upload_session(uri, content.len() as u64, None, None, None)
            .await?;
        let chunk_size = if session.chunk_size > 0 {
            session.chunk_size as usize
        } else {
            content.len().max(1)
        };

        let mut index = 0u64;
        let mut offset = 0usize;
        while offset < content.len() {
            let end = (offset + chunk_size).min(content.len());
            let chunk = &content[offset..end];
            self.client
                .upload_chunk(&session.session_id, index, chunk)
                .await?;
            if let Some(stats) = stats.as_deref_mut() {
                stats.uploaded_bytes = stats.uploaded_bytes.saturating_add(chunk.len() as u64);
                self.notify_progress(stats);
            }
            offset = end;
            index += 1;
        }
        Ok(())
    }
}

fn scan_local(root: &str) -> Result<Vec<LocalFileInfo>, Box<dyn Error>> {
    #[derive(Debug, Clone)]
    struct LocalFileSeed {
        relpath: String,
        abs_path: PathBuf,
        size: u64,
        mtime_ms: i64,
    }

    let mut seeds = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let abs_path = entry.path().to_path_buf();
        let metadata = entry.metadata()?;
        let mtime_ms = metadata
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64;
        let relpath = abs_path
            .strip_prefix(root)
            .unwrap_or(&abs_path)
            .to_string_lossy()
            .trim_start_matches(std::path::MAIN_SEPARATOR)
            .replace(std::path::MAIN_SEPARATOR, "/");
        seeds.push(LocalFileSeed {
            relpath,
            abs_path,
            size: metadata.len(),
            mtime_ms,
        });
    }
    let hashed = seeds
        .into_par_iter()
        .map(|item| {
            hash_file(&item.abs_path)
                .map(|sha256| LocalFileInfo {
                    relpath: item.relpath,
                    abs_path: item.abs_path,
                    size: item.size,
                    mtime_ms: item.mtime_ms,
                    sha256,
                })
                .map_err(|err| err.to_string())
        })
        .collect::<Vec<_>>();
    let mut out = Vec::with_capacity(hashed.len());
    for result in hashed {
        let file = result.map_err(|err| -> Box<dyn Error> { err.into() })?;
        out.push(file);
    }
    Ok(out)
}

fn to_local_map(files: Vec<LocalFileInfo>) -> HashMap<String, LocalFileInfo> {
    files
        .into_iter()
        .map(|item| (item.relpath.clone(), item))
        .collect()
}

fn to_remote_map(
    files: Vec<RemoteFile>,
    remote_root_uri: &str,
) -> Result<HashMap<String, RemoteFileInfo>, Box<dyn Error>> {
    let root_path = uri_path(remote_root_uri);
    let mut out = HashMap::new();
    for file in files {
        if file.is_dir {
            continue;
        }
        let file_path = uri_path(&file.uri);
        let relpath = file_path
            .strip_prefix(&root_path)
            .unwrap_or(&file_path)
            .trim_start_matches('/')
            .to_string();
        if relpath.is_empty() {
            continue;
        }
        let sha256 = file.metadata.get(META_SHA256).cloned().unwrap_or_default();
        let mtime_ms = file
            .metadata
            .get(META_MTIME)
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or_else(|| parse_updated_at(&file.updated_at));
        let deleted_at_ms = file
            .metadata
            .get(META_DELETED_AT)
            .and_then(|v| v.parse::<i64>().ok());

        out.insert(
            relpath.clone(),
            RemoteFileInfo {
                file_id: file.id,
                uri: file.uri,
                relpath,
                size: file.size,
                mtime_ms,
                sha256,
                deleted_at_ms,
                metadata: file.metadata,
            },
        );
    }
    Ok(out)
}

fn uri_path(uri: &str) -> String {
    let cleaned = uri.split('?').next().unwrap_or(uri);
    let path = if let Some(pos) = cleaned.find("cloudreve://") {
        let rest = &cleaned[pos + "cloudreve://".len()..];
        if let Some(idx) = rest.find('/') {
            &rest[idx..]
        } else {
            ""
        }
    } else {
        cleaned
    };
    urlencoding::decode(path)
        .unwrap_or_else(|_| path.into())
        .to_string()
}

fn build_remote_uri(root_uri: &str, relpath: &str) -> String {
    let root = root_uri.trim_end_matches('/');
    let rel = relpath.trim_start_matches('/');
    format!("{}/{}", root, rel)
}

fn hash_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 512];
    loop {
        let count = std::io::Read::read(&mut file, &mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn set_local_mtime(path: &Path, mtime_ms: i64) -> Result<(), Box<dyn Error>> {
    let secs = mtime_ms / 1000;
    let nanos = ((mtime_ms % 1000) * 1_000_000) as u32;
    let mtime = FileTime::from_unix_time(secs, nanos);
    filetime::set_file_mtime(path, mtime)?;
    Ok(())
}

fn parse_updated_at(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis())
        .unwrap_or_else(|_| now_ms())
}

fn remove_local_file(local: &LocalFileInfo) -> Result<(), Box<dyn Error>> {
    if local.abs_path.exists() {
        fs::remove_file(&local.abs_path)?;
    }
    Ok(())
}

fn file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_string())
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(path)
        .to_string()
}

fn is_file_too_large(err: &(dyn Error + 'static)) -> bool {
    if let Some(value) = err.downcast_ref::<CloudreveError>() {
        return matches!(value, CloudreveError::FileTooLarge);
    }
    err.to_string().contains("40049")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::now_ms;
    use std::collections::HashSet;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn uri_path_strips_and_decodes() {
        let uri = "cloudreve://root/Work/a%20b.txt?download=1";
        let result = uri_path(uri);
        assert_eq!(result, "/Work/a b.txt");
    }

    #[test]
    fn build_remote_uri_keeps_plain_segments() {
        let root = "cloudreve://root/Work";
        let result = build_remote_uri(root, "a b/c.txt");
        assert_eq!(result, "cloudreve://root/Work/a b/c.txt");
    }

    #[test]
    fn hash_file_matches_sha256() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("hello.txt");
        let mut file = fs::File::create(&path).expect("create file");
        file.write_all(b"hello").expect("write");
        let result = hash_file(&path).expect("hash");
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn scan_local_collects_relpaths() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let nested_dir = root.join("a");
        fs::create_dir_all(&nested_dir).expect("mkdir");
        fs::write(root.join("root.txt"), b"root").expect("write root");
        fs::write(nested_dir.join("child.txt"), b"child").expect("write child");

        let files = scan_local(root.to_str().unwrap()).expect("scan");
        let relpaths: HashSet<String> = files.into_iter().map(|f| f.relpath).collect();
        assert!(relpaths.contains("root.txt"));
        assert!(relpaths.contains("a/child.txt"));
    }

    #[test]
    fn parse_updated_at_valid_rfc3339() {
        let result = parse_updated_at("2024-01-01T00:00:00Z");
        assert_eq!(result, 1704067200000);
    }

    #[test]
    fn parse_updated_at_invalid_falls_back_to_now() {
        let before = now_ms();
        let result = parse_updated_at("not-a-time");
        let after = now_ms();
        assert!(result >= before);
        assert!(result <= after);
    }

    #[test]
    fn to_local_map_indexes_by_relpath() {
        let item = LocalFileInfo {
            relpath: "a.txt".to_string(),
            abs_path: PathBuf::from("/tmp/a.txt"),
            size: 1,
            mtime_ms: 1,
            sha256: "x".to_string(),
        };
        let map = to_local_map(vec![item]);
        assert!(map.contains_key("a.txt"));
    }

    #[test]
    fn to_remote_map_skips_dirs_and_parses_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert(META_SHA256.to_string(), "abc".to_string());
        metadata.insert(META_MTIME.to_string(), "123".to_string());
        metadata.insert(META_DELETED_AT.to_string(), "456".to_string());

        let files = vec![
            RemoteFile {
                id: "dir".to_string(),
                name: "dir".to_string(),
                uri: "cloudreve://root/Work/dir".to_string(),
                size: 0,
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                metadata: HashMap::new(),
                is_dir: true,
            },
            RemoteFile {
                id: "file".to_string(),
                name: "file".to_string(),
                uri: "cloudreve://root/Work/a.txt".to_string(),
                size: 10,
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                metadata,
                is_dir: false,
            },
        ];

        let map = to_remote_map(files, "cloudreve://root/Work").expect("map");
        let file = map.get("a.txt").expect("file");
        assert_eq!(file.sha256, "abc");
        assert_eq!(file.mtime_ms, 123);
        assert_eq!(file.deleted_at_ms, Some(456));
    }

    #[test]
    fn file_extension_and_stem() {
        assert_eq!(file_extension("a/b.tar.gz"), Some("gz".to_string()));
        assert_eq!(file_stem("a/b.tar.gz"), "b.tar");
    }

    #[test]
    fn remove_local_file_ignores_missing() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("gone.txt");
        let info = LocalFileInfo {
            relpath: "gone.txt".to_string(),
            abs_path: path.clone(),
            size: 0,
            mtime_ms: 0,
            sha256: String::new(),
        };
        remove_local_file(&info).expect("remove");
        assert!(!path.exists());
    }
}
