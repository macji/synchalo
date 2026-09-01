use std::{path::PathBuf, sync::Arc};

use synchalo_core::{
    AppSnapshot, ClipboardHistoryPage, ClipboardItemView, PairingCodeView, SettingsPatch,
    SettingsView, SyncStatusView, TransferHistoryFilter, TransferHistoryPage, TransferView,
    UserFacingError,
};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;
use uuid::Uuid;

use crate::runtime::AppRuntime;

type CommandResult<T> = Result<T, UserFacingError>;

#[tauri::command]
pub async fn get_app_state(state: State<'_, Arc<AppRuntime>>) -> CommandResult<AppSnapshot> {
    state.snapshot().map_err(Into::into)
}

#[tauri::command]
pub async fn list_clipboard_history(
    state: State<'_, Arc<AppRuntime>>,
    query: Option<String>,
    favorites_only: Option<bool>,
    page: Option<usize>,
) -> CommandResult<ClipboardHistoryPage> {
    state
        .list_clipboard(
            query.as_deref(),
            favorites_only.unwrap_or(false),
            page.unwrap_or(1),
        )
        .map_err(Into::into)
}

#[tauri::command]
pub async fn list_file_history(
    state: State<'_, Arc<AppRuntime>>,
    query: Option<String>,
    favorites_only: Option<bool>,
    filter: Option<TransferHistoryFilter>,
    page: Option<usize>,
) -> CommandResult<TransferHistoryPage> {
    state
        .list_file_history(
            query.as_deref(),
            favorites_only.unwrap_or(false),
            filter.unwrap_or_default(),
            page.unwrap_or(1),
        )
        .map_err(Into::into)
}

#[tauri::command]
pub async fn copy_history_item(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
) -> CommandResult<ClipboardItemView> {
    state.copy_history_item(id).map_err(Into::into)
}

#[tauri::command]
pub async fn delete_clipboard_item(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
) -> CommandResult<bool> {
    state.delete_clipboard(id).map_err(Into::into)
}

#[tauri::command]
pub async fn restore_clipboard_item(
    state: State<'_, Arc<AppRuntime>>,
    item: ClipboardItemView,
) -> CommandResult<()> {
    state.restore_clipboard(&item).map_err(Into::into)
}

#[tauri::command]
pub async fn clear_clipboard_history(state: State<'_, Arc<AppRuntime>>) -> CommandResult<usize> {
    state.database_clear_clipboard_history().map_err(Into::into)
}

