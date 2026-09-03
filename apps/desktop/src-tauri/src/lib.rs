mod commands;
mod runtime;
mod tray;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use runtime::AppRuntime;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, webview::PageLoadEvent};
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_updater::UpdaterExt as _;

const EVENT_UPDATE_STATUS: &str = "synchalo://update-status";
const UPDATE_STARTUP_DELAY: Duration = Duration::from_secs(5);
const UPDATE_POLL_INTERVAL: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    state: &'static str,
    version: Option<String>,
    message: Option<String>,
}

#[derive(Default)]
pub(crate) struct UpdateCoordinator {
    in_progress: AtomicBool,
}

struct UpdatePermit {
    coordinator: Arc<UpdateCoordinator>,
}

impl UpdateCoordinator {
    fn try_acquire(self: &Arc<Self>) -> Option<UpdatePermit> {
        self.in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| UpdatePermit {
                coordinator: self.clone(),
            })
    }
}

impl Drop for UpdatePermit {
    fn drop(&mut self) {
        self.coordinator.in_progress.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateTrigger {
    Automatic,
    Manual,
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
            let update_coordinator = Arc::new(UpdateCoordinator::default());
            app.manage::<Arc<AppRuntime>>(runtime.clone());
            app.manage::<Arc<UpdateCoordinator>>(update_coordinator.clone());
            tray::install(app.handle(), runtime.clone())?;
            start_automatic_update(app.handle().clone(), runtime.clone(), update_coordinator);

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
            commands::check_for_updates,
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

fn start_automatic_update(
    app: AppHandle,
    runtime: Arc<AppRuntime>,
    coordinator: Arc<UpdateCoordinator>,
) {
    if cfg!(debug_assertions) {
        return;
    }
    if !supports_in_app_updates() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let first_check = tokio::time::Instant::now() + UPDATE_STARTUP_DELAY;
        let mut interval = tokio::time::interval_at(first_check, UPDATE_POLL_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if runtime.settings().automatic_updates_enabled {
                let _ =
                    run_update_check(app.clone(), coordinator.clone(), UpdateTrigger::Automatic)
                        .await;
            }
        }
    });
}

pub(crate) async fn check_for_updates_manually(
    app: AppHandle,
    coordinator: Arc<UpdateCoordinator>,
) -> UpdateStatus {
    if cfg!(debug_assertions) {
        return emit_update_status(
            &app,
            UpdateStatus::message("unsupported", "开发版本不执行在线更新检查。"),
        );
    }
    if !supports_in_app_updates() {
        return emit_update_status(
            &app,
            UpdateStatus::message("unsupported", "DEB 安装请通过 APT 检查和安装更新。"),
        );
    }
    run_update_check(app, coordinator, UpdateTrigger::Manual).await
}

async fn run_update_check(
    app: AppHandle,
    coordinator: Arc<UpdateCoordinator>,
    trigger: UpdateTrigger,
) -> UpdateStatus {
    let Some(_permit) = coordinator.try_acquire() else {
        let status = UpdateStatus::message("busy", "正在检查或安装更新，请稍候。");
        return emit_for_trigger(&app, trigger, status);
    };

    if trigger == UpdateTrigger::Manual {
        emit_update_status(&app, UpdateStatus::message("checking", "正在检查更新…"));
    }

    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(error) => {
            tracing::debug!(%error, "updater is unavailable");
            let status = UpdateStatus::message("error", "更新服务暂时不可用，请稍后重试。");
            return emit_for_trigger(&app, trigger, status);
        }
    };
    let update = match updater.check().await {
        Ok(update) => update,
        Err(error) => {
            tracing::debug!(%error, "update check failed");
            let status = UpdateStatus::message("error", "检查更新失败，请确认网络连接后重试。");
            return emit_for_trigger(&app, trigger, status);
        }
    };
    let Some(update) = update else {
        let status = UpdateStatus::message("upToDate", "当前已是最新版本。");
        return emit_for_trigger(&app, trigger, status);
    };

    let version = update.version.to_string();
    emit_update_status(
        &app,
        UpdateStatus {
            state: "downloading",
            version: Some(version.clone()),
            message: None,
        },
    );
    if let Err(error) = update.download_and_install(|_, _| {}, || {}).await {
        tracing::warn!(%error, %version, "update installation failed");
        return emit_update_status(
            &app,
            UpdateStatus {
                state: "error",
                version: Some(version),
                message: Some("更新安装失败，请稍后重试或手动下载新版。".to_owned()),
            },
        );
    }

    emit_update_status(
        &app,
        UpdateStatus {
            state: "installed",
            version: Some(version),
            message: None,
        },
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    app.restart()
}

fn emit_for_trigger(app: &AppHandle, trigger: UpdateTrigger, status: UpdateStatus) -> UpdateStatus {
    if trigger == UpdateTrigger::Manual {
        emit_update_status(app, status)
    } else {
        status
    }
}

fn emit_update_status(app: &AppHandle, status: UpdateStatus) -> UpdateStatus {
    let _ = app.emit(EVENT_UPDATE_STATUS, status.clone());
    status
}

fn supports_in_app_updates() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("APPIMAGE").is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

impl UpdateStatus {
    fn message(state: &'static str, message: &str) -> Self {
        Self {
            state,
            version: None,
            message: Some(message.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_checks_are_mutually_exclusive() {
        let coordinator = Arc::new(UpdateCoordinator::default());
        let permit = coordinator
            .try_acquire()
            .expect("first check acquires lock");
        assert!(coordinator.try_acquire().is_none());
        drop(permit);
        assert!(coordinator.try_acquire().is_some());
    }

    #[test]
    fn automatic_update_schedule_matches_product_policy() {
        assert_eq!(UPDATE_STARTUP_DELAY, Duration::from_secs(5));
        assert_eq!(UPDATE_POLL_INTERVAL, Duration::from_secs(30 * 60));
    }
}
