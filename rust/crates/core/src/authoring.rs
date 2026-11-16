//! Source and build artifacts are different immutable identities. The API never builds code.
use crate::{error::require, runtime, storage, store::*, Error, Result};
use serde_json::{json, Value};
use std::{io::Read, os::unix::fs::PermissionsExt};
impl Application {
    pub fn verify_build(&self, manifest: &Value, source_sha: &Value) -> Result<Option<Value>> {
        let Some(receipt) = manifest.get("build_receipt") else {
            return Ok(None);
        };
        require(
            receipt["schema"] == "loom-producer-build/v1"
                && receipt["producer_protocol"] == "loom-producer/v1"
                && receipt["source_sha256"] == *source_sha,
            "build",
            "Build receipt does not match source or producer protocol",
            409,
        )?;
        let catalog = self.runtime_inventory()?;
        require(
            catalog["status"] == "ok",
            "build",
            "Fresh builder Runtime observation required",
            503,
        )?;
        let builder = &receipt["builder_runtime"];
        require(
            catalog["runtimes"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|r| {
                    r["id"] == builder["id"]
                        && r["revision"] == builder["revision"]
                        && r["sha256"] == builder["sha256"]
                        && r["legacy"] != true
                }),
            "build",
            "Builder Runtime outside exact instance grant",
            403,
        )?;
        let locator = &receipt["binary"];
        let path = storage::locator(&self.config, &self.bulk, locator)?;
        let file = self.verified_file(&path)?;
        require(
            locator["sha256"] == file.sha256
                && locator["size"].as_u64() == Some(file.size)
                && std::fs::metadata(&path)?.permissions().mode() & 0o111 != 0,
            "build",
            "Built executable differs from receipt",
            409,
        )?;
        let mut header = [0; 20];
        storage::open_regular(&path)?.read_exact(&mut header)?;
        require(
            &header[..4] == b"\x7fELF" && header[4] == 2 && header[5] == 1,
            "build",
            "A 64-bit little-endian ELF artifact is required",
            409,
        )?;
        let arch = u16::from_le_bytes([header[18], header[19]]);
        let target = receipt["target"]
            .as_str()
            .ok_or_else(|| Error::input("Build target required"))?;
        require(
            matches!(
                (arch, target),
                (62, "x86_64-unknown-linux-gnu") | (183, "aarch64-unknown-linux-gnu")
            ),
            "build",
            "Executable architecture differs from build target",
            409,
        )?;
        file.revalidate()?;
        Ok(Some(receipt.clone()))
    }
}
pub fn bind_build(profile: &mut Value, receipt: Option<&Value>) -> Result<()> {
    for stage in runtime::STAGES {
        let Some(runner) = profile.get_mut(stage) else {
            continue;
        };
        if runner["command"]
            .as_array()
            .is_some_and(|a| a.first() == Some(&json!("@producer")))
        {
            let receipt = receipt.ok_or_else(|| {
                Error::new("build", "@producer requires a verified build receipt", 409)
            })?;
            require(
                runner["executor"] == "container" || runner.get("runtime_ref").is_some(),
                "build",
                "User producers require an explicit container Runtime",
                409,
            )?;
            runner["producer_build"] = receipt.clone();
            runner["command"][0] = json!("/inputs/loom-producer");
        }
    }
    Ok(())
}
