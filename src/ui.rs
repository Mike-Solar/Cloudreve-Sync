use crate::config::{logs_path, AppConfig};
use crate::logging::{LogEntry, LogLevel, LogStore};
use crate::sync::SyncEngine;
use crossbeam_channel::{Receiver, Sender};
use fltk::app::{self, App};
use fltk::browser::HoldBrowser;
use fltk::button::{Button, CheckButton};
use fltk::dialog::dir_chooser;
use fltk::enums::{Color, Event, Font, FrameType};
use fltk::frame::Frame;
use fltk::group::Flex;
use fltk::input::Input;
use fltk::menu::MenuButton;
use fltk::prelude::*;
use fltk::window::Window;
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
enum SyncCommand {
    Start,
    Stop,
    RunOnce,
    UpdateConfig(AppConfig),
    CreateShare { remote_path: String },
}

#[derive(Debug, Clone)]
enum TrayEvent {
    Show,
    Hide,
    Exit,
    SyncNow,
}

pub fn run_ui() -> Result<(), Box<dyn Error>> {
    let mut config = AppConfig::load()?;
    let app = App::default();
    app::set_scheme(app::Scheme::Gtk);

    // 统一配色与排版，保持扁平化观感。
    let bg_color = Color::from_rgb(242, 243, 245);
    let panel_color = Color::from_rgb(255, 255, 255);
    let accent = Color::from_rgb(38, 91, 125);
    let accent_soft = Color::from_rgb(222, 234, 241);
    let text_dark = Color::from_rgb(35, 38, 42);
    let text_muted = Color::from_rgb(96, 103, 112);

    let (log_tx, log_rx) = crossbeam_channel::unbounded::<LogEntry>();
    let (ui_tx, ui_rx) = app::channel::<LogEntry>();
    thread::spawn(move || {
        while let Ok(entry) = log_rx.recv() {
            ui_tx.send(entry);
        }
    });

    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<SyncCommand>();
    let (tray_tx, tray_rx) = crossbeam_channel::unbounded::<TrayEvent>();
    spawn_sync_worker(config.clone(), log_tx.clone(), cmd_rx);

    let mut win = Window::new(100, 100, 980, 680, "Cloudreve Sync");
    win.set_color(bg_color);

    let mut header = Frame::new(0, 0, 980, 72, "Cloudreve Sync");
    header.set_frame(FrameType::FlatBox);
    header.set_color(accent);
    header.set_label_color(Color::from_rgb(247, 248, 249));
    header.set_label_font(Font::HelveticaBold);
    header.set_label_size(22);

    let mut sub_header = Frame::new(0, 50, 980, 22, "Two-way sync, conflict-safe");
    sub_header.set_frame(FrameType::FlatBox);
    sub_header.set_color(accent);
    sub_header.set_label_color(accent_soft);
    sub_header.set_label_font(Font::Helvetica);
    sub_header.set_label_size(12);

    let mut root = Flex::new(20, 96, 940, 560, None);
    root.set_type(fltk::group::FlexType::Row);
    root.set_pad(18);

    let mut left = Flex::default();
    left.set_type(fltk::group::FlexType::Column);
    left.set_margin(12);
    left.set_pad(12);

    let mut right = Flex::default();
    right.set_type(fltk::group::FlexType::Column);
    right.set_margin(12);
    right.set_pad(12);

    root.end();

    let mut config_title = section_title("Configuration", panel_color, text_dark);
    left.fixed(&config_title, 26);

    let mut base_url = labeled_input(&mut left, "Base URL", &config.base_url);
    let mut access_token = labeled_input(&mut left, "Access Token", &config.access_token);
    let mut local_root = labeled_input(&mut left, "Local Root", &config.local_root);
    let mut remote_root = labeled_input(&mut left, "Remote Root", &config.remote_root);
    let mut interval = labeled_input(
        &mut left,
        "Sync Interval (s)",
        &config.sync_interval_secs.to_string(),
    );
    let mut placeholders = CheckButton::default().with_label("Windows 11 Placeholders");
    placeholders.set_label_font(Font::Helvetica);
    placeholders.set_label_color(text_muted);
    placeholders.set_value(config.use_placeholders);
    left.fixed(&placeholders, 26);

    let mut actions_title = section_title("Actions", panel_color, text_dark);
    left.fixed(&actions_title, 26);

    let mut controls = Flex::default();
    controls.set_type(fltk::group::FlexType::Row);
    controls.set_pad(10);
    let mut save_btn = Button::default().with_label("Save");
    let mut browse_btn = Button::default().with_label("Pick Folder");
    let mut start_btn = Button::default().with_label("Start Sync");
    let mut once_btn = Button::default().with_label("Sync Now");
    style_button(&mut save_btn, false, accent, accent_soft, text_dark);
    style_button(&mut browse_btn, false, accent, accent_soft, text_dark);
    style_button(&mut start_btn, true, accent, accent_soft, text_dark);
    style_button(&mut once_btn, true, accent, accent_soft, text_dark);
    controls.fixed(&save_btn, 140);
    controls.fixed(&browse_btn, 160);
    controls.fixed(&start_btn, 160);
    controls.fixed(&once_btn, 140);
    controls.end();
    left.fixed(&controls, 46);
    left.end();

    let mut files_title = section_title("Local Files", panel_color, text_dark);
    let mut file_list = HoldBrowser::default();
    style_browser(&mut file_list, panel_color);

    let mut log_title = section_title("Sync Log", panel_color, text_dark);
    let mut log_list = HoldBrowser::default();
    style_browser(&mut log_list, panel_color);

    right.fixed(&files_title, 26);
    right.fixed(&file_list, 210);
    right.fixed(&log_title, 26);
    right.fixed(&log_list, 270);
    right.end();

    let log_list = Arc::new(Mutex::new(log_list));
    let file_list = Arc::new(Mutex::new(file_list));

    load_log_history(&log_list);
    load_file_list(&config, &file_list);

    let mut menu = MenuButton::new(0, 0, 0, 0, "");
    menu.add_choice("Create Share Link");

    let running = Arc::new(Mutex::new(false));
    spawn_tray(tray_tx.clone());
    {
        let cmd_tx = cmd_tx.clone();
        let config_ref = Arc::new(Mutex::new(config.clone()));
        let running_ref = running.clone();
        let file_list = Arc::clone(&file_list);
        win.handle(move |w, ev| match ev {
            Event::Hide | Event::Close => {
                w.hide();
                true
            }
            _ => false,
        });

        browse_btn.set_callback({
            let mut local_root = local_root.clone();
            move |_| {
                if let Some(path) = dir_chooser("Choose Sync Folder", ".", false) {
                    local_root.set_value(&path);
                }
            }
        });

        save_btn.set_callback({
            let mut config_ref = config_ref.clone();
            let cmd_tx = cmd_tx.clone();
            let file_list = Arc::clone(&file_list);
            let base_url = base_url.clone();
            let access_token = access_token.clone();
            let local_root = local_root.clone();
            let remote_root = remote_root.clone();
            let interval = interval.clone();
            let placeholders = placeholders.clone();
            move |_| {
                let mut cfg = config_ref.lock().expect("config lock");
                cfg.base_url = base_url.value();
                cfg.access_token = access_token.value();
                cfg.local_root = local_root.value();
                cfg.remote_root = remote_root.value();
                cfg.sync_interval_secs = interval.value().parse().unwrap_or(60);
                cfg.use_placeholders = placeholders.value();
                let _ = cfg.save();
                let _ = cmd_tx.send(SyncCommand::UpdateConfig(cfg.clone()));
                load_file_list(&cfg, &file_list);
            }
        });

        start_btn.set_callback({
            let cmd_tx = cmd_tx.clone();
            let running_ref = running_ref.clone();
            let mut start_btn = start_btn.clone();
            move |_| {
                let mut running = running_ref.lock().expect("running lock");
                if *running {
                    let _ = cmd_tx.send(SyncCommand::Stop);
                    *running = false;
                    start_btn.set_label("Start Sync");
                } else {
                    let _ = cmd_tx.send(SyncCommand::Start);
                    *running = true;
                    start_btn.set_label("Stop Sync");
                }
            }
        });

        once_btn.set_callback({
            let cmd_tx = cmd_tx.clone();
            move |_| {
                let _ = cmd_tx.send(SyncCommand::RunOnce);
            }
        });

        file_list.lock().unwrap().handle({
            let menu = menu.clone();
            let cmd_tx = cmd_tx.clone();
            let config_ref = config_ref.clone();
            let file_list = Arc::clone(&file_list);
            move |_, ev| {
                if ev == Event::Push && app::event_mouse_button() == app::MouseButton::Right {
                    menu.popup();
                    if menu.value() == 1 {
                        if let Some(path) = selected_path(&file_list) {
                            let cfg = config_ref.lock().expect("config lock");
                            let remote = build_remote_path(&cfg.remote_root, &cfg.local_root, &path);
                            let _ = cmd_tx.send(SyncCommand::CreateShare { remote_path: remote });
                        }
                    }
                    return true;
                }
                false
            }
        });
    }

    win.end();
    win.show();

    while app.wait() {
        if let Some(entry) = ui_rx.recv() {
            let line = format_log_line(&entry);
            if let Ok(mut list) = log_list.lock() {
                list.add(&line);
                let size = list.size();
                list.bottom_line(size);
            }
            if entry.message.contains("Share link") {
                // 通过弹窗反馈分享链接结果，提升友好度。
                fltk::dialog::message_default(&entry.message);
            }
        }
        if let Ok(event) = tray_rx.try_recv() {
            match event {
                TrayEvent::Show => win.show(),
                TrayEvent::Hide => win.hide(),
                TrayEvent::Exit => {
                    win.hide();
                    break;
                }
                TrayEvent::SyncNow => {
                    let _ = cmd_tx.send(SyncCommand::RunOnce);
                }
            }
        }
    }

    Ok(())
}

