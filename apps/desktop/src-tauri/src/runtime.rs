use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use chrono::Utc;
use keyring::Entry;
use parking_lot::{Mutex, RwLock};
use synchalo_core::{
    AppError, AppSnapshot, CLIPBOARD_PAGE_SIZE, ClipboardDirection, ClipboardEvent,
    ClipboardHistoryPage, ClipboardItemView, DeviceConnectionState, DeviceView,
    FILE_HISTORY_PAGE_SIZE, HistoryRetention, HlcTimestamp, PairingCodeView, PairingRequestView,
    SettingsPatch, SettingsView, SyncState, SyncStatusView, TransferDirection,
    TransferHistoryFilter, TransferHistoryPage, TransferState, TransferTargetView, TransferView,
    UserFacingError, content_hash,
};
use synchalo_network::{
    DEFAULT_QUIC_PORT, DiscoveredPeer, DiscoveryConfig, DiscoveryEvent, DiscoveryService,
    LanTransport, PairingCodeManager, TransportCredentials, TransportEvent, TransportIdentity,
    TrustedPeer,
};
use synchalo_platform::{
    ClipboardMonitor, default_device_name, default_receive_directory, platform_capabilities,
    read_clipboard_files,
};
use synchalo_storage::{
    Database, IdentityRecord, LocalKeySource, decode_data_key, finalize_local_key,
    generate_data_key, load_local_key, write_pending_local_key,
};
use synchalo_transfer::{FileManifest, inspect_file};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt as _;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const EVENT_CLIPBOARD_ADDED: &str = "synchalo://clipboard-added";
pub const EVENT_CLIPBOARD_DELETED: &str = "synchalo://clipboard-deleted";
pub const EVENT_DEVICES_CHANGED: &str = "synchalo://devices-changed";
pub const EVENT_PAIRING_CODE_CHANGED: &str = "synchalo://pairing-code-changed";
pub const EVENT_PAIRING_REQUESTED: &str = "synchalo://pairing-requested";
pub const EVENT_SETTINGS_CHANGED: &str = "synchalo://settings-changed";
pub const EVENT_TRANSFER_CHANGED: &str = "synchalo://transfer-changed";
pub const EVENT_SYNC_STATUS_CHANGED: &str = "synchalo://sync-status-changed";
pub const EVENT_USER_ERROR: &str = "synchalo://user-error";
pub const EVENT_NAVIGATE: &str = "synchalo://navigate";

const SECURE_STORAGE_SERVICE: &str = "io.synchalo.desktop";
const KEY_ENCRYPTION_KEY_ACCOUNT: &str = "local-key-encryption-key-v1";
const LEGACY_DATA_KEY_ACCOUNT: &str = "local-data-key";
const TRANSPORT_CREDENTIALS_ACCOUNT: &str = "transport-credentials-v1";

struct KeyBootstrap {
    key_encryption_key: Zeroizing<[u8; 32]>,
    legacy_data_key: Option<Zeroizing<[u8; 32]>>,
    source: LocalKeySource,
    pending: bool,
}

struct KeychainKeyMaterial {
    key_encryption_key: [u8; 32],
    legacy_data_key: Option<Zeroizing<[u8; 32]>>,
}

pub struct AppRuntime {
    app: AppHandle,
    database: Arc<Database>,
    identity: RwLock<IdentityRecord>,
    settings: RwLock<SettingsView>,
    pairing: PairingCodeManager,
    transport: LanTransport,
    transport_port: u16,
    nearby: RwLock<HashMap<Uuid, DiscoveredPeer>>,
    discovery: Mutex<Option<DiscoveryService>>,
    clipboard: Mutex<Option<ClipboardMonitor>>,
    transfer_tasks: Mutex<HashMap<(Uuid, Uuid), tokio::task::AbortHandle>>,
    seen_clipboard_events: Mutex<HashSet<Uuid>>,
    reported_space_mismatches: Mutex<HashSet<Uuid>>,
    last_applied_clipboard: Mutex<Option<ClipboardOrderKey>>,
    paused: AtomicBool,
    persistent_storage: bool,
}

impl AppRuntime {
    pub async fn initialize(app: AppHandle) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let device_name = default_device_name();
        let ephemeral = std::env::var_os("SYNCHALO_EPHEMERAL_KEYS").is_some();
        let (database, transport_credentials, persistent_storage) = if ephemeral {
            let key = Zeroizing::new(generate_data_key()?);
            let database = Database::open_in_memory(*key, None)?;
            let credentials = TransportCredentials::generate()?;
            (database, credentials, false)
        } else {
            let data_dir = app.path().app_data_dir()?;
            let database_path = data_dir.join("synchalo.db");
            let bootstrap = prepare_local_key(&data_dir, &database_path)?;
            Database::open(
                &database_path,
                *bootstrap.key_encryption_key,
                bootstrap.legacy_data_key.as_deref().copied(),
            )
            .and_then(|database| {
                let credentials = load_database_transport_credentials(
                    &database,
                    bootstrap.pending.then_some(bootstrap.source),
                )?;
                if bootstrap.pending {
                    if bootstrap.source == LocalKeySource::KeychainMigration {
                        database.backup_if_missing(
                            data_dir.join("synchalo.keychain-migration-backup.db"),
                        )?;
                    }
                    finalize_local_key(&data_dir)?;
                    if bootstrap.source == LocalKeySource::KeychainMigration {
                        delete_migrated_keychain_items();
                    }
                }
                Ok((database, credentials, true))
            })?
        };
        let database = Arc::new(database);
        let identity = database.load_or_create_identity(&device_name)?;
        let defaults = SettingsView {
            device_name: identity.display_name.clone(),
            receive_directory: default_receive_directory().to_string_lossy().into_owned(),
            clipboard_sync_enabled: true,
            history_retention: HistoryRetention::SevenDays,
            launch_at_startup: false,
            keep_in_tray: true,
            notifications_enabled: true,
        };
        let mut settings = database.load_settings(defaults)?;
        if enforce_notifications_enabled(&mut settings) {
            database.save_settings(&settings)?;
        }
        let pairing = PairingCodeManager::new();
        let trusted = database
            .list_peer_credentials()?
            .into_iter()
            .filter_map(|json| match serde_json::from_str::<TrustedPeer>(&json) {
                Ok(peer) => Some(peer),
                Err(error) => {
                    tracing::warn!(%error, "ignoring invalid trusted peer record");
                    None
                }
            })
            .collect();
        let transport_identity = TransportIdentity {
            device_id: identity.device_id,
            device_name: identity.display_name.clone(),
            platform: synchalo_core::DevicePlatform::current(),
            space_id: identity.space_id,
            credentials: transport_credentials,
        };
        let (transport, transport_events) = start_transport(
            transport_identity,
            pairing.clone(),
            trusted,
            PathBuf::from(&settings.receive_directory),
        )?;
        let transport_port = transport.local_addr()?.port();

