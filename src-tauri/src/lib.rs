mod codex;

use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{mpsc, OnceLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "codex-usage-tray";
const MENU_OPEN_ID: &str = "tray-open";
const MENU_HIDE_ID: &str = "tray-hide";
const MENU_QUIT_ID: &str = "tray-quit";
const STATE_INVALIDATED_EVENT: &str = "codex://state-invalidated";
const TRACE_REFRESH_ENV: &str = "CODEX_USAGE_TRACE_REFRESH";
const TRACE_REFRESH_LOG_PATH: &str = "/tmp/codex-usage-trace.log";
const WINDOW_MARGIN: i32 = 18;
const SESSION_INVALIDATION_MIN_INTERVAL: Duration = Duration::from_secs(15);
const WINDOW_HIDE_ON_BLUR_DELAY: Duration = Duration::from_millis(220);
static WATCHER_TX: OnceLock<mpsc::Sender<WatchMessage>> = OnceLock::new();

#[derive(Clone, Debug)]
struct WatchPaths {
    codex_home: PathBuf,
    registry_path: PathBuf,
    active_auth_path: PathBuf,
    sessions_root: PathBuf,
}

enum WatchMessage {
    Fs(notify::Result<Event>),
    SetSessionsTracking(bool),
}

#[tauri::command]
fn get_app_snapshot() -> Result<codex::AppSnapshot, String> {
    trace_refresh("get_app_snapshot");
    codex::load_app_snapshot()
}

#[tauri::command]
fn switch_account(account_key: String) -> Result<codex::AppSnapshot, String> {
    codex::switch_account(account_key)
}

#[tauri::command]
fn remove_account(account_key: String) -> Result<codex::AppSnapshot, String> {
    codex::remove_account(account_key)
}

#[tauri::command]
fn update_settings(input: codex::SettingsUpdate) -> Result<codex::AppSnapshot, String> {
    codex::update_settings(input)
}

#[tauri::command]
fn launch_add_account_login() -> Result<String, String> {
    codex::launch_add_account_login()
}

#[tauri::command]
fn manage_auto_switch_service(action: String) -> Result<codex::ServiceActionResult, String> {
    codex::manage_auto_switch_service(action)
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

pub fn try_run_cli_from_args() -> Option<Result<(), String>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return None;
    }

    let first = args[0].as_str();
    let result = match first {
        "--auto-switch-check" => codex::run_auto_switch_check()
            .map(|outcome| {
                if outcome.did_switch {
                    println!("{}", outcome.message);
                } else {
                    eprintln!("{}", outcome.message);
                }
            })
            .map_err(|error| format!("auto-switch check failed: {error}")),
        "--install-auto-switch-service" => codex::manage_auto_switch_service("install".to_string())
            .map(|result| println!("{}", result.message))
            .map_err(|error| format!("auto-switch service install failed: {error}")),
        "--start-auto-switch-service" => codex::manage_auto_switch_service("start".to_string())
            .map(|result| println!("{}", result.message))
            .map_err(|error| format!("auto-switch service start failed: {error}")),
        "--stop-auto-switch-service" => codex::manage_auto_switch_service("stop".to_string())
            .map(|result| println!("{}", result.message))
            .map_err(|error| format!("auto-switch service stop failed: {error}")),
        "--uninstall-auto-switch-service" => {
            codex::manage_auto_switch_service("uninstall".to_string())
                .map(|result| println!("{}", result.message))
                .map_err(|error| format!("auto-switch service uninstall failed: {error}"))
        }
        _ => return None,
    };

    Some(result)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            setup_main_window(app)?;
            setup_tray(app)?;
            start_state_watcher(app.handle().clone());
            Ok(())
        })
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_icon_event)
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            get_app_snapshot,
            switch_account,
            remove_account,
            update_settings,
            launch_add_account_login,
            manage_auto_switch_service,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("failed to run codex-usage");
}

fn setup_main_window<R: Runtime>(app: &mut tauri::App<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.set_skip_taskbar(true);
        let _ = window.set_always_on_top(true);
        let _ = position_main_window(&window);
        let _ = window.hide();
    }

    Ok(())
}

fn setup_tray<R: Runtime>(app: &mut tauri::App<R>) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, MENU_OPEN_ID, "Open Codex Usage", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, MENU_HIDE_ID, "Hide Window", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, MENU_QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &hide_item, &separator, &quit_item])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Codex Usage")
        .show_menu_on_left_click(false);

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    let _ = builder.build(app)?;
    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MENU_OPEN_ID => {
            let _ = show_main_window(app);
        }
        MENU_HIDE_ID => {
            let _ = hide_main_window(app);
        }
        MENU_QUIT_ID => app.exit(0),
        _ => {}
    }
}

