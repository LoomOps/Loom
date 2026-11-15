//! Exact effect-qualified logs. Segments are a rebuildable projection, not domain Items.
use crate::{error::require, execution::Work, items, storage, store::*, Error, Result};
use base64::Engine;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
pub fn relative(work: &Work) -> String {
    format!("log-spool/{}.log", hash(&work.effect))
}
pub fn path(state: &Path, work: &Work) -> PathBuf {
    state.join(relative(work))
}
pub fn state_path(state: &Path, work: &Work) -> PathBuf {
    path(state, work).with_extension("json")
}
pub fn source(app: &Application, id: &Value) -> Result<(Value, Work)> {
    let stream = app.get_item(&json!({"item_id":id}))?;
    require(
        stream["kind"] == "log_stream",
        "unknown_item",
        "Log stream not found",
        404,
    )?;
    let text: String = app.db.query_row(
        "SELECT request FROM rust_effects WHERE id=?",
        [stream["effect"].as_str()],
        |r| r.get(0),
    )?;
    let work: Work = serde_json::from_str(&text)?;
    Ok((stream, work))
}
pub fn retained_length(file: &fs::File, state: &Value) -> Result<u64> {
    let available = file.metadata()?.len();
    let committed = state["offset"].as_u64().unwrap_or_else(|| {
        if state["closed"] == true {
            available
        } else {
            0
        }
    });
    require(
        committed <= available,
        "retention_gap",
        "Committed log bytes are missing",
        409,
    )?;
    Ok(committed)
}
pub fn read_source(app: &Application, work: &Work) -> Result<(fs::File, Value)> {
    let root = app.local.clone();
    let path = path(&root, work);
    let reference = path
        .strip_prefix(&root)
        .ok()
        .and_then(|p| p.to_str())
        .ok_or_else(|| Error::input("Invalid log reference"))?;
    let state = read_state(app, work)?;
    let file = storage::open_registered(&root, reference)?;
    let current = read_state(app, work)?;
    require(
        current["generation"] == state["generation"],
        "source_generation_changed",
        "Log generation changed during read",
        409,
    )?;
    Ok((file, state))
}
impl Application {
    pub fn log_streams(&self, args: &Value) -> Result<Value> {
        let job = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Job ID required"))?;
        require(self.db.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?) OR EXISTS(SELECT 1 FROM job_workflows WHERE id=?)",params![job,job],|r|r.get(0))?,"job","Job not found",404)?;
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as i64;
        let after = args["after"].as_str().unwrap_or("");
        let rows=rows(&self.db,"SELECT r.body FROM catalog_items i JOIN catalog_revisions r ON i.current_revision=r.id WHERE i.kind='log_stream' AND i.deleted=0 AND (json_extract(r.body,'$.job')=? OR EXISTS(SELECT 1 FROM workflow_nodes n WHERE n.root=? AND n.job=json_extract(r.body,'$.job'))) AND (?='' OR json_extract(r.body,'$.attempt')=?) AND (?='' OR json_extract(r.body,'$.stage')=?) AND hex(i.id)>? ORDER BY i.id LIMIT ?",&[&job,&job,&args["attempt"].as_str().unwrap_or(""),&args["attempt"].as_str().unwrap_or(""),&args["stage"].as_str().unwrap_or(""),&args["stage"].as_str().unwrap_or(""),&after,&(limit+1)])?;
        let more = rows.len() > limit as usize;
        let mut streams = vec![];
        for row in rows.into_iter().take(limit as usize) {
            let mut v: Value = serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;
            let (current, work) = source(self, &v["item_id"])?;
            v["source_state"] = current["source_state"].clone();
            if let Ok((f, state)) = read_source(self, &work) {
                v["bytes"] = json!(retained_length(&f, &state)?.to_string());
                v["source_generation"] = state["generation"].clone();
                v["closed"] = state["closed"].clone();
                v["gap"] = state["gap"].clone();
            } else {
                v["availability"] = json!("source_unavailable");
            }
            streams.push(v);
        }
        let next = if more {
            streams
                .last()
                .map(|v| {
                    items::parse(&v["item_id"])
                        .map(|id| id.iter().map(|b| format!("{b:02X}")).collect::<String>())
                })
                .transpose()?
        } else {
            None
        };
        Ok(json!({"items":streams,"next_after":next,"partial":false,"warnings":[]}))
    }
    pub fn read_log(&self, args: &Value) -> Result<Value> {
        let (stream, work) = source(self, &args["stream_id"])?;
        let limit = args["limit"].as_u64().unwrap_or(65536).clamp(1, 262144);
        if let Some(Ok(value)) = &self.live_logs {
            let mut v = value.clone();
            for key in ["offset", "next_offset", "total_bytes"] {
                if let Some(n) = v[key].as_u64() {
                    v[key] = json!(n.to_string());
                }
            }
            let text = v["text"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(32768)
                .collect::<String>();
            v["text_truncated"] = json!(v["text"]
                .as_str()
                .is_some_and(|s| s.chars().count() > 32768));
            v["text"] = json!(text);
            v["stream_id"] = args["stream_id"].clone();
            return Ok(v);
        }
        let (mut f, state) = read_generation(self, &work, args["source_generation"].as_str())?;
        if let Some(g) = args["source_generation"].as_str() {
            require(
                state["generation"] == g,
                "source_generation_changed",
                "Log generation changed; restart at byte zero",
                409,
            )?;
        }
        let total = retained_length(&f, &state)?;
        let offset = if args["tail"] == true {
            total.saturating_sub(limit)
        } else {
            args["offset"]
                .as_str()
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| Error::input("Invalid log offset"))?
                .min(total)
        };
        f.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![];
        f.take(limit.min(total - offset)).read_to_end(&mut bytes)?;
        Ok(
            json!({"stream_id":args["stream_id"],"stream_returned":stream["source_state"]=="returned","source_generation":state["generation"],"offset":offset.to_string(),"next_offset":(offset+bytes.len() as u64).to_string(),"total_bytes":total.to_string(),"data_base64":base64::engine::general_purpose::STANDARD.encode(&bytes),"text":String::from_utf8_lossy(&bytes).chars().take(32768).collect::<String>(),"text_truncated":String::from_utf8_lossy(&bytes).chars().count()>32768,"encoding_errors":std::str::from_utf8(&bytes).is_err(),"eof":offset+bytes.len() as u64==total,"closed":state["closed"]==true,"gap":state["gap"]==true}),
        )
    }
}
pub fn project(index: &mut crate::index_store::IndexWriter, app: &Application) -> Result<usize> {
    index.db.execute_batch("CREATE TABLE IF NOT EXISTS log_segments(stream BLOB NOT NULL,generation TEXT NOT NULL,start INTEGER NOT NULL,end INTEGER NOT NULL,text TEXT NOT NULL,PRIMARY KEY(stream,generation,start)); CREATE VIRTUAL TABLE IF NOT EXISTS log_terms USING fts5(text,tokenize='trigram'); CREATE TRIGGER IF NOT EXISTS log_insert AFTER INSERT ON log_segments BEGIN INSERT INTO log_terms(rowid,text) VALUES(new.rowid,new.text); END; CREATE TRIGGER IF NOT EXISTS log_delete AFTER DELETE ON log_segments BEGIN DELETE FROM log_terms WHERE rowid=old.rowid; END;")?;
    let mut done = 0;
    let after: i64 = index
        .db
        .query_row(
            "SELECT offset FROM log_sources WHERE generation='scan'",
            [],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0);
    let batch=rows(&app.db,"SELECT r.body,i.rowid AS position FROM catalog_items i JOIN catalog_revisions r ON i.current_revision=r.id WHERE i.kind='log_stream' AND i.deleted=0 AND i.rowid>? ORDER BY i.rowid LIMIT 32",&[&after])?;
    if batch.is_empty() {
        index.db.execute(
            "UPDATE log_sources SET offset=0 WHERE generation='scan'",
            [],
        )?;
    }
    for row in batch {
        index.db.execute("INSERT INTO log_sources VALUES(x'00','scan',?,'') ON CONFLICT(stream,generation) DO UPDATE SET offset=excluded.offset",[row["position"].as_i64()])?;
        let v: Value = serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;
        let (_, work) = source(app, &v["item_id"])?;
        let Ok((mut file, state)) = read_source(app, &work) else {
            continue;
        };
        let generation = state["generation"].as_str().unwrap_or(&work.effect);
        let stream = items::parse(&v["item_id"])?;
        let offset: i64 = index.db.query_row(
            "SELECT COALESCE(max(end),0) FROM log_segments WHERE stream=? AND generation=?",
            params![stream, generation],
            |r| r.get(0),
        )?;
        let committed = retained_length(&file, &state)?;
        if committed <= offset as u64 {
            continue;
        }
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut bytes = vec![];
        file.take(65536.min(committed - offset as u64))
            .read_to_end(&mut bytes)?;
        let tx = index.db.transaction()?;
        // Overlap keeps boundary-spanning words discoverable; offsets refer to the raw bytes.
        let start = (offset - 1024).max(0);
        let (mut f, observed) = read_source(app, &work)?;
        if observed["generation"] != state["generation"] {
            continue;
        }
        f.seek(SeekFrom::Start(start as u64))?;
        let mut text = vec![];
        f.take((offset - start) as u64 + bytes.len() as u64)
            .read_to_end(&mut text)?;
        tx.execute(
            "INSERT OR IGNORE INTO log_segments VALUES(?,?,?,?,?)",
            params![
                stream,
                generation,
                start,
                offset + bytes.len() as i64,
                String::from_utf8_lossy(&text)
            ],
        )?;
        tx.commit()?;
        done += 1;
        if done >= 16 {
            break;
        }
    }
    Ok(done)
}
impl Application {
    pub fn search_logs(&self, actor: &crate::auth::Actor, args: &Value) -> Result<Value> {
        use crate::index_store::{context, cursor, sign};
        let text = args["text"].as_str().unwrap_or("").trim();
        require(
            text.chars().count() >= 3 && text.len() <= 1024,
            "query_too_broad",
            "Log search requires 3–1024 characters",
            400,
        )?;
        let streams = if args["stream_id"].is_string() {
            vec![source(self, &args["stream_id"])?.0]
        } else {
            let page=self.log_streams(&json!({"id":args["id"],"attempt":args.get("attempt").cloned().unwrap_or(Value::Null),"limit":200}))?;
            require(
                page["next_after"].is_null(),
                "query_too_broad",
                "Select an Attempt or stream for this Job",
                400,
            )?;
            page["items"].as_array().cloned().unwrap_or_default()
        };
        let mut allowed = vec![];
        let mut coverage_partial = false;
        for stream in streams {
            let (_, work) = source(self, &stream["item_id"])?;
            let (file, state) = read_source(self, &work)
                .map_err(|_| Error::new("source_unavailable", "Log source is unavailable", 404))?;
            if state["closed"] != true || state["gap"] == true {
                coverage_partial = true;
            }
            let generation = state["generation"].as_str().unwrap_or("");
            let db = crate::index_store::read(self)?;
            let through: i64 = db.query_row(
                "SELECT COALESCE(max(end),0) FROM log_segments WHERE stream=? AND generation=?",
                params![items::parse(&stream["item_id"])?, generation],
                |r| r.get(0),
            )?;
            coverage_partial |= (through as u64) < retained_length(&file, &state)?;
            allowed.push(
                items::parse(&stream["item_id"])?
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<String>(),
            );
        }
        let db = crate::index_store::read(self)?;
        let generation: String =
            db.query_row("SELECT generation FROM meta WHERE id=1", [], |r| r.get(0))?;
        let head: i64 =
            db.query_row("SELECT COALESCE(max(rowid),0) FROM log_segments", [], |r| {
                r.get(0)
            })?;
        let mut query = args.clone();
        query.as_object_mut().unwrap().remove("cursor");
        let qhash = hash(encode(&query)?);
        let (key, access) = context(self, actor)?;
        let previous = args["cursor"]
            .as_str()
            .map(|s| cursor(s, &key))
            .transpose()?;
        if let Some(c) = &previous {
            require(
                c["generation"] == generation
                    && c["query"] == qhash
                    && c["access"] == access
                    && c["expires"].as_f64().is_some_and(|v| v > now()),
                "cursor_expired",
                "Log search cursor expired",
                409,
            )?;
        }
        let watermark = previous
            .as_ref()
            .and_then(|c| c["watermark"].as_i64())
            .unwrap_or(head);
        let last = previous
            .as_ref()
            .and_then(|c| c["last"].as_i64())
            .unwrap_or(0);
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as i64;
        let pattern = format!("\"{}\"", text.replace('"', "\"\""));
        let mut q=db.prepare("SELECT rowid,stream,generation,start,end,text FROM log_segments WHERE rowid>? AND rowid<=? AND hex(stream) IN(SELECT value FROM json_each(?)) AND rowid IN(SELECT rowid FROM log_terms WHERE log_terms MATCH ?) ORDER BY rowid LIMIT ?")?;
        let batch = q
            .query_map(
                params![
                    last,
                    watermark,
                    encode(&json!(allowed))?,
                    pattern,
                    limit + 1
                ],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let more = batch.len() > limit as usize;
        let mut hits = vec![];
        let mut end = last;
        for (rowid, id, g, start, finish, t) in batch.into_iter().take(limit as usize) {
            end = rowid;
            let (_, work) = source(self, &json!(items::decimal(&id)?))?;
            let (file, state) = read_generation(self, &work, Some(&g)).map_err(|_| {
                Error::new(
                    "retention_gap",
                    "Indexed log range is no longer readable",
                    409,
                )
            })?;
            require(
                finish as u64 <= retained_length(&file, &state)?,
                "retention_gap",
                "Indexed log bytes are no longer retained",
                409,
            )?;
            let snippet = t
                .find(text)
                .map(|p| t[p..].chars().take(128).collect::<String>())
                .unwrap_or_else(|| t.chars().take(128).collect());
            hits.push(json!({"stream_id":items::decimal(&id)?,"source_generation":g,"offset":start.to_string(),"end_offset":finish.to_string(),"snippet":snippet}));
        }
        let next = if more {
            Some(sign(
                &json!({"generation":generation,"query":qhash,"access":access,"expires":previous.as_ref().and_then(|c|c["expires"].as_f64()).unwrap_or(now()+300.),"watermark":watermark,"last":end}),
                &key,
            )?)
        } else {
            None
        };
        Ok(
            json!({"items":hits,"next_cursor":next,"partial":coverage_partial,"warnings":if coverage_partial{json!(["log_coverage_is_partial"])}else{json!([])},"consistency":{"generation":generation,"snapshot":watermark.to_string()}}),
        )
    }
}

impl Application {
    pub fn log_collection_candidates(&self, after: &str) -> Result<Vec<Value>> {
        rows(&self.db,"SELECT id,request,evidence FROM rust_effects WHERE id>? AND stage!='publish' AND state IN('accepted','unknown','returned') ORDER BY id LIMIT 32",&[&after])
    }
}

pub fn read_state(app: &Application, work: &Work) -> Result<Value> {
    let reference = state_path(&app.local, work)
        .strip_prefix(&app.local)
        .map_err(|_| Error::input("Invalid log reference"))?
        .to_string_lossy()
        .to_string();
    let Ok(f) = storage::open_registered(&app.local, &reference) else {
        return Ok(json!({"generation":work.effect,"closed":false,"gap":false}));
    };
    let mut bytes = vec![];
    f.take(8193).read_to_end(&mut bytes)?;
    require(
        bytes.len() <= 8192,
        "logs",
        "Log source descriptor exceeds limit",
        413,
    )?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn read_generation(
    app: &Application,
    work: &Work,
    generation: Option<&str>,
) -> Result<(fs::File, Value)> {
    let state = read_state(app, work)?;
    if let Some(g) = generation.filter(|g| state["generation"].as_str() != Some(*g)) {
        let id = uuid::Uuid::parse_str(g).map_err(|_| Error::input("Invalid log generation"))?;
        let archive = path(&app.local, work).with_extension(format!("{id}.log"));
        let reference = archive
            .strip_prefix(&app.local)
            .map_err(|_| Error::input("Invalid log reference"))?
            .to_string_lossy()
            .to_string();
        let f = storage::open_registered(&app.local, &reference).map_err(|_| {
            Error::new(
                "source_generation_changed",
                "Log generation is unavailable",
                409,
            )
        })?;
        let descriptor = archive.with_extension("json");
        let rel = descriptor
            .strip_prefix(&app.local)
            .map_err(|_| Error::input("Invalid log reference"))?
            .to_string_lossy();
        let mut bytes = vec![];
        storage::open_registered(&app.local, &rel)
            .map_err(|_| {
                Error::new(
                    "retention_gap",
                    "Retained generation descriptor is missing",
                    409,
                )
            })?
            .take(8193)
            .read_to_end(&mut bytes)?;
        require(
            bytes.len() <= 8192,
            "logs",
            "Log descriptor exceeds limit",
            413,
        )?;
        let state: Value = serde_json::from_slice(&bytes)?;
        require(
            state["generation"] == g,
            "source_generation_changed",
            "Retained log generation differs",
            409,
        )?;
        return Ok((f, state));
    }
    read_source(app, work)
}