        let runtime = Arc::new(Self {
            app,
            database,
            identity: RwLock::new(identity),
            settings: RwLock::new(settings),
            pairing,
            transport,
            transport_port,
            nearby: RwLock::new(HashMap::new()),
            discovery: Mutex::new(None),
            clipboard: Mutex::new(None),
            transfer_tasks: Mutex::new(HashMap::new()),
            seen_clipboard_events: Mutex::new(HashSet::new()),
            reported_space_mismatches: Mutex::new(HashSet::new()),
            last_applied_clipboard: Mutex::new(None),
            paused: AtomicBool::new(false),
            persistent_storage,
        });
        let transport = runtime.transport.clone();
        tauri::async_runtime::spawn(async move { transport.run().await });
        runtime.start_transport_events(transport_events);
        runtime.start_clipboard();
        runtime.start_discovery();
        Ok(runtime)
    }

    pub fn keep_in_tray(&self) -> bool {
        self.settings.read().keep_in_tray
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> Result<AppSnapshot, AppError> {
        let devices = self.devices()?;
        let online_count = devices
            .iter()
            .filter(|device| device.connection_state == DeviceConnectionState::Online)
            .count();
        let offline_count = devices
            .iter()
            .filter(|device| {
                !device.is_current && device.connection_state == DeviceConnectionState::Offline
            })
            .count();
        let capabilities = platform_capabilities();
        let paused = self.paused.load(Ordering::Relaxed);
        let (state, label) = if paused {
            (SyncState::Paused, "同步已暂停".to_owned())
        } else if !self.persistent_storage {
            (
                SyncState::Limited,
                "安全存储不可用 · 历史仅保留到退出".to_owned(),
            )
        } else {
            (SyncState::Healthy, "同步正常".to_owned())
        };
        let clipboard_page =
            self.database
                .list_clipboard_page(None, false, 1, CLIPBOARD_PAGE_SIZE)?;
        let file_page = self.database.list_transfer_page(
            None,
            false,
            TransferHistoryFilter::All,
            1,
            FILE_HISTORY_PAGE_SIZE,
        )?;
        Ok(AppSnapshot {
            current_device_id: self.identity.read().device_id,
            sync_status: SyncStatusView {
                state,
                label,
                online_count,
                offline_count,
                clipboard_capability: capabilities.clipboard,
            },
            devices,
            clipboard_history_total: clipboard_page.total_items,
            clipboard_history: clipboard_page.items,
            file_history_total: file_page.total_items,
            file_history: file_page.items,
            settings: self.settings.read().clone(),
            pairing_code: self.pairing.current(),
            capabilities,
        })
    }

    pub fn devices(&self) -> Result<Vec<DeviceView>, AppError> {
        let identity = self.identity.read().clone();
        let now = Utc::now();
        let mut devices = vec![DeviceView {
            id: identity.device_id,
            name: identity.display_name,
            platform: synchalo_core::DevicePlatform::current(),
            connection_state: DeviceConnectionState::Online,
            is_current: true,
            address: None,
            last_seen_at: Some(now),
            last_sync_at: None,
            paused: false,
        }];
        let nearby = self.nearby.read();
        let online_ids = self.transport.online_peer_ids();
        for mut device in self.database.list_devices()? {
            if online_ids.contains(&device.id) {
                device.connection_state = DeviceConnectionState::Online;
                device.address = nearby
                    .get(&device.id)
                    .map(|peer| peer.address.to_string())
                    .or(device.address);
                device.last_seen_at = Some(now);
            } else {
                device.connection_state = DeviceConnectionState::Offline;
            }
            devices.push(device);
        }
        devices.sort_by_key(|device| {
            (
                !device.is_current,
                device.connection_state != DeviceConnectionState::Online,
                device.name.to_lowercase(),
            )
        });
        Ok(devices)
    }

    pub fn list_clipboard(
        &self,
        query: Option<&str>,
        favorites_only: bool,
        page: usize,
    ) -> Result<ClipboardHistoryPage, AppError> {
        self.database
            .list_clipboard_page(query, favorites_only, page, CLIPBOARD_PAGE_SIZE)
    }

    pub fn list_file_history(
        &self,
        query: Option<&str>,
        favorites_only: bool,
        filter: TransferHistoryFilter,
        page: usize,
    ) -> Result<TransferHistoryPage, AppError> {
        self.database.list_transfer_page(
            query,
            favorites_only,
            filter,
            page,
            FILE_HISTORY_PAGE_SIZE,
        )
    }

    pub fn copy_history_item(&self, id: Uuid) -> Result<ClipboardItemView, AppError> {
        let item = self
            .database
            .get_clipboard_item(id)?
            .ok_or_else(|| AppError::InvalidInput("clipboard item does not exist".to_owned()))?;
        let guard = self.clipboard.lock();
        let clipboard = guard
            .as_ref()
            .ok_or_else(|| AppError::Clipboard("clipboard monitor is unavailable".to_owned()))?;
        clipboard.set_text(item.content.clone())?;
        drop(guard);
        self.record_local_clipboard(item.content, Utc::now())
    }

    pub fn record_local_clipboard(
        &self,
        content: String,
        observed_at: chrono::DateTime<Utc>,
    ) -> Result<ClipboardItemView, AppError> {
        if content.is_empty() {
            return Err(AppError::InvalidInput("clipboard text is empty".to_owned()));
        }
        if content.len() > synchalo_core::MAX_CLIPBOARD_BYTES {
            return Err(AppError::InvalidInput(
                "clipboard text exceeds the 1 MiB limit".to_owned(),
            ));
        }
        let clipboard_hash = content_hash(content.as_bytes());
        let latest = self
            .database
            .list_clipboard_page(None, false, 1, 1)?
            .items
            .into_iter()
            .next();
        if is_consecutive_clipboard_duplicate(
            latest.as_ref().map(|item| item.content_hash.as_str()),
            &clipboard_hash,
        ) {
            return latest.ok_or_else(|| {
                AppError::Internal("clipboard duplicate lookup returned no item".to_owned())
            });
        }
        let identity = self.identity.read().clone();
        let (origin_sequence, hlc) = self
            .database
            .next_event_clock(observed_at.timestamp_millis())?;
        let item = ClipboardItemView {
            id: Uuid::now_v7(),
            content_hash: clipboard_hash,
            content,
            source_device_id: identity.device_id,
            source_device_name: identity.display_name,
            direction: ClipboardDirection::Local,
            created_at: observed_at,
            hlc,
            pinned: false,
        };
        *self.last_applied_clipboard.lock() = Some(ClipboardOrderKey::from_item(&item));
        let settings = self.settings.read().clone();
        if settings.history_retention != HistoryRetention::None {
            self.database.insert_clipboard_item(&item)?;
            self.database.prune_clipboard_history(
                settings.history_retention,
                synchalo_core::DEFAULT_HISTORY_LIMIT,
            )?;
            let _ = self.app.emit(EVENT_CLIPBOARD_ADDED, &item);
        }
        if settings.clipboard_sync_enabled && !self.paused.load(Ordering::Relaxed) {
            let transport = self.transport.clone();
            let target_ids: Vec<_> = self
                .database
                .list_devices()?
                .into_iter()
                .filter(|device| !device.paused)
                .map(|device| device.id)
                .collect();
            let event = ClipboardEvent {
                id: item.id,
                space_id: identity.space_id,
                origin_device_id: identity.device_id,
                origin_sequence,
                created_at: item.created_at,
                hlc: item.hlc,
                content: item.content.clone(),
                content_hash: item.content_hash.clone(),
            };
            tauri::async_runtime::spawn(async move {
                for peer_id in target_ids {
                    if let Err(error) = transport.send_clipboard_to(peer_id, event.clone()).await {
                        tracing::debug!(%peer_id, %error, "clipboard delivery failed");
                    }
                }
            });
        }
        Ok(item)
    }

    pub fn delete_clipboard(&self, id: Uuid) -> Result<bool, AppError> {
        let deleted = self.database.delete_clipboard_item(id)?;
        if deleted {
            let _ = self.app.emit(EVENT_CLIPBOARD_DELETED, id);
        }
        Ok(deleted)
    }

    pub fn database_clear_clipboard_history(&self) -> Result<usize, AppError> {
        self.database.clear_clipboard_history()
    }

    pub fn database_set_clipboard_pinned(&self, id: Uuid, pinned: bool) -> Result<bool, AppError> {
        self.database.set_clipboard_pinned(id, pinned)
    }

    pub fn restore_clipboard(&self, item: &ClipboardItemView) -> Result<(), AppError> {
        self.database.insert_clipboard_item(item)?;
        let _ = self.app.emit(EVENT_CLIPBOARD_ADDED, item);
        Ok(())
    }

    pub fn generate_pairing_code(self: &Arc<Self>) -> Result<PairingCodeView, AppError> {
        let code = self.pairing.generate(Duration::from_secs(60))?;
        if let Some(discovery) = self.discovery.lock().as_ref() {
            discovery.set_pairing_open(true)?;
        }
        let _ = self.app.emit(EVENT_PAIRING_CODE_CHANGED, Some(&code));

        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(61)).await;
            if runtime.pairing.current().is_none() {
                if let Some(discovery) = runtime.discovery.lock().as_ref() {
                    let _ = discovery.set_pairing_open(false);
                }
                let _ = runtime
                    .app
                    .emit::<Option<PairingCodeView>>(EVENT_PAIRING_CODE_CHANGED, None);
            }
        });
        Ok(code)
    }

    pub fn copy_pairing_code(&self) -> Result<PairingCodeView, AppError> {
        let code = self
            .pairing
            .current()
            .ok_or_else(|| AppError::InvalidInput("pairing code has expired".to_owned()))?;
        let clipboard = self.clipboard.lock();
        clipboard
            .as_ref()
            .ok_or_else(|| AppError::Clipboard("clipboard monitor is unavailable".to_owned()))?
            .set_text(code.code.replace(' ', ""))?;
        Ok(code)
    }

    pub fn respond_to_pairing(&self, request_id: Uuid, accepted: bool) -> Result<(), AppError> {
        if self.transport.respond_to_pairing(request_id, accepted) {
            Ok(())
        } else {
            Err(AppError::InvalidInput(
                "pairing request has expired".to_owned(),
            ))
        }
    }

    pub async fn join_with_code(self: &Arc<Self>, code: &str) -> Result<DeviceView, AppError> {
        let normalized: String = code.chars().filter(|char| char.is_ascii_digit()).collect();
        if normalized.len() != 6 {
            return Err(AppError::InvalidInput(
                "pairing code must contain six digits".to_owned(),
            ));
        }
        let mut peers: Vec<_> = self
            .nearby
            .read()
            .values()
            .filter(|peer| peer.pairing_open)
            .cloned()
            .collect();
        if peers.is_empty() {
            return Err(AppError::Network(
                "no nearby device currently accepts pairing".to_owned(),
            ));
        }
        peers.sort_by_key(|peer| peer.device_id);
        let mut last_error = None;
        let mut paired = None;
        for peer in peers.into_iter().take(3) {
            match self.transport.pair_with(&peer, &normalized).await {
                Ok(trusted) => {
                    paired = Some((peer, trusted));
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        let (peer, trusted) = paired.ok_or_else(|| {
            last_error.unwrap_or_else(|| {
                AppError::Network("no nearby device accepted this pairing code".to_owned())
            })
        })?;
        if self.identity.read().space_id != trusted.space_id {
            self.database.update_space_id(trusted.space_id)?;
            self.identity.write().space_id = trusted.space_id;
            self.transport.set_space_id(trusted.space_id);
        }
        self.persist_trusted_peer(&trusted, Some(peer.address))?;
        self.devices()?
            .into_iter()
            .find(|device| device.id == trusted.device_id)
            .ok_or_else(|| AppError::Internal("paired device was not persisted".to_owned()))
    }

    pub fn revoke_device(&self, id: Uuid) -> Result<bool, AppError> {
        if id == self.identity.read().device_id {
            return Err(AppError::InvalidInput(
                "the current device cannot revoke itself".to_owned(),
            ));
        }
        let changed = self.database.revoke_device(id)?;
        if changed {
            self.reported_space_mismatches.lock().remove(&id);
            self.transport.revoke_peer(id);
            self.emit_devices();
        }
        Ok(changed)
    }

    pub fn set_device_paused(&self, id: Uuid, paused: bool) -> Result<DeviceView, AppError> {
        if id == self.identity.read().device_id {
            return Err(AppError::InvalidInput(
                "use the global pause control for the current device".to_owned(),
            ));
        }
        let mut device = self
            .database
            .list_devices()?
            .into_iter()
            .find(|device| device.id == id)
            .ok_or_else(|| AppError::InvalidInput("device does not exist".to_owned()))?;
        device.paused = paused;
        self.database.upsert_device(&device)?;
        self.emit_devices();
        Ok(device)
    }

    pub fn pause_sync(&self, paused: bool) -> Result<SyncStatusView, AppError> {
        self.paused.store(paused, Ordering::Relaxed);
        let status = self.snapshot()?.sync_status;
        let _ = self.app.emit(EVENT_SYNC_STATUS_CHANGED, &status);
        Ok(status)
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> Result<SettingsView, AppError> {
        let mut settings = self.settings.write();
        if let Some(name) = patch.device_name {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 64 {
                return Err(AppError::InvalidInput(
                    "device name must contain 1 to 64 characters".to_owned(),
                ));
            }
            settings.device_name = name.to_owned();
            self.identity.write().display_name = name.to_owned();
            self.database.update_identity_name(name)?;
            self.transport.update_device_name(name.to_owned());
            if let Some(discovery) = self.discovery.lock().as_ref() {
                discovery.set_device_name(name.to_owned())?;
            }
        }
        if let Some(directory) = patch.receive_directory {
            let path = validate_receive_directory(&directory)?;
            settings.receive_directory = path.to_string_lossy().into_owned();
            self.transport.set_receive_directory(path);
        }
        if let Some(enabled) = patch.clipboard_sync_enabled {
            settings.clipboard_sync_enabled = enabled;
        }
        if let Some(retention) = patch.history_retention {
            settings.history_retention = retention;
        }
        if let Some(enabled) = patch.launch_at_startup {
            settings.launch_at_startup = enabled;
        }
        if let Some(enabled) = patch.keep_in_tray {
            settings.keep_in_tray = enabled;
        }
        enforce_notifications_enabled(&mut settings);
        self.database.save_settings(&settings)?;
        let result = settings.clone();
        let _ = self.app.emit(EVENT_SETTINGS_CHANGED, &result);
        Ok(result)
    }

    pub async fn enqueue_paths(
        self: &Arc<Self>,
        paths: Vec<PathBuf>,
        target_ids: Option<Vec<Uuid>>,
    ) -> Result<Vec<TransferView>, AppError> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        if paths.len() > 100 {
            return Err(AppError::InvalidInput(
                "no more than 100 files can be queued at once".to_owned(),
            ));
        }
        let requested: Option<HashSet<_>> = target_ids.map(|ids| ids.into_iter().collect());
        let targets = select_transfer_targets(self.devices()?, requested.as_ref());
        if let Some(requested) = requested.as_ref().filter(|ids| !ids.is_empty())
            && targets.len() != requested.len()
        {
            return Err(AppError::NoSyncDevices);
        }
        if targets.is_empty() {
            return Err(AppError::NoSyncDevices);
        }
        let online_ids = self.transport.online_peer_ids();
        let has_online_target = targets.iter().any(|device| online_ids.contains(&device.id));
        let has_offline_target = targets
            .iter()
            .any(|device| !online_ids.contains(&device.id));
        let mut transfers = Vec::with_capacity(paths.len());
        for path in paths {
            let canonical = path.canonicalize().map_err(file_error)?;
            let (manifest, _) = inspect_file(&canonical).await?;
            let state = if has_online_target {
                TransferState::Queued
            } else {
                TransferState::Failed
            };
            let transfer = TransferView {
                id: manifest.id,
                file_name: manifest.file_name.clone(),
                file_size: manifest.file_size,
                direction: TransferDirection::Sending,
                state,
                progress: 0.0,
                created_at: Utc::now(),
                source_device_name: Some(self.identity.read().display_name.clone()),
                targets: targets
                    .iter()
                    .map(|device| TransferTargetView {
                        device_id: device.id,
                        device_name: device.name.clone(),
                        state: if online_ids.contains(&device.id) {
                            TransferState::Queued
                        } else {
                            TransferState::Failed
                        },
                        progress: 0.0,
                        bytes_per_second: None,
                        error: (!online_ids.contains(&device.id))
                            .then(|| "目标设备当前离线，文件未发送".to_owned()),
                    })
                    .collect(),
                bytes_per_second: None,
                eta_seconds: None,
                display_path: Some(canonical.to_string_lossy().into_owned()),
                error: has_offline_target.then(|| {
                    if has_online_target {
                        "部分目标设备当前离线，文件未发送".to_owned()
                    } else {
                        "目标设备当前离线，文件未发送".to_owned()
                    }
                }),
                content_hash: Some(manifest.blake3.clone()),
                source_modified_unix_ms: manifest.modified_unix_ms,
                pinned: false,
            };
            self.database.upsert_transfer(&transfer)?;
            let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
            for device in &targets {
                if online_ids.contains(&device.id) {
                    self.launch_file_transfer(transfer.id, device.id)?;
                }
            }
            transfers.push(self.transfer(transfer.id)?);
        }
        Ok(transfers)
    }

    pub async fn paste_files(
        self: &Arc<Self>,
        target_ids: Option<Vec<Uuid>>,
    ) -> Result<Vec<TransferView>, AppError> {
        let paths = tokio::task::spawn_blocking(read_clipboard_files)
            .await
            .map_err(|error| AppError::Internal(error.to_string()))??;
        self.enqueue_paths(paths, target_ids).await
    }

    pub fn update_transfer_state(
        &self,
        id: Uuid,
        state: TransferState,
    ) -> Result<TransferView, AppError> {
        let mut transfer = self.transfer(id)?;
        transfer.state = state;
        transfer.error = None;
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        Ok(transfer)
    }

    pub async fn retry_transfer(self: &Arc<Self>, id: Uuid) -> Result<TransferView, AppError> {
        let mut transfer = self.transfer(id)?;
        if transfer.targets.is_empty() {
            return Err(AppError::NoSyncDevices);
        }
        let online = self.transport.online_peer_ids();
        let target_ids: Vec<_> = transfer
            .targets
            .iter_mut()
            .filter_map(|target| {
                target.progress = 0.0;
                target.bytes_per_second = None;
                if online.contains(&target.device_id) {
                    target.state = TransferState::Queued;
                    target.error = None;
                    Some(target.device_id)
                } else {
                    target.state = TransferState::Failed;
                    target.error = Some("目标设备当前离线，文件未发送".to_owned());
                    None
                }
            })
            .collect();
        transfer.progress = 0.0;
        transfer.bytes_per_second = None;
        transfer.eta_seconds = None;
        if target_ids.is_empty() {
            transfer.state = TransferState::Failed;
            transfer.error = Some("目标设备当前离线，文件未发送".to_owned());
        } else {
            transfer.state = TransferState::Queued;
            transfer.error = transfer
                .targets
                .iter()
                .any(|target| target.state == TransferState::Failed)
                .then(|| "部分目标设备当前离线，文件未发送".to_owned());
        }
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        for peer_id in target_ids {
            self.launch_file_transfer(id, peer_id)?;
        }
        Ok(transfer)
    }

    pub async fn resync_transfer(
        self: &Arc<Self>,
        id: Uuid,
        target_ids: Option<Vec<Uuid>>,
    ) -> Result<Vec<TransferView>, AppError> {
        let transfer = self.transfer(id)?;
        let path = transfer
            .display_path
            .ok_or_else(|| AppError::File("source path is missing".to_owned()))?;
        self.enqueue_paths(vec![PathBuf::from(path)], target_ids)
            .await
    }

    pub fn set_transfer_pinned(&self, id: Uuid, pinned: bool) -> Result<TransferView, AppError> {
        let mut transfer = self.transfer(id)?;
        transfer.pinned = pinned;
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        Ok(transfer)
    }

    pub fn cancel_transfer(&self, id: Uuid) -> Result<TransferView, AppError> {
        let keys: Vec<_> = self
            .transfer_tasks
            .lock()
            .keys()
            .filter(|(transfer_id, _)| *transfer_id == id)
            .copied()
            .collect();
        for key in keys {
            if let Some(handle) = self.transfer_tasks.lock().remove(&key) {
                handle.abort();
            }
        }
        let mut transfer = self.update_transfer_state(id, TransferState::Cancelled)?;
        for target in &mut transfer.targets {
            if target.state != TransferState::Completed {
                target.state = TransferState::Cancelled;
                target.bytes_per_second = None;
            }
        }
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        Ok(transfer)
    }

    fn launch_file_transfer(
        self: &Arc<Self>,
        transfer_id: Uuid,
        peer_id: Uuid,
    ) -> Result<(), AppError> {
        if self
            .transfer_tasks
            .lock()
            .contains_key(&(transfer_id, peer_id))
        {
            return Ok(());
        }
        let mut transfer = self.transfer(transfer_id)?;
        if transfer.direction != TransferDirection::Sending
            || transfer.state == TransferState::Cancelled
            || transfer.state == TransferState::Completed
        {
            return Ok(());
        }
        let path = PathBuf::from(
            transfer
                .display_path
                .clone()
                .ok_or_else(|| AppError::File("source path is missing".to_owned()))?,
        );
        if let Some(target) = transfer
            .targets
            .iter_mut()
            .find(|target| target.device_id == peer_id)
        {
            target.state = TransferState::Transferring;
            target.error = None;
        }
        transfer.state = TransferState::Transferring;
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);

        let runtime = self.clone();
        let (start_tx, start_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            if start_rx.await.is_err() {
                return;
            }
            let result = async {
                let expected = runtime.transfer(transfer_id)?;
                let manifest = manifest_from_transfer(&expected, &path).await?;
                runtime.transport.send_file(peer_id, &path, manifest).await
            }
            .await;
            runtime
                .transfer_tasks
                .lock()
                .remove(&(transfer_id, peer_id));
            if let Err(error) = result
                && runtime
                    .transfer(transfer_id)
                    .is_ok_and(|transfer| transfer.state != TransferState::Cancelled)
            {
                let _ = runtime.fail_file_transfer(transfer_id, peer_id, error.to_string(), false);
            }
        });
        self.transfer_tasks
            .lock()
            .insert((transfer_id, peer_id), task.abort_handle());
        let _ = start_tx.send(());
        Ok(())
    }

    pub fn delete_transfer(&self, id: Uuid) -> Result<bool, AppError> {
        self.database.delete_transfer(id)
    }

    pub fn clear_file_history(&self) -> Result<usize, AppError> {
        self.database.clear_transfer_history()
    }

    pub fn transfer(&self, id: Uuid) -> Result<TransferView, AppError> {
        self.database
            .get_transfer(id)?
            .ok_or_else(|| AppError::InvalidInput("transfer does not exist".to_owned()))
    }

    pub fn settings(&self) -> SettingsView {
        self.settings.read().clone()
    }

    pub fn app_handle(&self) -> &AppHandle {
        &self.app
    }

    fn start_transport_events(
        self: &Arc<Self>,
        mut events: tokio::sync::mpsc::UnboundedReceiver<TransportEvent>,
    ) {
        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(event) = events.recv().await {
                let result = match event {
                    TransportEvent::PairingApprovalRequested(candidate) => {
                        let request = PairingRequestView {
                            request_id: candidate.request_id,
                            device_id: candidate.device_id,
                            device_name: candidate.device_name,
                            platform: candidate.platform,
                        };
                        let _ = runtime.app.emit(EVENT_PAIRING_REQUESTED, request);
                        if let Some(window) = runtime.app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                        Ok(())
                    }
                    TransportEvent::Paired {
                        peer,
                        adopted_space_id,
                        ..
                    } => {
                        if runtime.identity.read().space_id != adopted_space_id {
                            runtime.database.update_space_id(adopted_space_id)?;
                            runtime.identity.write().space_id = adopted_space_id;
                            runtime.transport.set_space_id(adopted_space_id);
                        }
                        let address = runtime
                            .nearby
                            .read()
                            .get(&peer.device_id)
                            .map(|discovered| discovered.address);
                        runtime.persist_trusted_peer(&peer, address)?;
                        if let Some(discovery) = runtime.discovery.lock().as_ref() {
                            let _ = discovery.set_pairing_open(false);
                        }
                        let _ = runtime
                            .app
                            .emit::<Option<PairingCodeView>>(EVENT_PAIRING_CODE_CHANGED, None);
                        runtime.emit_devices();
                        Ok(())
                    }
                    TransportEvent::PeerOnline { device_id, address } => {
                        runtime.reported_space_mismatches.lock().remove(&device_id);
                        runtime.update_peer_connection(device_id, true, Some(address))
                    }
                    TransportEvent::PeerOffline { device_id } => {
                        runtime.update_peer_connection(device_id, false, None)?;
                        runtime.schedule_reconnect(device_id);
                        Ok(())
                    }
                    TransportEvent::ClipboardReceived {
                        from_device_id,
                        event,
                    } => runtime.apply_remote_clipboard(from_device_id, event),
                    TransportEvent::IncomingFileStarted {
                        from_device_id,
                        manifest,
                    } => runtime.record_incoming_file(from_device_id, manifest),
                    TransportEvent::FileProgress {
                        transfer_id,
                        peer_device_id,
                        transferred,
                        total,
                        bytes_per_second,
                        incoming,
                    } => runtime.update_file_progress(
                        transfer_id,
                        peer_device_id,
                        transferred,
                        total,
                        bytes_per_second,
                        incoming,
                    ),
                    TransportEvent::FileCompleted {
                        transfer_id,
                        peer_device_id,
                        path,
                        incoming,
                    } => {
                        runtime.complete_file_transfer(transfer_id, peer_device_id, path, incoming)
                    }
                    TransportEvent::FileFailed {
                        transfer_id,
                        peer_device_id,
                        error,
                        incoming,
                    } => runtime.fail_file_transfer(transfer_id, peer_device_id, error, incoming),
                    TransportEvent::Error(message) => {
                        tracing::debug!(%message, "transport event error");
                        Ok(())
                    }
                };
                if let Err(error) = result {
                    runtime.emit_error(error);
                }
            }
            Ok::<(), AppError>(())
        });
    }

    fn persist_trusted_peer(
        &self,
        peer: &TrustedPeer,
        address: Option<std::net::SocketAddr>,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let device = DeviceView {
            id: peer.device_id,
            name: peer.device_name.clone(),
            platform: peer.platform,
            connection_state: if address.is_some() {
                DeviceConnectionState::Online
            } else {
                DeviceConnectionState::Offline
            },
            is_current: false,
            address: address.map(|value| value.to_string()),
            last_seen_at: address.map(|_| now),
            last_sync_at: None,
            paused: false,
        };
        let credential_json =
            serde_json::to_string(peer).map_err(|error| AppError::Storage(error.to_string()))?;
        self.database
            .upsert_trusted_device(&device, &credential_json)
    }

    fn update_peer_connection(
        &self,
        device_id: Uuid,
        online: bool,
        address: Option<std::net::SocketAddr>,
    ) -> Result<(), AppError> {
        let Some(mut device) = self
            .database
            .list_devices()?
            .into_iter()
            .find(|device| device.id == device_id)
        else {
            return Ok(());
        };
        device.connection_state = if online {
            DeviceConnectionState::Online
        } else {
            DeviceConnectionState::Offline
        };
        if online {
            device.address = address.map(|value| value.to_string());
            device.last_seen_at = Some(Utc::now());
        }
        self.database.upsert_device(&device)?;
        self.emit_devices();
        Ok(())
    }

    fn should_initiate_connection(&self, peer_id: Uuid) -> bool {
        self.identity.read().device_id.as_bytes() < peer_id.as_bytes()
    }

    fn schedule_reconnect(self: &Arc<Self>, peer_id: Uuid) {
        if !self.should_initiate_connection(peer_id) {
            return;
        }
        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(750)).await;
            if runtime.transport.online_peer_ids().contains(&peer_id) {
                return;
            }
            let address = runtime.nearby.read().get(&peer_id).map(|peer| peer.address);
            if let Some(address) = address
                && let Err(error) = runtime.transport.connect_trusted(peer_id, address).await
            {
                runtime.handle_reconnect_error(peer_id, error);
            }
        });
    }

    fn handle_reconnect_error(&self, peer_id: Uuid, error: AppError) {
        if matches!(&error, AppError::SyncSpaceMismatch) {
            if self.reported_space_mismatches.lock().insert(peer_id) {
                tracing::warn!(%peer_id, %error, "trusted peer requires re-pairing");
                let user_error = UserFacingError::from(error);
                let _ = self.app.emit(EVENT_USER_ERROR, user_error);
            }
        } else {
            tracing::debug!(%peer_id, %error, "trusted peer reconnect failed");
        }
    }

    fn apply_remote_clipboard(
        &self,
        from_device_id: Uuid,
        event: ClipboardEvent,
    ) -> Result<(), AppError> {
        if event.space_id != self.identity.read().space_id {
            return Err(AppError::Network(
                "received clipboard event for another sync space".to_owned(),
            ));
        }
        {
            let mut seen = self.seen_clipboard_events.lock();
            if !seen.insert(event.id) {
                return Ok(());
            }
            if seen.len() > 10_000 {
                seen.clear();
                seen.insert(event.id);
            }
        }
        self.database
            .merge_remote_clock(event.hlc, Utc::now().timestamp_millis())?;
        let peer = self
            .transport
            .trusted_peer(from_device_id)
            .ok_or_else(|| AppError::Network("clipboard sender is not trusted".to_owned()))?;
        let item = ClipboardItemView {
            id: event.id,
            content: event.content,
            content_hash: event.content_hash,
            source_device_id: event.origin_device_id,
            source_device_name: peer.device_name,
            direction: ClipboardDirection::Received,
            created_at: event.created_at,
            hlc: event.hlc,
            pinned: false,
        };
        let settings = self.settings.read().clone();
        let inserted = if settings.history_retention == HistoryRetention::None {
            true
        } else {
            self.database.insert_clipboard_item(&item)?
        };
        if !inserted {
            return Ok(());
        }
        if settings.history_retention != HistoryRetention::None {
            let _ = self.app.emit(EVENT_CLIPBOARD_ADDED, &item);
        }
        let order_key = ClipboardOrderKey::from_item(&item);
        let should_apply = {
            let mut last = self.last_applied_clipboard.lock();
            if last.as_ref().is_none_or(|current| order_key > *current) {
                *last = Some(order_key);
                true
            } else {
                false
            }
        };
        if should_apply
            && settings.clipboard_sync_enabled
            && !self.paused.load(Ordering::Relaxed)
            && let Some(clipboard) = self.clipboard.lock().as_ref()
        {
            clipboard.set_text(item.content.clone())?;
        }
        if let Some(mut device) = self
            .database
            .list_devices()?
            .into_iter()
            .find(|device| device.id == from_device_id)
        {
            device.last_sync_at = Some(Utc::now());
            self.database.upsert_device(&device)?;
        }
        Ok(())
    }

    fn record_incoming_file(
        &self,
        from_device_id: Uuid,
        manifest: synchalo_transfer::FileManifest,
    ) -> Result<(), AppError> {
        let peer = self
            .transport
            .trusted_peer(from_device_id)
            .ok_or_else(|| AppError::Network("file sender is not trusted".to_owned()))?;
        let transfer = TransferView {
            id: manifest.id,
            file_name: manifest.file_name.clone(),
            file_size: manifest.file_size,
            direction: TransferDirection::Receiving,
            state: TransferState::Transferring,
            progress: 0.0,
            created_at: Utc::now(),
            source_device_name: Some(peer.device_name),
            targets: Vec::new(),
            bytes_per_second: None,
            eta_seconds: None,
            display_path: Some(
                PathBuf::from(&self.settings.read().receive_directory)
                    .join(&manifest.file_name)
                    .to_string_lossy()
                    .into_owned(),
            ),
            error: None,
            content_hash: Some(manifest.blake3),
            source_modified_unix_ms: manifest.modified_unix_ms,
            pinned: false,
        };
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        Ok(())
    }

    fn update_file_progress(
        &self,
        transfer_id: Uuid,
        peer_device_id: Uuid,
        transferred: u64,
        total: u64,
        bytes_per_second: u64,
        incoming: bool,
    ) -> Result<(), AppError> {
        let mut transfer = self.transfer(transfer_id)?;
        let progress = if total == 0 {
            1.0
        } else {
            (transferred as f64 / total as f64).clamp(0.0, 1.0) as f32
        };
        transfer.state = TransferState::Transferring;
        transfer.bytes_per_second = Some(bytes_per_second);
        transfer.eta_seconds = (bytes_per_second > 0)
            .then(|| total.saturating_sub(transferred) / bytes_per_second.max(1));
        if incoming {
            transfer.progress = progress;
        } else {
            if let Some(target) = transfer
                .targets
                .iter_mut()
                .find(|target| target.device_id == peer_device_id)
            {
                target.state = TransferState::Transferring;
                target.progress = progress;
                target.bytes_per_second = Some(bytes_per_second);
            }
            transfer.progress = aggregate_progress(&transfer.targets);
        }
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        Ok(())
    }

    fn complete_file_transfer(
        &self,
        transfer_id: Uuid,
        peer_device_id: Uuid,
        path: Option<PathBuf>,
        incoming: bool,
    ) -> Result<(), AppError> {
        let mut transfer = self.transfer(transfer_id)?;
        transfer.error = None;
        transfer.bytes_per_second = None;
        transfer.eta_seconds = None;
        if incoming {
            transfer.state = TransferState::Completed;
            transfer.progress = 1.0;
            if let Some(path) = path {
                transfer.display_path = Some(path.to_string_lossy().into_owned());
            }
        } else {
            if let Some(target) = transfer
                .targets
                .iter_mut()
                .find(|target| target.device_id == peer_device_id)
            {
                target.state = TransferState::Completed;
                target.progress = 1.0;
                target.bytes_per_second = None;
                target.error = None;
            }
            transfer.progress = aggregate_progress(&transfer.targets);
            transfer.state = if transfer
                .targets
                .iter()
                .all(|target| target.state == TransferState::Completed)
            {
                TransferState::Completed
            } else if transfer
                .targets
                .iter()
                .any(|target| target.state == TransferState::Failed)
            {
                TransferState::Failed
            } else {
                TransferState::Transferring
            };
            transfer.error = if transfer.state == TransferState::Failed {
                transfer
                    .targets
                    .iter()
                    .find_map(|target| target.error.clone())
            } else {
                None
            };
        }
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        if self.settings.read().notifications_enabled
            && (incoming || transfer.state == TransferState::Completed)
        {
            let body = if incoming {
                format!("{} 已保存到接收目录", transfer.file_name)
            } else {
                format!("{} 已发送完成", transfer.file_name)
            };
            let _ = self
                .app
                .notification()
                .builder()
                .title("SyncHalo 文件同步完成")
                .body(body)
                .show();
        }
        Ok(())
    }

    fn fail_file_transfer(
        &self,
        transfer_id: Uuid,
        peer_device_id: Uuid,
        error: String,
        incoming: bool,
    ) -> Result<(), AppError> {
        let mut transfer = self.transfer(transfer_id)?;
        transfer.error = Some(error.clone());
        transfer.bytes_per_second = None;
        transfer.eta_seconds = None;
        if incoming {
            transfer.state = TransferState::Failed;
        } else if let Some(target) = transfer
            .targets
            .iter_mut()
            .find(|target| target.device_id == peer_device_id)
        {
            target.state = TransferState::Failed;
            target.error = Some(error);
            target.bytes_per_second = None;
            transfer.state = TransferState::Failed;
        }
        self.database.upsert_transfer(&transfer)?;
        let _ = self.app.emit(EVENT_TRANSFER_CHANGED, &transfer);
        Ok(())
    }

    fn start_clipboard(self: &Arc<Self>) {
        match ClipboardMonitor::start(Duration::from_millis(350)) {
            Ok(mut monitor) => {
                let mut observations = monitor
                    .take_observations()
                    .expect("clipboard observations are available once");
                *self.clipboard.lock() = Some(monitor);
                let runtime = self.clone();
                tauri::async_runtime::spawn(async move {
                    while let Some(observation) = observations.recv().await {
                        match observation {
                            Ok(observation) => {
                                if let Err(error) = runtime.record_local_clipboard(
                                    observation.text,
                                    observation.observed_at,
                                ) {
                                    runtime.emit_error(error);
                                }
                            }
                            Err(error) => runtime.emit_error(error),
                        }
                    }
                });
            }
            Err(error) => self.emit_error(error),
        }
    }

    fn start_discovery(self: &Arc<Self>) {
        let identity = self.identity.read().clone();
        let config = DiscoveryConfig {
            device_id: identity.device_id,
            device_name: identity.display_name,
            platform: synchalo_core::DevicePlatform::current(),
            port: self.transport_port,
            pairing_open: false,
        };
        match DiscoveryService::start(config) {
            Ok(mut discovery) => {
                let mut events = discovery
                    .take_events()
                    .expect("discovery events are available once");
                *self.discovery.lock() = Some(discovery);
                let runtime = self.clone();
                tauri::async_runtime::spawn(async move {
                    while let Some(event) = events.recv().await {
                        match event {
                            DiscoveryEvent::Resolved(peer) => {
                                let peer_id = peer.device_id;
                                let address = peer.address;
                                let should_connect =
                                    runtime.transport.trusted_peer(peer_id).is_some()
                                        && !runtime.transport.online_peer_ids().contains(&peer_id)
                                        && runtime.should_initiate_connection(peer_id)
                                        && peer.protocol_version == synchalo_core::PROTOCOL_VERSION;
                                runtime.nearby.write().insert(peer_id, peer);
                                runtime.emit_devices();
                                if should_connect {
                                    let connect_runtime = runtime.clone();
                                    tauri::async_runtime::spawn(async move {
                                        if let Err(error) = connect_runtime
                                            .transport
                                            .connect_trusted(peer_id, address)
                                            .await
                                        {
                                            connect_runtime.handle_reconnect_error(peer_id, error);
                                        }
                                    });
                                }
                            }
                            DiscoveryEvent::Removed { fullname } => {
                                runtime
                                    .nearby
                                    .write()
                                    .retain(|_, peer| peer.fullname != fullname);
                                runtime.emit_devices();
                            }
                            DiscoveryEvent::Error(message) => {
                                runtime.emit_error(AppError::Network(message));
                            }
                        }
                    }
                });
            }
            Err(error) => self.emit_error(error),
        }
    }

    fn emit_devices(&self) {
        match self.devices() {
            Ok(devices) => {
                let _ = self.app.emit(EVENT_DEVICES_CHANGED, devices);
            }
            Err(error) => self.emit_error(error),
        }
    }

    fn emit_error(&self, error: AppError) {
        tracing::warn!(%error, "user-facing runtime error");
        let error = UserFacingError::from(error);
        let _ = self.app.emit(EVENT_USER_ERROR, error);
    }
}