fn handle_tray_icon_event<R: Runtime>(app: &AppHandle<R>, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
        | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => {
            let _ = show_main_window(app);
        }
        _ => {}
    }
}

fn handle_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL {
        return;
    }

    match event {
        WindowEvent::Focused(true) => {
            set_sessions_tracking(true);
        }
        WindowEvent::Focused(false) => {
            schedule_hide_after_blur(window.app_handle().clone(), window.label().to_string());
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            set_sessions_tracking(false);
            let _ = window.hide();
        }
        _ => {}
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    position_main_window(&window)?;
    window.show()?;
    window.set_focus()?;
    set_sessions_tracking(true);
    Ok(())
}

fn hide_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Ok(());
    };
    set_sessions_tracking(false);
    window.hide()?;
    Ok(())
}

fn schedule_hide_after_blur<R: Runtime>(app: AppHandle<R>, label: String) {
    thread::spawn(move || {
        thread::sleep(WINDOW_HIDE_ON_BLUR_DELAY);

        let Some(window) = app.get_webview_window(&label) else {
            return;
        };

        if !window.is_visible().unwrap_or(false) {
            return;
        }

        if window.is_focused().unwrap_or(false) {
            set_sessions_tracking(true);
            return;
        }

        if cursor_is_inside_window(&window) {
            trace_refresh("blur-hide-skipped:pointer-inside");
            return;
        }

        set_sessions_tracking(false);
        let _ = window.hide();
    });
}

fn cursor_is_inside_window<R: Runtime>(window: &tauri::WebviewWindow<R>) -> bool {
    let Ok(cursor) = window.cursor_position() else {
        return false;
    };
    let Ok(position) = window.outer_position() else {
        return false;
    };
    let Ok(size) = window.outer_size() else {
        return false;
    };

    let left = f64::from(position.x);
    let top = f64::from(position.y);
    let right = left + f64::from(size.width);
    let bottom = top + f64::from(size.height);

    cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom
}

fn position_main_window<R: Runtime>(window: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
    let monitor = window
        .current_monitor()?
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return Ok(());
    };

    let work_area = monitor.work_area();
    let size = window
        .outer_size()
        .unwrap_or_else(|_| PhysicalSize::new(460_u32, 860_u32));
    let x = work_area.position.x + work_area.size.width as i32 - size.width as i32 - WINDOW_MARGIN;
    let y = work_area.position.y + WINDOW_MARGIN;
    let clamped_x = x.max(work_area.position.x);
    let clamped_y = y.max(work_area.position.y);

    window.set_position(PhysicalPosition::new(clamped_x, clamped_y))?;
    Ok(())
}

