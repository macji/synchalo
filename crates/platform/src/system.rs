use std::path::PathBuf;

use directories::UserDirs;
use synchalo_core::{ClipboardCapability, DevicePlatform, PlatformCapabilitiesView};

pub fn default_device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|name| name.into_string().ok())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| match DevicePlatform::current() {
            DevicePlatform::Macos => "我的 Mac".to_owned(),
            DevicePlatform::Linux => "我的 Ubuntu".to_owned(),
            DevicePlatform::Unknown => "我的设备".to_owned(),
        })
}

pub fn default_receive_directory() -> PathBuf {
    UserDirs::new()
        .and_then(|dirs| dirs.download_dir().map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn platform_capabilities() -> PlatformCapabilitiesView {
    PlatformCapabilitiesView {
        platform: DevicePlatform::current(),
        architecture: std::env::consts::ARCH.to_owned(),
        clipboard: clipboard_capability(),
        supports_tray: cfg!(any(target_os = "macos", target_os = "linux")),
        supports_autostart: cfg!(any(target_os = "macos", target_os = "linux")),
    }
}

pub fn read_clipboard_files() -> Result<Vec<PathBuf>, synchalo_core::AppError> {
    #[cfg(target_os = "macos")]
    let paths = clipboard_files::read().map_err(|error| match error {
        clipboard_files::Error::NoFiles => {
            synchalo_core::AppError::InvalidInput("the clipboard does not contain files".to_owned())
        }
        clipboard_files::Error::SystemError(message) => synchalo_core::AppError::Clipboard(message),
    })?;

    #[cfg(target_os = "linux")]
    let paths = read_linux_clipboard_files()?;

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let paths: Vec<PathBuf> = return Err(synchalo_core::AppError::Clipboard(
        "file clipboard is not supported on this platform".to_owned(),
    ));
    let mut safe_paths = Vec::new();
    for path in paths.into_iter().take(100) {
        if !path.is_absolute() {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| synchalo_core::AppError::File(error.to_string()))?;
        if metadata.is_file() && !metadata.file_type().is_symlink() {
            safe_paths.push(path);
        }
    }
    if safe_paths.is_empty() {
        return Err(synchalo_core::AppError::InvalidInput(
            "the clipboard does not contain supported regular files".to_owned(),
        ));
    }
    Ok(safe_paths)
}

#[cfg(target_os = "linux")]
fn read_linux_clipboard_files() -> Result<Vec<PathBuf>, synchalo_core::AppError> {
    let wayland_result = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        read_wayland_uri_list()
    } else {
        Err(synchalo_core::AppError::Clipboard(
            "Wayland is not active".to_owned(),
        ))
    };
    let contents = match wayland_result {
        Ok(contents) => contents,
        Err(wayland_error) if std::env::var_os("DISPLAY").is_some() => read_x11_uri_list()
            .map_err(|x11_error| {
                synchalo_core::AppError::Clipboard(format!(
                    "Wayland: {wayland_error}; X11: {x11_error}"
                ))
            })?,
        Err(error) => return Err(error),
    };
    parse_uri_list(&contents)
}

#[cfg(target_os = "linux")]
fn read_wayland_uri_list() -> Result<String, synchalo_core::AppError> {
    use std::io::Read as _;
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

    let (mut reader, _) = get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        MimeType::Specific("text/uri-list"),
    )
    .map_err(|error| synchalo_core::AppError::Clipboard(error.to_string()))?;
    let mut contents = String::new();
    reader
        .read_to_string(&mut contents)
        .map_err(|error| synchalo_core::AppError::Clipboard(error.to_string()))?;
    Ok(contents)
}

#[cfg(target_os = "linux")]
fn read_x11_uri_list() -> Result<String, synchalo_core::AppError> {
    use std::time::Duration;

    let clipboard = x11_clipboard::Clipboard::new()
        .map_err(|error| synchalo_core::AppError::Clipboard(error.to_string()))?;
    for target_name in ["text/uri-list", "x-special/gnome-copied-files"] {
        let target = clipboard
            .getter
            .get_atom(target_name)
            .map_err(|error| synchalo_core::AppError::Clipboard(error.to_string()))?;
        if let Ok(bytes) = clipboard.load(
            clipboard.getter.atoms.clipboard,
            target,
            clipboard.getter.atoms.property,
            Duration::from_secs(2),
        ) {
            if let Ok(contents) = String::from_utf8(bytes) {
                return Ok(contents);
            }
        }
    }
    Err(synchalo_core::AppError::InvalidInput(
        "the X11 clipboard does not contain a file URI list".to_owned(),
    ))
}

#[cfg(target_os = "linux")]
fn parse_uri_list(contents: &str) -> Result<Vec<PathBuf>, synchalo_core::AppError> {
    let paths: Vec<_> = contents
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.eq_ignore_ascii_case("copy")
                && !line.eq_ignore_ascii_case("cut")
        })
        .filter_map(|line| url::Url::parse(line).ok())
        .filter_map(|url| url.to_file_path().ok())
        .collect();
    if paths.is_empty() {
        return Err(synchalo_core::AppError::InvalidInput(
            "the clipboard URI list contains no local files".to_owned(),
        ));
    }
    Ok(paths)
}

fn clipboard_capability() -> ClipboardCapability {
    #[cfg(target_os = "macos")]
    {
        ClipboardCapability::Full
    }

    #[cfg(target_os = "linux")]
    {
        let session_type = std::env::var("XDG_SESSION_TYPE")
            .unwrap_or_default()
            .to_lowercase();
        if session_type == "wayland" {
            // The monitor performs a real runtime probe. Until that succeeds, Wayland
            // must be presented as limited rather than silently promising full access.
            ClipboardCapability::AppActiveOnly
        } else {
            ClipboardCapability::Full
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        ClipboardCapability::Unsupported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_name_and_receive_directory_have_safe_fallbacks() {
        assert!(!default_device_name().trim().is_empty());
        assert!(!default_receive_directory().as_os_str().is_empty());
    }
}
