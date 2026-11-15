//! Private bounded incidents: codes and operation IDs only, never request bodies or credentials.
use crate::{
    store::{id, now, Application},
    Error,
};
impl Application {
    pub fn incident(&self, operation: &str, error: &Error) -> Option<String> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let directory = std::env::var_os("LOOM_DIAGNOSTICS_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| self.local.join("diagnostics"));
        let result = (|| -> std::io::Result<String> {
            std::fs::create_dir_all(&directory)?;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
            let file = directory.join("rust-incidents.jsonl");
            if file
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err(std::io::Error::other("linked diagnostic destination"));
            }
            if file.metadata().is_ok_and(|m| m.len() >= 8 * 1024 * 1024) {
                for n in (1..4).rev() {
                    let a = directory.join(format!("rust-incidents.{n}.jsonl"));
                    let b = directory.join(format!("rust-incidents.{}.jsonl", n + 1));
                    if a.exists() {
                        std::fs::rename(a, b)?;
                    }
                }
                std::fs::rename(&file, directory.join("rust-incidents.1.jsonl"))?;
            }
            let incident = id();
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(file)?;
            let operation: String = operation
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || "._-/".contains(*c))
                .take(160)
                .collect();
            writeln!(
                file,
                "{}",
                serde_json::json!({"id":incident,"recorded":now(),"operation":operation,"code":error.code,"status":error.status})
            )?;
            file.sync_data()?;
            Ok(incident)
        })();
        result.ok()
    }
}
