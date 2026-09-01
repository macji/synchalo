mod commands;
mod runtime;
mod tray;

use std::sync::Arc;

use runtime::AppRuntime;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(MacosLauncher::LaunchAgent)
                .build(),
        )
        .setup(|app| {
            let runtime =
                tauri::async_runtime::block_on(AppRuntime::initialize(app.handle().clone()))?;
            app.manage::<Arc<AppRuntime>>(runtime.clone());
            tray::install(app.handle(), runtime.clone())?;

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
            commands::open_transfer,
            commands::reveal_transfer,
            commands::open_receive_directory,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run SyncHalo");
}