fn spawn_sync_worker(config: AppConfig, log_tx: Sender<LogEntry>, cmd_rx: Receiver<SyncCommand>) {
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let mut running = false;
        let mut cfg = config;
        let log_store = LogStore::new(logs_path().expect("logs path"));
        let mut engine = SyncEngine::new(cfg.clone(), log_store.clone());

        loop {
            if running {
                match cmd_rx.recv_timeout(Duration::from_secs(cfg.sync_interval_secs)) {
                    Ok(cmd) => handle_command(
                        cmd,
                        &mut running,
                        &mut cfg,
                        &mut engine,
                        &log_tx,
                        &rt,
                    ),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        rt.block_on(engine.sync_once(&log_tx));
                    }
                    Err(_) => break,
                }
            } else {
                match cmd_rx.recv() {
                    Ok(cmd) => handle_command(
                        cmd,
                        &mut running,
                        &mut cfg,
                        &mut engine,
                        &log_tx,
                        &rt,
                    ),
                    Err(_) => break,
                }
            }
        }
    });
}

fn handle_command(
    cmd: SyncCommand,
    running: &mut bool,
    cfg: &mut AppConfig,
    engine: &mut SyncEngine,
    log_tx: &Sender<LogEntry>,
    rt: &tokio::runtime::Runtime,
) {
    match cmd {
        SyncCommand::Start => *running = true,
        SyncCommand::Stop => *running = false,
        SyncCommand::RunOnce => {
            rt.block_on(engine.sync_once(log_tx));
        }
        SyncCommand::UpdateConfig(new_cfg) => {
            *cfg = new_cfg.clone();
            *engine = SyncEngine::new(new_cfg, LogStore::new(logs_path().expect("logs path")));
        }
        SyncCommand::CreateShare { remote_path } => {
            let result = rt.block_on(engine.create_share_link(&remote_path));
            let message = match result {
                Ok(link) => format!("Share link created for {}: {}", remote_path, link),
                Err(err) => format!("Failed to create share link for {}: {}", remote_path, err),
            };
            let entry = LogEntry::new(LogLevel::Info, message);
            let _ = LogStore::new(logs_path().expect("logs path")).append(&entry);
            let _ = log_tx.send(entry);
        }
    }
}

