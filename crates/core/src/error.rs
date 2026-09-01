use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const NO_SYNC_DEVICES_MESSAGE: &str = "当前没有可同步的在线设备，请至少保持 1 台其他设备在线";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NetworkUnreachable,
    MdnsUnavailable,
    PairingTimeout,
    PairingRejected,
    InvalidPairingCode,
    ClipboardUnavailable,
    ClipboardTooLarge,
    SourceFileChanged,
    SourceFileMissing,
    DiskFull,
    PermissionDenied,
    ReceiveDirectoryInvalid,
    TransferFailed,
    StorageUnavailable,
    NoSyncDevices,
    InvalidInput,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFacingError {
    pub code: ErrorCode,
    pub message: String,
    pub detail: Option<String>,
    pub recoverable: bool,
}

impl UserFacingError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            recoverable: true,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn fatal(mut self) -> Self {
        self.recoverable = false;
        self
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("no sync devices are available")]
    NoSyncDevices,
    #[error("storage unavailable: {0}")]
    Storage(String),
    #[error("clipboard unavailable: {0}")]
    Clipboard(String),
    #[error("network unavailable: {0}")]
    Network(String),
    #[error("file operation failed: {0}")]
    File(String),
    #[error("insufficient disk space: need {required} bytes, {available} available")]
    DiskFull { required: u64, available: u64 },
    #[error("cryptographic operation failed")]
    Crypto,
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<AppError> for UserFacingError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::InvalidInput(detail) => {
                Self::new(ErrorCode::InvalidInput, "输入内容无效").detail(detail)
            }
            AppError::NoSyncDevices => Self::new(ErrorCode::NoSyncDevices, NO_SYNC_DEVICES_MESSAGE),
            AppError::Storage(detail) => {
                Self::new(ErrorCode::StorageUnavailable, "无法读取或保存本地数据").detail(detail)
            }
            AppError::Clipboard(detail) => {
                Self::new(ErrorCode::ClipboardUnavailable, "无法访问系统粘贴板").detail(detail)
            }
            AppError::Network(detail) => {
                Self::new(ErrorCode::NetworkUnreachable, "局域网连接不可用").detail(detail)
            }
            AppError::File(detail) => {
                Self::new(ErrorCode::TransferFailed, "文件操作失败").detail(detail)
            }
            AppError::DiskFull {
                required,
                available,
            } => Self::new(ErrorCode::DiskFull, "接收目录磁盘空间不足")
                .detail(format!("需要 {required} 字节，可用 {available} 字节")),
            AppError::Crypto => Self::new(ErrorCode::Internal, "安全存储初始化失败").fatal(),
            AppError::Internal(detail) => {
                Self::new(ErrorCode::Internal, "SyncHalo 遇到内部错误").detail(detail)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_sync_devices_has_one_stable_user_message() {
        let error = UserFacingError::from(AppError::NoSyncDevices);
        assert_eq!(error.code, ErrorCode::NoSyncDevices);
        assert_eq!(error.message, NO_SYNC_DEVICES_MESSAGE);
        assert!(error.recoverable);
    }
}
