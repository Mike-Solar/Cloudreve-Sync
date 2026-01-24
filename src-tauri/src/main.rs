mod core;

use core::cloudreve::{
    finish_sign_in_with_2fa, get_captcha, password_sign_in, refresh_token, CloudreveClient,
    SignInResult,
};
use core::config::{ApiPaths, AppSettings, config_dir, ensure_dir};
use core::credentials::{load_tokens, store_tokens};
use core::db::{
    count_logs, create_task, delete_all_accounts, delete_conflict, delete_task, init_db, list_accounts, list_conflicts, list_logs,
    list_tasks, now_ms, upsert_account, AccountRow, TaskRow,
};
use core::sync::{SyncEngine, SyncStats};
use chrono::{Local, TimeZone};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Emitter;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle,
    Manager,
    WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use uuid::Uuid;

#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

#[derive(Clone)]
struct RunnerHandle {
    stop: Arc<AtomicBool>,
}

struct AppState {
    db_path: PathBuf,
    api_paths: ApiPaths,
    runners: Mutex<HashMap<String, RunnerHandle>>,
    stats: Arc<Mutex<HashMap<String, TaskStats>>>,
}

const TOKEN_REFRESH_INTERVAL_SECS: u64 = 20 * 60;

#[derive(Serialize)]
struct DashboardCard {
    label: String,
    value: String,
    tone: String,
}

#[derive(Serialize)]
struct TaskItem {
    id: String,
    name: String,
    mode: String,
    local_path: String,
    remote_path: String,
    status: String,
    rate_up: String,
    rate_down: String,
    queue: u32,
    last_sync: String,
}

#[derive(Clone, Debug)]
struct TaskStats {
    rate_up: String,
    rate_down: String,
    queue: u32,
}

#[derive(Serialize)]
struct AccountItem {
    account_key: String,
    base_url: String,
    email: String,
    created_at_ms: i64,
}

#[derive(Serialize)]
struct ActivityItem {
    timestamp: String,
    event: String,
    detail: String,
    level: String,
}

#[derive(Serialize)]
struct ConflictItem {
    id: String,
    task_id: String,
    original_relpath: String,
    conflict_relpath: String,
    name: String,
    task: String,
    path: String,
    local_path: String,
    local_dir: String,
    device: String,
    time: String,
    status: String,
}

#[derive(Serialize)]
struct DiagnosticInfo {
    app_version: String,
    os: String,
    arch: String,
    db_path: String,
    config_dir: String,
    accounts: usize,
    tasks: usize,
}

#[derive(Serialize)]
struct BootstrapPayload {
    cards: Vec<DashboardCard>,
    tasks: Vec<TaskItem>,
    activities: Vec<ActivityItem>,
    conflicts: Vec<ConflictItem>,
}

#[derive(Deserialize)]
struct LoginRequest {
    base_url: String,
    email: String,
    password: String,
    captcha: Option<String>,
    ticket: Option<String>,
}

#[derive(Deserialize)]
struct TwoFaFinishRequest {
    base_url: String,
    email: String,
    session_id: String,
    opt: String,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum LoginCommandResult {
    Success { account_key: String },
    TwoFaRequired { session_id: String },
}

#[derive(Deserialize)]
struct CreateTaskRequest {
    name: String,
    base_url: String,
    account_key: String,
    local_root: String,
    remote_root_uri: String,
    mode: String,
    sync_interval_secs: u64,
}

#[derive(Deserialize)]
struct LogsQuery {
    task_id: Option<String>,
    level: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Deserialize)]
struct SyncRequest {
    task_id: String,
}

#[derive(Deserialize)]
struct DeleteTaskRequest {
    task_id: String,
}

#[derive(Deserialize)]
struct ListRemoteEntriesRequest {
    account_key: String,
    base_url: String,
    uri: String,
}

