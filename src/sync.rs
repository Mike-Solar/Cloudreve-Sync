use crate::cloudreve::{CloudreveClient, RemoteFile};
use crate::config::{ensure_dir, state_path, AppConfig};
use crate::logging::{LogEntry, LogLevel, LogStore};
use crate::placeholders::create_placeholder;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashSet;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub local_mtime: u64,
    pub local_size: u64,
    pub remote_mtime: String,
    pub remote_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncState {
    pub entries: HashMap<String, FileState>,
}

#[derive(Debug, Clone)]
pub struct LocalFile {
    pub path: String,
    pub abs_path: PathBuf,
    pub size: u64,
    pub mtime: u64,
}

#[derive(Clone)]
pub struct SyncEngine {
    config: AppConfig,
    client: CloudreveClient,
    log_store: LogStore,
}

impl SyncEngine {
    pub fn new(config: AppConfig, log_store: LogStore) -> Self {
        let client = CloudreveClient::new(
            config.base_url.clone(),
            config.access_token.clone(),
            config.api_paths.clone(),
        );
        Self {
            config,
            client,
            log_store,
        }
    }

    pub async fn sync_once(&self, log_tx: &crossbeam_channel::Sender<LogEntry>) {
        if self.config.local_root.is_empty() {
            self.log(log_tx, LogLevel::Warn, "Local root is empty; sync skipped".to_string());
            return;
        }
        let state_path = match state_path() {
            Ok(p) => p,
            Err(err) => {
                self.log(
                    log_tx,
                    LogLevel::Error,
                    format!("Failed to resolve state path: {}", err),
                );
                return;
            }
        };
        let mut state = match load_state(&state_path) {
            Ok(state) => state,
            Err(err) => {
                self.log(
                    log_tx,
                    LogLevel::Warn,
                    format!("Failed to load state, rebuilding: {}", err),
                );
                SyncState::default()
            }
        };

        let local = match scan_local(&self.config.local_root) {
            Ok(list) => list,
            Err(err) => {
                self.log(
                    log_tx,
                    LogLevel::Error,
                    format!("Local scan failed: {}", err),
                );
                return;
            }
        };
        let remote = match self.client.list_files(&self.config.remote_root).await {
            Ok(list) => list,
            Err(err) => {
                self.log(
                    log_tx,
                    LogLevel::Error,
                    format!("Remote list failed: {}", err),
                );
                return;
            }
        };

        let local_map = local
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect::<HashMap<_, _>>();
        let remote_map = remote
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect::<HashMap<_, _>>();

        let mut touched = HashSet::new();
        for path in local_map
            .keys()
            .chain(remote_map.keys())
            .chain(state.entries.keys())
            .cloned()
            .collect::<Vec<_>>()
        {
            if touched.contains(&path) {
                continue;
            }
            touched.insert(path.clone());

            let local_entry = local_map.get(&path);
            let remote_entry = remote_map.get(&path);
            let prev = state.entries.get(&path);

            let prev_local_exists = prev
                .map(|p| p.local_mtime > 0 || p.local_size > 0)
                .unwrap_or(false);
            let prev_remote_exists = prev
                .map(|p| !p.remote_mtime.is_empty() || p.remote_size > 0)
                .unwrap_or(false);

            let local_exists = local_entry.is_some();
            let remote_exists = remote_entry.is_some();

            let local_changed = local_entry.map_or(false, |entry| {
                prev.map_or(true, |p| p.local_mtime != entry.mtime || p.local_size != entry.size)
            });
            let remote_changed = remote_entry.map_or(false, |entry| {
                prev.map_or(true, |p| p.remote_size != entry.size || p.remote_mtime != entry.modified)
            });

            match decide_delete_action(
                prev_local_exists,
                prev_remote_exists,
                local_exists,
                remote_exists,
                local_changed,
                remote_changed,
            ) {
                DeleteDecision::ConflictLocalDeleted => {
                    if let Some(remote) = remote_entry {
                        if let Err(err) = self.materialize_remote_conflict(remote, "remote").await {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Conflict materialize failed for {}: {}", remote.path, err),
                            );
                        }
                        self.log(
                            log_tx,
                            LogLevel::Warn,
                            format!("Conflict detected (local delete vs remote update) for {}", remote.path),
                        );
                    }
                    continue;
                }
                DeleteDecision::DeleteRemote => {
                    if let Some(remote) = remote_entry {
                        if let Err(err) = self.delete_remote_file(&remote.path).await {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Remote delete failed for {}: {}", remote.path, err),
                            );
                        }
                        self.log(
                            log_tx,
                            LogLevel::Info,
                            format!("Deleted remote file {}", remote.path),
                        );
                    }
                    continue;
                }
                DeleteDecision::ConflictRemoteDeleted => {
                    if let Some(local) = local_entry {
                        let timestamp: DateTime<Local> = Local::now();
                        let suffix = timestamp.format("%Y%m%d-%H%M%S").to_string();
                        let conflict_remote_path = add_suffix(Path::new(&local.path), &suffix, "local");
                        let conflict_remote_path = normalize_remote_path(&conflict_remote_path);
                        let payload = match fs::read(&local.abs_path) {
                            Ok(bytes) => bytes,
                            Err(err) => {
                                self.log(
                                    log_tx,
                                    LogLevel::Error,
                                    format!("Read local file failed for {}: {}", local.path, err),
                                );
                                continue;
                            }
                        };
                        if let Err(err) = self.client.upload_file(&conflict_remote_path, &payload).await {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Conflict upload failed for {}: {}", local.path, err),
                            );
                        }
                        self.log(
                            log_tx,
                            LogLevel::Warn,
                            format!("Conflict detected (remote delete vs local update) for {}", local.path),
                        );
                    }
                    continue;
                }
                DeleteDecision::DeleteLocal => {
                    if let Some(local) = local_entry {
                        if let Err(err) = fs::remove_file(&local.abs_path) {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Local delete failed for {}: {}", local.path, err),
                            );
                        }
                        self.log(
                            log_tx,
                            LogLevel::Info,
                            format!("Deleted local file {}", local.path),
                        );
                    }
                    continue;
                }
                DeleteDecision::None => {}
            }

            match (local_entry, remote_entry) {
                (Some(local), None) => {
                    if let Err(err) = self.upload_file(local).await {
                        self.log(
                            log_tx,
                            LogLevel::Error,
                            format!("Upload failed for {}: {}", local.path, err),
                        );
                        continue;
                    }
                    self.log(
                        log_tx,
                        LogLevel::Info,
                        format!("Uploaded new local file {}", local.path),
                    );
                }
                (None, Some(remote)) => {
                    if let Err(err) = self.download_file(remote).await {
                        self.log(
                            log_tx,
                            LogLevel::Error,
                            format!("Download failed for {}: {}", remote.path, err),
                        );
                        continue;
                    }
                    self.log(
                        log_tx,
                        LogLevel::Info,
                        format!("Downloaded new remote file {}", remote.path),
                    );
                }
                (Some(local), Some(remote)) => {
                    if local_changed && remote_changed {
                        if let Err(err) = self.resolve_conflict(local, remote).await {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Conflict resolution failed for {}: {}", local.path, err),
                            );
                            continue;
                        }
                        self.log(
                            log_tx,
                            LogLevel::Warn,
                            format!("Conflict resolved for {}", local.path),
                        );
                    } else if local_changed {
                        if let Err(err) = self.upload_file(local).await {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Upload failed for {}: {}", local.path, err),
                            );
                            continue;
                        }
                        self.log(
                            log_tx,
                            LogLevel::Info,
                            format!("Uploaded local update {}", local.path),
                        );
                    } else if remote_changed {
                        if let Err(err) = self.download_file(remote).await {
                            self.log(
                                log_tx,
                                LogLevel::Error,
                                format!("Download failed for {}: {}", remote.path, err),
                            );
                            continue;
                        }
                        self.log(
                            log_tx,
                            LogLevel::Info,
                            format!("Downloaded remote update {}", remote.path),
                        );
                    }
                }
                (None, None) => {}
            }
        }

        let refreshed_local = scan_local(&self.config.local_root).unwrap_or_default();
        let refreshed_remote = self
            .client
            .list_files(&self.config.remote_root)
            .await
            .unwrap_or_default();
        state.entries = build_state(
            refreshed_local.iter(),
            refreshed_remote.iter(),
        );
        if let Err(err) = save_state(&state_path, &state) {
            self.log(
                log_tx,
                LogLevel::Error,
                format!("Failed to save state: {}", err),
            );
        }
    }

    async fn upload_file(&self, local: &LocalFile) -> Result<(), Box<dyn Error>> {
        let content = fs::read(&local.abs_path)?;
        self.client.upload_file(&local.path, &content).await?;
        Ok(())
    }

    async fn download_file(&self, remote: &RemoteFile) -> Result<(), Box<dyn Error>> {
        let rel = remote.path.trim_start_matches('/');
        let target = Path::new(&self.config.local_root).join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.config.use_placeholders && cfg!(target_os = "windows") && !target.exists() {
            if let Err(err) = create_placeholder(&target, remote.size, &remote.modified) {
                return Err(err);
            }
            return Ok(());
        }
        let bytes = self.client.download_file(&remote.path).await?;
        fs::write(&target, bytes)?;
        Ok(())
    }

    async fn resolve_conflict(
        &self,
        local: &LocalFile,
        remote: &RemoteFile,
    ) -> Result<(), Box<dyn Error>> {
        let timestamp: DateTime<Local> = Local::now();
        let suffix = timestamp.format("%Y%m%d-%H%M%S").to_string();
        let conflict_local = add_suffix(&local.abs_path, &suffix, "local");
        fs::rename(&local.abs_path, &conflict_local)?;
        let conflict_remote_path = add_suffix(Path::new(&remote.path), &suffix, "remote");
        self.download_file(remote).await?;
        self.client
            .upload_file(
                conflict_remote_path.to_str().ok_or("invalid remote path")?,
                &fs::read(&conflict_local)?,
            )
            .await?;
        Ok(())
    }

    async fn delete_remote_file(&self, remote_path: &str) -> Result<(), Box<dyn Error>> {
        let uri = CloudreveClient::build_file_uri(remote_path);
        self.client.delete_files(vec![uri]).await?;
        Ok(())
    }

    async fn materialize_remote_conflict(
        &self,
        remote: &RemoteFile,
        tag: &str,
    ) -> Result<(), Box<dyn Error>> {
        let timestamp: DateTime<Local> = Local::now();
        let suffix = timestamp.format("%Y%m%d-%H%M%S").to_string();
        let target = Path::new(&self.config.local_root)
            .join(remote.path.trim_start_matches('/'));
        let conflict = add_suffix(&target, &suffix, tag);
        if let Some(parent) = conflict.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = self.client.download_file(&remote.path).await?;
        fs::write(conflict, bytes)?;
        Ok(())
    }

    pub async fn create_share_link(&self, remote_path: &str) -> Result<String, Box<dyn Error>> {
        self.client.create_share_link(remote_path).await
    }

    fn log(&self, log_tx: &crossbeam_channel::Sender<LogEntry>, level: LogLevel, message: String) {
        let entry = LogEntry::new(level, message);
        let _ = self.log_store.append(&entry);
        let _ = log_tx.send(entry);
    }
}