#[tauri::command]
pub async fn set_clipboard_pinned(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
    pinned: bool,
) -> CommandResult<bool> {
    state
        .database_set_clipboard_pinned(id, pinned)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn generate_pairing_code(
    state: State<'_, Arc<AppRuntime>>,
) -> CommandResult<PairingCodeView> {
    state.generate_pairing_code().map_err(Into::into)
}

#[tauri::command]
pub async fn copy_pairing_code(
    state: State<'_, Arc<AppRuntime>>,
) -> CommandResult<PairingCodeView> {
    state.copy_pairing_code().map_err(Into::into)
}

#[tauri::command]
pub async fn respond_to_pairing(
    state: State<'_, Arc<AppRuntime>>,
    request_id: Uuid,
    accepted: bool,
) -> CommandResult<()> {
    state
        .respond_to_pairing(request_id, accepted)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn join_with_code(
    state: State<'_, Arc<AppRuntime>>,
    code: String,
) -> CommandResult<synchalo_core::DeviceView> {
    state.join_with_code(&code).await.map_err(Into::into)
}

#[tauri::command]
pub async fn revoke_device(state: State<'_, Arc<AppRuntime>>, id: Uuid) -> CommandResult<bool> {
    state.revoke_device(id).map_err(Into::into)
}

#[tauri::command]
pub async fn set_device_paused(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
    paused: bool,
) -> CommandResult<synchalo_core::DeviceView> {
    state.set_device_paused(id, paused).map_err(Into::into)
}

#[tauri::command]
pub async fn pause_sync(
    state: State<'_, Arc<AppRuntime>>,
    paused: bool,
) -> CommandResult<SyncStatusView> {
    state.pause_sync(paused).map_err(Into::into)
}

#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, Arc<AppRuntime>>,
    patch: SettingsPatch,
) -> CommandResult<SettingsView> {
    let launch_at_startup = patch.launch_at_startup;
    if let Some(enabled) = launch_at_startup {
        let result = if enabled {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        result.map_err(|error| {
            UserFacingError::new(
                synchalo_core::ErrorCode::PermissionDenied,
                "无法更新开机启动设置",
            )
            .detail(error.to_string())
        })?;
    }
    state.update_settings(patch).map_err(UserFacingError::from)
}

#[tauri::command]
pub async fn select_receive_directory(
    app: AppHandle,
    state: State<'_, Arc<AppRuntime>>,
) -> CommandResult<Option<SettingsView>> {
    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|error| {
        UserFacingError::new(synchalo_core::ErrorCode::InvalidInput, "无法读取所选目录")
            .detail(error.to_string())
    })?;
    let settings = state
        .update_settings(SettingsPatch {
            receive_directory: Some(path.to_string_lossy().into_owned()),
            ..SettingsPatch::default()
        })
        .map_err(UserFacingError::from)?;
    Ok(Some(settings))
}

#[tauri::command]
pub async fn select_files(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, Arc<AppRuntime>>,
    target_ids: Option<Vec<Uuid>>,
) -> CommandResult<Vec<TransferView>> {
    let paths = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("选择要同步的文件")
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .collect();
    state
        .enqueue_paths(paths, target_ids)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn paste_files(
    state: State<'_, Arc<AppRuntime>>,
    target_ids: Option<Vec<Uuid>>,
) -> CommandResult<Vec<TransferView>> {
    state.paste_files(target_ids).await.map_err(Into::into)
}

#[tauri::command]
pub async fn enqueue_files(
    state: State<'_, Arc<AppRuntime>>,
    paths: Vec<String>,
    target_ids: Option<Vec<Uuid>>,
) -> CommandResult<Vec<TransferView>> {
    state
        .enqueue_paths(paths.into_iter().map(PathBuf::from).collect(), target_ids)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn resync_transfer(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
    target_ids: Option<Vec<Uuid>>,
) -> CommandResult<Vec<TransferView>> {
    state
        .resync_transfer(id, target_ids)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn set_transfer_pinned(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
    pinned: bool,
) -> CommandResult<TransferView> {
    state.set_transfer_pinned(id, pinned).map_err(Into::into)
}

#[tauri::command]
pub async fn retry_transfer(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
) -> CommandResult<TransferView> {
    state.retry_transfer(id).await.map_err(Into::into)
}

#[tauri::command]
pub async fn cancel_transfer(
    state: State<'_, Arc<AppRuntime>>,
    id: Uuid,
) -> CommandResult<TransferView> {
    state.cancel_transfer(id).map_err(Into::into)
}

#[tauri::command]
pub async fn delete_transfer(state: State<'_, Arc<AppRuntime>>, id: Uuid) -> CommandResult<bool> {
    state.delete_transfer(id).map_err(Into::into)
}

#[tauri::command]
pub async fn open_transfer(state: State<'_, Arc<AppRuntime>>, id: Uuid) -> CommandResult<()> {
    let transfer = state.transfer(id).map_err(UserFacingError::from)?;
    let path = transfer.display_path.ok_or_else(|| {
        UserFacingError::new(
            synchalo_core::ErrorCode::SourceFileMissing,
            "文件路径不可用",
        )
    })?;
    state
        .app_handle()
        .opener()
        .open_path(path, None::<&str>)
        .map_err(|error| {
            UserFacingError::new(synchalo_core::ErrorCode::PermissionDenied, "无法打开文件")
                .detail(error.to_string())
        })
}

#[tauri::command]
pub async fn reveal_transfer(state: State<'_, Arc<AppRuntime>>, id: Uuid) -> CommandResult<()> {
    let transfer = state.transfer(id).map_err(UserFacingError::from)?;
    let path = transfer.display_path.ok_or_else(|| {
        UserFacingError::new(
            synchalo_core::ErrorCode::SourceFileMissing,
            "文件路径不可用",
        )
    })?;
    state
        .app_handle()
        .opener()
        .reveal_item_in_dir(path)
        .map_err(|error| {
            UserFacingError::new(
                synchalo_core::ErrorCode::PermissionDenied,
                "无法在文件管理器中定位文件",
            )
            .detail(error.to_string())
        })
}

#[tauri::command]
pub async fn open_receive_directory(state: State<'_, Arc<AppRuntime>>) -> CommandResult<()> {
    let path = state.settings().receive_directory;
    state
        .app_handle()
        .opener()
        .open_path(path, None::<&str>)
        .map_err(|error| {
            UserFacingError::new(
                synchalo_core::ErrorCode::PermissionDenied,
                "无法打开接收目录",
            )
            .detail(error.to_string())
        })
}
