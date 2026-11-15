use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct Error {
    pub code: String,
    pub message: String,
    pub status: u16,
}
pub type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub fn new(code: &str, message: impl Into<String>, status: u16) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            status,
        }
    }
    pub fn input(message: impl Into<String>) -> Self {
        Self::new("input", message, 400)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new("conflict", message, 409)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}
impl From<rusqlite::Error> for Error {
    fn from(_: rusqlite::Error) -> Self {
        Self::new("storage", "Storage operation failed", 503)
    }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match error.kind() {
            ErrorKind::PermissionDenied => Self::new(
                "filesystem_permission",
                "Service identity cannot access the configured file",
                403,
            ),
            ErrorKind::NotFound => {
                Self::new("filesystem_missing", "Configured file is unavailable", 404)
            }
            ErrorKind::AlreadyExists => Self::new(
                "filesystem_conflict",
                "Filesystem destination already exists",
                409,
            ),
            ErrorKind::TimedOut => Self::new(
                "io_timeout",
                "I/O operation timed out; reconcile the original operation",
                503,
            ),
            _ => Self::new("filesystem", "Filesystem operation failed", 503),
        }
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::input("Invalid JSON contract")
    }
}
impl From<loom_domain::DomainError> for Error {
    fn from(e: loom_domain::DomainError) -> Self {
        Self::conflict(e.0)
    }
}
pub fn require(ok: bool, code: &str, message: &str, status: u16) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::new(code, message, status))
    }
}