fn scan_local(root: &str) -> Result<Vec<LocalFile>, Box<dyn Error>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if is_placeholder_metadata(entry.path()) || has_placeholder_metadata(entry.path()) {
            continue;
        }
        let metadata = entry.metadata()?;
        let mtime = metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs();
        let abs_path = entry.path().to_path_buf();
        let path = abs_path
            .strip_prefix(root)
            .unwrap_or(&abs_path)
            .to_string_lossy()
            .trim_start_matches(std::path::MAIN_SEPARATOR)
            .replace(std::path::MAIN_SEPARATOR, "/");
        out.push(LocalFile {
            path,
            abs_path,
            size: metadata.len(),
            mtime,
        });
    }
    Ok(out)
}

fn build_state<'a>(
    locals: impl Iterator<Item = &'a LocalFile>,
    remotes: impl Iterator<Item = &'a RemoteFile>,
) -> HashMap<String, FileState> {
    let mut entries = HashMap::new();
    for local in locals {
        entries.insert(
            local.path.clone(),
            FileState {
                local_mtime: local.mtime,
                local_size: local.size,
                remote_mtime: String::new(),
                remote_size: 0,
            },
        );
    }
    for remote in remotes {
        entries
            .entry(remote.path.clone())
            .and_modify(|entry| {
                entry.remote_mtime = remote.modified.clone();
                entry.remote_size = remote.size;
            })
            .or_insert_with(|| FileState {
                local_mtime: 0,
                local_size: 0,
                remote_mtime: remote.modified.clone(),
                remote_size: remote.size,
            });
    }
    entries
}