#[derive(Deserialize)]
struct CreateShareLinkRequest {
    local_path: String,
    password: Option<String>,
    expire_seconds: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct TaskSettings {
    name: String,
    account_key: String,
    sync_interval_secs: u64,
}

#[derive(Serialize, Clone)]
struct ShareRequestPayload {
    path: String,
}

#[tauri::command]
fn login(state: tauri::State<AppState>, payload: LoginRequest) -> Result<LoginCommandResult, String> {
    let result = tauri::async_runtime::block_on(password_sign_in(
        &payload.base_url,
        &payload.email,
        &payload.password,
        payload.captcha.as_deref(),
        payload.ticket.as_deref(),
    ))
    .map_err(|err| err.to_string())?;

    match result {
        SignInResult::Success(result) => {
            let account_key = format!("{}|{}", payload.base_url, payload.email);
            store_tokens(
                &account_key,
                &result.token.access_token,
                &result.token.refresh_token,
            )
            .map_err(|err| err.to_string())?;

            let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
            init_db(&conn).map_err(|err| err.to_string())?;
            upsert_account(
                &conn,
                &AccountRow {
                    account_key: account_key.clone(),
                    base_url: payload.base_url,
                    email: payload.email,
                    created_at_ms: now_ms(),
                },
            )
            .map_err(|err| err.to_string())?;

            Ok(LoginCommandResult::Success { account_key })
        }
        SignInResult::TwoFaRequired(session_id) => {
            Ok(LoginCommandResult::TwoFaRequired { session_id })
        }
    }
}

#[tauri::command]
fn finish_sign_in_with_2fa_command(
    state: tauri::State<AppState>,
    payload: TwoFaFinishRequest,
) -> Result<LoginCommandResult, String> {
    let result = tauri::async_runtime::block_on(finish_sign_in_with_2fa(
        &payload.base_url,
        &payload.opt,
        &payload.session_id,
    ))
    .map_err(|err| err.to_string())?;

    let account_key = format!("{}|{}", payload.base_url, payload.email);
    store_tokens(
        &account_key,
        &result.token.access_token,
        &result.token.refresh_token,
    )
    .map_err(|err| err.to_string())?;

    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;
    upsert_account(
        &conn,
        &AccountRow {
            account_key: account_key.clone(),
            base_url: payload.base_url,
            email: payload.email,
            created_at_ms: now_ms(),
        },
    )
    .map_err(|err| err.to_string())?;

    Ok(LoginCommandResult::Success { account_key })
}

#[tauri::command]
fn get_captcha_command(payload: String) -> Result<core::cloudreve::CaptchaData, String> {
    tauri::async_runtime::block_on(get_captcha(&payload)).map_err(|err| err.to_string())
}

#[tauri::command]
fn test_connection(state: tauri::State<AppState>, account_key: String, base_url: String) -> Result<(), String> {
    let tokens = load_tokens(&account_key).map_err(|err| err.to_string())?;
    let client = CloudreveClient::new(base_url, Some(tokens.access_token), state.api_paths.clone());
    tauri::async_runtime::block_on(client.ping()).map_err(|err| err.to_string())
}

#[tauri::command]
fn create_task_command(state: tauri::State<AppState>, payload: CreateTaskRequest) -> Result<(), String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;

    let task_id = Uuid::new_v4().to_string();
    let device_id = Uuid::new_v4().to_string();
    let remote_root = if payload.remote_root_uri.starts_with("cloudreve://") {
        payload.remote_root_uri.clone()
    } else {
        CloudreveClient::build_file_uri(&payload.remote_root_uri)
    };
    let settings = TaskSettings {
        name: payload.name.clone(),
        account_key: payload.account_key.clone(),
        sync_interval_secs: payload.sync_interval_secs,
    };
    let task = TaskRow {
        task_id: task_id.clone(),
        base_url: payload.base_url,
        local_root: payload.local_root,
        remote_root_uri: remote_root,
        device_id,
        mode: payload.mode,
        settings_json: serde_json::to_string(&settings).map_err(|err| err.to_string())?,
        created_at_ms: now_ms(),
    };
    create_task(&conn, &task).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn list_tasks_command(state: tauri::State<AppState>) -> Result<Vec<TaskItem>, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    build_task_items(&state, &conn).map_err(|err| err.to_string())
}

