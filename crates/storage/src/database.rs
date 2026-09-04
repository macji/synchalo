use std::{
    collections::HashMap,
    fs::{self, File},
    path::Path,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use chrono::{DateTime, TimeZone, Utc};
use parking_lot::Mutex;
use rusqlite::{
    Connection, MAIN_DB, OptionalExtension, Transaction, params, params_from_iter, types::Value,
};
use synchalo_core::{
    AppError, ClipboardHistoryPage, ClipboardItemView, DeviceView, HistoryRetention, HlcTimestamp,
    SettingsView, TransferHistoryFilter, TransferHistoryPage, TransferView,
};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::{CryptoBox, DATA_KEY_BYTES, generate_data_key};

const MIN_SQLITE_VERSION: (u32, u32, u32) = (3, 51, 3);
const SCHEMA_VERSION: i64 = 4;
const DATA_KEY_PURPOSE: &str = "clipboard-content";
const DATA_KEY_ALGORITHM: &str = "xchacha20poly1305-v1";
const KEY_WRAP_ALGORITHM: &str = "xchacha20poly1305-wrap-v1";
const LEGACY_CLIPBOARD_CRYPTO_VERSION: i64 = 1;
const CLIPBOARD_CRYPTO_VERSION: i64 = 2;
const LOCAL_SECRET_CRYPTO_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub struct IdentityRecord {
    pub device_id: Uuid,
    pub space_id: Uuid,
    pub display_name: String,
    pub origin_sequence: u64,
    pub hlc: HlcTimestamp,
}

pub struct Database {
    connection: Mutex<Connection>,
    crypto: DataKeyring,
}

struct DataKeyring {
    active_key_id: String,
    keys: HashMap<String, CryptoBox>,
}

impl DataKeyring {
    fn active(&self) -> Result<(&str, &CryptoBox), AppError> {
        self.keys
            .get(&self.active_key_id)
            .map(|crypto| (self.active_key_id.as_str(), crypto))
            .ok_or_else(|| AppError::Storage("active data key is unavailable".to_owned()))
    }

    fn get(&self, key_id: &str) -> Result<&CryptoBox, AppError> {
        self.keys
            .get(key_id)
            .ok_or_else(|| AppError::Storage(format!("unknown data key {key_id}")))
    }
}

impl Database {
    pub fn open(
        path: impl AsRef<Path>,
        key_encryption_key: [u8; DATA_KEY_BYTES],
        legacy_data_key: Option<[u8; DATA_KEY_BYTES]>,
    ) -> Result<Self, AppError> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).map_err(storage_error)?;
        }
        let connection = Connection::open(path).map_err(storage_error)?;
        Self::initialize(connection, key_encryption_key, legacy_data_key)
    }

    pub fn open_in_memory(
        key_encryption_key: [u8; DATA_KEY_BYTES],
        legacy_data_key: Option<[u8; DATA_KEY_BYTES]>,
    ) -> Result<Self, AppError> {
        let connection = Connection::open_in_memory().map_err(storage_error)?;
        Self::initialize(connection, key_encryption_key, legacy_data_key)
    }

    fn initialize(
        mut connection: Connection,
        key_encryption_key: [u8; DATA_KEY_BYTES],
        legacy_data_key: Option<[u8; DATA_KEY_BYTES]>,
    ) -> Result<Self, AppError> {
        let key_encryption_key = Zeroizing::new(key_encryption_key);
        let legacy_data_key = legacy_data_key.map(Zeroizing::new);
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )
            .map_err(storage_error)?;
        assert_sqlite_version(&connection)?;
        let transaction = connection.transaction().map_err(storage_error)?;
        migrate(&transaction)?;
        let crypto = load_or_create_data_keyring(
            &transaction,
            &key_encryption_key,
            legacy_data_key.as_deref(),
        )?;
        transaction.commit().map_err(storage_error)?;
        Ok(Self {
            connection: Mutex::new(connection),
            crypto,
        })
    }

    pub fn sqlite_version(&self) -> Result<String, AppError> {
        self.connection
            .lock()
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(storage_error)
    }

    pub fn backup_if_missing(&self, path: impl AsRef<Path>) -> Result<(), AppError> {
        let path = path.as_ref();
        if path.exists() {
            return finalize_backup_file(path);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(storage_error)?;
        }
        let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
        self.connection
            .lock()
            .backup(MAIN_DB, &temporary, None)
            .map_err(storage_error)?;
        fs::rename(&temporary, path).map_err(storage_error)?;
        finalize_backup_file(path)?;
        if let Some(parent) = path.parent() {
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(storage_error)?;
        }
        Ok(())
    }

    pub fn save_local_secret(&self, secret_id: &str, plaintext: &[u8]) -> Result<(), AppError> {
        if secret_id.is_empty() || secret_id.len() > 128 {
            return Err(AppError::InvalidInput(
                "local secret identifier is invalid".to_owned(),
            ));
        }
        let (key_id, crypto) = self.crypto.active()?;
        let associated_data = local_secret_associated_data(secret_id, key_id);
        let (nonce, ciphertext) = crypto.encrypt(plaintext, &associated_data)?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO local_secrets (
                    secret_id, key_id, crypto_version, nonce, ciphertext, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(secret_id) DO UPDATE SET
                    key_id = excluded.key_id,
                    crypto_version = excluded.crypto_version,
                    nonce = excluded.nonce,
                    ciphertext = excluded.ciphertext,
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    secret_id,
                    key_id,
                    LOCAL_SECRET_CRYPTO_VERSION,
                    nonce,
                    ciphertext,
                    Utc::now().timestamp_millis(),
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn load_local_secret(&self, secret_id: &str) -> Result<Option<Vec<u8>>, AppError> {
        let row: Option<(String, i64, Vec<u8>, Vec<u8>)> = self
            .connection
            .lock()
            .query_row(
                "SELECT key_id, crypto_version, nonce, ciphertext
                 FROM local_secrets WHERE secret_id = ?1",
                [secret_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(storage_error)?;
        let Some((key_id, crypto_version, nonce, ciphertext)) = row else {
            return Ok(None);
        };
        if crypto_version != LOCAL_SECRET_CRYPTO_VERSION {
            return Err(AppError::Storage(format!(
                "unsupported local secret encryption version {crypto_version}"
            )));
        }
        let associated_data = local_secret_associated_data(secret_id, &key_id);
        self.crypto
            .get(&key_id)?
            .decrypt(&nonce, &ciphertext, &associated_data)
            .map(Some)
    }

    pub fn load_or_create_identity(&self, default_name: &str) -> Result<IdentityRecord, AppError> {
        let connection = self.connection.lock();
        if let Some(identity) = read_identity(&connection)? {
            return Ok(identity);
        }

        let identity = IdentityRecord {
            device_id: Uuid::new_v4(),
            space_id: Uuid::new_v4(),
            display_name: default_name.to_owned(),
            origin_sequence: 0,
            hlc: HlcTimestamp::default(),
        };
        connection
            .execute(
                "INSERT INTO identity (
                    singleton, device_id, space_id, display_name,
                    origin_sequence, hlc_physical_ms, hlc_logical
                 ) VALUES (1, ?1, ?2, ?3, 0, 0, 0)",
                params![
                    identity.device_id.to_string(),
                    identity.space_id.to_string(),
                    identity.display_name
                ],
            )
            .map_err(storage_error)?;
        Ok(identity)
    }

    pub fn has_identity(&self) -> Result<bool, AppError> {
        Ok(read_identity(&self.connection.lock())?.is_some())
    }

    pub fn next_event_clock(&self, now_ms: i64) -> Result<(u64, HlcTimestamp), AppError> {
        let mut connection = self.connection.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        let mut identity = read_identity_from_transaction(&transaction)?.ok_or_else(|| {
            AppError::Storage("local identity has not been initialized".to_owned())
        })?;

        identity.origin_sequence = identity.origin_sequence.saturating_add(1);
        if now_ms > identity.hlc.physical_ms {
            identity.hlc = HlcTimestamp {
                physical_ms: now_ms,
                logical: 0,
            };
        } else {
            identity.hlc.logical = identity.hlc.logical.saturating_add(1);
        }

        transaction
            .execute(
                "UPDATE identity
                 SET origin_sequence = ?1, hlc_physical_ms = ?2, hlc_logical = ?3
                 WHERE singleton = 1",
                params![
                    identity.origin_sequence as i64,
                    identity.hlc.physical_ms,
                    identity.hlc.logical
                ],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        Ok((identity.origin_sequence, identity.hlc))
    }

    pub fn update_identity_name(&self, name: &str) -> Result<(), AppError> {
        self.connection
            .lock()
            .execute(
                "UPDATE identity SET display_name = ?1 WHERE singleton = 1",
                [name],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn update_space_id(&self, space_id: Uuid) -> Result<(), AppError> {
        self.connection
            .lock()
            .execute(
                "UPDATE identity SET space_id = ?1 WHERE singleton = 1",
                [space_id.to_string()],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn merge_remote_clock(
        &self,
        remote: HlcTimestamp,
        now_ms: i64,
    ) -> Result<HlcTimestamp, AppError> {
        let mut connection = self.connection.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        let identity = read_identity_from_transaction(&transaction)?.ok_or_else(|| {
            AppError::Storage("local identity has not been initialized".to_owned())
        })?;
        let physical_ms = now_ms.max(identity.hlc.physical_ms).max(remote.physical_ms);
        let logical =
            if physical_ms == identity.hlc.physical_ms && physical_ms == remote.physical_ms {
                identity.hlc.logical.max(remote.logical).saturating_add(1)
            } else if physical_ms == identity.hlc.physical_ms {
                identity.hlc.logical.saturating_add(1)
            } else if physical_ms == remote.physical_ms {
                remote.logical.saturating_add(1)
            } else {
                0
            };
        let merged = HlcTimestamp {
            physical_ms,
            logical,
        };
        transaction
            .execute(
                "UPDATE identity SET hlc_physical_ms = ?1, hlc_logical = ?2
                 WHERE singleton = 1",
                params![merged.physical_ms, merged.logical],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)?;
        Ok(merged)
    }

    pub fn load_settings(&self, defaults: SettingsView) -> Result<SettingsView, AppError> {
        let connection = self.connection.lock();
        let json: Option<String> = connection
            .query_row(
                "SELECT value_json FROM app_state WHERE key = 'settings'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        match json {
            Some(json) => serde_json::from_str(&json).map_err(storage_error),
            None => {
                drop(connection);
                self.save_settings(&defaults)?;
                Ok(defaults)
            }
        }
    }

    pub fn save_settings(&self, settings: &SettingsView) -> Result<(), AppError> {
        let json = serde_json::to_string(settings).map_err(storage_error)?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO app_state (key, value_json) VALUES ('settings', ?1)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                [json],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn insert_clipboard_item(&self, item: &ClipboardItemView) -> Result<bool, AppError> {
        let event_id = item.id.to_string();
        let source_device_id = item.source_device_id.to_string();
        let direction = serde_json::to_string(&item.direction).map_err(storage_error)?;
        let created_at_ms = item.created_at.timestamp_millis();
        let (key_id, crypto) = self.crypto.active()?;
        let associated_data = clipboard_associated_data_v2(
            &event_id,
            &source_device_id,
            &item.source_device_name,
            &direction,
            created_at_ms,
            item.hlc.physical_ms,
            item.hlc.logical,
            &item.content_hash,
            key_id,
        );
        let (nonce, ciphertext) = crypto.encrypt(item.content.as_bytes(), &associated_data)?;
        let changed = self
            .connection
            .lock()
            .execute(
                "INSERT OR IGNORE INTO clipboard_items (
                    event_id, source_device_id, source_device_name, direction,
                    created_at_ms, hlc_physical_ms, hlc_logical, content_hash,
                    key_id, crypto_version, nonce, ciphertext, pinned
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    event_id,
                    source_device_id,
                    item.source_device_name,
                    direction,
                    created_at_ms,
                    item.hlc.physical_ms,
                    item.hlc.logical,
                    item.content_hash,
                    key_id,
                    CLIPBOARD_CRYPTO_VERSION,
                    nonce,
                    ciphertext,
                    i64::from(item.pinned),
                ],
            )
            .map_err(storage_error)?;
        Ok(changed > 0)
    }

    pub fn list_clipboard_items(
        &self,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ClipboardItemView>, AppError> {
        Ok(self.list_clipboard_page(query, false, 1, limit)?.items)
    }

    pub fn list_clipboard_page(
        &self,
        query: Option<&str>,
        favorites_only: bool,
        page: usize,
        page_size: usize,
    ) -> Result<ClipboardHistoryPage, AppError> {
        let page_size = page_size.clamp(1, 2_000);
        let normalized_query = query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let connection = self.connection.lock();
        let where_clause = if favorites_only {
            " WHERE pinned = 1"
        } else {
            ""
        };

        if normalized_query.is_none() {
            let count_sql = format!("SELECT COUNT(*) FROM clipboard_items{where_clause}");
            let total_items: usize = connection
                .query_row(&count_sql, [], |row| row.get::<_, i64>(0))
                .map_err(storage_error)?
                .max(0) as usize;
            let total_pages = total_items.div_ceil(page_size).max(1);
            let page = page.max(1).min(total_pages);
            let offset = (page - 1).saturating_mul(page_size);
            let select_sql = format!(
                "SELECT event_id, source_device_id, source_device_name, direction,
                        created_at_ms, hlc_physical_ms, hlc_logical, content_hash,
                        key_id, crypto_version, nonce, ciphertext, pinned
                 FROM clipboard_items{where_clause}
                 ORDER BY hlc_physical_ms DESC, hlc_logical DESC, event_id DESC
                 LIMIT ?1 OFFSET ?2"
            );
            let mut statement = connection.prepare(&select_sql).map_err(storage_error)?;
            let rows = statement
                .query_map(params![page_size as i64, offset as i64], encrypted_row)
                .map_err(storage_error)?;
            let mut items = Vec::with_capacity(page_size.min(total_items));
            for row in rows {
                items.push(self.decrypt_clipboard_row(row.map_err(storage_error)?)?);
            }
            return Ok(ClipboardHistoryPage {
                items,
                page,
                page_size,
                total_items,
                total_pages,
            });
        }

        let select_sql = format!(
            "SELECT event_id, source_device_id, source_device_name, direction,
                    created_at_ms, hlc_physical_ms, hlc_logical, content_hash,
                    key_id, crypto_version, nonce, ciphertext, pinned
             FROM clipboard_items{where_clause}
             ORDER BY hlc_physical_ms DESC, hlc_logical DESC, event_id DESC"
        );
        let mut statement = connection.prepare(&select_sql).map_err(storage_error)?;
        let rows = statement
            .query_map([], encrypted_row)
            .map_err(storage_error)?;
        let query = normalized_query.expect("checked above");
        let mut matching = Vec::new();
        for row in rows {
            let item = self.decrypt_clipboard_row(row.map_err(storage_error)?)?;
            if item.content.to_lowercase().contains(&query) {
                matching.push(item);
            }
        }
        let total_items = matching.len();
        let total_pages = total_items.div_ceil(page_size).max(1);
        let page = page.max(1).min(total_pages);
        let offset = (page - 1).saturating_mul(page_size);
        let items = matching.into_iter().skip(offset).take(page_size).collect();
        Ok(ClipboardHistoryPage {
            items,
            page,
            page_size,
            total_items,
            total_pages,
        })
    }

    pub fn get_clipboard_item(&self, id: Uuid) -> Result<Option<ClipboardItemView>, AppError> {
        let connection = self.connection.lock();
        let row = connection
            .query_row(
                "SELECT event_id, source_device_id, source_device_name, direction,
                        created_at_ms, hlc_physical_ms, hlc_logical, content_hash,
                        key_id, crypto_version, nonce, ciphertext, pinned
                 FROM clipboard_items WHERE event_id = ?1",
                [id.to_string()],
                encrypted_row,
            )
            .optional()
            .map_err(storage_error)?;
        row.map(|row| self.decrypt_clipboard_row(row)).transpose()
    }

    fn decrypt_clipboard_row(
        &self,
        row: EncryptedClipboardRow,
    ) -> Result<ClipboardItemView, AppError> {
        let event_id = parse_uuid(&row.event_id)?;
        let key_id = row
            .key_id
            .as_deref()
            .ok_or_else(|| AppError::Storage("clipboard row has no data key".to_owned()))?;
        let associated_data = match row.crypto_version {
            LEGACY_CLIPBOARD_CRYPTO_VERSION => event_id.as_bytes().to_vec(),
            CLIPBOARD_CRYPTO_VERSION => clipboard_associated_data_v2(
                &row.event_id,
                &row.source_device_id,
                &row.source_device_name,
                &row.direction,
                row.created_at_ms,
                row.hlc_physical_ms,
                row.hlc_logical,
                &row.content_hash,
                key_id,
            ),
            version => {
                return Err(AppError::Storage(format!(
                    "unsupported clipboard encryption version {version}"
                )));
            }
        };
        let content = String::from_utf8(self.crypto.get(key_id)?.decrypt(
            &row.nonce,
            &row.ciphertext,
            &associated_data,
        )?)
        .map_err(storage_error)?;
        Ok(ClipboardItemView {
            id: event_id,
            content,
            content_hash: row.content_hash,
            source_device_id: parse_uuid(&row.source_device_id)?,
            source_device_name: row.source_device_name,
            direction: serde_json::from_str(&row.direction).map_err(storage_error)?,
            created_at: timestamp(row.created_at_ms)?,
            hlc: HlcTimestamp {
                physical_ms: row.hlc_physical_ms,
                logical: row.hlc_logical,
            },
            pinned: row.pinned,
        })
    }

    pub fn delete_clipboard_item(&self, id: Uuid) -> Result<bool, AppError> {
        let changed = self
            .connection
            .lock()
            .execute(
                "DELETE FROM clipboard_items WHERE event_id = ?1",
                [id.to_string()],
            )
            .map_err(storage_error)?;
        Ok(changed > 0)
    }

    pub fn clear_clipboard_history(&self) -> Result<usize, AppError> {
        self.connection
            .lock()
            .execute("DELETE FROM clipboard_items WHERE pinned = 0", [])
            .map_err(storage_error)
    }

    pub fn set_clipboard_pinned(&self, id: Uuid, pinned: bool) -> Result<bool, AppError> {
        let changed = self
            .connection
            .lock()
            .execute(
                "UPDATE clipboard_items SET pinned = ?1 WHERE event_id = ?2",
                params![i64::from(pinned), id.to_string()],
            )
            .map_err(storage_error)?;
        Ok(changed > 0)
    }

    pub fn prune_clipboard_history(
        &self,
        retention: HistoryRetention,
        max_items: usize,
    ) -> Result<(), AppError> {
        let cutoff = match retention {
            HistoryRetention::None => Some(Utc::now().timestamp_millis()),
            HistoryRetention::OneDay => {
                Some((Utc::now() - chrono::Duration::days(1)).timestamp_millis())
            }
            HistoryRetention::SevenDays => {
                Some((Utc::now() - chrono::Duration::days(7)).timestamp_millis())
            }
            HistoryRetention::ThirtyDays => {
                Some((Utc::now() - chrono::Duration::days(30)).timestamp_millis())
            }
            HistoryRetention::Forever => None,
        };
        let connection = self.connection.lock();
        if let Some(cutoff) = cutoff {
            connection
                .execute(
                    "DELETE FROM clipboard_items WHERE pinned = 0 AND created_at_ms < ?1",
                    [cutoff],
                )
                .map_err(storage_error)?;
        }
        connection
            .execute(
                "DELETE FROM clipboard_items
                 WHERE pinned = 0 AND event_id NOT IN (
                    SELECT event_id FROM clipboard_items
                    ORDER BY hlc_physical_ms DESC, hlc_logical DESC
                    LIMIT ?1
                 )",
                [max_items as i64],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn upsert_device(&self, device: &DeviceView) -> Result<(), AppError> {
        let json = serde_json::to_string(device).map_err(storage_error)?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO devices (device_id, state_json, last_seen_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(device_id) DO UPDATE SET
                   state_json = excluded.state_json,
                   last_seen_at_ms = excluded.last_seen_at_ms",
                params![
                    device.id.to_string(),
                    json,
                    device.last_seen_at.map(|value| value.timestamp_millis())
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn upsert_trusted_device(
        &self,
        device: &DeviceView,
        credential_json: &str,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(device).map_err(storage_error)?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction().map_err(storage_error)?;
        transaction
            .execute(
                "INSERT INTO devices (device_id, state_json, last_seen_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(device_id) DO UPDATE SET
                   state_json = excluded.state_json,
                   last_seen_at_ms = excluded.last_seen_at_ms",
                params![
                    device.id.to_string(),
                    json,
                    device.last_seen_at.map(|value| value.timestamp_millis())
                ],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "INSERT INTO peer_credentials (device_id, credential_json)
                 VALUES (?1, ?2)
                 ON CONFLICT(device_id) DO UPDATE SET credential_json = excluded.credential_json",
                params![device.id.to_string(), credential_json],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }

    pub fn list_peer_credentials(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare("SELECT credential_json FROM peer_credentials ORDER BY device_id")
            .map_err(storage_error)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .map(|row| row.map_err(storage_error))
            .collect()
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceView>, AppError> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare("SELECT state_json FROM devices ORDER BY last_seen_at_ms DESC")
            .map_err(storage_error)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .map(|row| {
                let json = row.map_err(storage_error)?;
                serde_json::from_str(&json).map_err(storage_error)
            })
            .collect()
    }

    pub fn revoke_device(&self, id: Uuid) -> Result<bool, AppError> {
        let changed = self
            .connection
            .lock()
            .execute("DELETE FROM devices WHERE device_id = ?1", [id.to_string()])
            .map_err(storage_error)?;
        Ok(changed > 0)
    }

    pub fn upsert_transfer(&self, transfer: &TransferView) -> Result<(), AppError> {
        let json = serde_json::to_string(transfer).map_err(storage_error)?;
        self.connection
            .lock()
            .execute(
                "INSERT INTO transfers (event_id, state_json, created_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(event_id) DO UPDATE SET state_json = excluded.state_json",
                params![
                    transfer.id.to_string(),
                    json,
                    transfer.created_at.timestamp_millis()
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    pub fn list_transfers(&self, limit: usize) -> Result<Vec<TransferView>, AppError> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT state_json FROM transfers
                 ORDER BY created_at_ms DESC LIMIT ?1",
            )
            .map_err(storage_error)?;
        statement
            .query_map([limit.min(2_000) as i64], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .map(|row| {
                let json = row.map_err(storage_error)?;
                serde_json::from_str(&json).map_err(storage_error)
            })
            .collect()
    }

    pub fn list_active_transfers(&self) -> Result<Vec<TransferView>, AppError> {
        let connection = self.connection.lock();
        let mut statement = connection
            .prepare(
                "SELECT state_json FROM transfers
                 WHERE json_extract(state_json, '$.state') IN
                       ('queued', 'transferring', 'verifying')
                 ORDER BY created_at_ms DESC",
            )
            .map_err(storage_error)?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(storage_error)?
            .map(|row| {
                let json = row.map_err(storage_error)?;
                serde_json::from_str(&json).map_err(storage_error)
            })
            .collect()
    }

    pub fn get_transfer(&self, id: Uuid) -> Result<Option<TransferView>, AppError> {
        let json: Option<String> = self
            .connection
            .lock()
            .query_row(
                "SELECT state_json FROM transfers WHERE event_id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)?;
        json.map(|value| serde_json::from_str(&value).map_err(storage_error))
            .transpose()
    }

    pub fn list_transfer_page(
        &self,
        query: Option<&str>,
        favorites_only: bool,
        filter: TransferHistoryFilter,
        page: usize,
        page_size: usize,
    ) -> Result<TransferHistoryPage, AppError> {
        let page_size = page_size.clamp(1, 2_000);
        let normalized_query = query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_lowercase);
        let mut conditions = Vec::new();
        let mut values = Vec::<Value>::new();

        if let Some(query) = normalized_query {
            conditions.push(
                "LOWER(CAST(json_extract(state_json, '$.fileName') AS TEXT)) LIKE ? ESCAPE '\\'",
            );
            values.push(Value::Text(format!("%{}%", escape_like_pattern(&query))));
        }
        if favorites_only {
            conditions.push("COALESCE(json_extract(state_json, '$.pinned'), 0) = 1");
        }
        match filter {
            TransferHistoryFilter::All => {}
            TransferHistoryFilter::Sending => {
                conditions.push("json_extract(state_json, '$.direction') = 'sending'");
            }
            TransferHistoryFilter::Receiving => {
                conditions.push("json_extract(state_json, '$.direction') = 'receiving'");
            }
            TransferHistoryFilter::Active => conditions.push(
                "json_extract(state_json, '$.state') IN
                 ('queued', 'transferring', 'verifying')",
            ),
            TransferHistoryFilter::Failed => {
                conditions
                    .push("json_extract(state_json, '$.state') IN ('failed', 'waitingForDevice')");
            }
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };
        let connection = self.connection.lock();
        let count_sql = format!("SELECT COUNT(*) FROM transfers{where_clause}");
        let total_items = connection
            .query_row(&count_sql, params_from_iter(values.iter()), |row| {
                row.get::<_, i64>(0)
            })
            .map_err(storage_error)?
            .max(0) as usize;
        let total_pages = total_items.div_ceil(page_size).max(1);
        let page = page.max(1).min(total_pages);
        let offset = (page - 1).saturating_mul(page_size);
        let select_sql = format!(
            "SELECT state_json FROM transfers{where_clause}
             ORDER BY created_at_ms DESC, event_id DESC LIMIT ? OFFSET ?"
        );
        let mut select_values = values;
        select_values.push(Value::Integer(page_size as i64));
        select_values.push(Value::Integer(offset as i64));
        let mut statement = connection.prepare(&select_sql).map_err(storage_error)?;
        let items = statement
            .query_map(params_from_iter(select_values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(storage_error)?
            .map(|row| {
                let json = row.map_err(storage_error)?;
                serde_json::from_str(&json).map_err(storage_error)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TransferHistoryPage {
            items,
            page,
            page_size,
            total_items,
            total_pages,
        })
    }

    pub fn delete_transfer(&self, id: Uuid) -> Result<bool, AppError> {
        let changed = self
            .connection
            .lock()
            .execute(
                "DELETE FROM transfers WHERE event_id = ?1",
                [id.to_string()],
            )
            .map_err(storage_error)?;
        Ok(changed > 0)
    }

    pub fn clear_transfer_history(&self) -> Result<usize, AppError> {
        self.connection
            .lock()
            .execute(
                "DELETE FROM transfers
                 WHERE COALESCE(json_extract(state_json, '$.pinned'), 0) = 0
                   AND json_extract(state_json, '$.state') IN
                       ('completed', 'failed', 'waitingForDevice', 'cancelled')",
                [],
            )
            .map_err(storage_error)
    }
}

fn finalize_backup_file(path: &Path) -> Result<(), AppError> {
    let connection = Connection::open(path).map_err(storage_error)?;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
        .map_err(storage_error)?;
    if !journal_mode.eq_ignore_ascii_case("delete") {
        return Err(AppError::Storage(format!(
            "unexpected migration backup journal mode {journal_mode}"
        )));
    }
    let integrity: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(storage_error)?;
    if integrity != "ok" {
        return Err(AppError::Storage(format!(
            "migration backup integrity check failed: {integrity}"
        )));
    }
    drop(connection);
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(storage_error)?;
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(storage_error)
}

struct EncryptedClipboardRow {
    event_id: String,
    source_device_id: String,
    source_device_name: String,
    direction: String,
    created_at_ms: i64,
    hlc_physical_ms: i64,
    hlc_logical: u32,
    content_hash: String,
    key_id: Option<String>,
    crypto_version: i64,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    pinned: bool,
}

fn encrypted_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EncryptedClipboardRow> {
    Ok(EncryptedClipboardRow {
        event_id: row.get(0)?,
        source_device_id: row.get(1)?,
        source_device_name: row.get(2)?,
        direction: row.get(3)?,
        created_at_ms: row.get(4)?,
        hlc_physical_ms: row.get(5)?,
        hlc_logical: row.get(6)?,
        content_hash: row.get(7)?,
        key_id: row.get(8)?,
        crypto_version: row.get(9)?,
        nonce: row.get(10)?,
        ciphertext: row.get(11)?,
        pinned: row.get::<_, i64>(12)? != 0,
    })
}

struct WrappedDataKeyRow {
    key_id: String,
    purpose: String,
    algorithm: String,
    wrap_algorithm: String,
    wrap_nonce: Vec<u8>,
    wrapped_key: Vec<u8>,
    status: String,
}

fn load_or_create_data_keyring(
    transaction: &Transaction<'_>,
    key_encryption_key: &[u8; DATA_KEY_BYTES],
    legacy_data_key: Option<&[u8; DATA_KEY_BYTES]>,
) -> Result<DataKeyring, AppError> {
    let database_id: Option<String> = transaction
        .query_row(
            "SELECT database_id FROM crypto_metadata WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage_error)?;
    let database_id = match database_id {
        Some(database_id) => database_id,
        None => {
            let database_id = Uuid::new_v4().to_string();
            transaction
                .execute(
                    "INSERT INTO crypto_metadata (singleton, database_id) VALUES (1, ?1)",
                    [&database_id],
                )
                .map_err(storage_error)?;
            database_id
        }
    };

    let wrapped_keys = {
        let mut statement = transaction
            .prepare(
                "SELECT key_id, purpose, algorithm, wrap_algorithm,
                        wrap_nonce, wrapped_key, status
                 FROM wrapped_data_keys ORDER BY created_at_ms, key_id",
            )
            .map_err(storage_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok(WrappedDataKeyRow {
                    key_id: row.get(0)?,
                    purpose: row.get(1)?,
                    algorithm: row.get(2)?,
                    wrap_algorithm: row.get(3)?,
                    wrap_nonce: row.get(4)?,
                    wrapped_key: row.get(5)?,
                    status: row.get(6)?,
                })
            })
            .map_err(storage_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(storage_error)?
    };

    if wrapped_keys.is_empty() {
        let unbound_rows: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_id IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        if unbound_rows > 0 && legacy_data_key.is_none() {
            return Err(AppError::Storage(
                "legacy clipboard data exists but its data key is unavailable".to_owned(),
            ));
        }

        let data_key = Zeroizing::new(match legacy_data_key {
            Some(key) => *key,
            None => generate_data_key()?,
        });
        if unbound_rows > 0 {
            validate_legacy_clipboard_rows(transaction, &data_key)?;
        }
        let key_id = Uuid::new_v4().to_string();
        let key_wrap = CryptoBox::new(key_encryption_key);
        let associated_data = wrapped_key_associated_data(&database_id, &key_id);
        let (wrap_nonce, wrapped_key) = key_wrap.encrypt(&data_key[..], &associated_data)?;
        transaction
            .execute(
                "INSERT INTO wrapped_data_keys (
                    key_id, purpose, algorithm, wrap_algorithm,
                    wrap_nonce, wrapped_key, created_at_ms, status
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active')",
                params![
                    key_id,
                    DATA_KEY_PURPOSE,
                    DATA_KEY_ALGORITHM,
                    KEY_WRAP_ALGORITHM,
                    wrap_nonce,
                    wrapped_key,
                    Utc::now().timestamp_millis(),
                ],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "UPDATE clipboard_items SET key_id = ?1 WHERE key_id IS NULL",
                [&key_id],
            )
            .map_err(storage_error)?;
        let mut keys = HashMap::new();
        keys.insert(key_id.clone(), CryptoBox::new(&data_key));
        return Ok(DataKeyring {
            active_key_id: key_id,
            keys,
        });
    }

    let key_wrap = CryptoBox::new(key_encryption_key);
    let mut active_key_id = None;
    let mut keys = HashMap::with_capacity(wrapped_keys.len());
    for row in wrapped_keys {
        if row.purpose != DATA_KEY_PURPOSE
            || row.algorithm != DATA_KEY_ALGORITHM
            || row.wrap_algorithm != KEY_WRAP_ALGORITHM
        {
            return Err(AppError::Storage(format!(
                "unsupported wrapped data key metadata for {}",
                row.key_id
            )));
        }
        if row.status == "active" {
            if active_key_id.replace(row.key_id.clone()).is_some() {
                return Err(AppError::Storage(
                    "multiple active clipboard data keys".to_owned(),
                ));
            }
        } else if row.status != "retired" {
            return Err(AppError::Storage(format!(
                "invalid wrapped data key status {}",
                row.status
            )));
        }
        let associated_data = wrapped_key_associated_data(&database_id, &row.key_id);
        let plaintext = Zeroizing::new(key_wrap.decrypt(
            &row.wrap_nonce,
            &row.wrapped_key,
            &associated_data,
        )?);
        let data_key: &[u8; DATA_KEY_BYTES] = plaintext
            .as_slice()
            .try_into()
            .map_err(|_| AppError::Crypto)?;
        if keys
            .insert(row.key_id.clone(), CryptoBox::new(data_key))
            .is_some()
        {
            return Err(AppError::Storage(format!(
                "duplicate wrapped data key {}",
                row.key_id
            )));
        }
    }
    let active_key_id = active_key_id
        .ok_or_else(|| AppError::Storage("active clipboard data key is missing".to_owned()))?;
    let invalid_references: i64 = transaction
        .query_row(
            "SELECT COUNT(*)
             FROM clipboard_items AS clipboard
             LEFT JOIN wrapped_data_keys AS keys ON keys.key_id = clipboard.key_id
             WHERE clipboard.key_id IS NULL OR keys.key_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(storage_error)?;
    if invalid_references > 0 {
        return Err(AppError::Storage(
            "clipboard rows reference an unavailable data key".to_owned(),
        ));
    }
    Ok(DataKeyring {
        active_key_id,
        keys,
    })
}

fn validate_legacy_clipboard_rows(
    transaction: &Transaction<'_>,
    legacy_data_key: &[u8; DATA_KEY_BYTES],
) -> Result<(), AppError> {
    let crypto = CryptoBox::new(legacy_data_key);
    let mut statement = transaction
        .prepare(
            "SELECT event_id, nonce, ciphertext
             FROM clipboard_items WHERE key_id IS NULL",
        )
        .map_err(storage_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(storage_error)?;
    for row in rows {
        let (event_id, nonce, ciphertext) = row.map_err(storage_error)?;
        let event_id = parse_uuid(&event_id)?;
        crypto.decrypt(&nonce, &ciphertext, event_id.as_bytes())?;
    }
    Ok(())
}

fn wrapped_key_associated_data(database_id: &str, key_id: &str) -> Vec<u8> {
    let mut associated_data = b"synchalo:wrapped-data-key".to_vec();
    append_aad_field(&mut associated_data, database_id.as_bytes());
    append_aad_field(&mut associated_data, key_id.as_bytes());
    append_aad_field(&mut associated_data, DATA_KEY_PURPOSE.as_bytes());
    append_aad_field(&mut associated_data, DATA_KEY_ALGORITHM.as_bytes());
    append_aad_field(&mut associated_data, KEY_WRAP_ALGORITHM.as_bytes());
    associated_data
}

fn local_secret_associated_data(secret_id: &str, key_id: &str) -> Vec<u8> {
    let mut associated_data = b"synchalo:local-secret".to_vec();
    associated_data.extend_from_slice(&LOCAL_SECRET_CRYPTO_VERSION.to_le_bytes());
    append_aad_field(&mut associated_data, secret_id.as_bytes());
    append_aad_field(&mut associated_data, key_id.as_bytes());
    associated_data
}

#[allow(clippy::too_many_arguments)]
fn clipboard_associated_data_v2(
    event_id: &str,
    source_device_id: &str,
    source_device_name: &str,
    direction: &str,
    created_at_ms: i64,
    hlc_physical_ms: i64,
    hlc_logical: u32,
    content_hash: &str,
    key_id: &str,
) -> Vec<u8> {
    let mut associated_data = b"synchalo:clipboard-content".to_vec();
    associated_data.extend_from_slice(&CLIPBOARD_CRYPTO_VERSION.to_le_bytes());
    append_aad_field(&mut associated_data, event_id.as_bytes());
    append_aad_field(&mut associated_data, source_device_id.as_bytes());
    append_aad_field(&mut associated_data, source_device_name.as_bytes());
    append_aad_field(&mut associated_data, direction.as_bytes());
    associated_data.extend_from_slice(&created_at_ms.to_le_bytes());
    associated_data.extend_from_slice(&hlc_physical_ms.to_le_bytes());
    associated_data.extend_from_slice(&hlc_logical.to_le_bytes());
    append_aad_field(&mut associated_data, content_hash.as_bytes());
    append_aad_field(&mut associated_data, key_id.as_bytes());
    associated_data
}

fn append_aad_field(associated_data: &mut Vec<u8>, value: &[u8]) {
    associated_data.extend_from_slice(&(value.len() as u64).to_le_bytes());
    associated_data.extend_from_slice(value);
}

fn migrate(transaction: &Transaction<'_>) -> Result<(), AppError> {
    let current_version: i64 = transaction
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(storage_error)?;
    if current_version > SCHEMA_VERSION {
        return Err(AppError::Storage(format!(
            "database schema {current_version} is newer than supported schema {SCHEMA_VERSION}"
        )));
    }

    transaction
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS identity (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                device_id TEXT NOT NULL UNIQUE,
                space_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                origin_sequence INTEGER NOT NULL DEFAULT 0,
                hlc_physical_ms INTEGER NOT NULL DEFAULT 0,
                hlc_logical INTEGER NOT NULL DEFAULT 0
             );

             CREATE TABLE IF NOT EXISTS app_state (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                state_json TEXT NOT NULL,
                last_seen_at_ms INTEGER
             );

             CREATE TABLE IF NOT EXISTS peer_credentials (
                device_id TEXT PRIMARY KEY REFERENCES devices(device_id) ON DELETE CASCADE,
                credential_json TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS crypto_metadata (
                singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                database_id TEXT NOT NULL UNIQUE
             );

             CREATE TABLE IF NOT EXISTS wrapped_data_keys (
                key_id TEXT PRIMARY KEY,
                purpose TEXT NOT NULL,
                algorithm TEXT NOT NULL,
                wrap_algorithm TEXT NOT NULL,
                wrap_nonce BLOB NOT NULL CHECK (length(wrap_nonce) = 24),
                wrapped_key BLOB NOT NULL CHECK (length(wrapped_key) = 48),
                created_at_ms INTEGER NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('active', 'retired'))
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_wrapped_data_keys_active
                ON wrapped_data_keys(status) WHERE status = 'active';

             CREATE TABLE IF NOT EXISTS local_secrets (
                secret_id TEXT PRIMARY KEY,
                key_id TEXT NOT NULL,
                crypto_version INTEGER NOT NULL,
                nonce BLOB NOT NULL CHECK (length(nonce) = 24),
                ciphertext BLOB NOT NULL,
                updated_at_ms INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS clipboard_items (
                event_id TEXT PRIMARY KEY,
                source_device_id TEXT NOT NULL,
                source_device_name TEXT NOT NULL,
                direction TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                hlc_physical_ms INTEGER NOT NULL,
                hlc_logical INTEGER NOT NULL,
                content_hash TEXT NOT NULL,
                key_id TEXT,
                crypto_version INTEGER NOT NULL DEFAULT 1,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS idx_clipboard_order
                ON clipboard_items(hlc_physical_ms DESC, hlc_logical DESC, event_id DESC);

             CREATE TABLE IF NOT EXISTS transfers (
                event_id TEXT PRIMARY KEY,
                state_json TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_transfers_created
                ON transfers(created_at_ms DESC);",
        )
        .map_err(storage_error)?;

    if !table_has_column(transaction, "clipboard_items", "key_id")? {
        transaction
            .execute("ALTER TABLE clipboard_items ADD COLUMN key_id TEXT", [])
            .map_err(storage_error)?;
    }
    if !table_has_column(transaction, "clipboard_items", "crypto_version")? {
        transaction
            .execute(
                "ALTER TABLE clipboard_items
                 ADD COLUMN crypto_version INTEGER NOT NULL DEFAULT 1",
                [],
            )
            .map_err(storage_error)?;
    }
    transaction
        .execute_batch("PRAGMA user_version = 4;")
        .map_err(storage_error)
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, AppError> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(storage_error)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(storage_error)?;
    for column in columns {
        if column.map_err(storage_error)? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn read_identity(connection: &Connection) -> Result<Option<IdentityRecord>, AppError> {
    connection
        .query_row(
            "SELECT device_id, space_id, display_name, origin_sequence,
                    hlc_physical_ms, hlc_logical
             FROM identity WHERE singleton = 1",
            [],
            identity_from_row,
        )
        .optional()
        .map_err(storage_error)?
        .map(parse_identity)
        .transpose()
}

fn read_identity_from_transaction(
    transaction: &Transaction<'_>,
) -> Result<Option<IdentityRecord>, AppError> {
    transaction
        .query_row(
            "SELECT device_id, space_id, display_name, origin_sequence,
                    hlc_physical_ms, hlc_logical
             FROM identity WHERE singleton = 1",
            [],
            identity_from_row,
        )
        .optional()
        .map_err(storage_error)?
        .map(parse_identity)
        .transpose()
}

fn identity_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(String, String, String, i64, i64, u32)> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
    ))
}

fn parse_identity(
    row: (String, String, String, i64, i64, u32),
) -> Result<IdentityRecord, AppError> {
    Ok(IdentityRecord {
        device_id: parse_uuid(&row.0)?,
        space_id: parse_uuid(&row.1)?,
        display_name: row.2,
        origin_sequence: row.3.max(0) as u64,
        hlc: HlcTimestamp {
            physical_ms: row.4,
            logical: row.5,
        },
    })
}

fn assert_sqlite_version(connection: &Connection) -> Result<(), AppError> {
    let version: String = connection
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .map_err(storage_error)?;
    let mut components = version
        .split('.')
        .take(3)
        .map(|part| part.parse::<u32>().unwrap_or_default());
    let parsed = (
        components.next().unwrap_or_default(),
        components.next().unwrap_or_default(),
        components.next().unwrap_or_default(),
    );
    if parsed < MIN_SQLITE_VERSION {
        return Err(AppError::Storage(format!(
            "SQLite {version} is below the required 3.51.3"
        )));
    }
    Ok(())
}

fn timestamp(value: i64) -> Result<DateTime<Utc>, AppError> {
    Utc.timestamp_millis_opt(value)
        .single()
        .ok_or_else(|| AppError::Storage(format!("invalid timestamp {value}")))
}

fn parse_uuid(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(storage_error)
}

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(error.to_string())
}

fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use synchalo_core::{
        ClipboardDirection, ClipboardItemView, TransferDirection, TransferState, content_hash,
    };
    use tempfile::tempdir;

    use super::*;

    fn database() -> Database {
        Database::open_in_memory([3_u8; DATA_KEY_BYTES], None).unwrap()
    }

    #[test]
    fn legacy_transfer_json_defaults_to_not_favorited() {
        let mut transfer: TransferView = serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "fileName": "legacy.zip",
            "fileSize": 42,
            "direction": "sending",
            "state": "completed",
            "progress": 1.0,
            "createdAt": Utc::now(),
            "sourceDeviceName": "Mac",
            "targets": [],
            "bytesPerSecond": null,
            "etaSeconds": null,
            "displayPath": "/tmp/legacy.zip",
            "error": null,
            "contentHash": null,
            "sourceModifiedUnixMs": null
        }))
        .unwrap();
        assert!(!transfer.pinned);
        assert!(transfer.source_device_id.is_none());

        transfer.pinned = true;
        let database = database();
        database.upsert_transfer(&transfer).unwrap();
        assert!(database.list_transfers(10).unwrap()[0].pinned);
    }

    fn sample_clipboard_item(content: &str) -> ClipboardItemView {
        ClipboardItemView {
            id: Uuid::new_v4(),
            content: content.to_owned(),
            content_hash: content_hash(content.as_bytes()),
            source_device_id: Uuid::new_v4(),
            source_device_name: "Test Mac".to_owned(),
            direction: ClipboardDirection::Local,
            created_at: Utc.timestamp_millis_opt(1_700_000_000_000).unwrap(),
            hlc: HlcTimestamp {
                physical_ms: 1_700_000_000_000,
                logical: 4,
            },
            pinned: false,
        }
    }

    fn sample_transfer(index: u32) -> TransferView {
        TransferView {
            id: Uuid::new_v4(),
            file_name: format!("history-file-{index}.zip"),
            file_size: u64::from(index) + 1,
            direction: if index.is_multiple_of(2) {
                TransferDirection::Sending
            } else {
                TransferDirection::Receiving
            },
            state: if index.is_multiple_of(3) {
                TransferState::Failed
            } else {
                TransferState::Completed
            },
            progress: 1.0,
            created_at: Utc
                .timestamp_millis_opt(1_700_000_000_000 + i64::from(index))
                .unwrap(),
            source_device_id: Some(Uuid::new_v4()),
            source_device_name: Some("Test Mac".to_owned()),
            targets: Vec::new(),
            bytes_per_second: None,
            eta_seconds: None,
            display_path: Some(format!("/tmp/history-file-{index}.zip")),
            error: None,
            content_hash: None,
            source_modified_unix_ms: None,
            pinned: index.is_multiple_of(50),
        }
    }

    fn create_legacy_database(
        path: &Path,
        legacy_data_key: &[u8; DATA_KEY_BYTES],
        item: &ClipboardItemView,
    ) {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE clipboard_items (
                    event_id TEXT PRIMARY KEY,
                    source_device_id TEXT NOT NULL,
                    source_device_name TEXT NOT NULL,
                    direction TEXT NOT NULL,
                    created_at_ms INTEGER NOT NULL,
                    hlc_physical_ms INTEGER NOT NULL,
                    hlc_logical INTEGER NOT NULL,
                    content_hash TEXT NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL,
                    pinned INTEGER NOT NULL DEFAULT 0
                 );
                 PRAGMA user_version = 2;",
            )
            .unwrap();
        let crypto = CryptoBox::new(legacy_data_key);
        let (nonce, ciphertext) = crypto
            .encrypt(item.content.as_bytes(), item.id.as_bytes())
            .unwrap();
        connection
            .execute(
                "INSERT INTO clipboard_items (
                    event_id, source_device_id, source_device_name, direction,
                    created_at_ms, hlc_physical_ms, hlc_logical, content_hash,
                    nonce, ciphertext, pinned
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    item.id.to_string(),
                    item.source_device_id.to_string(),
                    item.source_device_name,
                    serde_json::to_string(&item.direction).unwrap(),
                    item.created_at.timestamp_millis(),
                    item.hlc.physical_ms,
                    item.hlc.logical,
                    item.content_hash,
                    nonce,
                    ciphertext,
                    i64::from(item.pinned),
                ],
            )
            .unwrap();
    }

    #[test]
    fn identity_is_stable_and_clock_is_monotonic() {
        let db = database();
        let first = db.load_or_create_identity("Mac").unwrap();
        let second = db.load_or_create_identity("Other").unwrap();
        assert_eq!(first.device_id, second.device_id);

        let (sequence_a, clock_a) = db.next_event_clock(100).unwrap();
        let (sequence_b, clock_b) = db.next_event_clock(90).unwrap();
        assert_eq!(sequence_b, sequence_a + 1);
        assert!(clock_b > clock_a);
    }

    #[test]
    fn local_secret_is_encrypted_and_authenticated() {
        let database = database();
        database
            .save_local_secret("transport-credentials-v1", b"private identity")
            .unwrap();
        assert_eq!(
            database
                .load_local_secret("transport-credentials-v1")
                .unwrap()
                .unwrap(),
            b"private identity"
        );
        {
            let connection = database.connection.lock();
            let ciphertext: Vec<u8> = connection
                .query_row(
                    "SELECT ciphertext FROM local_secrets WHERE secret_id = ?1",
                    ["transport-credentials-v1"],
                    |row| row.get(0),
                )
                .unwrap();
            assert_ne!(ciphertext, b"private identity");
            connection
                .execute(
                    "UPDATE local_secrets SET ciphertext = ?1 WHERE secret_id = ?2",
                    params![vec![0_u8; ciphertext.len()], "transport-credentials-v1"],
                )
                .unwrap();
        }
        assert!(
            database
                .load_local_secret("transport-credentials-v1")
                .is_err()
        );
    }

    #[test]
    fn migration_backup_contains_encrypted_local_secrets() {
        let directory = tempdir().unwrap();
        let backup_path = directory.path().join("migration-backup.db");
        let key = [49_u8; DATA_KEY_BYTES];
        let database = Database::open_in_memory(key, None).unwrap();
        database
            .save_local_secret("transport-credentials-v1", b"identity backup")
            .unwrap();
        database.backup_if_missing(&backup_path).unwrap();

        let read_only =
            Connection::open_with_flags(&backup_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        assert_eq!(
            read_only
                .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
                .unwrap()
                .to_lowercase(),
            "delete"
        );
        drop(read_only);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&backup_path).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let restored = Database::open(&backup_path, key, None).unwrap();
        assert_eq!(
            restored
                .load_local_secret("transport-credentials-v1")
                .unwrap()
                .unwrap(),
            b"identity backup"
        );
    }

    #[test]
    fn clipboard_content_is_encrypted_and_searchable_after_decryption() {
        let db = database();
        let identity = db.load_or_create_identity("Mac").unwrap();
        let item = ClipboardItemView {
            id: Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)),
            content: "cargo test --workspace".to_owned(),
            content_hash: content_hash(b"cargo test --workspace"),
            source_device_id: identity.device_id,
            source_device_name: identity.display_name,
            direction: ClipboardDirection::Local,
            created_at: Utc::now(),
            hlc: HlcTimestamp {
                physical_ms: Utc::now().timestamp_millis(),
                logical: 0,
            },
            pinned: false,
        };
        db.insert_clipboard_item(&item).unwrap();

        let results = db.list_clipboard_items(Some("WORKSPACE"), 500).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, item.content);
        assert!(db.delete_clipboard_item(item.id).unwrap());
        assert!(db.list_clipboard_items(None, 500).unwrap().is_empty());
    }

    #[test]
    fn database_requires_the_key_encryption_key_to_unwrap_its_data_key() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("synchalo.db");
        let key_encryption_key = [11_u8; DATA_KEY_BYTES];
        let item = sample_clipboard_item("wrapped secret");

        let database = Database::open(&path, key_encryption_key, None).unwrap();
        database.insert_clipboard_item(&item).unwrap();
        drop(database);

        assert!(Database::open(&path, [12_u8; DATA_KEY_BYTES], None).is_err());
        let reopened = Database::open(&path, key_encryption_key, None).unwrap();
        assert_eq!(
            reopened
                .get_clipboard_item(item.id)
                .unwrap()
                .unwrap()
                .content,
            item.content
        );
    }

    #[test]
    fn wrapped_data_key_tampering_is_detected() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("synchalo.db");
        let key_encryption_key = [16_u8; DATA_KEY_BYTES];
        drop(Database::open(&path, key_encryption_key, None).unwrap());

        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE wrapped_data_keys SET wrapped_key = ?1 WHERE status = 'active'",
                [vec![0_u8; DATA_KEY_BYTES + 16]],
            )
            .unwrap();
        drop(connection);

        assert!(Database::open(&path, key_encryption_key, None).is_err());
    }

    #[test]
    fn legacy_data_key_migrates_into_sqlite_as_wrapped_ciphertext() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        let legacy_data_key = [21_u8; DATA_KEY_BYTES];
        let key_encryption_key = [31_u8; DATA_KEY_BYTES];
        let item = sample_clipboard_item("legacy clipboard content");
        create_legacy_database(&path, &legacy_data_key, &item);

        let database = Database::open(&path, key_encryption_key, Some(legacy_data_key)).unwrap();
        assert_eq!(
            database
                .get_clipboard_item(item.id)
                .unwrap()
                .unwrap()
                .content,
            item.content
        );
        drop(database);

        let connection = Connection::open(&path).unwrap();
        let (wrapped_key, key_id, crypto_version): (Vec<u8>, Option<String>, i64) = connection
            .query_row(
                "SELECT keys.wrapped_key, clipboard.key_id, clipboard.crypto_version
                 FROM clipboard_items AS clipboard
                 JOIN wrapped_data_keys AS keys ON keys.key_id = clipboard.key_id
                 WHERE clipboard.event_id = ?1",
                [item.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(wrapped_key.len(), DATA_KEY_BYTES + 16);
        assert_ne!(wrapped_key.as_slice(), &legacy_data_key[..]);
        assert!(key_id.is_some());
        assert_eq!(crypto_version, LEGACY_CLIPBOARD_CRYPTO_VERSION);
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
        drop(connection);

        let reopened = Database::open(&path, key_encryption_key, None).unwrap();
        assert_eq!(
            reopened
                .get_clipboard_item(item.id)
                .unwrap()
                .unwrap()
                .content,
            item.content
        );
    }

    #[test]
    fn failed_legacy_migration_rolls_back_without_touching_the_old_schema() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        let item = sample_clipboard_item("must survive migration failure");
        create_legacy_database(&path, &[41_u8; DATA_KEY_BYTES], &item);

        assert!(Database::open(&path, [51_u8; DATA_KEY_BYTES], None).is_err());

        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'table' AND name = 'wrapped_data_keys'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn incorrect_legacy_key_is_authenticated_before_migration_commits() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        let item = sample_clipboard_item("keep this decryptable");
        create_legacy_database(&path, &[61_u8; DATA_KEY_BYTES], &item);

        assert!(
            Database::open(
                &path,
                [71_u8; DATA_KEY_BYTES],
                Some([62_u8; DATA_KEY_BYTES])
            )
            .is_err()
        );

        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'table' AND name = 'wrapped_data_keys'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn clipboard_metadata_is_authenticated() {
        let database = database();
        let item = sample_clipboard_item("authenticated metadata");
        database.insert_clipboard_item(&item).unwrap();
        {
            let connection = database.connection.lock();
            connection
                .execute(
                    "UPDATE clipboard_items SET source_device_id = ?1 WHERE event_id = ?2",
                    [Uuid::new_v4().to_string(), item.id.to_string()],
                )
                .unwrap();
        }

        assert!(matches!(
            database.get_clipboard_item(item.id),
            Err(AppError::Crypto)
        ));
    }

    #[test]
    fn clipboard_history_uses_fixed_pages_and_favorite_filtering() {
        let db = database();
        let identity = db.load_or_create_identity("Mac").unwrap();
        let mut last_id = None;
        for index in 0..205_u32 {
            let content = format!("pagination item {index}");
            let id = Uuid::new_v4();
            last_id = Some(id);
            let item = ClipboardItemView {
                id,
                content_hash: content_hash(content.as_bytes()),
                content,
                source_device_id: identity.device_id,
                source_device_name: identity.display_name.clone(),
                direction: ClipboardDirection::Local,
                created_at: Utc
                    .timestamp_millis_opt(1_700_000_000_000 + index as i64)
                    .unwrap(),
                hlc: HlcTimestamp {
                    physical_ms: 1_700_000_000_000 + index as i64,
                    logical: 0,
                },
                pinned: index % 50 == 0,
            };
            assert!(db.insert_clipboard_item(&item).unwrap());
        }

        let first = db.list_clipboard_page(None, false, 1, 100).unwrap();
        assert_eq!(first.items.len(), 100);
        assert_eq!(first.total_items, 205);
        assert_eq!(first.total_pages, 3);

        let last = db.list_clipboard_page(None, false, 99, 100).unwrap();
        assert_eq!(last.page, 3);
        assert_eq!(last.items.len(), 5);

        let favorites = db.list_clipboard_page(None, true, 1, 100).unwrap();
        assert_eq!(favorites.total_items, 5);
        assert!(favorites.items.iter().all(|item| item.pinned));

        let search = db
            .list_clipboard_page(Some("ITEM 204"), false, 1, 100)
            .unwrap();
        assert_eq!(search.total_items, 1);
        assert_eq!(search.items[0].content, "pagination item 204");
        assert_eq!(
            db.get_clipboard_item(last_id.unwrap())
                .unwrap()
                .unwrap()
                .content,
            "pagination item 204"
        );
    }

    #[test]
    fn file_history_uses_fixed_pages_and_server_side_filters() {
        let database = database();
        for index in 0..205_u32 {
            database.upsert_transfer(&sample_transfer(index)).unwrap();
        }

        let first = database
            .list_transfer_page(None, false, TransferHistoryFilter::All, 1, 100)
            .unwrap();
        assert_eq!(first.items.len(), 100);
        assert_eq!(first.total_items, 205);
        assert_eq!(first.total_pages, 3);

        let last = database
            .list_transfer_page(None, false, TransferHistoryFilter::All, 99, 100)
            .unwrap();
        assert_eq!(last.page, 3);
        assert_eq!(last.items.len(), 5);

        let favorites = database
            .list_transfer_page(None, true, TransferHistoryFilter::All, 1, 100)
            .unwrap();
        assert_eq!(favorites.total_items, 5);
        assert!(favorites.items.iter().all(|transfer| transfer.pinned));

        let receiving = database
            .list_transfer_page(None, false, TransferHistoryFilter::Receiving, 1, 100)
            .unwrap();
        assert_eq!(receiving.total_items, 102);
        assert!(
            receiving
                .items
                .iter()
                .all(|transfer| transfer.direction == TransferDirection::Receiving)
        );

        let search = database
            .list_transfer_page(
                Some("HISTORY-FILE-204"),
                false,
                TransferHistoryFilter::All,
                1,
                100,
            )
            .unwrap();
        assert_eq!(search.total_items, 1);
        assert_eq!(search.items[0].file_name, "history-file-204.zip");
        assert!(database.get_transfer(search.items[0].id).unwrap().is_some());
    }

    #[test]
    fn clearing_file_history_preserves_favorites_and_active_transfers() {
        let database = database();
        let removable = sample_transfer(1);
        let mut favorite = sample_transfer(2);
        favorite.pinned = true;
        let mut active = sample_transfer(3);
        active.state = TransferState::Transferring;
        let favorite_id = favorite.id;
        let active_id = active.id;

        database.upsert_transfer(&removable).unwrap();
        database.upsert_transfer(&favorite).unwrap();
        database.upsert_transfer(&active).unwrap();

        assert_eq!(database.clear_transfer_history().unwrap(), 1);
        assert!(database.get_transfer(removable.id).unwrap().is_none());
        assert!(database.get_transfer(favorite_id).unwrap().is_some());
        assert!(database.get_transfer(active_id).unwrap().is_some());
    }
}
