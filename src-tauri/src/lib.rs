mod commands;
mod lyrics;
mod models;
mod network;
mod player;
mod runtime;
mod storage;

use commands::AppState;
use models::{AppRole, PlayerCommand};
use runtime::RuntimeManager;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn send_tray_player_command(app: &AppHandle, command: PlayerCommand) {
    let runtime = app.state::<AppState>().0.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = runtime.send_player_command(command).await {
            log::warn!("Tray player command failed: {error}");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let app_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            let runtime = RuntimeManager::new(app_dir)?;
            app.manage(AppState(runtime.clone()));
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                runtime.start_role(app_handle, AppRole::Host).await;
            });
            commands::prepare_desktop_lyrics_window(app.handle())?;

            let show_item = MenuItem::with_id(app, "show-main", "显示主窗口", true, None::<&str>)?;
            let lyrics_item = MenuItem::with_id(
                app,
                "toggle-desktop-lyrics",
                "显示/隐藏桌面歌词",
                true,
                None::<&str>,
            )?;
            let lock_item = MenuItem::with_id(
                app,
                "toggle-desktop-lyrics-lock",
                "锁定/解锁桌面歌词",
                true,
                None::<&str>,
            )?;
            let previous_item = MenuItem::with_id(app, "previous", "上一曲", true, None::<&str>)?;
            let play_pause_item =
                MenuItem::with_id(app, "play-pause", "播放/暂停", true, None::<&str>)?;
            let next_item = MenuItem::with_id(app, "next", "下一曲", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let first_separator = PredefinedMenuItem::separator(app)?;
            let second_separator = PredefinedMenuItem::separator(app)?;
            let third_separator = PredefinedMenuItem::separator(app)?;
            let tray_menu = Menu::with_items(
                app,
                &[
                    &show_item,
                    &first_separator,
                    &lyrics_item,
                    &lock_item,
                    &second_separator,
                    &previous_item,
                    &play_pause_item,
                    &next_item,
                    &third_separator,
                    &quit_item,
                ],
            )?;
            let mut tray = TrayIconBuilder::with_id("main-tray")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .tooltip("QQMusic LAN Sync")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show-main" => show_main_window(app),
                    "toggle-desktop-lyrics" => {
                        let _ = app.emit("desktop-lyrics://tray-action", "toggle_enabled");
                    }
                    "toggle-desktop-lyrics-lock" => {
                        let _ = app.emit("desktop-lyrics://tray-action", "toggle_locked");
                    }
                    "previous" => send_tray_player_command(app, PlayerCommand::Previous),
                    "play-pause" => send_tray_player_command(app, PlayerCommand::TogglePlayPause),
                    "next" => send_tray_player_command(app, PlayerCommand::Next),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Resized(_) => {
                    if window.is_minimized().unwrap_or(false) {
                        let _ = window.hide();
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_runtime_status,
            commands::set_role,
            commands::set_allow_control,
            commands::send_player_command,
            commands::discover_hosts,
            commands::connect_manual_host,
            commands::start_automatic_discovery,
            commands::configure_desktop_lyrics_window,
            commands::reset_desktop_lyrics_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running QQMusic LAN Sync");
}