#[tauri::command]
fn list_accounts_command(state: tauri::State<AppState>) -> Result<Vec<AccountItem>, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;
    let accounts = list_accounts(&conn).map_err(|err| err.to_string())?;
    Ok(accounts
        .into_iter()
        .map(|item| AccountItem {
            account_key: item.account_key,
            base_url: item.base_url,
            email: item.email,
            created_at_ms: item.created_at_ms,
        })
        .collect())
}

#[tauri::command]
fn list_remote_entries_command(
    state: tauri::State<AppState>,
    payload: ListRemoteEntriesRequest,
) -> Result<Vec<core::cloudreve::RemoteEntry>, String> {
    let tokens = load_tokens(&payload.account_key).map_err(|err| err.to_string())?;
    let client = CloudreveClient::new(
        payload.base_url,
        Some(tokens.access_token),
        state.api_paths.clone(),
    );
    tauri::async_runtime::block_on(client.list_directory_entries(&payload.uri))
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn create_share_link_command(
    state: tauri::State<AppState>,
    payload: CreateShareLinkRequest,
) -> Result<String, String> {
    let local_path = PathBuf::from(&payload.local_path);
    let metadata = local_path.metadata().map_err(|err| err.to_string())?;
    let is_dir = metadata.is_dir();
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;
    let tasks = list_tasks(&conn).map_err(|err| err.to_string())?;
    let task = find_task_for_local_path(&tasks, &local_path)
        .ok_or_else(|| "未找到匹配的同步任务".to_string())?;
    let settings = parse_settings(&task.settings_json);
    let tokens = load_tokens(&settings.account_key).map_err(|err| err.to_string())?;
    let relpath = relpath_from_local(&task.local_root, &local_path)?;
    let uri = if relpath.is_empty() {
        task.remote_root_uri.clone()
    } else {
        build_remote_uri(&task.remote_root_uri, &relpath)
    };
    let password = payload
        .password
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let expire_seconds = payload.expire_seconds.filter(|value| *value > 0);
    let client = CloudreveClient::new(task.base_url.clone(), Some(tokens.access_token), state.api_paths.clone());
    let link = tauri::async_runtime::block_on(client.create_share_link(
        &uri,
        password,
        expire_seconds,
        Some(is_dir),
    ))
    .map_err(|err| err.to_string())?;
    log_info(&state.db_path, &task.task_id, "share", &format!("{} -> {}", payload.local_path, link));
    Ok(link)
}
#[tauri::command]
fn list_conflicts_command(state: tauri::State<AppState>, task_id: Option<String>) -> Result<Vec<ConflictItem>, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    let conflicts = list_conflicts(&conn, task_id.as_deref()).map_err(|err| err.to_string())?;
    let tasks = list_tasks(&conn).map_err(|err| err.to_string())?;
    let task_map = tasks
        .into_iter()
        .map(|task| {
            let settings = parse_settings(&task.settings_json);
            (
                task.task_id,
                (settings.name, task.local_root),
            )
        })
        .collect::<HashMap<_, _>>();
    Ok(conflicts
        .into_iter()
        .map(|item| {
            let (task_name, local_root) = task_map
                .get(&item.task_id)
                .cloned()
                .unwrap_or_else(|| (item.task_id.clone(), String::new()));
            let local_path = if local_root.is_empty() {
                item.conflict_relpath.clone()
            } else {
                PathBuf::from(&local_root)
                    .join(&item.conflict_relpath)
                    .to_string_lossy()
                    .to_string()
            };
            let local_dir = parent_path(&local_path);
            ConflictItem {
                id: format!("{}:{}", item.task_id, item.conflict_relpath),
                task_id: item.task_id.clone(),
                original_relpath: item.original_relpath.clone(),
                conflict_relpath: item.conflict_relpath.clone(),
                name: file_name(&item.original_relpath),
                task: task_name,
                path: parent_path(&item.original_relpath),
                local_path,
                local_dir,
                device: "".to_string(),
                time: format_time(item.created_at_ms),
                status: "未处理".to_string(),
            }
        })
        .collect())
}

#[tauri::command]
fn get_settings_command() -> Result<AppSettings, String> {
    AppSettings::load().map_err(|err| err.to_string())
}