fn add_suffix(path: &Path, suffix: &str, tag: &str) -> PathBuf {
    let file_name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
    let mut parts = file_name.rsplitn(2, '.').collect::<Vec<_>>();
    let new_name = if parts.len() == 2 {
        format!("{}.{}-{}.{}", parts[1], tag, suffix, parts[0])
    } else {
        format!("{}-{}-{}", file_name, tag, suffix)
    };
    path.with_file_name(new_name)
}

#[derive(Debug, PartialEq)]
enum DeleteDecision {
    None,
    DeleteRemote,
    DeleteLocal,
    ConflictLocalDeleted,
    ConflictRemoteDeleted,
}

// 根据同步状态决定删除或冲突处理，方便单元测试覆盖。
fn decide_delete_action(
    prev_local_exists: bool,
    prev_remote_exists: bool,
    local_exists: bool,
    remote_exists: bool,
    local_changed: bool,
    remote_changed: bool,
) -> DeleteDecision {
    if !local_exists && prev_local_exists && remote_exists {
        if remote_changed {
            return DeleteDecision::ConflictLocalDeleted;
        }
        return DeleteDecision::DeleteRemote;
    }
    if !remote_exists && prev_remote_exists && local_exists {
        if local_changed {
            return DeleteDecision::ConflictRemoteDeleted;
        }
        return DeleteDecision::DeleteLocal;
    }
    DeleteDecision::None
}

