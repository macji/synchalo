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
use tauri_plugin_notification::NotificationExt as _;
use tauri_plugin_updater::{Update, UpdaterExt as _};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

const EVENT_UPDATE_STATUS: &str = "synchalo://update-status";
const UPDATE_STARTUP_DELAY: Duration = Duration::from_secs(5);
const UPDATE_POLL_INTERVAL: Duration = Duration::from_secs(30 * 60);
const MAX_UPDATE_NOTES_CHARS: usize = 4_000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    state: &'static str,
    version: Option<String>,
    notes: Option<String>,
    message: Option<String>,
}

#[derive(Default)]
pub(crate) struct UpdateCoordinator {
    in_progress: AtomicBool,
    pending: tokio::sync::Mutex<Option<PendingUpdate>>,
}

enum PendingUpdate {
    Available(Update),
    Ready(PreparedUpdate),
}

struct PreparedUpdate {
    update: Update,
    package: tempfile::NamedTempFile,
    digest: blake3::Hash,
}

struct PrepareUpdateError {
    update: Box<Update>,
    message: String,
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
            commands::refresh_devices,
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
            commands::install_update,
            commands::ignore_update,
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
            let settings = runtime.settings();
            let _ = run_update_check(
                app.clone(),
                coordinator.clone(),
                UpdateTrigger::Automatic,
                settings.automatic_updates_enabled,
                settings.ignored_update_version.as_deref(),
            )
            .await;
        }
    });
}

pub(crate) async fn check_for_updates_manually(
    app: AppHandle,
    coordinator: Arc<UpdateCoordinator>,
    automatic_download: bool,
    ignored_update_version: Option<&str>,
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
    run_update_check(
        app,
        coordinator,
        UpdateTrigger::Manual,
        automatic_download,
        ignored_update_version,
    )
    .await
}

async fn run_update_check(
    app: AppHandle,
    coordinator: Arc<UpdateCoordinator>,
    trigger: UpdateTrigger,
    automatic_download: bool,
    ignored_update_version: Option<&str>,
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
        coordinator.pending.lock().await.take();
        let status = UpdateStatus::message("upToDate", "当前已是最新版本。");
        return emit_for_trigger(&app, trigger, status);
    };

    if is_ignored_update(ignored_update_version, &update.version) {
        coordinator.pending.lock().await.take();
        let status = UpdateStatus::from_update("ignored", &update)
            .with_message("这个版本已被忽略，将在更高版本发布后再次提醒。");
        return emit_for_trigger(&app, trigger, status);
    }

    let version = update.version.clone();
    if let Some(PendingUpdate::Ready(prepared)) = coordinator.pending.lock().await.take()
        && prepared.update.version == version
    {
        let status = UpdateStatus::from_update("ready", &prepared.update);
        coordinator
            .pending
            .lock()
            .await
            .replace(PendingUpdate::Ready(prepared));
        if trigger == UpdateTrigger::Automatic {
            notify_update_status(&app, &status);
        }
        return emit_update_status(&app, status);
    }

    if !automatic_download {
        let status = UpdateStatus::from_update("available", &update);
        coordinator
            .pending
            .lock()
            .await
            .replace(PendingUpdate::Available(update));
        if trigger == UpdateTrigger::Automatic {
            notify_update_status(&app, &status);
        }
        return emit_update_status(&app, status);
    }

    download_update(
        &app,
        &coordinator,
        update,
        trigger == UpdateTrigger::Automatic,
    )
    .await
}

async fn download_update(
    app: &AppHandle,
    coordinator: &Arc<UpdateCoordinator>,
    update: Update,
    notify_when_ready: bool,
) -> UpdateStatus {
    emit_update_status(app, UpdateStatus::from_update("downloading", &update));
    match prepare_update(update).await {
        Ok(prepared) => {
            let status = UpdateStatus::from_update("ready", &prepared.update);
            coordinator
                .pending
                .lock()
                .await
                .replace(PendingUpdate::Ready(prepared));
            if notify_when_ready {
                notify_update_status(app, &status);
            }
            emit_update_status(app, status)
        }
        Err(error) => {
            let PrepareUpdateError { update, message } = error;
            let update = *update;
            tracing::warn!(error = %message, version = %update.version, "update download failed");
            let status = UpdateStatus::from_update("error", &update)
                .with_message("更新下载或验签失败，请稍后重试。");
            coordinator
                .pending
                .lock()
                .await
                .replace(PendingUpdate::Available(update));
            emit_update_status(app, status)
        }
    }
}