fn start_transport(
    identity: TransportIdentity,
    pairing: PairingCodeManager,
    trusted: Vec<TrustedPeer>,
    receive_directory: PathBuf,
) -> Result<
    (
        LanTransport,
        tokio::sync::mpsc::UnboundedReceiver<TransportEvent>,
    ),
    AppError,
> {
    let mut last_error = None;
    for port in DEFAULT_QUIC_PORT..=DEFAULT_QUIC_PORT + 10 {
        match LanTransport::start(
            identity.clone(),
            pairing.clone(),
            trusted.clone(),
            receive_directory.clone(),
            port,
        ) {
            Ok(transport) => return Ok(transport),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        AppError::Network("no QUIC port is available in the configured range".to_owned())
    }))
}

fn prepare_local_key(data_dir: &Path, database_path: &Path) -> Result<KeyBootstrap, AppError> {
    if let Some(local_key) = load_local_key(data_dir)? {
        let legacy_data_key =
            if local_key.pending && local_key.source == LocalKeySource::KeychainMigration {
                load_optional_legacy_keychain_key()?
            } else {
                None
            };
        return Ok(KeyBootstrap {
            key_encryption_key: local_key.key,
            legacy_data_key,
            source: local_key.source,
            pending: local_key.pending,
        });
    }

    let (key_encryption_key, legacy_data_key, source) = if database_path.exists() {
        let material = load_keychain_key_material()?;
        (
            Zeroizing::new(material.key_encryption_key),
            material.legacy_data_key,
            LocalKeySource::KeychainMigration,
        )
    } else {
        (
            Zeroizing::new(generate_data_key()?),
            None,
            LocalKeySource::Fresh,
        )
    };
    write_pending_local_key(data_dir, &key_encryption_key, source)?;
    Ok(KeyBootstrap {
        key_encryption_key,
        legacy_data_key,
        source,
        pending: true,
    })
}