fn spawn_tray(tx: Sender<TrayEvent>) {
    thread::spawn(move || {
        let mut tray = match tray_item::TrayItem::new(
            "Cloudreve Sync",
            tray_item::IconSource::Resource(""),
        ) {
            Ok(tray) => tray,
            Err(_) => return,
        };
        let tx_show = tx.clone();
        let _ = tray.add_menu_item("Show", move || {
            let _ = tx_show.send(TrayEvent::Show);
        });
        let tx_hide = tx.clone();
        let _ = tray.add_menu_item("Hide", move || {
            let _ = tx_hide.send(TrayEvent::Hide);
        });
        let tx_sync = tx.clone();
        let _ = tray.add_menu_item("Sync Now", move || {
            let _ = tx_sync.send(TrayEvent::SyncNow);
        });
        let tx_exit = tx.clone();
        let _ = tray.add_menu_item("Exit", move || {
            let _ = tx_exit.send(TrayEvent::Exit);
        });
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn labeled_input(parent: &mut Flex, label: &str, value: &str) -> Input {
    let mut frame = Frame::default().with_label(label);
    frame.set_label_font(Font::Helvetica);
    frame.set_label_size(12);
    frame.set_frame(FrameType::FlatBox);
    frame.set_color(Color::from_rgb(237, 240, 244));
    parent.fixed(&frame, 22);

    let mut input = Input::default();
    input.set_value(value);
    input.set_text_font(Font::Helvetica);
    input.set_text_size(12);
    input.set_frame(FrameType::FlatBox);
    input.set_color(Color::from_rgb(255, 255, 255));
    parent.fixed(&input, 32);
    input
}

fn style_browser(browser: &mut HoldBrowser, panel: Color) {
    browser.set_frame(FrameType::FlatBox);
    browser.set_text_size(12);
    browser.set_color(panel);
    browser.set_selection_color(Color::from_rgb(210, 224, 236));
}

fn section_title(title: &str, panel: Color, text: Color) -> Frame {
    let mut frame = Frame::default().with_label(title);
    frame.set_label_font(Font::HelveticaBold);
    frame.set_label_size(13);
    frame.set_label_color(text);
    frame.set_frame(FrameType::FlatBox);
    frame.set_color(panel);
    frame
}

fn style_button(btn: &mut Button, primary: bool, accent: Color, soft: Color, text: Color) {
    btn.set_frame(FrameType::FlatBox);
    if primary {
        btn.set_color(accent);
        btn.set_label_color(Color::from_rgb(247, 248, 249));
    } else {
        btn.set_color(soft);
        btn.set_label_color(text);
    }
    btn.set_label_font(Font::HelveticaBold);
    btn.set_label_size(12);
}

fn format_log_line(entry: &LogEntry) -> String {
    let level = match entry.level {
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERROR",
    };
    format!("[{}] {} - {}", entry.timestamp, level, entry.message)
}

fn load_file_list(config: &AppConfig, list: &Arc<Mutex<HoldBrowser>>) {
    if config.local_root.is_empty() {
        return;
    }
    let mut entries = Vec::new();
    for entry in WalkDir::new(&config.local_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        entries.push(entry.path().to_path_buf());
    }
    if let Ok(mut list) = list.lock() {
        list.clear();
        for path in entries {
            list.add(&path.to_string_lossy());
        }
    }
}

fn load_log_history(list: &Arc<Mutex<HoldBrowser>>) {
    let store = LogStore::new(logs_path().expect("logs path"));
    let entries = match store.load_all() {
        Ok(entries) => entries,
        Err(_) => return,
    };
    if let Ok(mut list) = list.lock() {
        for entry in entries {
            list.add(&format_log_line(&entry));
        }
        let size = list.size();
        list.bottom_line(size);
    }
}

fn selected_path(list: &Arc<Mutex<HoldBrowser>>) -> Option<String> {
    let list = list.lock().ok()?;
    let idx = list.value();
    if idx <= 0 {
        return None;
    }
    list.text(idx)
}

fn build_remote_path(root: &str, local_root: &str, local_path: &str) -> String {
    let root = root.trim_end_matches('/');
    let rel = Path::new(local_path)
        .strip_prefix(local_root)
        .unwrap_or(Path::new(local_path))
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    if rel.is_empty() {
        root.to_string()
    } else {
        format!("{}/{}", root, rel.trim_start_matches('/'))
    }
}
