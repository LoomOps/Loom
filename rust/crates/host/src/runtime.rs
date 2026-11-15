//! One administrator-owned Runtime catalog, independent of the Job DB.
use crate::executor::Config;
use loom_core::{
    compat,
    error::require,
    store::{hash, now, rows},
    Error, Result,
};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use std::path::Path;
fn open(root: &Path, write: bool) -> Result<Connection> {
    let path = root.join("runtimes.sqlite3");
    require(
        !path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink()),
        "runtime",
        "Runtime DB must not be linked",
        409,
    )?;
    let db = if write {
        Connection::open(&path)?
    } else {
        Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?
    };
    db.busy_timeout(std::time::Duration::from_secs(10))?;
    if write {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;CREATE TABLE IF NOT EXISTS runtime_revisions(id TEXT,revision INTEGER,manifest TEXT,sha256 TEXT,created REAL,PRIMARY KEY(id,revision));CREATE TABLE IF NOT EXISTS runtime_grants(instance TEXT,id TEXT,revision INTEGER,PRIMARY KEY(instance,id,revision));CREATE TABLE IF NOT EXISTS runtime_installations(id TEXT PRIMARY KEY,runtime TEXT,state TEXT,evidence TEXT,updated REAL);")?;
    }
    Ok(db)
}
pub fn detail(root: &Path, instance: &str, pin: &Value) -> Result<Value> {
    let db = open(root, false)?;
    let id = pin["id"]
        .as_str()
        .ok_or_else(|| Error::input("Runtime ID required"))?;
    let revision = pin["revision"]
        .as_i64()
        .ok_or_else(|| Error::input("Runtime revision required"))?;
    let mut r=rows(&db,"SELECT r.* FROM runtime_revisions r JOIN runtime_grants g ON r.id=g.id AND r.revision=g.revision WHERE g.instance=? AND r.id=? AND r.revision=?",&[&instance,&id,&revision])?.into_iter().next().ok_or_else(||Error::new("runtime","Runtime revision outside instance grant",403))?;
    if pin.get("sha256").is_some() {
        require(
            r["sha256"] == pin["sha256"],
            "runtime",
            "Runtime pin changed",
            409,
        )?;
    }
    let manifest: Value = serde_json::from_str(r["manifest"].as_str().unwrap_or("{}"))?;
    require(
        r["sha256"] == hash(compat::seal(&manifest)),
        "runtime",
        "Runtime catalog integrity mismatch",
        409,
    )?;
    r["manifest"] = manifest;
    r["runtime_ref"] = json!({"id":r["id"],"revision":r["revision"],"sha256":r["sha256"]});
    Ok(r)
}
pub fn verify_inventory(manifest: &Value) -> Result<()> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    for (name, expected) in manifest["reconciliation"]["source_pins"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let path = Path::new(name);
        require(
            path.is_absolute(),
            "runtime_inventory",
            "Absolute metadata pin required",
            409,
        )?;
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        require(
            file.metadata()?.is_file(),
            "runtime_inventory",
            "Runtime metadata file unavailable",
            409,
        )?;
        let mut bytes = vec![];
        file.by_ref()
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        require(
            bytes.len() <= 4 * 1024 * 1024 && expected.as_str() == Some(&hash(&bytes)),
            "runtime_inventory",
            "Observed dependency metadata changed; reconcile before binding this Runtime",
            409,
        )?;
    }
    Ok(())
}
pub fn listing(root: &Path, config: &Config, instance: &str) -> Result<Value> {
    let mut result = vec![];
    if root.join("runtimes.sqlite3").exists() {
        let db = open(root, false)?;
        for row in rows(
            &db,
            "SELECT id,revision FROM runtime_grants WHERE instance=? ORDER BY id,revision",
            &[&instance],
        )? {
            result.push(detail(root, instance, &row)?);
        }
    }
    for id in &config.instances[instance].runtimes {
        if let Some(runtime) = config.runtimes.get(id) {
            result.push(
                json!({"id":id,"revision":null,"legacy":true,"manifest":{"runtime":runtime,"dependencies":[]},"inventory_status":"unrecorded"}),
            );
        }
    }
    let mut installations = vec![];
    if root.join("runtimes.sqlite3").exists() {
        let db = open(root, false)?;
        for mut row in rows(
            &db,
            "SELECT * FROM runtime_installations ORDER BY updated DESC",
            &[],
        )? {
            let evidence: Value = serde_json::from_str(row["evidence"].as_str().unwrap_or("{}"))?;
            if evidence["instance"] == instance {
                row.as_object_mut().unwrap().remove("evidence");
                for key in ["path", "error", "container_returned", "runtime_ref"] {
                    if let Some(v) = evidence.get(key) {
                        row[key] = v.clone();
                    }
                }
                installations.push(row);
            }
        }
    }
    let count = installations.len();
    installations.truncate(100);
    Ok(
        json!({"status":"ok","runtimes":result,"configured_dependencies":config.instances[instance].extra.get("dependencies"),"database":"runtimes.sqlite3","installations":installations,"installation_count":count,"installations_truncated":count>100}),
    )
}
pub fn register(
    root: &Path,
    config: &Config,
    id: &str,
    manifest: Value,
    instances: &[String],
) -> Result<Value> {
    require(
        !id.is_empty()
            && id.len() <= 81
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
        "runtime",
        "Invalid Runtime identity",
        400,
    )?;
    require(
        !instances.is_empty() && instances.iter().all(|i| config.instances.contains_key(i)),
        "runtime",
        "Explicit known instance grants required",
        400,
    )?;
    let image = manifest["runtime"]["image"]
        .as_str()
        .ok_or_else(|| Error::input("Runtime image required"))?;
    require(
        image.starts_with("sha256:")
            && image.len() == 71
            && image[7..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "runtime",
        "Runtime requires exact image identity",
        400,
    )?;
    require(
        manifest["validation"]["status"] == "passed",
        "validation",
        "Runtime requires explicit successful execution validation",
        409,
    )?;
    loom_core::runtime::primary(&manifest, manifest.get("primary_dependencies"))?;
    for instance in instances {
        let cfg = &config.instances[instance];
        let dependency = cfg
            .extra
            .get("dependencies")
            .and_then(Value::as_str)
            .or_else(|| config.extra.get("dependencies").and_then(Value::as_str))
            .ok_or_else(|| {
                Error::new(
                    "dependencies",
                    "Configured dependency directory required",
                    409,
                )
            })?;
        let path = Path::new(dependency);
        require(
            path.is_absolute() && path.canonicalize()? == path,
            "dependencies",
            "Dependency root missing or redirected",
            409,
        )?;
        for item in manifest["dependencies"].as_array().into_iter().flatten() {
            if let Some(location) = item["path"].as_str() {
                let p = Path::new(location).canonicalize()?;
                require(
                    p.starts_with(path),
                    "dependencies",
                    "Dependency leaves configured root",
                    403,
                )?;
            }
        }
    }
    let mut db = open(root, true)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for row in rows(&tx,"SELECT state,evidence FROM runtime_installations WHERE state IN ('installing','validating','unknown')",&[])? {
        let evidence:Value=serde_json::from_str(row["evidence"].as_str().unwrap_or("{}"))?;
        require(manifest["installation"]["container_returned"]==true&&manifest["installation"]["path"]==evidence["path"],"runtime_busy","Another Runtime installation or validation owner is unresolved",409)?;
    }
    let digest = hash(compat::seal(&manifest));
    let existing = rows(
        &tx,
        "SELECT revision FROM runtime_revisions WHERE id=? AND sha256=?",
        &[&id, &digest],
    )?;
    let revision = if let Some(row) = existing.first() {
        row["revision"].as_i64().unwrap_or(1)
    } else {
        let revision: i64 = tx.query_row(
            "SELECT COALESCE(max(revision),0)+1 FROM runtime_revisions WHERE id=?",
            [id],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO runtime_revisions VALUES(?,?,?,?,?)",
            params![id, revision, compat::seal(&manifest), digest, now()],
        )?;
        revision
    };
    for instance in instances {
        tx.execute(
            "INSERT OR IGNORE INTO runtime_grants VALUES(?,?,?)",
            params![instance, id, revision],
        )?;
    }
    tx.commit()?;
    Ok(json!({"id":id,"revision":revision,"sha256":digest,"instances":instances}))
}

pub fn validation_fenced(root: &Path) -> Result<bool> {
    if !root.join("runtimes.sqlite3").exists() {
        return Ok(false);
    }
    let db = open(root, false)?;
    for row in rows(&db, "SELECT state,evidence FROM runtime_installations", &[])? {
        let evidence: Value = serde_json::from_str(row["evidence"].as_str().unwrap_or("{}"))?;
        if matches!(
            row["state"].as_str(),
            Some("installing" | "validating" | "unknown")
        ) || (evidence["validation_gpu"] == true && evidence["container_returned"] == false)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