fn load_keychain_key_material() -> Result<KeychainKeyMaterial, AppError> {
    let entry =
        Entry::new(SECURE_STORAGE_SERVICE, KEY_ENCRYPTION_KEY_ACCOUNT).map_err(keychain_error)?;
    match entry.get_password() {
        Ok(encoded) => Ok(KeychainKeyMaterial {
            key_encryption_key: decode_data_key(&encoded)?,
            legacy_data_key: load_optional_legacy_keychain_key()?,
        }),
        Err(keyring::Error::NoEntry) => {
            let legacy = load_optional_legacy_keychain_key()?.ok_or_else(|| {
                AppError::Storage("existing database key is unavailable".to_owned())
            })?;
            Ok(KeychainKeyMaterial {
                key_encryption_key: generate_data_key()?,
                legacy_data_key: Some(legacy),
            })
        }
        Err(error) => Err(keychain_error(error)),
    }
}

fn load_optional_legacy_keychain_key() -> Result<Option<Zeroizing<[u8; 32]>>, AppError> {
    let entry =
        Entry::new(SECURE_STORAGE_SERVICE, LEGACY_DATA_KEY_ACCOUNT).map_err(keychain_error)?;
    match entry.get_password() {
        Ok(encoded) => Ok(Some(Zeroizing::new(decode_data_key(&encoded)?))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(keychain_error(error)),
    }
}

fn load_database_transport_credentials(
    database: &Database,
    pending_source: Option<LocalKeySource>,
) -> Result<TransportCredentials, AppError> {
    if let Some(json) = database.load_local_secret(TRANSPORT_CREDENTIALS_ACCOUNT)? {
        return serde_json::from_slice(&json).map_err(storage_serialization_error);
    }

    let credentials = match pending_source {
        Some(LocalKeySource::KeychainMigration) => load_keychain_transport_credentials()?,
        Some(LocalKeySource::Fresh) => TransportCredentials::generate()?,
        None if !database.has_identity()? => TransportCredentials::generate()?,
        None => {
            return Err(AppError::Storage(
                "encrypted transport credentials are missing".to_owned(),
            ));
        }
    };
    let json = serde_json::to_vec(&credentials).map_err(storage_serialization_error)?;
    database.save_local_secret(TRANSPORT_CREDENTIALS_ACCOUNT, &json)?;
    let verified = database
        .load_local_secret(TRANSPORT_CREDENTIALS_ACCOUNT)?
        .ok_or_else(|| AppError::Storage("transport credential migration failed".to_owned()))?;
    serde_json::from_slice(&verified).map_err(storage_serialization_error)
}

fn load_keychain_transport_credentials() -> Result<TransportCredentials, AppError> {
    let entry = Entry::new(SECURE_STORAGE_SERVICE, TRANSPORT_CREDENTIALS_ACCOUNT)
        .map_err(keychain_error)?;
    let json = entry.get_password().map_err(keychain_error)?;
    serde_json::from_str(&json).map_err(storage_serialization_error)
}

fn delete_migrated_keychain_items() {
    for account in [
        KEY_ENCRYPTION_KEY_ACCOUNT,
        LEGACY_DATA_KEY_ACCOUNT,
        TRANSPORT_CREDENTIALS_ACCOUNT,
    ] {
        let Ok(entry) = Entry::new(SECURE_STORAGE_SERVICE, account) else {
            continue;
        };
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => {
                tracing::warn!(%error, account, "failed to remove migrated keychain item")
            }
        }
    }
}