async fn prepare_update(update: Update) -> Result<PreparedUpdate, PrepareUpdateError> {
    let bytes = match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => bytes,
        Err(error) => return Err(PrepareUpdateError::new(update, error.to_string())),
    };
    let digest = blake3::hash(&bytes);
    let package = match tempfile::Builder::new()
        .prefix("synchalo-update-")
        .tempfile()
    {
        Ok(package) => package,
        Err(error) => return Err(PrepareUpdateError::new(update, error.to_string())),
    };
    let file = match package.reopen() {
        Ok(file) => file,
        Err(error) => return Err(PrepareUpdateError::new(update, error.to_string())),
    };
    let mut file = tokio::fs::File::from_std(file);
    if let Err(error) = file.write_all(&bytes).await {
        return Err(PrepareUpdateError::new(update, error.to_string()));
    }
    if let Err(error) = file.sync_all().await {
        return Err(PrepareUpdateError::new(update, error.to_string()));
    }
    drop(bytes);
    Ok(PreparedUpdate {
        update,
        package,
        digest,
    })
}

pub(crate) async fn install_pending_update(
    app: AppHandle,
    coordinator: Arc<UpdateCoordinator>,
) -> UpdateStatus {
    if cfg!(debug_assertions) {
        return emit_update_status(
            &app,
            UpdateStatus::message("unsupported", "开发版本不执行在线更新安装。"),
        );
    }
    if !supports_in_app_updates() {
        return emit_update_status(
            &app,
            UpdateStatus::message("unsupported", "DEB 安装请通过 APT 安装更新。"),
        );
    }
    let Some(_permit) = coordinator.try_acquire() else {
        return emit_update_status(
            &app,
            UpdateStatus::message("busy", "正在检查或准备更新，请稍候。"),
        );
    };
    let Some(pending) = coordinator.pending.lock().await.take() else {
        return emit_update_status(
            &app,
            UpdateStatus::message("error", "没有可安装的更新，请先检查更新。"),
        );
    };

    let prepared = match pending {
        PendingUpdate::Available(update) => {
            emit_update_status(&app, UpdateStatus::from_update("downloading", &update));
            match prepare_update(update).await {
                Ok(prepared) => prepared,
                Err(error) => {
                    let PrepareUpdateError { update, message } = error;
                    let update = *update;
                    tracing::warn!(error = %message, version = %update.version, "update download failed");
                    let status = UpdateStatus::from_update("error", &update)
                        .with_message("更新下载或验签失败，请稍后重试。");
                    coordinator
                        .pending
                        .lock()
                        .await
                        .replace(PendingUpdate::Available(update));
                    return emit_update_status(&app, status);
                }
            }
        }
        PendingUpdate::Ready(prepared) => prepared,
    };

    emit_update_status(
        &app,
        UpdateStatus::from_update("installing", &prepared.update),
    );
    let bytes = match read_prepared_update(&prepared).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, version = %prepared.update.version, "cached update validation failed");
            let status = UpdateStatus::from_update("error", &prepared.update)
                .with_message("已下载的更新校验失败，请重新检查更新。");
            return emit_update_status(&app, status);
        }
    };
    if let Err(error) = prepared.update.install(&bytes) {
        tracing::warn!(%error, version = %prepared.update.version, "update installation failed");
        let status = UpdateStatus::from_update("error", &prepared.update)
            .with_message("更新安装失败，请稍后重试或手动下载新版。");
        coordinator
            .pending
            .lock()
            .await
            .replace(PendingUpdate::Ready(prepared));
        return emit_update_status(&app, status);
    }

    emit_update_status(
        &app,
        UpdateStatus::from_update("installed", &prepared.update),
    );
    tokio::time::sleep(Duration::from_secs(1)).await;
    app.restart()
}