#[tauri::command]
fn save_settings_command(payload: AppSettings) -> Result<(), String> {
    payload.save().map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_credentials_command(state: tauri::State<AppState>) -> Result<(), String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;
    let accounts = list_accounts(&conn).map_err(|err| err.to_string())?;
    for account in &accounts {
        let _ = core::credentials::clear_tokens(&account.account_key);
    }
    delete_all_accounts(&conn).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_local_path(path: String) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err("path not found".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn export_logs_command(state: tauri::State<AppState>, task_id: Option<String>, level: Option<String>) -> Result<String, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;
    let logs = list_logs(&conn, task_id.as_deref(), level.as_deref(), None, None).map_err(|err| err.to_string())?;
    let base_dir = config_dir().map_err(|err| err.to_string())?;
    let export_dir = base_dir.join("exports");
    ensure_dir(&export_dir).map_err(|err| err.to_string())?;
    let filename = format!("logs-{}.jsonl", Local::now().format("%Y%m%d-%H%M%S"));
    let path = export_dir.join(filename);
    let mut file = std::fs::File::create(&path).map_err(|err| err.to_string())?;
    for log in logs {
        let line = serde_json::to_string(&log).map_err(|err| err.to_string())?;
        file.write_all(line.as_bytes()).map_err(|err| err.to_string())?;
        file.write_all(b"\n").map_err(|err| err.to_string())?;
    }
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_diagnostics_command(state: tauri::State<AppState>) -> Result<DiagnosticInfo, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    init_db(&conn).map_err(|err| err.to_string())?;
    let accounts = list_accounts(&conn).map_err(|err| err.to_string())?;
    let tasks = list_tasks(&conn).map_err(|err| err.to_string())?;
    let cfg_dir = config_dir().map_err(|err| err.to_string())?;
    Ok(DiagnosticInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        db_path: state.db_path.to_string_lossy().to_string(),
        config_dir: cfg_dir.to_string_lossy().to_string(),
        accounts: accounts.len(),
        tasks: tasks.len(),
    })
}

#[tauri::command]
fn mark_conflict_resolved(state: tauri::State<AppState>, task_id: String, conflict_relpath: String) -> Result<(), String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    delete_conflict(&conn, &task_id, &conflict_relpath).map_err(|err| err.to_string())
}

#[tauri::command]
fn download_conflict_remote(state: tauri::State<AppState>, task_id: String, original_relpath: String) -> Result<(), String> {
    let (task, settings) = load_task_settings(&state.db_path, &task_id).map_err(|err| err.to_string())?;
    let tokens = load_tokens(&settings.account_key).map_err(|err| err.to_string())?;
    let uri = build_remote_uri(&task.remote_root_uri, &original_relpath);
    let client = CloudreveClient::new(task.base_url, Some(tokens.access_token), state.api_paths.clone());
    let result = tauri::async_runtime::block_on(client.create_download_urls(vec![uri], true))
        .map_err(|err| err.to_string())?;
    let url = result
        .urls
        .first()
        .map(|item| item.url.clone())
        .ok_or_else(|| "download url missing".to_string())?;
    open_external(url)
}