fn keychain_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(format!("keychain migration failed: {error}"))
}

fn storage_serialization_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(error.to_string())
}

fn enforce_notifications_enabled(settings: &mut SettingsView) -> bool {
    let changed = !settings.notifications_enabled;
    settings.notifications_enabled = true;
    changed
}

fn validate_receive_directory(value: &str) -> Result<PathBuf, AppError> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(AppError::InvalidInput(
            "receive directory must be an absolute path".to_owned(),
        ));
    }
    std::fs::create_dir_all(path).map_err(file_error)?;
    let canonical = path.canonicalize().map_err(file_error)?;
    if !canonical.is_dir() {
        return Err(AppError::InvalidInput(
            "receive directory is not a directory".to_owned(),
        ));
    }
    Ok(canonical)
}

fn file_error(error: impl std::fmt::Display) -> AppError {
    AppError::File(error.to_string())
}

fn select_transfer_targets(
    devices: Vec<DeviceView>,
    requested: Option<&HashSet<Uuid>>,
) -> Vec<DeviceView> {
    let requested = requested.filter(|ids| !ids.is_empty());
    devices
        .into_iter()
        .filter(|device| !device.is_current && !device.paused)
        .filter(|device| {
            requested.map_or(
                device.connection_state == DeviceConnectionState::Online,
                |ids| ids.contains(&device.id),
            )
        })
        .collect()
}

