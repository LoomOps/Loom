//! Private bounded incidents: codes and operation IDs only, never request bodies or credentials.
use crate::{
    store::{id, now, Application},
    Error,
};
static INCIDENT_WRITER: std::sync::Mutex<()> = std::sync::Mutex::new(());
impl Application {
    pub fn incident(&self, operation: &str, error: &Error) -> Option<String> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let directory = std::env::var_os("LOOM_DIAGNOSTICS_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| self.local.join("diagnostics"));
        let result = (|| -> std::io::Result<String> {
            let _writer = INCIDENT_WRITER
                .lock()
                .map_err(|_| std::io::Error::other("diagnostic writer unavailable"))?;
            if directory
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err(std::io::Error::other("linked diagnostic directory"));
            }
            std::fs::create_dir_all(&directory)?;
            if directory.canonicalize()? != directory {
                return Err(std::io::Error::other("redirected diagnostic directory"));
            }
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
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(file)?;
            if !file.metadata()?.is_file() {
                return Err(std::io::Error::other("nonregular diagnostic destination"));
            }
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

pub(crate) const EXECUTION_ISSUE_SOURCE: &str = "loom-troubleshooting";

/// Publish only operational facts already visible through the Job and log APIs.
/// Raw errors, request parameters, paths and leases never enter the issue catalog.
pub(crate) fn record_execution_issue(
    db: &rusqlite::Connection,
    record: &crate::jobs::Record,
    stage: &str,
    code: &str,
    facts: serde_json::Value,
) -> crate::Result<()> {
    use serde_json::json;
    let job = record.aggregate.id.as_str();
    let Some(attempt) = record.aggregate.fence.attempt.as_ref().map(|a| a.as_str()) else {
        return Ok(());
    };
    let root: Option<String> = rusqlite::OptionalExtension::optional(db.query_row(
        "SELECT root FROM execution_memberships WHERE job=?",
        [job],
        |r| r.get(0),
    ))?;
    let key = format!("{job}.{attempt}.{stage}.{code}");
    let spec = &record.spec;
    let runner = match stage {
        "prepare" | "finalize" => &spec["profile"][stage],
        _ => &spec["profile"]["runtime"],
    };
    let pin = &runner["runtime_pin"];
    let runtime = if pin.is_object() {
        json!({"id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"]})
    } else {
        json!({"id":runner["runtime_id"]})
    };
    let diagnostic_code = facts["code"].as_str().unwrap_or(code);
    let body = json!({
        "entity":{
            "kind":"issue",
            "label":format!("{} · {} · {}", spec["kind"].as_str().unwrap_or("Execution"), stage, code),
            "summary":format!("{} · {} · {}", job, stage, diagnostic_code),
            "properties":{
                "job_id":root.as_deref().unwrap_or(job),
                "execution_id":job,"attempt_id":attempt,"stage":stage,"code":code,
                "status":"open","process_group_returned":true,
                "scenario_ref":spec["scenario_ref"],
                "profile":{"revision":spec["profile"]["revision"],"sha256":spec["profile"]["sha256"]},
                "runtime":runtime,
                "logs":{"id":job,"attempt":attempt},
                "execution":facts
            }
        },
        "record":{"created":now(),"state":"open"}
    });
    db.execute(
        "INSERT OR IGNORE INTO history_records(source,kind,id,revision,snapshot,body)
         VALUES(?,'issue',?,'1','runtime',?)",
        rusqlite::params![EXECUTION_ISSUE_SOURCE, key, crate::store::encode(&body)?],
    )?;
    Ok(())
}

pub(crate) fn execution_issues(
    db: &rusqlite::Connection,
    job: &str,
    attempt: Option<&str>,
) -> crate::Result<Vec<serde_json::Value>> {
    let prefix = attempt.map_or_else(|| job.to_owned(), |attempt| format!("{job}.{attempt}"));
    let min = format!("{prefix}.");
    let max = format!("{prefix}/");
    crate::store::rows(
        db,
        "SELECT h.id,h.revision,h.body FROM history_records h
         WHERE h.source=? AND h.kind='issue' AND h.snapshot='runtime'
           AND h.id>=? AND h.id<?
           AND NOT EXISTS(SELECT 1 FROM history_records later WHERE later.source=h.source
             AND later.kind=h.kind AND later.id=h.id AND later.snapshot=h.snapshot
             AND CAST(later.revision AS INTEGER)>CAST(h.revision AS INTEGER))
         ORDER BY h.rowid DESC LIMIT 30",
        &[&EXECUTION_ISSUE_SOURCE, &min, &max],
    )?
    .into_iter()
    .map(|row| {
        let doc: serde_json::Value = serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;
        Ok(
            serde_json::json!({"historical_ref":{"source":EXECUTION_ISSUE_SOURCE,"kind":"issue","id":row["id"],
                         "revision":row["revision"],"snapshot":"runtime"},
            "label":doc["entity"]["label"],"properties":doc["entity"]["properties"]}),
        )
    })
    .collect()
}

pub(crate) fn resolve_execution_issues(
    db: &rusqlite::Connection,
    job: &str,
    attempt: &str,
) -> crate::Result<()> {
    db.execute(
        "INSERT INTO history_records(source,kind,id,revision,snapshot,body)
         SELECT source,kind,id,CAST(CAST(revision AS INTEGER)+1 AS TEXT),snapshot,
           json_set(body,'$.entity.properties.status','resolved',
                    '$.entity.properties.verified_attempt_id',?,'$.record.state','resolved')
         FROM history_records h
         WHERE h.source=? AND h.kind='issue' AND h.snapshot='runtime' AND h.id>=? AND h.id<?
           AND json_extract(h.body,'$.record.state')='open'
           AND NOT EXISTS(SELECT 1 FROM history_records later WHERE later.source=h.source
             AND later.kind=h.kind AND later.id=h.id AND later.snapshot=h.snapshot
             AND CAST(later.revision AS INTEGER)>CAST(h.revision AS INTEGER))",
        rusqlite::params![
            attempt,
            EXECUTION_ISSUE_SOURCE,
            format!("{job}."),
            format!("{job}/")
        ],
    )?;
    Ok(())
}

/// Append a claimant's explicit troubleshooting finding to the current failure.
pub(crate) fn add_execution_finding(
    db: &rusqlite::Connection,
    job: &str,
    attempt: &str,
    finding: &str,
) -> crate::Result<()> {
    let mut matches = execution_issues(db, job, Some(attempt))?;
    let issue = matches
        .iter_mut()
        .find(|issue| issue["properties"]["status"] == "open")
        .ok_or_else(|| crate::Error::conflict("No open troubleshooting record for this Attempt"))?;
    let id = issue["historical_ref"]["id"].as_str().unwrap_or_default();
    let existing = issue["properties"]["findings"].as_array();
    if existing.is_some_and(|findings| findings.iter().any(|value| value == finding)) {
        return Ok(());
    }
    crate::error::require(
        existing.is_none_or(|findings| findings.len() < 8),
        "troubleshooting",
        "This execution already has eight troubleshooting findings",
        409,
    )?;
    db.execute(
        "INSERT INTO history_records(source,kind,id,revision,snapshot,body)
         SELECT source,kind,id,CAST(CAST(revision AS INTEGER)+1 AS TEXT),snapshot,
          json_insert(json_insert(body,'$.entity.properties.findings',json('[]')),
            '$.entity.properties.findings[#]',?)
         FROM history_records WHERE source=? AND kind='issue' AND id=? AND revision=? AND snapshot='runtime'",
        rusqlite::params![finding, EXECUTION_ISSUE_SOURCE, id,issue["historical_ref"]["revision"].as_str()],
    )?;
    Ok(())
}