#[tauri::command]
fn hash_local_file(path: String) -> Result<String, String> {
    let mut file = std::fs::File::open(&path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 512];
    loop {
        let count = std::io::Read::read(&mut file, &mut buffer).map_err(|err| err.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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

fn normalize_path_for_match(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while normalized.ends_with('/') {
        normalized.pop();
    }
    if cfg!(target_os = "windows") {
        normalized = normalized.to_lowercase();
    }
    normalized
}

fn find_task_for_local_path(tasks: &[TaskRow], local_path: &Path) -> Option<TaskRow> {
    let target = normalize_path_for_match(local_path);
    let mut best: Option<(usize, TaskRow)> = None;
    for task in tasks {
        if task.local_root.trim().is_empty() {
            continue;
        }
        let root = normalize_path_for_match(Path::new(&task.local_root));
        if root.is_empty() {
            continue;
        }
        let is_match = target == root || target.starts_with(&format!("{}/", root));
        if !is_match {
            continue;
        }
        let score = root.len();
        if best.as_ref().map(|(len, _)| score > *len).unwrap_or(true) {
            best = Some((score, task.clone()));
        }
    }
    best.map(|(_, task)| task)
}

fn relpath_from_local(local_root: &str, local_path: &Path) -> Result<String, String> {
    let root = Path::new(local_root);
    let rel = local_path.strip_prefix(root).map_err(|_| "路径不在同步目录下".to_string())?;
    let mut parts = Vec::new();
    for component in rel.components() {
        if let std::path::Component::Normal(value) = component {
            parts.push(value.to_string_lossy().to_string());
        }
    }
    Ok(parts.join("/"))
}

#[tauri::command]
fn list_logs_command(state: tauri::State<AppState>, query: LogsQuery) -> Result<LogsPage, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(10, 200);
    let offset = (page - 1) * page_size;
    let total = count_logs(&conn, query.task_id.as_deref(), query.level.as_deref())
        .map_err(|err| err.to_string())?;
    let logs = list_logs(
        &conn,
        query.task_id.as_deref(),
        query.level.as_deref(),
        Some(page_size),
        Some(offset),
    )
    .map_err(|err| err.to_string())?;
    Ok(LogsPage {
        total,
        items: logs
        .into_iter()
        .map(|log| ActivityItem {
            timestamp: format_time(log.created_at_ms),
            event: log.event,
            detail: log.detail,
            level: log.level,
        })
        .collect(),
    })
}

#[tauri::command]
fn run_sync_command(state: tauri::State<AppState>, payload: SyncRequest) -> Result<(), String> {
    let mut runners = state.runners.lock().map_err(|_| "runner lock error".to_string())?;
    if runners.contains_key(&payload.task_id) {
        return Ok(());
    }
    let stop_flag = Arc::new(AtomicBool::new(false));
    let task_id = payload.task_id.clone();
    let db_path = state.db_path.clone();
    let api_paths = state.api_paths.clone();
    let stats_map = state.stats.clone();
    let stop_for_thread = stop_flag.clone();
    thread::spawn(move || {
        let settings = match load_task_settings(&db_path, &task_id) {
            Ok((_, settings)) => settings,
            Err(err) => {
                let detail = err.to_string();
                log_error(&db_path, &task_id, &detail);
                return;
            }
        };
        let interval = settings.sync_interval_secs.max(5);
        loop {
            if stop_for_thread.load(Ordering::SeqCst) {
                break;
            }
            let start = Instant::now();
            match run_sync_once(&db_path, &api_paths, &task_id) {
                Ok(stats) => update_task_stats(&stats_map, &task_id, stats, start.elapsed()),
                Err(err) => {
                    let detail = err.to_string();
                    log_error(&db_path, &task_id, &detail);
                }
            }
            thread::sleep(Duration::from_secs(interval));
        }
    });
    runners.insert(payload.task_id, RunnerHandle { stop: stop_flag });
    Ok(())
}

#[tauri::command]
fn stop_sync_command(state: tauri::State<AppState>, payload: SyncRequest) -> Result<(), String> {
    let mut runners = state.runners.lock().map_err(|_| "runner lock error".to_string())?;
    if let Some(handle) = runners.remove(&payload.task_id) {
        handle.stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
fn delete_task_command(state: tauri::State<AppState>, payload: DeleteTaskRequest) -> Result<(), String> {
    {
        let mut runners = state.runners.lock().map_err(|_| "runner lock error".to_string())?;
        if let Some(handle) = runners.remove(&payload.task_id) {
            handle.stop.store(true, Ordering::SeqCst);
        }
    }
    if let Ok(mut stats) = state.stats.lock() {
        stats.remove(&payload.task_id);
    }
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    delete_task(&conn, &payload.task_id).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn bootstrap(state: tauri::State<AppState>) -> Result<BootstrapPayload, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    let tasks = build_task_items(&state, &conn).map_err(|err| err.to_string())?;
    let conflicts = list_conflicts(&conn, None).map_err(|err| err.to_string())?;
    let logs = list_logs(&conn, None, None, None, None).map_err(|err| err.to_string())?;

    let today = Local::now().date_naive();
    let mut upload_count = 0;
    let mut download_count = 0;
    for log in &logs {
        let dt = Local.timestamp_millis_opt(log.created_at_ms).single();
        if let Some(dt) = dt {
            if dt.date_naive() == today {
                if log.event == "upload" {
                    upload_count += 1;
                }
                if log.event == "download" {
                    download_count += 1;
                }
            }
        }
    }

    let cards = vec![
        DashboardCard {
            label: "同步状态".to_string(),
            value: if tasks.iter().any(|t| t.status == "Syncing") {
                "运行中".to_string()
            } else {
                "已暂停".to_string()
            },
            tone: if tasks.iter().any(|t| t.status == "Syncing") {
                "success".to_string()
            } else {
                "warn".to_string()
            },
        },
        DashboardCard {
            label: "今日上传".to_string(),
            value: format!("{} 文件", upload_count),
            tone: "info".to_string(),
        },
        DashboardCard {
            label: "今日下载".to_string(),
            value: format!("{} 文件", download_count),
            tone: "info".to_string(),
        },
        DashboardCard {
            label: "未处理冲突".to_string(),
            value: conflicts.len().to_string(),
            tone: if conflicts.is_empty() {
                "info".to_string()
            } else {
                "danger".to_string()
            },
        },
    ];

    let activities = logs
        .into_iter()
        .map(|log| ActivityItem {
            timestamp: format_time(log.created_at_ms),
            event: log.event,
            detail: log.detail,
            level: log.level,
        })
        .collect();

    let conflict_items = list_conflicts_command(state, None)?;

    Ok(BootstrapPayload {
        cards,
        tasks,
        activities,
        conflicts: conflict_items,
    })
}

fn run_sync_once(db_path: &PathBuf, api_paths: &ApiPaths, task_id: &str) -> Result<SyncStats, Box<dyn Error>> {
    let (task, settings) = load_task_settings(db_path, task_id)?;
    let tokens = load_tokens(&settings.account_key)?;
    let engine = SyncEngine::new(task, api_paths.clone(), Some(tokens.access_token), db_path.clone());
    tauri::async_runtime::block_on(engine.sync_once())
}

fn update_task_stats(
    stats_map: &Arc<Mutex<HashMap<String, TaskStats>>>,
    task_id: &str,
    stats: SyncStats,
    elapsed: Duration,
) {
    let secs = elapsed.as_secs_f64().max(0.001);
    let rate_up = format_rate(stats.uploaded_bytes as f64 / secs);
    let rate_down = format_rate(stats.downloaded_bytes as f64 / secs);
    let snapshot = TaskStats {
        rate_up,
        rate_down,
        queue: stats.operations,
    };
    if let Ok(mut map) = stats_map.lock() {
        map.insert(task_id.to_string(), snapshot);
    }
}

fn log_error(db_path: &PathBuf, task_id: &str, detail: &str) {
    if let Ok(conn) = Connection::open(db_path) {
        let _ = conn.execute(
            "INSERT INTO logs (task_id, level, event, detail, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                task_id.to_string(),
                "error",
                "sync",
                detail.to_string(),
                now_ms(),
            ),
        );
    }
}

fn log_info(db_path: &PathBuf, task_id: &str, event: &str, detail: &str) {
    if let Ok(conn) = Connection::open(db_path) {
        let _ = conn.execute(
            "INSERT INTO logs (task_id, level, event, detail, created_at_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                task_id.to_string(),
                "info",
                event.to_string(),
                detail.to_string(),
                now_ms(),
            ),
        );
    }
}

fn parse_settings(raw: &str) -> TaskSettings {
    serde_json::from_str(raw).unwrap_or(TaskSettings {
        name: "未命名任务".to_string(),
        account_key: "".to_string(),
        sync_interval_secs: 60,
    })
}

fn load_task_settings(db_path: &PathBuf, task_id: &str) -> Result<(TaskRow, TaskSettings), Box<dyn Error>> {
    let conn = Connection::open(db_path)?;
    let tasks = list_tasks(&conn)?;
    let task = tasks
        .into_iter()
        .find(|item| item.task_id == task_id)
        .ok_or("task not found")?;
    let settings = parse_settings(&task.settings_json);
    Ok((task, settings))
}

fn latest_log_time(conn: &Connection, task_id: &str) -> Option<i64> {
    let logs = list_logs(conn, Some(task_id), None, None, None).ok()?;
    logs.first().map(|log| log.created_at_ms)
}

fn format_time(timestamp_ms: i64) -> String {
    let dt = Local.timestamp_millis_opt(timestamp_ms).single();
    dt.map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "--".to_string())
}

fn parent_path(path: &str) -> String {
    let path = PathBuf::from(path);
    path.parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "".to_string())
}

