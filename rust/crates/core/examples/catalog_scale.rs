//! Opt-in synthetic scale workload. Requires a new private directory, never an installation.
use loom_core::{
    auth, compat, index_store, inventory, items, log_index, storage,
    store::{hash, now},
    Application, Config, Result,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
fn number(args: &[String], name: &str, default: u64) -> u64 {
    args.iter()
        .position(|a| a == name)
        .and_then(|n| args.get(n + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn percentiles(mut times: Vec<f64>) -> Value {
    times.sort_by(f64::total_cmp);
    let p = |n: f64| times[((times.len() - 1) as f64 * n).round() as usize];
    json!({"samples":times.len(),"p50_ms":p(0.5),"p95_ms":p(0.95),"p99_ms":p(0.99),"maximum_ms":p(1.)})
}
fn size(path: &Path) -> u64 {
    fs::read_dir(path)
        .map(|v| {
            v.filter_map(|e| e.ok())
                .map(|e| {
                    let m = e.path().symlink_metadata().unwrap();
                    if m.is_dir() {
                        size(&e.path())
                    } else {
                        m.len()
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}

fn reuse_queries(args: &[String], root: &Path) -> Result<()> {
    loom_core::error::require(
        serde_json::from_slice::<Value>(&fs::read(root.join("data/.loom-storage.json"))?)?
            == json!({"storage_id":"scale-only"}),
        "fixture",
        "This is not the synthetic scale fixture",
        409,
    )?;
    let actor = auth::person_actor("scale-fixture");
    let config: Config = serde_json::from_value(
        json!({"machine_id":"synthetic-scale","storage_id":"scale-only","actors":[actor],"catalog":{"index_max_bytes":number(args,"--disk-budget",16*1024*1024*1024)}}),
    )?;
    let state = root.join("state");
    let source = Connection::open_with_flags(
        state.join("ops.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let item_count: i64 = source.query_row("SELECT count(*) FROM scenarios", [], |r| r.get(0))?;
    let safe: i64 = source.query_row(
        "SELECT count(*) FROM jobs WHERE owner!='scale-fixture'",
        [],
        |r| r.get(0),
    )?;
    loom_core::error::require(
        safe == 0 && item_count > 0,
        "fixture",
        "Synthetic fixture ownership differs",
        409,
    )?;
    let mut app =
        Application::open_read_only(&state, &root.join("data"), &root.join("components"), config)?;
    let upgrade = Instant::now();
    let index = index_store::IndexWriter::open(&app)?;
    let upgrade_seconds = upgrade.elapsed().as_secs_f64();
    let file_count: i64 = index.db.query_row(
        "SELECT count(*) FROM docs WHERE kind='file' AND valid_to IS NULL",
        [],
        |r| r.get(0),
    )?;
    let segment_count: i64 = index
        .db
        .query_row("SELECT count(*) FROM log_segments", [], |r| r.get(0))?;
    let stream:String=source.query_row("SELECT json_extract(body,'$.item_id') FROM catalog_revisions r JOIN catalog_items i ON i.id=r.item WHERE i.kind='log_stream' LIMIT 1",[],|r|r.get(0))?;
    let mut metadata = vec![];
    let mut paths = vec![];
    let mut logs = vec![];
    let mut ranges = vec![];
    let mut partial = [0usize; 3];
    for n in 0..40 {
        let started = Instant::now();
        let page=app.execute(&actor,"results","search_items",json!({"text":format!("scale-{:09}",n*7919%item_count),"kinds":["scenario"],"view":"all","role":"all","limit":50}))?;
        metadata.push(started.elapsed().as_secs_f64() * 1000.);
        loom_core::error::require(
            page["items"].as_array().is_some_and(|a| a.len() == 1),
            "fixture",
            "Selective metadata query differed",
            409,
        )?;
        partial[0] += usize::from(page["partial"] == true);
        let started = Instant::now();
        let page=app.execute(&actor,"results","search_items",json!({"text":format!("file-{:012}.txt",n*7919%file_count),"kinds":["file"],"view":"all","role":"all","limit":50}))?;
        paths.push(started.elapsed().as_secs_f64() * 1000.);
        loom_core::error::require(
            page["items"].as_array().is_some_and(|a| a.len() == 1),
            "fixture",
            "Selective path query differed",
            409,
        )?;
        partial[1] += usize::from(page["partial"] == true);
        let started = Instant::now();
        let page = app.execute(
            &actor,
            "jobs",
            "search_logs",
            json!({"stream_id":stream,"text":"needle","limit":50}),
        )?;
        logs.push(started.elapsed().as_secs_f64() * 1000.);
        loom_core::error::require(
            page["items"].as_array().is_some_and(|a| a.len() == 50),
            "fixture",
            "Log query differed",
            409,
        )?;
        partial[2] += usize::from(page["partial"] == true);
        let started = Instant::now();
        let _ = app.execute(
            &actor,
            "jobs",
            "read_log",
            json!({"stream_id":stream,"offset":"0","limit":262144}),
        )?;
        ranges.push(started.elapsed().as_secs_f64() * 1000.);
    }
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    unsafe {
        libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr());
    }
    println!(
        "{}",
        json!({"schema":"loom-synthetic-scale-query/v1","scope":"read-only authority; same synthetic corpus after query correction","items":item_count,"files":file_count,"log_documents":segment_count,"metadata_search":percentiles(metadata),"path_search":percentiles(paths),"log_search":percentiles(logs),"range_read":percentiles(ranges),"partial_query_counts":{"metadata":partial[0],"path":partial[1],"log":partial[2]},"query_index_upgrade_seconds":upgrade_seconds,"peak_rss_kib":unsafe{usage.assume_init()}.ru_maxrss,"scientific_validation":false})
    );
    Ok(())
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .iter()
        .position(|a| a == "--root")
        .and_then(|n| args.get(n + 1))
        .ok_or_else(|| {
            loom_core::Error::input("Use --root with a new private fixture directory")
        })?;
    let root = PathBuf::from(path);
    if args.iter().any(|a| a == "--reuse-synthetic") {
        return reuse_queries(&args, &root);
    }
    loom_core::error::require(!root.exists(), "fixture", "Scale root must not exist", 409)?;
    fs::create_dir_all(&root)?;
    index_store::secure_dir(&root)?;
    let item_count = number(&args, "--items", 10000).min(100000);
    let file_count = number(&args, "--files", 100000).min(1000000);
    let segments = number(&args, "--segments", 100000).min(10000000);
    let budget = number(&args, "--disk-budget", 16 * 1024 * 1024 * 1024);
    storage::reserve_space(&root, budget, 64 * 1024 * 1024)?;
    let data = root.join("data");
    fs::create_dir(&data)?;
    fs::write(
        data.join(".loom-storage.json"),
        r#"{"storage_id":"scale-only"}"#,
    )?;
    let components = root.join("components");
    fs::create_dir(&components)?;
    let state = root.join("state");
    fs::create_dir(&state)?;
    let actor = auth::person_actor("scale-fixture");
    let config: Config = serde_json::from_value(
        json!({"machine_id":"synthetic-scale","storage_id":"scale-only","actors":[actor],"catalog":{"index_max_bytes":budget}}),
    )?;
    let began = Instant::now();
    // Seed a pre-cutover fixture so backfill exercises the actual bounded adoption path.
    {
        let mut db = Connection::open(state.join("ops.sqlite3"))?;
        db.execute_batch(include_str!("../../../contracts/legacy-schema.sql"))?;
        db.execute_batch(include_str!("../../../contracts/redesign-schema.sql"))?;
        db.pragma_update(None, "user_version", 5)?;
        for from in (0..item_count).step_by(1000) {
            let tx = db.transaction()?;
            for n in from..(from + 1000).min(item_count) {
                let id = format!("scale-{n:09}");
                let definition = json!({"label":format!("Scale record {n:09}")});
                tx.execute(
                    "INSERT INTO scenarios VALUES(?,?,1,0,?,'scale-fixture')",
                    params![id, definition["label"].as_str(), now()],
                )?;
                tx.execute("INSERT INTO scenario_revisions VALUES(?,1,?,?,?,'scale-fixture','synthetic fixture')",params![id,definition.to_string(),hash(compat::seal(&definition)),now()])?;
            }
            tx.commit()?;
        }
    }
    let mut app = Application::open_schema(&state, &data, &components, config, 7)?;
    for _ in 0..(item_count / 64 + 100) {
        let result = app.catalog_tick()?;
        if result["adopted"] == 0 && result["dirty"] == 0 {
            break;
        }
    }
    // One technical Job owns the file/log fixture; no child process or scientific task runs.
    let job=app.execute(&actor,"jobs","submit",json!({"key":"scale-owner","kind":"cpu.smoke","owner":"scale-fixture","notify":"scale-fixture","params":{"seconds":0,"fail":false},"presentation":{"scope":"operations","role":"test"}}))?;
    let owner =
        app.resolve_item(&json!({"native_ref":{"kind":"job","identity":{"id":job["id"]}}}))?;
    let item = items::parse(&owner["item_id"])?;
    let revision = items::parse(&owner["revision_id"])?;
    let prepared=inventory::prepare_entries(&app,&item,&revision,(0..file_count).map(|n|Ok(json!({"entry_key":format!("file-{n:012}.txt"),"path":format!("file-{n:012}.txt"),"role":"synthetic","size":0,"sha256":hash(""),"copies":[]}))))?;
    app.commit_inventory(prepared)?;
    let work = app
        .next_work(&Default::default())?
        .ok_or_else(|| loom_core::Error::input("Fixture owner missing"))?;
    app.stage_returned(
        &work,
        1,
        json!({"returned":true,"exit_code":1,"scope":"synthetic scale fixture"}),
    )?;
    let projection = Instant::now();
    let mut index = index_store::IndexWriter::open(&app)?;
    loop {
        index.sync(&app, 128)?;
        let checkpoint: i64 =
            index
                .db
                .query_row("SELECT checkpoint FROM meta WHERE id=1", [], |r| r.get(0))?;
        if checkpoint == app.catalog_head()? {
            break;
        }
    }
    let projection_seconds = projection.elapsed().as_secs_f64();
    log_index::project(&mut index, &app)?;
    let stream = app.execute(&actor, "jobs", "log_streams", json!({"id":job["id"]}))?["items"][0]
        ["item_id"]
        .clone();
    let stream_id = items::parse(&stream)?;
    let generation = uuid::Uuid::now_v7().to_string();
    fs::create_dir_all(log_index::path(&state, &work).parent().unwrap())?;
    // Overlapping 64-byte documents allow a large FTS corpus with bounded actual raw bytes.
    let pattern = b"scale needle fixture\n";
    let raw_length = segments + 63;
    let mut raw = fs::File::create(log_index::path(&state, &work))?;
    let block: Vec<u8> = (0..65520).map(|n| pattern[n % pattern.len()]).collect();
    let mut left = raw_length;
    while left > 0 {
        let count = left.min(block.len() as u64) as usize;
        raw.write_all(&block[..count])?;
        left -= count as u64;
    }
    raw.sync_all()?;
    fs::write(
        log_index::state_path(&state, &work),
        json!({"generation":generation,"closed":true,"gap":false,"offset":raw_length}).to_string(),
    )?;
    let log_started = Instant::now();
    for from in (0..segments).step_by(1000) {
        let tx = index.db.transaction()?;
        {
            let mut insert = tx.prepare("INSERT INTO log_segments VALUES(?,?,?,?,?)")?;
            for n in from..(from + 1000).min(segments) {
                let text: String = (n..n + 64)
                    .map(|n| pattern[n as usize % pattern.len()] as char)
                    .collect();
                insert.execute(params![
                    stream_id,
                    generation,
                    n as i64,
                    (n + 64) as i64,
                    text
                ])?;
            }
        }
        tx.commit()?;
        if from % 100000 == 0 {
            loom_core::error::require(
                size(&root) < budget,
                "fixture_budget",
                "Synthetic disk budget exhausted",
                409,
            )?;
            eprintln!("scale log documents: {from}/{segments}");
        }
    }
    index.db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    let log_seconds = log_started.elapsed().as_secs_f64();
    let mut metadata_times = vec![];
    let mut file_times = vec![];
    let mut log_times = vec![];
    let mut range_times = vec![];
    for n in 0..40 {
        let start = Instant::now();
        let selected = (n as u64 * 7919) % item_count.max(1);
        let _=app.execute(&actor,"results","search_items",json!({"text":format!("scale-{selected:09}"),"kinds":["scenario"],"view":"all","role":"all","limit":50}))?;
        metadata_times.push(start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        let selected = (n as u64 * 7919) % file_count.max(1);
        let files=app.execute(&actor,"results","search_items",json!({"text":format!("file-{selected:012}.txt"),"kinds":["file"],"view":"all","role":"all","limit":50}))?;
        loom_core::error::require(
            files["items"].as_array().is_some_and(|a| !a.is_empty()),
            "fixture",
            "Selective file query missed fixture",
            409,
        )?;
        file_times.push(start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        let logs = app.execute(
            &actor,
            "jobs",
            "search_logs",
            json!({"stream_id":stream,"text":"needle","limit":50}),
        )?;
        loom_core::error::require(
            logs["items"].as_array().is_some_and(|a| !a.is_empty()),
            "fixture",
            "Log search missed fixture",
            409,
        )?;
        log_times.push(start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        let _ = app.execute(
            &actor,
            "jobs",
            "read_log",
            json!({"stream_id":stream,"offset":"0","limit":262144}),
        )?;
        range_times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    unsafe {
        libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr());
    }
    let rss = unsafe { usage.assume_init() }.ru_maxrss;
    println!(
        "{}",
        json!({"schema":"loom-synthetic-scale/v1","scope":"synthetic private fixture; no operational installation or scientific payload","items":item_count,"files":file_count,"log_documents":segments,"raw_log_bytes":raw_length,"raw_document_bytes":64,"raw_document_overlap":63,"metadata_search":percentiles(metadata_times),"path_search":percentiles(file_times),"log_search":percentiles(log_times),"range_read":percentiles(range_times),"projection_seconds":projection_seconds,"log_build_seconds":log_seconds,"total_seconds":began.elapsed().as_secs_f64(),"peak_rss_kib":rss,"core_bytes":fs::metadata(state.join("ops.sqlite3"))?.len(),"manifest_bytes":size(&inventory::root(&app)),"index_bytes":size(&index_store::directory(&state,&app.config)),"fixture_bytes":size(&root),"sqlite":rusqlite::version(),"scientific_validation":false})
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e:?}");
        std::process::exit(1)
    }
}