// 远端路径统一使用正斜杠，避免不同平台路径分隔符混入。
fn normalize_remote_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

// 使用占位符元数据判断文件是否为占位符，避免上传空文件或元数据。
fn is_placeholder_metadata(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.ends_with(".cloudreve.placeholder.json"))
        .unwrap_or(false)
}

fn has_placeholder_metadata(path: &Path) -> bool {
    let meta_path = path.with_extension("cloudreve.placeholder.json");
    meta_path.exists()
}

fn load_state(path: &Path) -> Result<SyncState, Box<dyn Error>> {
    if !path.exists() {
        return Ok(SyncState::default());
    }
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn save_state(path: &Path, state: &SyncState) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(state)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        dir.push(format!("cloudreve-sync-test-{}-{}", name, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn decide_delete_action_local_deleted_remote_unchanged() {
        let decision = decide_delete_action(true, false, false, true, false, false);
        assert_eq!(decision, DeleteDecision::DeleteRemote);
    }

    #[test]
    fn decide_delete_action_remote_deleted_local_unchanged() {
        let decision = decide_delete_action(false, true, true, false, false, false);
        assert_eq!(decision, DeleteDecision::DeleteLocal);
    }

    #[test]
    fn decide_delete_action_local_deleted_remote_changed() {
        let decision = decide_delete_action(true, false, false, true, false, true);
        assert_eq!(decision, DeleteDecision::ConflictLocalDeleted);
    }

    #[test]
    fn decide_delete_action_remote_deleted_local_changed() {
        let decision = decide_delete_action(false, true, true, false, true, false);
        assert_eq!(decision, DeleteDecision::ConflictRemoteDeleted);
    }

    #[test]
    fn add_suffix_uses_tag_and_timestamp() {
        let path = Path::new("demo.txt");
        let out = add_suffix(path, "20250101-000000", "local");
        assert_eq!(out.file_name().unwrap(), "demo.local-20250101-000000.txt");
    }

    #[test]
    fn placeholder_metadata_detection() {
        let dir = temp_dir("placeholder");
        let file_path = dir.join("note.txt");
        let meta_path = dir.join("note.cloudreve.placeholder.json");
        fs::write(&file_path, b"demo").expect("write file");
        fs::write(&meta_path, b"{}").expect("write meta");

        assert!(has_placeholder_metadata(&file_path));
        assert!(is_placeholder_metadata(&meta_path));

        fs::remove_dir_all(dir).expect("cleanup temp dir");
    }
}
