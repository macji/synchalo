use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use synchalo_core::AppError;
use zeroize::Zeroizing;

use crate::DATA_KEY_BYTES;

const KEY_FILE_NAME: &str = "synchalo.key";
const PENDING_KEY_FILE_NAME: &str = ".synchalo.key.pending";
const KEY_FILE_MAGIC: &[u8] = b"SYNCHALO-LOCAL-KEY-V1\n";
const KEY_FILE_BYTES: usize = KEY_FILE_MAGIC.len() + 1 + DATA_KEY_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalKeySource {
    Fresh = 1,
    KeychainMigration = 2,
}

pub struct LocalKeyFile {
    pub key: Zeroizing<[u8; DATA_KEY_BYTES]>,
    pub source: LocalKeySource,
    pub pending: bool,
}

pub fn load_local_key(data_dir: &Path) -> Result<Option<LocalKeyFile>, AppError> {
    let final_path = data_dir.join(KEY_FILE_NAME);
    if final_path.exists() {
        return read_key_file(&final_path, false).map(Some);
    }
    let pending_path = data_dir.join(PENDING_KEY_FILE_NAME);
    if pending_path.exists() {
        return read_key_file(&pending_path, true).map(Some);
    }
    Ok(None)
}

pub fn write_pending_local_key(
    data_dir: &Path,
    key: &[u8; DATA_KEY_BYTES],
    source: LocalKeySource,
) -> Result<PathBuf, AppError> {
    fs::create_dir_all(data_dir).map_err(storage_error)?;
    let path = data_dir.join(PENDING_KEY_FILE_NAME);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path).map_err(storage_error)?;
    file.write_all(KEY_FILE_MAGIC).map_err(storage_error)?;
    file.write_all(&[source as u8]).map_err(storage_error)?;
    file.write_all(key).map_err(storage_error)?;
    file.sync_all().map_err(storage_error)?;
    enforce_private_permissions(&path)?;
    sync_directory(data_dir)?;
    Ok(path)
}

pub fn finalize_local_key(data_dir: &Path) -> Result<PathBuf, AppError> {
    let pending_path = data_dir.join(PENDING_KEY_FILE_NAME);
    let final_path = data_dir.join(KEY_FILE_NAME);
    if final_path.exists() {
        return Err(AppError::Storage(
            "local key already finalized while a migration key is pending".to_owned(),
        ));
    }
    fs::rename(&pending_path, &final_path).map_err(storage_error)?;
    enforce_private_permissions(&final_path)?;
    sync_directory(data_dir)?;
    Ok(final_path)
}

fn read_key_file(path: &Path, pending: bool) -> Result<LocalKeyFile, AppError> {
    let metadata = fs::symlink_metadata(path).map_err(storage_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::Storage(
            "local key path is not a regular file".to_owned(),
        ));
    }
    enforce_private_permissions(path)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(KEY_FILE_BYTES));
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(storage_error)?;
    if bytes.len() != KEY_FILE_BYTES || !bytes.starts_with(KEY_FILE_MAGIC) {
        return Err(AppError::Storage("invalid local key file".to_owned()));
    }
    let source = match bytes[KEY_FILE_MAGIC.len()] {
        1 => LocalKeySource::Fresh,
        2 => LocalKeySource::KeychainMigration,
        _ => return Err(AppError::Storage("invalid local key source".to_owned())),
    };
    let key = bytes[KEY_FILE_MAGIC.len() + 1..]
        .try_into()
        .map_err(|_| AppError::Storage("invalid local key length".to_owned()))?;
    Ok(LocalKeyFile {
        key: Zeroizing::new(key),
        source,
        pending,
    })
}

#[cfg(unix)]
fn enforce_private_permissions(path: &Path) -> Result<(), AppError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(storage_error)
}

#[cfg(not(unix))]
fn enforce_private_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), AppError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(storage_error)
}

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn local_key_is_private_and_finalized_atomically() {
        let directory = tempdir().unwrap();
        let key = [37_u8; DATA_KEY_BYTES];
        let pending =
            write_pending_local_key(directory.path(), &key, LocalKeySource::KeychainMigration)
                .unwrap();
        let loaded = load_local_key(directory.path()).unwrap().unwrap();
        assert!(loaded.pending);
        assert_eq!(loaded.source, LocalKeySource::KeychainMigration);
        assert_eq!(&*loaded.key, &key);
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&pending).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let final_path = finalize_local_key(directory.path()).unwrap();
        assert_eq!(final_path, directory.path().join(KEY_FILE_NAME));
        let loaded = load_local_key(directory.path()).unwrap().unwrap();
        assert!(!loaded.pending);
        assert_eq!(&*loaded.key, &key);
    }

    #[test]
    fn malformed_local_key_is_rejected() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(KEY_FILE_NAME);
        fs::write(path, b"not-a-key").unwrap();
        assert!(load_local_key(directory.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn local_key_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().unwrap();
        let target = directory.path().join("target");
        fs::write(&target, [0_u8; KEY_FILE_BYTES]).unwrap();
        symlink(&target, directory.path().join(KEY_FILE_NAME)).unwrap();
        assert!(load_local_key(directory.path()).is_err());
    }
}