fn aggregate_progress(targets: &[TransferTargetView]) -> f32 {
    if targets.is_empty() {
        return 0.0;
    }
    targets.iter().map(|target| target.progress).sum::<f32>() / targets.len() as f32
}

async fn manifest_from_transfer(
    transfer: &TransferView,
    path: &Path,
) -> Result<FileManifest, AppError> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(file_error)?;
    let modified_unix_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|value| value.as_millis().min(u64::MAX as u128) as u64);
    let content_hash = transfer
        .content_hash
        .clone()
        .ok_or_else(|| AppError::File("stored source hash is missing".to_owned()))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() != transfer.file_size
        || modified_unix_ms != transfer.source_modified_unix_ms
    {
        return Err(AppError::File(
            "source file changed after the task was created".to_owned(),
        ));
    }
    Ok(FileManifest {
        id: transfer.id,
        file_name: transfer.file_name.clone(),
        file_size: transfer.file_size,
        blake3: content_hash,
        modified_unix_ms,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ClipboardOrderKey {
    hlc: HlcTimestamp,
    origin_device_id: Uuid,
    event_id: Uuid,
}

fn is_consecutive_clipboard_duplicate(
    latest_content_hash: Option<&str>,
    candidate_content_hash: &str,
) -> bool {
    latest_content_hash == Some(candidate_content_hash)
}

impl ClipboardOrderKey {
    fn from_item(item: &ClipboardItemView) -> Self {
        Self {
            hlc: item.hlc,
            origin_device_id: item.source_device_id,
            event_id: item.id,
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn finalized_local_key_bypasses_keychain_migration() {
        let directory = tempdir().unwrap();
        let key = [83_u8; 32];
        write_pending_local_key(directory.path(), &key, LocalKeySource::Fresh).unwrap();
        finalize_local_key(directory.path()).unwrap();
        let database_path = directory.path().join("synchalo.db");
        std::fs::write(&database_path, b"existing").unwrap();

        let bootstrap = prepare_local_key(directory.path(), &database_path).unwrap();
        assert!(!bootstrap.pending);
        assert_eq!(bootstrap.source, LocalKeySource::Fresh);
        assert_eq!(&*bootstrap.key_encryption_key, &key);
        assert!(bootstrap.legacy_data_key.is_none());
    }

    #[test]
    fn transport_credentials_round_trip_through_encrypted_database_secret() {
        let database = Database::open_in_memory([91_u8; 32], None).unwrap();
        let first =
            load_database_transport_credentials(&database, Some(LocalKeySource::Fresh)).unwrap();
        let second = load_database_transport_credentials(&database, None).unwrap();
        assert_eq!(first.verifying_key(), second.verifying_key());
        assert_eq!(
            first.certificate_fingerprint(),
            second.certificate_fingerprint()
        );
    }

    #[test]
    fn notifications_are_always_enabled() {
        let mut settings = SettingsView {
            device_name: "Mac".to_owned(),
            receive_directory: "/tmp".to_owned(),
            clipboard_sync_enabled: true,
            history_retention: HistoryRetention::SevenDays,
            launch_at_startup: false,
            keep_in_tray: true,
            notifications_enabled: false,
        };
        assert!(enforce_notifications_enabled(&mut settings));
        assert!(settings.notifications_enabled);
        assert!(!enforce_notifications_enabled(&mut settings));
    }

    #[test]
    fn unselected_file_targets_resolve_to_every_online_device() {
        let current = test_device(DeviceConnectionState::Online, true);
        let online = test_device(DeviceConnectionState::Online, false);
        let offline = test_device(DeviceConnectionState::Offline, false);

        let targets = select_transfer_targets(
            vec![current, online.clone(), offline],
            Some(&HashSet::new()),
        );

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, online.id);
    }

    #[test]
    fn selected_file_targets_preserve_the_explicit_device_set() {
        let online = test_device(DeviceConnectionState::Online, false);
        let offline = test_device(DeviceConnectionState::Offline, false);
        let requested = HashSet::from([online.id, offline.id]);

        let targets =
            select_transfer_targets(vec![online.clone(), offline.clone()], Some(&requested));

        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|device| device.id == online.id));
        assert!(targets.iter().any(|device| device.id == offline.id));
    }

    #[test]
    fn consecutive_clipboard_content_is_deduplicated() {
        assert!(is_consecutive_clipboard_duplicate(Some("same"), "same"));
        assert!(!is_consecutive_clipboard_duplicate(Some("older"), "newer"));
        assert!(!is_consecutive_clipboard_duplicate(None, "first"));
    }

    fn test_device(connection_state: DeviceConnectionState, is_current: bool) -> DeviceView {
        DeviceView {
            id: Uuid::new_v4(),
            name: "Test device".to_owned(),
            platform: synchalo_core::DevicePlatform::Linux,
            connection_state,
            is_current,
            address: None,
            last_seen_at: None,
            last_sync_at: None,
            paused: false,
        }
    }
}