fn file_name(path: &str) -> String {
    let path = PathBuf::from(path);
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path.to_string_lossy().as_ref())
        .to_string()
}

fn format_rate(bytes_per_sec: f64) -> String {
    if bytes_per_sec <= 0.0 {
        return "0 B/s".to_string();
    }
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut value = bytes_per_sec;
    let mut idx = 0;
    while value >= 1024.0 && idx < units.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{:.0} {}", value, units[idx])
    } else {
        format!("{:.1} {}", value, units[idx])
    }
}

fn is_running(state: &AppState, task_id: &str) -> bool {
    state.runners.lock().map(|r| r.contains_key(task_id)).unwrap_or(false)
}

fn build_task_items(state: &AppState, conn: &Connection) -> Result<Vec<TaskItem>, Box<dyn Error>> {
    let tasks = list_tasks(conn)?;
    let stats_map = state.stats.lock().map_err(|_| "stats lock error")?;
    let mut output = Vec::new();
    for task in tasks {
        let settings = parse_settings(&task.settings_json);
        let status = if is_running(state, &task.task_id) {
            "Syncing".to_string()
        } else {
            "Idle".to_string()
        };
        let last_sync = latest_log_time(conn, &task.task_id)
            .map(format_time)
            .unwrap_or_else(|| "--".to_string());
        let stats = stats_map.get(&task.task_id).cloned().unwrap_or(TaskStats {
            rate_up: "0 B/s".to_string(),
            rate_down: "0 B/s".to_string(),
            queue: 0,
        });
        output.push(TaskItem {
            id: task.task_id.clone(),
            name: settings.name,
            mode: task.mode.clone(),
            local_path: task.local_root.clone(),
            remote_path: task.remote_root_uri.clone(),
            status,
            rate_up: stats.rate_up,
            rate_down: stats.rate_down,
            queue: stats.queue,
            last_sync,
        });
    }
    Ok(output)
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn Error>> {
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "立即同步", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &sync, &quit])?;
    let _tray = TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .ok_or("missing default window icon")?
                .clone(),
        )
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            "sync" => {
                let state = app.state::<AppState>();
                let db_path = state.db_path.clone();
                let api_paths = state.api_paths.clone();
                let stats_map = state.stats.clone();
                thread::spawn(move || {
                    if let Ok(conn) = Connection::open(&db_path) {
                        if let Ok(tasks) = list_tasks(&conn) {
                            for task in tasks {
                                let start = Instant::now();
                                if let Ok(stats) = run_sync_once(&db_path, &api_paths, &task.task_id) {
                                    update_task_stats(&stats_map, &task.task_id, stats, start.elapsed());
                                }
                            }
                        }
                    }
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn setup_window_events(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let window_for_event = window.clone();
        window.on_window_event(move |event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window_for_event.hide();
                }
                WindowEvent::Resized(_) => {
                    if window_for_event.is_minimized().unwrap_or(false) {
                        let _ = window_for_event.hide();
                    }
                }
                _ => {}
            }
        });
    }
}

