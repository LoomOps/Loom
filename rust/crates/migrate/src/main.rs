//! Explicit administrator migration. No daemon activation, credential rotation or Job submission.
use loom_core::{
    error::require,
    storage,
    store::{hash, now, WriterLock},
    Application, Config, Error, Result,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
fn table_hashes(db: &Connection) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    let mut names=db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    for name in names.query_map([], |r| r.get::<_, String>(0))? {
        let name = name?;
        require(
            name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "schema",
            "Unsupported table identity",
            409,
        )?;
        let mut query = db.prepare(&format!("SELECT * FROM \"{name}\" ORDER BY rowid"))?;
        let mut rows = query.query([])?;
        let mut count = 0;
        let mut digest = Sha256::new();
        while let Some(row) = rows.next()? {
            count += 1;
            for index in 0..row.as_ref().column_count() {
                use rusqlite::types::ValueRef;
                let bytes = match row.get_ref(index)? {
                    ValueRef::Null => vec![0],
                    ValueRef::Integer(n) => {
                        let mut v = vec![1];
                        v.extend(n.to_le_bytes());
                        v
                    }
                    ValueRef::Real(n) => {
                        let mut v = vec![2];
                        v.extend(n.to_bits().to_le_bytes());
                        v
                    }
                    ValueRef::Text(s) => {
                        let mut v = vec![3];
                        v.extend(s);
                        v
                    }
                    ValueRef::Blob(s) => {
                        let mut v = vec![4];
                        v.extend(s);
                        v
                    }
                };
                digest.update((bytes.len() as u64).to_le_bytes());
                digest.update(bytes);
            }
        }
        out.insert(
            name,
            json!({"rows":count,"sha256":format!("{:x}",digest.finalize())}),
        );
    }
    Ok(out)
}
fn connect(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)
}
fn inspect(state: &Path) -> Result<Value> {
    let db = connect(&state.join("ops.sqlite3"))?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    let active: i64 = db.query_row(
        "SELECT count(*) FROM jobs WHERE state IN ('running','preparing','processing')",
        [],
        |r| r.get(0),
    )?;
    let callbacks: i64 = db.query_row(
        "SELECT count(*) FROM hooks WHERE state IN ('running','pending')",
        [],
        |r| r.get(0),
    )?;
    Ok(
        json!({"schema":version,"integrity":integrity,"active_executions":active,"callbacks":callbacks,"preservation":table_hashes(&db)?}),
    )
}
fn roots(config: &Config, data: &Path, components: &Path) -> Result<Value> {
    let (primary, id) = storage::store(config, data, "data/0", Some(&config.storage_id))?;
    let mut stores = json!({"data/0":{"path":primary,"storage_id":id}});
    for selector in config.data_stores.keys().filter(|s| s.as_str() != "data/0") {
        let (root, id) = storage::store(config, data, selector, None)?;
        stores[selector] = json!({"path":root,"storage_id":id});
    }
    require(
        components.is_dir() && components.canonicalize()? == components,
        "components",
        "Component root missing or redirected",
        409,
    )?;
    Ok(
        json!({"machine_id":config.machine_id,"stores":stores,"components":components,"dependencies":config.extra.get("dependency_directory")}),
    )
}
fn expected_upgrade(
    backup: &Path,
    data: &Path,
    components: &Path,
    config: &Config,
) -> Result<Value> {
    let temporary = backup.join(format!(".expected-{}", loom_core::store::id()));
    std::fs::create_dir(&temporary)?;
    let result = (|| -> Result<Value> {
        let source = connect(&backup.join("ops.sqlite3"))?;
        let mut dest = Connection::open(temporary.join("ops.sqlite3"))?;
        rusqlite::backup::Backup::new(&source, &mut dest)?.run_to_completion(
            128,
            std::time::Duration::from_millis(5),
            None,
        )?;
        drop(dest);
        drop(source);
        let app = Application::open(&temporary, data, components, config.clone())?;
        let expected = json!(table_hashes(&app_connection(&temporary)?)?);
        drop(app);
        Ok(expected)
    })();
    let _ = std::fs::remove_dir_all(temporary);
    result
}
fn app_connection(state: &Path) -> Result<Connection> {
    connect(&state.join("ops.sqlite3"))
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let arg = |key: &str| -> Result<&str> {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .ok_or_else(|| Error::input(format!("{key} required")))
    };
    if args.get(1).map(String::as_str) == Some("--version") {
        println!("loom-migrate {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let state = Path::new(arg("--state")?);
    match args.get(1).map(String::as_str) {
        Some("inspect")=>println!("{}",inspect(state)?),
        Some("apply")=>{
            let _lock=WriterLock::acquire(state)?;let before=inspect(state)?;
            require(before["integrity"]=="ok"&&before["active_executions"]==0&&before["callbacks"]==0,"drain","Stop old writers and return all execution/callback owners before migration",409)?;
            let config_path=Path::new(arg("--config")?);let config=Config::read(config_path)?;let data=Path::new(arg("--data")?);let components=Path::new(arg("--components")?);let parity=roots(&config,data,components)?;
            if let Some(executor_root)=args.iter().position(|a|a=="--executor-root").and_then(|i|args.get(i+1)) {
                let root=Path::new(executor_root);
                if root.join("executor.sqlite3").exists(){let db=connect(&root.join("executor.sqlite3"))?;let unresolved:i64=db.query_row("SELECT count(*) FROM tickets WHERE state!='returned'",[],|r|r.get(0))?;require(unresolved==0,"executor_drain","Offline Executor still owns unreturned tickets",409)?;}
                require(!loom_host::runtime::validation_fenced(root)?,"executor_drain","Runtime validation owner unresolved",409)?;
            }else if let Some(executor)=config.extra.get("executor") {
                let evidence=loom_host::wire::call(Path::new(executor["socket"].as_str().ok_or_else(||Error::input("Executor socket required"))?),Path::new(executor["credential"].as_str().ok_or_else(||Error::input("Executor credential required"))?),"drain_status",&json!({}))?;
                require(evidence["ready"]==true,"executor_drain","Executor or Runtime owner has not returned",409)?;
            }
            let backup=Path::new(arg("--backup")?);
            let before;let expected;
            if backup.exists(){
                require(!backup.symlink_metadata()?.file_type().is_symlink(),"backup","Backup redirected",409)?;
                let intent:Value=serde_json::from_slice(&std::fs::read(backup.join("intent.json"))?)?;
                require(intent["state"].as_str()==state.to_str()&&intent["config_sha256"]==hash(std::fs::read(config_path)?),"migration_intent","Backup belongs to another instance/configuration",409)?;
                before=intent["before"].clone();expected=intent["expected_after"].clone();
                require(json!(table_hashes(&connect(&backup.join("ops.sqlite3"))?)?)==before["preservation"],"backup","Migration backup changed",409)?;
                let current=inspect(state)?;
                require(current["preservation"]==before["preservation"]||current["preservation"]==expected,"migration_changed","Records changed after cutover; retain all evidence for review",409)?;
            }else{
                before=inspect(state)?;std::fs::create_dir(backup)?;
                use std::os::unix::fs::PermissionsExt;std::fs::set_permissions(backup,std::fs::Permissions::from_mode(0o700))?;
                let source=connect(&state.join("ops.sqlite3"))?;let mut destination=Connection::open(backup.join("ops.sqlite3"))?;
                rusqlite::backup::Backup::new(&source,&mut destination)?.run_to_completion(128,std::time::Duration::from_millis(5),None)?;drop(destination);drop(source);
                std::fs::set_permissions(backup.join("ops.sqlite3"),std::fs::Permissions::from_mode(0o600))?;
                storage::atomic(&backup.join("config.json"),&std::fs::read(config_path)?,0o600)?;
                expected=expected_upgrade(backup,data,components,&config)?;
                storage::atomic(&backup.join("intent.json"),&serde_json::to_vec(&json!({"state":state,"config_sha256":hash(std::fs::read(config_path)?),"before":before,"expected_after":expected}))?,0o600)?;
            }
            let app=Application::open(state,data,components,config)?;let after=inspect(state)?;
            require(after["preservation"]==expected,"preservation","Migration differs from its independently reconstructed additive result",409)?;
            let receipt=json!({"schema":"loom-rust-migration/v1","created":now(),"state":state,"backup":backup,"config_sha256":hash(std::fs::read(config_path)?),"before":before,"after":after,"roots":parity,"authority":app.status()?,"rollback":"Allowed only while the post-migration table hashes remain unchanged"});
            storage::atomic(&backup.join("receipt.json"),&serde_json::to_vec_pretty(&receipt)?,0o600)?;println!("{}",json!({"applied":true,"schema":5,"receipt":backup.join("receipt.json")}));
        }
        Some("rollback")=>{
            let _lock=WriterLock::acquire(state)?;let receipt:Value=serde_json::from_slice(&std::fs::read(arg("--receipt")?)?)?;require(receipt["schema"]=="loom-rust-migration/v1"&&receipt["state"].as_str()==state.to_str(),"receipt","Migration receipt belongs to another instance",409)?;
            let current=inspect(state)?;require(current["active_executions"]==0&&current["callbacks"]==0&&current["preservation"]==receipt["after"]["preservation"],"rollback","Data changed after cutover; preserve new records instead of restoring an old DB",409)?;
            let backup=Path::new(receipt["backup"].as_str().ok_or_else(||Error::input("Backup missing"))?).join("ops.sqlite3");let source=connect(&backup)?;
            require(json!(table_hashes(&source)?)==receipt["before"]["preservation"],"backup","Backup differs from migration receipt",409)?;
            let mut destination=Connection::open(state.join("ops.sqlite3"))?;rusqlite::backup::Backup::new(&source,&mut destination)?.run_to_completion(128,std::time::Duration::from_millis(5),None)?;
            println!("{}",json!({"restored":true,"schema":receipt["before"]["schema"]}));
        }
        _=>return Err(Error::input("Usage: loom-migrate inspect|apply|rollback --state PATH [--config FILE --data ROOT --components ROOT --backup NEW_DIRECTORY | --receipt FILE]"))
    }
    Ok(())
}
