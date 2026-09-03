mod commands;
mod runtime;
mod tray;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use runtime::AppRuntime;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, webview::PageLoadEvent};
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_updater::UpdaterExt as _;

const EVENT_UPDATE_STATUS: &str = "synchalo://update-status";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateStatus {
    state: &'static str,
    version: Option<String>,
    message: Option<String>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "synchalo=info,warn".into()),
        )
        .with_target(false)
        .compact()
        .init();

    let autostart = tauri_plugin_autostart::Builder::new();
    #[cfg(target_os = "macos")]
    let autostart = autostart.macos_launcher(MacosLauncher::LaunchAgent);
    let initial_page_revealed = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(autostart.build())
        .on_page_load(move |webview, payload| {
            if webview.label() == "main"
                && payload.event() == PageLoadEvent::Finished
                && !initial_page_revealed.swap(true, Ordering::Relaxed)
            {
                let _ = webview.window().show();
            }
        })
        .setup(|app| {
            let runtime =
                tauri::async_runtime::block_on(AppRuntime::initialize(app.handle().clone()))?;
            app.manage::<Arc<AppRuntime>>(runtime.clone());
            tray::install(app.handle(), runtime.clone())?;
            start_automatic_update(app.handle().clone(), runtime.clone());

            if let Some(window) = app.get_webview_window("main") {
                let close_runtime = runtime;
                let hide_window = window.clone();
                let close_app = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if close_runtime.keep_in_tray() {
                            api.prevent_close();
                            let _ = hide_window.hide();
                        } else {
                            close_app.exit(0);
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::list_clipboard_history,
            commands::list_file_history,
            commands::copy_history_item,
            commands::delete_clipboard_item,
            commands::restore_clipboard_item,
            commands::clear_clipboard_history,
            commands::set_clipboard_pinned,
            commands::generate_pairing_code,
            commands::copy_pairing_code,
            commands::respond_to_pairing,
            commands::join_with_code,
            commands::revoke_device,
            commands::set_device_paused,
            commands::pause_sync,
            commands::update_settings,
            commands::select_receive_directory,
            commands::select_files,
            commands::paste_files,
            commands::enqueue_files,
            commands::resync_transfer,
            commands::set_transfer_pinned,
            commands::retry_transfer,
            commands::cancel_transfer,
            commands::delete_transfer,
            commands::clear_file_history,
            commands::open_transfer,
            commands::reveal_transfer,
            commands::open_receive_directory,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SyncHalo");
}

fn start_automatic_update(app: AppHandle, runtime: Arc<AppRuntime>) {
    if cfg!(debug_assertions) {
        return;
    }
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if !runtime.settings().automatic_updates_enabled {
            return;
        }
        let update = match app.updater() {
            Ok(updater) => match updater.check().await {
                Ok(update) => update,
                Err(error) => {
                    tracing::debug!(%error, "automatic update check failed");
                    return;
                }
            },
            Err(error) => {
                tracing::debug!(%error, "automatic updater is unavailable");
                return;
            }
        };
        let Some(update) = update else {
            return;
        };
        let version = update.version.to_string();
        let _ = app.emit(
            EVENT_UPDATE_STATUS,
            UpdateStatus {
                state: "downloading",
                version: Some(version.clone()),
                message: None,
            },
        );
        if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
            tracing::warn!(%error, %version, "automatic update failed");
            let _ = app.emit(
                EVENT_UPDATE_STATUS,
                UpdateStatus {
                    state: "error",
                    version: Some(version),
                    message: Some("自动更新失败，请稍后重试或手动下载新版。".to_owned()),
                },
            );
            return;
        }
        let _ = app.emit(
            EVENT_UPDATE_STATUS,
            UpdateStatus {
                state: "installed",
                version: Some(version),
                message: None,
            },
        );
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        app.restart();
    });
}
