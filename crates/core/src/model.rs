use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::HlcTimestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    Clipboard,
    Files,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DevicePlatform {
    Macos,
    Linux,
    Unknown,
}

impl DevicePlatform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Unknown
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceConnectionState {
    Online,
    Offline,
    Nearby,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: Uuid,
    pub name: String,
    pub platform: DevicePlatform,
    pub connection_state: DeviceConnectionState,
    pub is_current: bool,
    pub address: Option<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub paused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncState {
    Healthy,
    Paused,
    Offline,
    Limited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardCapability {
    Full,
    AppActiveOnly,
    Manual,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatusView {
    pub state: SyncState,
    pub label: String,
    pub online_count: usize,
    pub offline_count: usize,
    pub clipboard_capability: ClipboardCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardDirection {
    Local,
    Received,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItemView {
    pub id: Uuid,
    pub content: String,
    pub content_hash: String,
    pub source_device_id: Uuid,
    pub source_device_name: String,
    pub direction: ClipboardDirection,
    pub created_at: DateTime<Utc>,
    pub hlc: HlcTimestamp,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardHistoryPage {
    pub items: Vec<ClipboardItemView>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferDirection {
    Sending,
    Receiving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferState {
    Queued,
    Transferring,
    Verifying,
    Completed,
    #[serde(alias = "waitingForDevice")]
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferTargetView {
    pub device_id: Uuid,
    pub device_name: String,
    pub state: TransferState,
    pub progress: f32,
    pub bytes_per_second: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferView {
    pub id: Uuid,
    pub file_name: String,
    pub file_size: u64,
    pub direction: TransferDirection,
    pub state: TransferState,
    pub progress: f32,
    pub created_at: DateTime<Utc>,
    pub source_device_name: Option<String>,
    pub targets: Vec<TransferTargetView>,
    pub bytes_per_second: Option<u64>,
    pub eta_seconds: Option<u64>,
    pub display_path: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub source_modified_unix_ms: Option<u64>,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferHistoryFilter {
    #[default]
    All,
    Sending,
    Receiving,
    Active,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferHistoryPage {
    pub items: Vec<TransferView>,
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryRetention {
    None,
    OneDay,
    #[default]
    SevenDays,
    ThirtyDays,
    Forever,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub device_name: String,
    pub receive_directory: String,
    #[serde(default)]
    pub delete_sync_enabled: bool,
    #[serde(default)]
    pub favorite_sync_enabled: bool,
    pub history_retention: HistoryRetention,
    pub launch_at_startup: bool,
    pub keep_in_tray: bool,
    pub notifications_enabled: bool,
    #[serde(default = "default_true")]
    pub automatic_updates_enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch {
    pub device_name: Option<String>,
    pub receive_directory: Option<String>,
    pub delete_sync_enabled: Option<bool>,
    pub favorite_sync_enabled: Option<bool>,
    pub history_retention: Option<HistoryRetention>,
    pub launch_at_startup: Option<bool>,
    pub keep_in_tray: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub automatic_updates_enabled: Option<bool>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCodeView {
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequestView {
    pub request_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: DevicePlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilitiesView {
    pub platform: DevicePlatform,
    pub architecture: String,
    pub clipboard: ClipboardCapability,
    pub supports_tray: bool,
    pub supports_autostart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub current_device_id: Uuid,
    pub sync_status: SyncStatusView,
    pub devices: Vec<DeviceView>,
    pub clipboard_history: Vec<ClipboardItemView>,
    pub clipboard_history_total: usize,
    pub file_history: Vec<TransferView>,
    pub file_history_total: usize,
    pub settings: SettingsView,
    pub pairing_code: Option<PairingCodeView>,
    pub capabilities: PlatformCapabilitiesView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEvent {
    pub id: Uuid,
    pub space_id: Uuid,
    pub origin_device_id: Uuid,
    pub origin_sequence: u64,
    pub created_at: DateTime<Utc>,
    pub hlc: HlcTimestamp,
    pub content: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HistoryItemKind {
    Clipboard,
    Transfer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HistoryMutation {
    Delete {
        item_kind: HistoryItemKind,
        item_id: Uuid,
    },
    SetPinned {
        item_kind: HistoryItemKind,
        item_id: Uuid,
        pinned: bool,
    },
    RestoreClipboard {
        item: ClipboardItemView,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryMutationEvent {
    pub id: Uuid,
    pub space_id: Uuid,
    pub origin_device_id: Uuid,
    pub mutation: HistoryMutation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_waiting_transfer_state_is_read_as_failed() {
        let state: TransferState = serde_json::from_str("\"waitingForDevice\"").unwrap();

        assert_eq!(state, TransferState::Failed);
        assert_eq!(serde_json::to_string(&state).unwrap(), "\"failed\"");
    }

    #[test]
    fn history_sync_settings_default_to_disabled_for_existing_installations() {
        let settings: SettingsView = serde_json::from_str(
            r#"{
                "deviceName": "Mac",
                "receiveDirectory": "/tmp",
                "clipboardSyncEnabled": true,
                "historyRetention": "sevenDays",
                "launchAtStartup": false,
                "keepInTray": true,
                "notificationsEnabled": true
            }"#,
        )
        .unwrap();

        assert!(!settings.delete_sync_enabled);
        assert!(!settings.favorite_sync_enabled);
        assert!(settings.automatic_updates_enabled);
    }
}
