mod error;
mod hlc;
mod model;

pub use error::{AppError, ErrorCode, NO_SYNC_DEVICES_MESSAGE, UserFacingError};
pub use hlc::{HlcClock, HlcTimestamp};
pub use model::*;

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024;
pub const DEFAULT_HISTORY_LIMIT: usize = 500;
pub const CLIPBOARD_PAGE_SIZE: usize = 100;
pub const FILE_HISTORY_PAGE_SIZE: usize = 100;

pub fn content_hash(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}