fn start_state_watcher<R: Runtime>(app: AppHandle<R>) {
    let Some(paths) = resolve_watch_paths() else {
        return;
    };
    let initial_sessions_tracking = is_main_window_visible(&app);
    let (tx, rx) = mpsc::channel();
    let _ = WATCHER_TX.set(tx.clone());

    thread::spawn(move || {
        let mut watcher = match recommended_watcher(move |result| {
            let _ = tx.send(WatchMessage::Fs(result));
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                let message = format!("watch-error:init:{error}");
                trace_refresh(&message);
                return;
            }
        };

        if let Err(error) = configure_state_watches(&mut watcher, &paths) {
            let message = format!("watch-error:configure:{error}");
            trace_refresh(&message);
            return;
        }

        let mut sessions_tracking = initial_sessions_tracking;
        let mut sessions_watched = false;
        let mut last_session_emit_at = None;

        if let Err(error) = sync_sessions_watch(
            &mut watcher,
            &paths,
            sessions_tracking,
            &mut sessions_watched,
        ) {
            let message = format!("watch-error:sessions-init:{error}");
            trace_refresh(&message);
        }

        while let Ok(message) = rx.recv() {
            match message {
                WatchMessage::SetSessionsTracking(enabled) => {
                    sessions_tracking = enabled;

                    if let Err(error) = sync_sessions_watch(
                        &mut watcher,
                        &paths,
                        sessions_tracking,
                        &mut sessions_watched,
                    ) {
                        let message = format!("watch-error:sessions-sync:{error}");
                        trace_refresh(&message);
                    }

                    continue;
                }
                WatchMessage::Fs(result) => {
                    let event = match result {
                        Ok(event) => event,
                        Err(error) => {
                            let message = format!("watch-error:event:{error}");
                            trace_refresh(&message);
                            continue;
                        }
                    };

                    if sessions_tracking
                        && !sessions_watched
                        && paths.sessions_root.exists()
                        && sync_sessions_watch(
                            &mut watcher,
                            &paths,
                            sessions_tracking,
                            &mut sessions_watched,
                        )
                        .is_err()
                    {
                        trace_refresh("watch-error:sessions-attach");
                    }

                    if !event_has_relevant_kind(&event) {
                        continue;
                    }

                    if is_stable_event(&paths, &event) {
                        trace_refresh("state-invalidated:stable");
                        let _ = app.emit_to(
                            MAIN_WINDOW_LABEL,
                            STATE_INVALIDATED_EVENT,
                            "external-change",
                        );
                        continue;
                    }

                    if sessions_tracking && is_rollout_event(&paths, &event) {
                        let now = SystemTime::now();
                        let should_emit = last_session_emit_at
                            .and_then(|at| now.duration_since(at).ok())
                            .map(|elapsed| elapsed >= SESSION_INVALIDATION_MIN_INTERVAL)
                            .unwrap_or(true);

                        if should_emit {
                            trace_refresh("state-invalidated:session");
                            last_session_emit_at = Some(now);
                            let _ = app.emit_to(
                                MAIN_WINDOW_LABEL,
                                STATE_INVALIDATED_EVENT,
                                "external-change",
                            );
                        }
                    }
                }
            }
        }
    });
}

fn is_main_window_visible<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

fn resolve_watch_paths() -> Option<WatchPaths> {
    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    let codex_home = PathBuf::from(home).join(".codex");

    Some(WatchPaths {
        codex_home: codex_home.clone(),
        registry_path: codex_home.join("accounts").join("registry.json"),
        active_auth_path: codex_home.join("auth.json"),
        sessions_root: codex_home.join("sessions"),
    })
}

fn configure_state_watches(
    watcher: &mut RecommendedWatcher,
    paths: &WatchPaths,
) -> notify::Result<()> {
    watcher.watch(&paths.codex_home, RecursiveMode::NonRecursive)?;

    if let Some(accounts_dir) = paths.registry_path.parent() {
        watcher.watch(accounts_dir, RecursiveMode::NonRecursive)?;
    }

    Ok(())
}

fn sync_sessions_watch(
    watcher: &mut RecommendedWatcher,
    paths: &WatchPaths,
    sessions_tracking: bool,
    sessions_watched: &mut bool,
) -> notify::Result<()> {
    if sessions_tracking && !*sessions_watched && paths.sessions_root.exists() {
        watcher.watch(&paths.sessions_root, RecursiveMode::Recursive)?;
        *sessions_watched = true;
        trace_refresh("watch:sessions-attached");
    } else if !sessions_tracking && *sessions_watched {
        watcher.unwatch(&paths.sessions_root)?;
        *sessions_watched = false;
        trace_refresh("watch:sessions-detached");
    }

    Ok(())
}

fn event_has_relevant_kind(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Any | EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

fn is_stable_event(paths: &WatchPaths, event: &Event) -> bool {
    event.paths.iter().any(|path| {
        is_exact_path(path, &paths.registry_path) || is_exact_path(path, &paths.active_auth_path)
    })
}

fn is_rollout_event(paths: &WatchPaths, event: &Event) -> bool {
    event
        .paths
        .iter()
        .any(|path| path.starts_with(&paths.sessions_root) && is_rollout_file(path))
}

fn is_exact_path(path: &Path, target: &Path) -> bool {
    path == target
}

fn is_rollout_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    name.starts_with("rollout-") && name.ends_with(".jsonl")
}

fn trace_refresh(message: &str) {
    if env::var_os(TRACE_REFRESH_ENV).is_some() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_millis())
            .unwrap_or_default();

        if let Ok(mut file) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(TRACE_REFRESH_LOG_PATH)
        {
            let _ = writeln!(file, "{now_ms} {message}");
        }
    }
}

fn set_sessions_tracking(enabled: bool) {
    if let Some(tx) = WATCHER_TX.get() {
        let _ = tx.send(WatchMessage::SetSessionsTracking(enabled));
    }
}
