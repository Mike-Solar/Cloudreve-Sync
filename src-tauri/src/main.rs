mod core;

use core::cloudreve::{get_captcha, password_sign_in, CloudreveClient};
use core::config::ApiPaths;
use core::credentials::{load_tokens, store_tokens};
use core::db::{create_task, init_db, list_conflicts, list_logs, list_tasks, now_ms, TaskRow};
use core::sync::SyncEngine;
use chrono::{Local, TimeZone};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{
    AppHandle,
    Manager,
    WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use uuid::Uuid;

#[derive(Clone)]
struct RunnerHandle {
    stop: Arc<AtomicBool>,
}

struct AppState {
    db_path: PathBuf,
    api_paths: ApiPaths,
    runners: Mutex<HashMap<String, RunnerHandle>>,
}

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
    name: String,
    task: String,
    path: String,
    device: String,
    time: String,
    status: String,
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

#[derive(Serialize)]
struct LoginResult {
    account_key: String,
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
}

#[derive(Deserialize)]
struct SyncRequest {
    task_id: String,
}

#[derive(Serialize, Deserialize)]
struct TaskSettings {
    name: String,
    account_key: String,
    sync_interval_secs: u64,
}

#[tauri::command]
fn login(_state: tauri::State<AppState>, payload: LoginRequest) -> Result<LoginResult, String> {
    let result = tauri::async_runtime::block_on(password_sign_in(
        &payload.base_url,
        &payload.email,
        &payload.password,
        payload.captcha.as_deref(),
        payload.ticket.as_deref(),
    ))
    .map_err(|err| err.to_string())?;

    let account_key = format!("{}|{}", payload.base_url, payload.email);
    store_tokens(
        &account_key,
        &result.token.access_token,
        &result.token.refresh_token,
    )
    .map_err(|err| err.to_string())?;

    Ok(LoginResult { account_key })
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
fn list_conflicts_command(state: tauri::State<AppState>, task_id: Option<String>) -> Result<Vec<ConflictItem>, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    let conflicts = list_conflicts(&conn, task_id.as_deref()).map_err(|err| err.to_string())?;
    let tasks = list_tasks(&conn).map_err(|err| err.to_string())?;
    let task_map = tasks
        .into_iter()
        .map(|task| (task.task_id, parse_settings(&task.settings_json).name))
        .collect::<HashMap<_, _>>();
    Ok(conflicts
        .into_iter()
        .map(|item| ConflictItem {
            id: format!("{}:{}", item.task_id, item.conflict_relpath),
            name: file_name(&item.original_relpath),
            task: task_map.get(&item.task_id).cloned().unwrap_or_else(|| item.task_id.clone()),
            path: parent_path(&item.original_relpath),
            device: "".to_string(),
            time: format_time(item.created_at_ms),
            status: "未处理".to_string(),
        })
        .collect())
}

#[tauri::command]
fn list_logs_command(state: tauri::State<AppState>, query: LogsQuery) -> Result<Vec<ActivityItem>, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    let logs = list_logs(&conn, query.task_id.as_deref(), query.level.as_deref()).map_err(|err| err.to_string())?;
    Ok(logs
        .into_iter()
        .map(|log| ActivityItem {
            timestamp: format_time(log.created_at_ms),
            event: log.event,
            detail: log.detail,
            level: log.level,
        })
        .collect())
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
            let result = run_sync_once(&db_path, &api_paths, &task_id);
            if let Err(err) = result {
                let detail = err.to_string();
                log_error(&db_path, &task_id, &detail);
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
fn bootstrap(state: tauri::State<AppState>) -> Result<BootstrapPayload, String> {
    let conn = Connection::open(&state.db_path).map_err(|err| err.to_string())?;
    let tasks = build_task_items(&state, &conn).map_err(|err| err.to_string())?;
    let conflicts = list_conflicts(&conn, None).map_err(|err| err.to_string())?;
    let logs = list_logs(&conn, None, None).map_err(|err| err.to_string())?;

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

fn run_sync_once(db_path: &PathBuf, api_paths: &ApiPaths, task_id: &str) -> Result<(), Box<dyn Error>> {
    let (task, settings) = load_task_settings(db_path, task_id)?;
    let tokens = load_tokens(&settings.account_key)?;
    let engine = SyncEngine::new(task, api_paths.clone(), Some(tokens.access_token), db_path.clone());
    tauri::async_runtime::block_on(engine.sync_once())?;
    Ok(())
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
    let logs = list_logs(conn, Some(task_id), None).ok()?;
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

fn is_running(state: &AppState, task_id: &str) -> bool {
    state.runners.lock().map(|r| r.contains_key(task_id)).unwrap_or(false)
}

fn build_task_items(state: &AppState, conn: &Connection) -> Result<Vec<TaskItem>, Box<dyn Error>> {
    let tasks = list_tasks(conn)?;
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
        output.push(TaskItem {
            id: task.task_id.clone(),
            name: settings.name,
            mode: task.mode.clone(),
            local_path: task.local_root.clone(),
            remote_path: task.remote_root_uri.clone(),
            status,
            rate_up: "--".to_string(),
            rate_down: "--".to_string(),
            queue: 0,
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
                thread::spawn(move || {
                    if let Ok(conn) = Connection::open(&db_path) {
                        if let Ok(tasks) = list_tasks(&conn) {
                            for task in tasks {
                                let _ = run_sync_once(&db_path, &api_paths, &task.task_id);
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
    let proj = directories::ProjectDirs::from("org", "cloudreve", "CloudreveSync")
        .ok_or("failed to locate config dir")?;
    let path = proj.config_dir().join("cloudreve.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}

fn main() {
    let db_path = db_path().expect("db path");
    let conn = Connection::open(&db_path).expect("db open");
    init_db(&conn).expect("db init");

    let state = AppState {
        db_path,
        api_paths: ApiPaths::default(),
        runners: Mutex::new(HashMap::new()),
    };

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            let handle = app.handle();
            setup_tray(&handle)?;
            setup_window_events(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            login,
            get_captcha_command,
            test_connection,
            create_task_command,
            list_tasks_command,
            list_conflicts_command,
            list_logs_command,
            run_sync_command,
            stop_sync_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
