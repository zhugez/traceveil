//! Read-only, offline diagnostics. Adapters emit events; rules never read source files.
pub mod adapters;
pub mod diagnostics;
pub mod events;
pub mod report;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SCHEMA_VERSION: &str = "1.0";
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_RECORD_BYTES: usize = 1024 * 1024;
pub const MAX_RECORDS: usize = 100_000;
pub const MAX_WARNINGS: usize = 32;
pub const MAX_FINDINGS: usize = 64;
pub const MAX_EVIDENCE: usize = 12;
pub const MAX_PENDING: usize = 256;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: &'static str,
}
impl Error {
    pub fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }
    pub fn input() -> Self {
        Self::new(
            "INVALID_EVENT",
            "The input does not satisfy the supported event contract.",
        )
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub fn digest(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn io_error(e: std::io::Error) -> Error {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => Error::new(
            "SESSION_NOT_FOUND",
            "The requested input or directory does not exist.",
        ),
        ErrorKind::PermissionDenied => Error::new(
            "PERMISSION_DENIED",
            "Read permission is required for this input.",
        ),
        _ => Error::new("IO_ERROR", "An input or output operation failed."),
    }
}
