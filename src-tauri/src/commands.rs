use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Position, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use crate::models::{AppRole, HostInfo, PlayerCommand, RuntimeStatus};
use crate::runtime::RuntimeManager;

pub struct AppState(pub Arc<RuntimeManager>);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct DesktopLyricsWindowPosition {
    x: i32,
    y: i32,
}

fn desktop_lyrics_position_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("desktop-lyrics-window.json"))
        .map_err(|error| error.to_string())
}

fn load_desktop_lyrics_position(app: &AppHandle) -> Option<PhysicalPosition<i32>> {
    let path = desktop_lyrics_position_path(app).ok()?;
    let raw = fs::read_to_string(path).ok()?;
    let position = serde_json::from_str::<DesktopLyricsWindowPosition>(&raw).ok()?;
    Some(PhysicalPosition::new(position.x, position.y))
}

fn save_desktop_lyrics_position(
    app: &AppHandle,
    position: PhysicalPosition<i32>,
) -> Result<(), String> {
    let path = desktop_lyrics_position_path(app)?;
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    }
    let raw = serde_json::to_string(&DesktopLyricsWindowPosition {
        x: position.x,
        y: position.y,
    })
    .map_err(|error| error.to_string())?;
    fs::write(path, raw).map_err(|error| error.to_string())
}

fn is_position_visible(window: &WebviewWindow, position: PhysicalPosition<i32>) -> bool {
    let Ok(window_size) = window.outer_size() else {
        return false;
    };
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    let window_left = i64::from(position.x);
    let window_top = i64::from(position.y);
    let window_right = window_left + i64::from(window_size.width);
    let window_bottom = window_top + i64::from(window_size.height);

    monitors.into_iter().any(|monitor| {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let monitor_left = i64::from(monitor_position.x);
        let monitor_top = i64::from(monitor_position.y);
        let monitor_right = monitor_left + i64::from(monitor_size.width);
        let monitor_bottom = monitor_top + i64::from(monitor_size.height);
        let visible_width = window_right.min(monitor_right) - window_left.max(monitor_left);
        let visible_height = window_bottom.min(monitor_bottom) - window_top.max(monitor_top);
        visible_width >= 100 && visible_height >= 40
    })
}

#[tauri::command]
pub fn get_runtime_status(state: State<'_, AppState>) -> RuntimeStatus {
    state.0.snapshot()
}

#[tauri::command]
pub async fn set_role(
    app: AppHandle,
    state: State<'_, AppState>,
    role: AppRole,
) -> Result<RuntimeStatus, String> {
    let status = state.0.start_role(app.clone(), role).await;
    let _ = app.emit("runtime://status", &status);
    Ok(status)
}

#[tauri::command]
pub fn set_allow_control(
    app: AppHandle,
    state: State<'_, AppState>,
    allow_control: bool,
) -> RuntimeStatus {
    let status = state.0.set_allow_control(allow_control);
    let _ = app.emit("runtime://status", &status);
    status
}

#[tauri::command]
pub async fn send_player_command(
    state: State<'_, AppState>,
    command: PlayerCommand,
) -> Result<(), String> {
    state.0.send_player_command(command).await
}

#[tauri::command]
pub async fn discover_hosts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<HostInfo>, String> {
    let hosts = state.0.discover_hosts().await?;
    let _ = app.emit("runtime://status", state.0.snapshot());
    Ok(hosts)
}

#[tauri::command]
pub async fn connect_manual_host(
    app: AppHandle,
    state: State<'_, AppState>,
    address: String,
) -> Result<RuntimeStatus, String> {
    let status = state.0.connect_manual_host(app.clone(), &address).await?;
    let _ = app.emit("runtime://status", &status);
    Ok(status)
}

#[tauri::command]
pub async fn start_automatic_discovery(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RuntimeStatus, String> {
    let status = state.0.start_automatic_discovery(app.clone()).await;
    let _ = app.emit("runtime://status", &status);
    Ok(status)
}

fn ensure_desktop_lyrics_window(
    app: &AppHandle,
    always_on_top: bool,
) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("desktop-lyrics") {
        return Ok(window);
    }

    let window_url = if cfg!(debug_assertions) {
        app.config()
            .build
            .dev_url
            .clone()
            .map(WebviewUrl::External)
            .unwrap_or_else(|| WebviewUrl::App("index.html".into()))
    } else {
        WebviewUrl::App("index.html".into())
    };

    let window = WebviewWindowBuilder::new(app, "desktop-lyrics", window_url)
        .title("QQMusic LAN Sync 桌面歌词")
        .inner_size(900.0, 190.0)
        .min_inner_size(420.0, 118.0)
        .transparent(true)
        .accept_first_mouse(true)
        .decorations(false)
        .shadow(false)
        .resizable(true)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(always_on_top)
        .skip_taskbar(true)
        .visible(false)
        .center()
        .build()
        .map_err(|error| error.to_string())?;

    if let Some(position) = load_desktop_lyrics_position(app) {
        if is_position_visible(&window, position) {
            window
                .set_position(Position::Physical(position))
                .map_err(|error| error.to_string())?;
        }
    }

    let app_handle = app.clone();
    let move_generation = Arc::new(AtomicU64::new(0));
    let observed_window = window.clone();
    window.on_window_event(move |event| {
        let WindowEvent::Moved(position) = event else {
            return;
        };
        if !observed_window.is_visible().unwrap_or(false) {
            return;
        }
        let position = *position;
        let generation = move_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let move_generation = move_generation.clone();
        let app_handle = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if move_generation.load(Ordering::Relaxed) == generation {
                if let Err(error) = save_desktop_lyrics_position(&app_handle, position) {
                    log::warn!("Failed to save desktop lyrics position: {error}");
                }
            }
        });
    });

    Ok(window)
}

pub fn prepare_desktop_lyrics_window(app: &AppHandle) -> Result<(), String> {
    ensure_desktop_lyrics_window(app, true).map(|_| ())
}

#[tauri::command]
pub fn configure_desktop_lyrics_window(
    app: AppHandle,
    visible: bool,
    always_on_top: bool,
    locked: bool,
) -> Result<(), String> {
    if !visible {
        if let Some(window) = app.get_webview_window("desktop-lyrics") {
            window.hide().map_err(|error| error.to_string())?;
        }
        return Ok(());
    }

    let window = ensure_desktop_lyrics_window(&app, always_on_top)?;
    window
        .set_always_on_top(always_on_top)
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(locked)
        .map_err(|error| error.to_string())?;
    let _ = window.set_focusable(!locked);
    if window.is_minimized().map_err(|error| error.to_string())? {
        window.unminimize().map_err(|error| error.to_string())?;
    }
    window.show().map_err(|error| error.to_string())?;
    if !locked {
        window.set_focus().map_err(|error| error.to_string())?;
    }
    if !window.is_visible().map_err(|error| error.to_string())? {
        return Err("Windows 未能显示桌面歌词窗口，请重试".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn reset_desktop_lyrics_position(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("desktop-lyrics")
        .ok_or_else(|| "桌面歌词尚未启用".to_string())?;
    window.center().map_err(|error| error.to_string())?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    save_desktop_lyrics_position(&app, position)
}