fn db_path() -> Result<PathBuf, Box<dyn Error>> {
    let path = config_dir()?.join("cloudreve.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn collect_share_paths_from_args() -> Vec<String> {
    let mut args = std::env::args().skip(1);
    let mut paths = Vec::new();
    let mut collect_all = false;
    while let Some(arg) = args.next() {
        if collect_all {
            if !arg.trim().is_empty() {
                paths.push(arg);
            }
            continue;
        }
        if arg == "--share" {
            collect_all = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--share=") {
            if !value.trim().is_empty() {
                paths.push(value.to_string());
            }
        }
    }
    paths
}

fn emit_share_requests(app: &AppHandle, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        for path in paths {
            let _ = window.emit(
                "share-request",
                ShareRequestPayload { path },
            );
        }
    }
}

#[cfg(target_os = "linux")]
fn install_linux_share_menus() -> Result<(), Box<dyn Error>> {
    let exe_path = std::env::current_exe()?.to_string_lossy().to_string();
    let base = directories::BaseDirs::new().ok_or("failed to locate data dir")?;
    let data_dir = base.data_dir();

    let nautilus_dir = data_dir.join("nautilus/scripts");
    fs::create_dir_all(&nautilus_dir)?;
    let nautilus_script = nautilus_dir.join("Cloudreve Sync - Create Share Link");
    let script_body = format!("#!/bin/sh\n\"{}\" --share \"$@\"\n", exe_path.replace('"', "\\\""));
    fs::write(&nautilus_script, script_body)?;
    let mut perms = fs::metadata(&nautilus_script)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&nautilus_script, perms)?;

    let kde_dir = data_dir.join("kservices5/ServiceMenus");
    fs::create_dir_all(&kde_dir)?;
    let kde_menu = kde_dir.join("cloudreve-sync-share.desktop");
    let menu_body = format!(
        "[Desktop Entry]\nType=Service\nX-KDE-ServiceTypes=KonqPopupMenu/Plugin\nMimeType=all/all;\nActions=cloudreveShare;\nX-KDE-Submenu=Cloudreve Sync\n\n[Desktop Action cloudreveShare]\nName=创建分享链接\nIcon=cloudreve-sync\nExec=\"{}\" --share %F\n",
        exe_path.replace('"', "\\\"")
    );
    fs::write(&kde_menu, menu_body)?;

    Ok(())
}

