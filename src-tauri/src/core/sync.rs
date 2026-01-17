use crate::core::cloudreve::{CloudreveClient, MetadataPatch, RemoteFile};
use crate::core::config::ApiPaths;
use crate::core::db::{
    insert_conflict, insert_tombstone, list_entries_by_task, list_tombstones, now_ms, upsert_entry,
    ConflictRow, EntryRow, TaskRow, TombstoneRow,
};
use crate::core::logging::{LogEntry, LogLevel, LogStore};
use chrono::{DateTime, Local, Utc};
use filetime::FileTime;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
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
}

impl SyncEngine {
    pub fn new(task: TaskRow, api_paths: ApiPaths, access_token: Option<String>, db_path: PathBuf) -> Self {
        let client = CloudreveClient::new(task.base_url.clone(), access_token, api_paths);
        let log_store = LogStore::new(db_path.clone());
        Self {
            task,
            client,
            db_path,
            log_store,
        }
    }

    pub async fn sync_once(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = Connection::open(&self.db_path)?;
        let entries = list_entries_by_task(&conn, &self.task.task_id)?;
        let tombstones = list_tombstones(&conn, &self.task.task_id)?;

        let local_files = scan_local(&self.task.local_root)?;
        let remote_files = self.client.list_all_files(&self.task.remote_root_uri).await?;
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
            let local = local_map.get(&relpath);
            let remote = remote_map.get(&relpath);
            let entry = entry_map.get(&relpath);
            let tombstone = tombstone_map.get(&relpath);

            if let Some(remote) = remote {
                if remote.deleted_at_ms.is_some() {
                    if let Some(local) = local {
                        remove_local_file(local)?;
                        self.log_db(&mut conn, LogLevel::Warn, "delete", &format!(
                            "本地删除: {} (远端标记删除)",
                            local.relpath
                        ))?;
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
                    continue;
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
                    self.log_db(&mut conn, LogLevel::Warn, "delete", &format!(
                        "远端标记删除: {}", relpath
                    ))?;
                }
                continue;
            }

            match (local, remote) {
                (Some(local), Some(remote)) => {
                    let local_changed = entry
                        .map(|e| e.last_local_sha256 != local.sha256 || e.last_local_mtime_ms != local.mtime_ms)
                        .unwrap_or(true);
                    let remote_changed = entry
                        .map(|e| e.last_remote_sha256 != remote.sha256 || e.last_remote_mtime_ms != remote.mtime_ms)
                        .unwrap_or(true);

                    if local_changed && remote_changed && local.sha256 != remote.sha256 {
                        self.handle_conflict(&mut conn, local, remote).await?;
                        continue;
                    }

                    if local_changed {
                        self.upload_local(&mut conn, local, remote).await?;
                    } else if remote_changed {
                        self.download_remote(&mut conn, local, remote).await?;
                    }
                }
                (Some(local), None) => {
                    self.upload_new_local(&mut conn, local).await?;
                }
                (None, Some(remote)) => {
                    self.download_new_remote(&mut conn, remote).await?;
                }
                (None, None) => {}
            }
        }

        Ok(())
    }

    async fn upload_new_local(&self, conn: &mut Connection, local: &LocalFileInfo) -> Result<(), Box<dyn Error>> {
        let uri = build_remote_uri(&self.task.remote_root_uri, &local.relpath);
        self.client.update_file_content(&uri, &fs::read(&local.abs_path)?).await?;
        self.patch_sync_metadata(&uri, local, None).await?;
        upsert_entry(conn, &EntryRow {
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
        })?;
        self.log_db(conn, LogLevel::Info, "upload", &format!("上传新文件: {}", local.relpath))?;
        Ok(())
    }

    async fn upload_local(
        &self,
        conn: &mut Connection,
        local: &LocalFileInfo,
        remote: &RemoteFileInfo,
    ) -> Result<(), Box<dyn Error>> {
        self.client.update_file_content(&remote.uri, &fs::read(&local.abs_path)?).await?;
        self.patch_sync_metadata(&remote.uri, local, Some(remote)).await?;
        upsert_entry(conn, &EntryRow {
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
        })?;
        self.log_db(conn, LogLevel::Info, "upload", &format!("上传更新: {}", local.relpath))?;
        Ok(())
    }

    async fn download_new_remote(&self, conn: &mut Connection, remote: &RemoteFileInfo) -> Result<(), Box<dyn Error>> {
        let target = Path::new(&self.task.local_root).join(&remote.relpath);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = self.client.download_file(&remote.uri).await?;
        fs::write(&target, bytes)?;
        set_local_mtime(&target, remote.mtime_ms)?;
        upsert_entry(conn, &EntryRow {
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
        })?;
        self.log_db(conn, LogLevel::Info, "download", &format!("下载新文件: {}", remote.relpath))?;
        Ok(())
    }

    async fn download_remote(
        &self,
        conn: &mut Connection,
        local: &LocalFileInfo,
        remote: &RemoteFileInfo,
    ) -> Result<(), Box<dyn Error>> {
        let bytes = self.client.download_file(&remote.uri).await?;
        fs::write(&local.abs_path, bytes)?;
        set_local_mtime(&local.abs_path, remote.mtime_ms)?;
        upsert_entry(conn, &EntryRow {
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
        })?;
        self.log_db(conn, LogLevel::Info, "download", &format!("下载更新: {}", local.relpath))?;
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
        self.client.update_file_content(&conflict_uri, &fs::read(&conflict_abs)?).await?;
        self.patch_conflict_metadata(&conflict_uri, local, remote).await?;

        insert_conflict(conn, &ConflictRow {
            task_id: self.task.task_id.clone(),
            original_relpath: local.relpath.clone(),
            conflict_relpath: conflict_relpath.clone(),
            created_at_ms: now_ms(),
            reason: "both_modified".to_string(),
        })?;

        self.log_db(conn, LogLevel::Warn, "conflict", &format!(
            "冲突生成: {} -> {}",
            local.relpath, conflict_relpath
        ))?;
        Ok(())
    }

    async fn set_remote_deleted(&self, uri: &str, deleted_at_ms: i64) -> Result<(), Box<dyn Error>> {
        let patches = vec![MetadataPatch {
            key: META_DELETED_AT.to_string(),
            value: Some(deleted_at_ms.to_string()),
            remove: Some(false),
        }];
        self.client.patch_metadata(vec![uri.to_string()], patches).await
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
        self.client.patch_metadata(vec![uri.to_string()], patches).await
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
        self.client.patch_metadata(vec![uri.to_string()], patches).await
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
}

fn scan_local(root: &str) -> Result<Vec<LocalFileInfo>, Box<dyn Error>> {
    let mut out = Vec::new();
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
        let sha256 = hash_file(&abs_path)?;
        out.push(LocalFileInfo {
            relpath,
            abs_path,
            size: metadata.len(),
            mtime_ms,
            sha256,
        });
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
    urlencoding::decode(path).unwrap_or_else(|_| path.into()).to_string()
}

fn build_remote_uri(root_uri: &str, relpath: &str) -> String {
    let root = root_uri.trim_end_matches('/');
    let encoded = relpath
        .split('/')
        .map(|part| urlencoding::encode(part).to_string())
        .collect::<Vec<_>>()
        .join("/");
    format!("{}/{}", root, encoded)
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
    Path::new(path).extension().and_then(|ext| ext.to_str()).map(|s| s.to_string())
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(path)
        .to_string()
}
