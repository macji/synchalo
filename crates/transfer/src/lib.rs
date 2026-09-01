use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};
use synchalo_core::AppError;
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom},
};
use uuid::Uuid;

pub const TRANSFER_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileManifest {
    pub id: Uuid,
    pub file_name: String,
    pub file_size: u64,
    pub blake3: String,
    pub modified_unix_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SourceFingerprint {
    pub file_size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub struct IncomingTarget {
    pub temp_path: PathBuf,
    pub final_path: PathBuf,
    pub resume_offset: u64,
    pub already_complete: bool,
}

impl SourceFingerprint {
    pub async fn capture(path: &Path) -> Result<Self, AppError> {
        let metadata = fs::symlink_metadata(path).await.map_err(file_error)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(AppError::File(
                "only regular files are supported in the MVP".to_owned(),
            ));
        }
        Ok(Self {
            file_size: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    pub async fn still_matches(&self, path: &Path) -> Result<bool, AppError> {
        let current = Self::capture(path).await?;
        Ok(self.file_size == current.file_size && self.modified == current.modified)
    }
}

pub async fn inspect_file(path: &Path) -> Result<(FileManifest, SourceFingerprint), AppError> {
    let fingerprint = SourceFingerprint::capture(path).await?;
    let file_name = safe_file_name(path)?;
    let mut file = File::open(path).await.map_err(file_error)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; TRANSFER_CHUNK_BYTES];
    loop {
        let read = file.read(&mut buffer).await.map_err(file_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if !fingerprint.still_matches(path).await? {
        return Err(AppError::File(
            "source file changed while it was being inspected".to_owned(),
        ));
    }
    Ok((
        FileManifest {
            id: Uuid::new_v4(),
            file_name,
            file_size: fingerprint.file_size,
            blake3: hasher.finalize().to_hex().to_string(),
            modified_unix_ms: fingerprint
                .modified
                .and_then(|value| value.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|value| value.as_millis().min(u64::MAX as u128) as u64),
        },
        fingerprint,
    ))
}

pub async fn copy_verified<F>(
    source: &Path,
    receive_directory: &Path,
    manifest: &FileManifest,
    mut on_progress: F,
) -> Result<PathBuf, AppError>
where
    F: FnMut(u64, u64) + Send,
{
    fs::create_dir_all(receive_directory)
        .await
        .map_err(file_error)?;
    let temp_path = receive_directory.join(format!(".synchalo-{}.part", manifest.id));
    let final_path = resolve_destination(receive_directory, manifest).await?;

    let existing = fs::metadata(&temp_path)
        .await
        .ok()
        .map(|meta| meta.len())
        .unwrap_or(0);
    let mut resume_at = existing.min(manifest.file_size);
    if existing > manifest.file_size {
        fs::remove_file(&temp_path).await.map_err(file_error)?;
        resume_at = 0;
    }

    let mut source_file = File::open(source).await.map_err(file_error)?;
    source_file
        .seek(SeekFrom::Start(resume_at))
        .await
        .map_err(file_error)?;
    let mut target_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&temp_path)
        .await
        .map_err(file_error)?;

    let mut transferred = resume_at;
    let mut buffer = vec![0_u8; TRANSFER_CHUNK_BYTES];
    on_progress(transferred, manifest.file_size);
    while transferred < manifest.file_size {
        let remaining = (manifest.file_size - transferred) as usize;
        let read = source_file
            .read(&mut buffer[..remaining.min(TRANSFER_CHUNK_BYTES)])
            .await
            .map_err(file_error)?;
        if read == 0 {
            return Err(AppError::File(
                "source file ended before the advertised size".to_owned(),
            ));
        }
        target_file
            .write_all(&buffer[..read])
            .await
            .map_err(file_error)?;
        transferred += read as u64;
        on_progress(transferred, manifest.file_size);
    }
    target_file.flush().await.map_err(file_error)?;
    target_file.sync_all().await.map_err(file_error)?;
    drop(target_file);

    let actual_hash = hash_file(&temp_path).await?;
    if actual_hash != manifest.blake3 {
        return Err(AppError::File(format!(
            "file hash mismatch: expected {}, got {actual_hash}",
            manifest.blake3
        )));
    }

    if final_path == temp_path {
        return Ok(final_path);
    }
    fs::rename(&temp_path, &final_path)
        .await
        .map_err(file_error)?;
    Ok(final_path)
}

pub async fn hash_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).await.map_err(file_error)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; TRANSFER_CHUNK_BYTES];
    loop {
        let read = file.read(&mut buffer).await.map_err(file_error)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub async fn prepare_incoming(
    receive_directory: &Path,
    manifest: &FileManifest,
) -> Result<IncomingTarget, AppError> {
    validate_manifest(manifest)?;
    fs::create_dir_all(receive_directory)
        .await
        .map_err(file_error)?;
    let final_path = resolve_destination(receive_directory, manifest).await?;
    if final_path == receive_directory.join(&manifest.file_name)
        && final_path.exists()
        && hash_file(&final_path).await.ok().as_deref() == Some(manifest.blake3.as_str())
    {
        return Ok(IncomingTarget {
            temp_path: final_path.clone(),
            final_path,
            resume_offset: manifest.file_size,
            already_complete: true,
        });
    }

    let temp_path = receive_directory.join(format!(".synchalo-{}.part", manifest.id));
    let existing = fs::metadata(&temp_path)
        .await
        .ok()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let resume_offset = if existing > manifest.file_size {
        fs::remove_file(&temp_path).await.map_err(file_error)?;
        0
    } else {
        existing
    };
    let remaining = manifest.file_size.saturating_sub(resume_offset);
    let safety_margin = 64 * 1024 * 1024_u64;
    let available = fs2::available_space(receive_directory).map_err(file_error)?;
    if available < remaining.saturating_add(safety_margin) {
        return Err(AppError::DiskFull {
            required: remaining.saturating_add(safety_margin),
            available,
        });
    }
    Ok(IncomingTarget {
        temp_path,
        final_path,
        resume_offset,
        already_complete: false,
    })
}

pub async fn verify_and_commit_incoming(
    target: &IncomingTarget,
    manifest: &FileManifest,
) -> Result<PathBuf, AppError> {
    if target.already_complete {
        return Ok(target.final_path.clone());
    }
    let metadata = fs::metadata(&target.temp_path).await.map_err(file_error)?;
    if metadata.len() != manifest.file_size {
        return Err(AppError::File(format!(
            "received {} bytes but expected {}",
            metadata.len(),
            manifest.file_size
        )));
    }
    let actual_hash = hash_file(&target.temp_path).await?;
    if actual_hash != manifest.blake3 {
        return Err(AppError::File(format!(
            "file hash mismatch: expected {}, got {actual_hash}",
            manifest.blake3
        )));
    }
    fs::rename(&target.temp_path, &target.final_path)
        .await
        .map_err(file_error)?;
    Ok(target.final_path.clone())
}

async fn resolve_destination(
    receive_directory: &Path,
    manifest: &FileManifest,
) -> Result<PathBuf, AppError> {
    let desired = receive_directory.join(&manifest.file_name);
    if !desired.exists() {
        return Ok(desired);
    }
    if hash_file(&desired).await.ok().as_deref() == Some(manifest.blake3.as_str()) {
        return Ok(desired);
    }

    let source = Path::new(&manifest.file_name);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let extension = source.extension().and_then(|value| value.to_str());
    for suffix in 2..10_000 {
        let candidate_name = match extension {
            Some(extension) => format!("{stem} ({suffix}).{extension}"),
            None => format!("{stem} ({suffix})"),
        };
        let candidate = receive_directory.join(candidate_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(AppError::File(
        "unable to choose a non-conflicting destination name".to_owned(),
    ))
}

fn safe_file_name(path: &Path) -> Result<String, AppError> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && *value != "." && *value != "..")
        .ok_or_else(|| AppError::File("invalid file name".to_owned()))?;
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(AppError::File("unsafe file name".to_owned()));
    }
    Ok(name.to_owned())
}

fn validate_manifest(manifest: &FileManifest) -> Result<(), AppError> {
    let path = Path::new(&manifest.file_name);
    if path.components().count() != 1 || safe_file_name(path)? != manifest.file_name {
        return Err(AppError::File("unsafe file name in manifest".to_owned()));
    }
    if manifest.blake3.len() != 64
        || !manifest
            .blake3
            .chars()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err(AppError::File(
            "invalid BLAKE3 digest in manifest".to_owned(),
        ));
    }
    Ok(())
}

fn file_error(error: impl std::fmt::Display) -> AppError {
    AppError::File(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn copy_is_verified_and_does_not_overwrite_different_content() {
        let source_dir = tempfile::tempdir().unwrap();
        let receive_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("notes.txt");
        fs::write(&source, b"synchalo").await.unwrap();
        fs::write(receive_dir.path().join("notes.txt"), b"existing")
            .await
            .unwrap();
        let (manifest, _) = inspect_file(&source).await.unwrap();

        let destination = copy_verified(&source, receive_dir.path(), &manifest, |_, _| {})
            .await
            .unwrap();

        assert_eq!(destination.file_name().unwrap(), "notes (2).txt");
        assert_eq!(fs::read(destination).await.unwrap(), b"synchalo");
        assert_eq!(
            fs::read(receive_dir.path().join("notes.txt"))
                .await
                .unwrap(),
            b"existing"
        );
    }

    #[tokio::test]
    async fn incoming_manifest_rejects_path_traversal() {
        let receive_dir = tempfile::tempdir().unwrap();
        let manifest = FileManifest {
            id: Uuid::new_v4(),
            file_name: "../../escape.txt".to_owned(),
            file_size: 1,
            blake3: "0".repeat(64),
            modified_unix_ms: None,
        };
        assert!(
            prepare_incoming(receive_dir.path(), &manifest)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn incoming_file_resumes_from_existing_partial_offset() {
        let receive_dir = tempfile::tempdir().unwrap();
        let payload = b"resume this verified payload";
        let manifest = FileManifest {
            id: Uuid::new_v4(),
            file_name: "resume.bin".to_owned(),
            file_size: payload.len() as u64,
            blake3: blake3::hash(payload).to_hex().to_string(),
            modified_unix_ms: None,
        };
        let partial_path = receive_dir
            .path()
            .join(format!(".synchalo-{}.part", manifest.id));
        fs::write(&partial_path, &payload[..7]).await.unwrap();

        let target = prepare_incoming(receive_dir.path(), &manifest)
            .await
            .unwrap();
        assert_eq!(target.resume_offset, 7);
        let mut output = OpenOptions::new()
            .append(true)
            .open(&target.temp_path)
            .await
            .unwrap();
        output.write_all(&payload[7..]).await.unwrap();
        output.flush().await.unwrap();
        drop(output);

        let final_path = verify_and_commit_incoming(&target, &manifest)
            .await
            .unwrap();
        assert_eq!(fs::read(final_path).await.unwrap(), payload);
    }
}