fn refresh_tokens_once(db_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let conn = Connection::open(db_path)?;
    init_db(&conn)?;
    let accounts = list_accounts(&conn)?;
    for account in accounts {
        let tokens = match load_tokens(&account.account_key) {
            Ok(tokens) => tokens,
            Err(_) => continue,
        };
        if tokens.refresh_token.is_empty() {
            continue;
        }
        let refreshed = tauri::async_runtime::block_on(refresh_token(
            &account.base_url,
            &tokens.refresh_token,
        ));
        let refreshed = match refreshed {
            Ok(value) => value,
            Err(_) => continue,
        };
        let _ = store_tokens(
            &account.account_key,
            &refreshed.access_token,
            &refreshed.refresh_token,
        );
    }
    Ok(())
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    let db_path = db_path().expect("db path");
    let conn = Connection::open(&db_path).expect("db open");
    init_db(&conn).expect("db init");

    let state = AppState {
        db_path,
        api_paths: ApiPaths::default(),
        runners: Mutex::new(HashMap::new()),
        stats: Arc::new(Mutex::new(HashMap::new())),
    };

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle();
            setup_tray(&handle)?;
            setup_window_events(&handle);
            #[cfg(target_os = "linux")]
            {
                if let Err(err) = install_linux_share_menus() {
                    eprintln!("failed to install share menu: {}", err);
                }
            }
            emit_share_requests(&handle, collect_share_paths_from_args());
            let state = app.state::<AppState>();
            let db_path = state.db_path.clone();
            thread::spawn(move || loop {
                let _ = refresh_tokens_once(&db_path);
                thread::sleep(Duration::from_secs(TOKEN_REFRESH_INTERVAL_SECS));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            login,
            finish_sign_in_with_2fa_command,
            get_captcha_command,
            test_connection,
            create_task_command,
            list_tasks_command,
            list_accounts_command,
            list_remote_entries_command,
            create_share_link_command,
            get_settings_command,
            save_settings_command,
            clear_credentials_command,
            open_local_path,
            open_external,
            mark_conflict_resolved,
            download_conflict_remote,
            hash_local_file,
            get_diagnostics_command,
            export_logs_command,
            list_conflicts_command,
            list_logs_command,
            run_sync_command,
            stop_sync_command,
            delete_task_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
#[derive(Serialize)]
struct LogsPage {
    total: u32,
    items: Vec<ActivityItem>,
}