pub(crate) async fn ignore_pending_update(
    app: AppHandle,
    coordinator: Arc<UpdateCoordinator>,
    runtime: Arc<AppRuntime>,
    version: String,
) -> Result<UpdateStatus, synchalo_core::AppError> {
    let Some(_permit) = coordinator.try_acquire() else {
        return Err(synchalo_core::AppError::Internal(
            "an update operation is already in progress".to_owned(),
        ));
    };
    let pending_version = coordinator
        .pending
        .lock()
        .await
        .as_ref()
        .map(PendingUpdate::version);
    if pending_version.as_deref() != Some(version.as_str()) {
        return Err(synchalo_core::AppError::InvalidInput(
            "the requested update is no longer pending".to_owned(),
        ));
    }
    coordinator.pending.lock().await.take();
    runtime.ignore_update_version(version.clone())?;
    Ok(emit_update_status(
        &app,
        UpdateStatus {
            state: "ignored",
            version: Some(version),
            notes: None,
            message: Some("已忽略这个版本；有更高版本时会再次提醒。".to_owned()),
        },
    ))
}

async fn read_prepared_update(prepared: &PreparedUpdate) -> Result<Vec<u8>, String> {
    let file = prepared
        .package
        .reopen()
        .map_err(|error| error.to_string())?;
    let mut file = tokio::fs::File::from_std(file);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .await
        .map_err(|error| error.to_string())?;
    if blake3::hash(&bytes) != prepared.digest {
        return Err("cached update digest mismatch".to_owned());
    }
    Ok(bytes)
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

fn notify_update_status(app: &AppHandle, status: &UpdateStatus) {
    if let Some(window) = app.get_webview_window("main") {
        let visible = window.is_visible().unwrap_or(false);
        let minimized = window.is_minimized().unwrap_or(false);
        if visible && !minimized {
            return;
        }
    }
    let Some(version) = status.version.as_deref() else {
        return;
    };
    let (title, body) = if status.state == "ready" {
        (
            "SyncHalo 更新已下载",
            format!("SyncHalo {version} 已完成验证，打开应用安装并重启。"),
        )
    } else {
        (
            "SyncHalo 发现新版本",
            format!("SyncHalo {version} 可用，打开应用查看发布说明。"),
        )
    };
    let _ = app.notification().builder().title(title).body(body).show();
}

fn supports_in_app_updates() -> bool {
    #[cfg(target_os = "linux")]
    {
        false
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
            notes: None,
            message: Some(message.to_owned()),
        }
    }

    fn from_update(state: &'static str, update: &Update) -> Self {
        Self {
            state,
            version: Some(update.version.clone()),
            notes: update.body.as_deref().and_then(bounded_update_notes),
            message: None,
        }
    }

    fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_owned());
        self
    }
}

impl PendingUpdate {
    fn version(&self) -> String {
        match self {
            Self::Available(update) => update.version.clone(),
            Self::Ready(prepared) => prepared.update.version.clone(),
        }
    }
}

impl PrepareUpdateError {
    fn new(update: Update, message: String) -> Self {
        Self {
            update: Box::new(update),
            message,
        }
    }
}

fn bounded_update_notes(notes: &str) -> Option<String> {
    let notes = notes.trim();
    if notes.is_empty() {
        return None;
    }
    Some(notes.chars().take(MAX_UPDATE_NOTES_CHARS).collect())
}

fn is_ignored_update(ignored_version: Option<&str>, available_version: &str) -> bool {
    ignored_version == Some(available_version)
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

    #[test]
    fn update_notes_are_trimmed_and_bounded_for_the_webview() {
        assert_eq!(
            bounded_update_notes("  release notes  ").as_deref(),
            Some("release notes")
        );
        assert_eq!(bounded_update_notes("   "), None);
        let oversized = "更".repeat(MAX_UPDATE_NOTES_CHARS + 10);
        assert_eq!(
            bounded_update_notes(&oversized)
                .expect("notes")
                .chars()
                .count(),
            MAX_UPDATE_NOTES_CHARS
        );
    }

    #[test]
    fn ignored_update_only_suppresses_the_exact_version() {
        assert!(is_ignored_update(Some("0.1.5"), "0.1.5"));
        assert!(!is_ignored_update(Some("0.1.5"), "0.1.6"));
        assert!(!is_ignored_update(None, "0.1.5"));
    }
}
