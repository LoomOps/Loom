//! Durable work reservation and evidence-fenced observations. No process/network I/O here.
use crate::{
    error::require,
    jobs::{self, Record},
    store::*,
    Error, Result,
};
use loom_domain::{Event, JobState};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub effect: String,
    pub job: String,
    pub attempt: String,
    pub stage: String,
    pub epoch: i64,
    pub revision: i64,
    pub spec: Value,
    pub folder: std::path::PathBuf,
    pub publication: bool,
    pub hook: bool,
    #[serde(default)]
    pub generation: i64,
    #[serde(default)]
    pub result: Value,
}
impl Application {
    pub fn next_work(
        &mut self,
        exclude: &std::collections::BTreeSet<String>,
    ) -> Result<Option<Work>> {
        require(
            !self.read_only,
            "read_only",
            "Worker disabled in read-only mode",
            409,
        )?;
        let config = self.config.clone();
        let bulk = self.bulk.clone();
        let tx = self.transaction()?;
        return_allowed(&tx)?;
        tx.execute("UPDATE recovery SET state='deferred',token=NULL,updated=? WHERE state IN ('waiting','claimed') AND deadline<=?",params![now(),now()])?;
        for row in rows(&tx,"SELECT * FROM rust_effects WHERE state IN ('pending','accepted','unknown') AND updated<=? ORDER BY created,id",&[&now()])? {let id=row["id"].as_str().unwrap_or("");if exclude.contains(id){continue}let work:Work=serde_json::from_str(row["request"].as_str().unwrap_or("{}"))?;let record=jobs::load(&tx,&work.job)?;if work.hook || record.aggregate.fence.attempt.as_ref().is_some_and(|a|a.as_str()==work.attempt)&&record.aggregate.fence.epoch as i64==work.epoch&&record.aggregate.fence.revision as i64==work.revision{tx.commit()?;return Ok(Some(work))}}
        for row in rows(&tx,"SELECT id FROM jobs WHERE state IN ('queued','ready','processing') AND available_at<=? ORDER BY COALESCE(queue_order,created),created,id",&[&now()])? {
   let job=row["id"].as_str().unwrap_or("");let mut record=jobs::load(&tx,job)?;
   let publication=record.aggregate.state==JobState::Processing;
   if record.aggregate.cancel_requested&&!publication{continue}
   let stage=if publication {
    let ready:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM result_outbox WHERE job=? AND next_try<=?)",params![job,now()],|r|r.get(0))?;if !ready{continue}
    let finalizer:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM finalizer_receipts WHERE job=? AND attempt=?)",params![job,record.aggregate.fence.attempt.as_ref().map(|a|a.as_str())],|r|r.get(0))?;
    if !record.aggregate.cancel_requested&&record.spec["profile"].get("finalize").is_some()&&!finalizer{"finalize"}else{"publish"}
   } else {
    let ready=jobs::readiness(&tx,&record)?;
    let prepare=record.aggregate.state==JobState::Queued&&record.spec["profile"].get("prepare").is_some();
    if ready.reason().is_some(){continue}
    if record.aggregate.state==JobState::Queued {
     let attempt=crate::store::id();tx.execute("UPDATE jobs SET attempt=? WHERE id=? AND state='queued'",params![attempt,job])?;
     tx.execute("INSERT INTO attempts VALUES(?,?,'running',?,NULL)",params![attempt,job,now()])?;
     tx.execute("INSERT INTO rust_attempt_starts VALUES(?,NULL)",[&attempt])?;
     tx.execute("UPDATE recovery SET state='executing',rounds=rounds+1,updated=? WHERE job=? AND state='pending'",params![now(),job])?;
     record=jobs::load(&tx,job)?;
    }
    if prepare{jobs::transition(&tx,&record,Event::Prepare)?;"prepare"}else{jobs::transition(&tx,&record,Event::Start)?;"run"}
   };
   if stage=="finalize"&&lane_in_use(&tx,"cpu")?{continue}
   if publication {tx.execute("UPDATE recovery SET state='executing',rounds=rounds+1,updated=? WHERE job=? AND state='pending'",params![now(),job])?;}
   let work=make_work(&tx,&config,&bulk,&record,stage,stage=="publish",false)?;tx.commit()?;return Ok(Some(work))
  }
        if let Some(row) = rows(&tx,"SELECT h.*,j.id FROM hooks h JOIN jobs j ON j.id=h.job WHERE h.state='pending' AND h.next_try<=? ORDER BY h.next_try LIMIT 100",&[&now()])?.into_iter().next() {if lane_in_use(&tx,"cpu")?{tx.commit()?;return Ok(None)}
            let mut record=jobs::load(&tx,row["job"].as_str().unwrap_or(""))?;
            if let Some(snapshot)=row["spec_snapshot"].as_str(){record.spec=serde_json::from_str(snapshot)?;}
            record.result=row["result_snapshot"].as_str().map(serde_json::from_str).transpose()?;
            if let Some(owner)=row["owner_snapshot"].as_str(){record.owner=owner.into();}
                if let Some(notify)=row["notify_snapshot"].as_str(){record.notify=notify.into();}
            record.aggregate.fence.attempt=row["attempt"].as_str().map(loom_domain::AttemptId::new).transpose()?;
            let work=make_work(&tx,&config,&bulk,&record,row["stage"].as_str().unwrap_or("on_error"),false,true)?;tx.execute("UPDATE hooks SET state='running' WHERE job=? AND attempt=? AND stage=?",params![work.job,work.attempt,work.stage])?;tx.commit()?;return Ok(Some(work));}
        tx.commit()?;
        Ok(None)
    }
    pub fn effect_accepted(&mut self, work: &Work, evidence: Value) -> Result<()> {
        let tx = self.transaction()?;
        fence(&tx, work)?;
        tx.execute("UPDATE rust_effects SET state='accepted',evidence=?,updated=? WHERE id=? AND state IN ('pending','unknown','accepted')",params![encode(&evidence)?,now(),work.effect])?;
        if !work.hook && !work.publication {
            tx.execute(
                "UPDATE rust_attempt_starts SET accepted=COALESCE(accepted,?) WHERE attempt=?",
                params![now(), work.attempt],
            )?;
        }
        if !work.publication {
            tx.execute("UPDATE stage_runs SET started=COALESCE(started,?),state='running' WHERE job=? AND attempt=? AND stage=?",params![now(),work.job,work.attempt,work.stage])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn effect_defer(&mut self, work: &Work) -> Result<()> {
        let tx = self.transaction()?;
        fence(&tx, work)?;
        tx.execute("UPDATE rust_effects SET state='pending',evidence=?,updated=? WHERE id=? AND state!='returned'",params![encode(&json!({"not_started":true,"reason":"executor_busy"}))?,now()+2.,work.effect])?;
        tx.commit()?;
        Ok(())
    }
    pub fn effect_unknown(&mut self, work: &Work) -> Result<()> {
        self.db.execute(
            "UPDATE rust_effects SET state='unknown',updated=? WHERE id=? AND state!='returned'",
            params![now(), work.effect],
        )?;
        Ok(())
    }
    pub fn stage_returned(
        &mut self,
        work: &Work,
        exit_code: i32,
        evidence: Value,
    ) -> Result<Value> {
        require(
            evidence["returned"] == true,
            "return",
            "Resource return evidence required",
            409,
        )?;
        let tx = self.transaction()?;
        let record = fence(&tx, work)?;
        let state: String = tx.query_row(
            "SELECT state FROM rust_effects WHERE id=?",
            [&work.effect],
            |r| r.get(0),
        )?;
        if state == "returned" {
            return Ok(json!({"duplicate":true}));
        }
        tx.execute(
            "UPDATE rust_effects SET state='returned',evidence=?,updated=? WHERE id=?",
            params![encode(&evidence)?, now(), work.effect],
        )?;
        tx.execute("UPDATE stage_runs SET state='returned',ended=COALESCE(ended,?) WHERE job=? AND attempt=? AND stage=?",params![now(),work.job,work.attempt,work.stage])?;
        if work.hook {
            let retries: i64 = tx.query_row(
                "SELECT retries FROM hooks WHERE job=? AND attempt=? AND stage=?",
                params![work.job, work.attempt, work.stage],
                |r| r.get(0),
            )?;
            tx.execute("UPDATE hooks SET state=?,retries=retries+1,next_try=?,error=? WHERE job=? AND attempt=? AND stage=?",params![if exit_code==0{"complete"}else if retries>=2{"failed"}else{"pending"},now()+5.,if exit_code==0{None}else{Some("callback_failed")},work.job,work.attempt,work.stage])?;
            tx.commit()?;
            return Ok(json!({"hook":work.stage,"exit_code":exit_code}));
        }
        if record.aggregate.cancel_requested {
            jobs::transition(&tx, &record, Event::ReturnCancelled)?;
            jobs::event(&tx, &record, "cancelled")?;
            enqueue_hook(&tx, &record, "on_cancel")?;
        } else if exit_code != 0 {
            let reason = if evidence["interrupted"] == true {
                "interrupted"
            } else if evidence["reason"] == "timeout" {
                "timeout"
            } else if evidence["not_started"] == true {
                "launch_failed"
            } else {
                "exit_nonzero"
            };
            if work.stage == "finalize" {
                jobs::transition(&tx, &record, Event::PublicationFailed)?;
            } else {
                jobs::transition(
                    &tx,
                    &record,
                    if reason == "interrupted" {
                        Event::Interrupt
                    } else {
                        Event::Fail
                    },
                )?;
            }
            let failure = json!({"reason":reason,"stage":work.stage,"exit_code":exit_code,"process_group_returned":true});
            tx.execute(
                "UPDATE jobs SET result=? WHERE id=?",
                params![encode(&failure)?, work.job],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO attempt_results VALUES(?,?,?)",
                params![work.job, work.attempt, encode(&failure)?],
            )?;
            open_incident(
                &tx,
                &record,
                if work.stage == "finalize" {
                    "finalization_failed"
                } else {
                    reason
                },
            )?;
            jobs::event(
                &tx,
                &record,
                if work.stage == "finalize" {
                    "result_failed"
                } else {
                    "failed"
                },
            )?;
            let failed = jobs::load(&tx, &work.job)?;
            enqueue_hook(&tx, &failed, "on_error")?;
            if work.stage != "finalize" {
                let attempts: i64 = tx.query_row(
                    "SELECT count(*) FROM attempts WHERE job=?",
                    [&work.job],
                    |r| r.get(0),
                )?;
                let rounds: i64 = tx.query_row(
                    "SELECT rounds FROM recovery WHERE job=?",
                    [&work.job],
                    |r| r.get(0),
                )?;
                let retry = record.spec["execution"]["retry_on"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|r| r == reason));
                if retry
                    && rounds < 3
                    && attempts
                        < record.spec["execution"]["max_attempts"]
                            .as_i64()
                            .unwrap_or(1)
                            .min(4)
                {
                    jobs::transition(&tx, &failed, Event::RetryExecution)?;
                    let revision = record.aggregate.fence.revision as i64 + 1;
                    tx.execute(
                        "UPDATE rust_job_fences SET revision=? WHERE job=?",
                        params![revision, work.job],
                    )?;
                    tx.execute("INSERT INTO job_revisions SELECT id,?,spec,'automatic retry',? FROM jobs WHERE id=?",params![revision,now(),work.job])?;
                    tx.execute(
                        "UPDATE jobs SET result=NULL,available_at=? WHERE id=?",
                        params![
                            now()
                                + record.spec["execution"]["retry_backoff_seconds"]
                                    .as_f64()
                                    .unwrap_or(5.),
                            work.job
                        ],
                    )?;
                    tx.execute(
                        "UPDATE recovery SET state='pending',deadline=NULL,updated=? WHERE job=?",
                        params![now(), work.job],
                    )?;
                    jobs::event(&tx, &record, "retry_scheduled")?;
                }
            }
        } else if work.stage == "prepare" {
            jobs::transition(&tx, &record, Event::Prepared)?;
        } else if work.stage == "finalize" {
            tx.execute(
                "INSERT OR IGNORE INTO finalizer_receipts VALUES(?,?,?)",
                params![work.job, work.attempt, now()],
            )?;
        } else {
            jobs::transition(&tx, &record, Event::Returned { success: true })?;
            let result = json!({"execution":{"exit_code":exit_code,"process_group_returned":true},"process_group_returned":true});
            tx.execute(
                "UPDATE jobs SET result=? WHERE id=? AND attempt=?",
                params![encode(&result)?, work.job, work.attempt],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO result_outbox(job,attempt) VALUES(?,?)",
                params![work.job, work.attempt],
            )?;
        }
        if work.stage == "run" || exit_code != 0 || record.aggregate.cancel_requested {
            tx.execute(
                "UPDATE attempts SET state=?,ended=COALESCE(ended,?) WHERE id=?",
                params![
                    if record.aggregate.cancel_requested {
                        "cancelled"
                    } else if exit_code == 0 {
                        "complete"
                    } else {
                        "failed"
                    },
                    now(),
                    work.attempt
                ],
            )?;
        }
        tx.commit()?;
        Ok(json!({"job":work.job,"attempt":work.attempt,"stage":work.stage,"exit_code":exit_code}))
    }
    pub fn publish_work(&mut self, work: &Work, manifest: Value) -> Result<Value> {
        self.publish_work_verified(work, manifest, None)
    }
    pub fn publish_work_verified(
        &mut self,
        work: &Work,
        manifest: Value,
        proofs: Option<
            &std::collections::BTreeMap<std::path::PathBuf, crate::storage::VerifiedFile>,
        >,
    ) -> Result<Value> {
        fence(&self.db, work)?;
        let result =
            self.publish_result_verified(&work.job, &work.attempt, manifest, &work.folder, proofs);
        let tx = self.transaction()?;
        let record = fence(&tx, work)?;
        match &result {
            Ok(_) => {
                tx.execute(
                    "UPDATE rust_effects SET state='returned',updated=? WHERE id=?",
                    params![now(), work.effect],
                )?;
                enqueue_hook(
                    &tx,
                    &record,
                    if record.aggregate.state == JobState::Cancelled {
                        "on_cancel"
                    } else {
                        "on_complete"
                    },
                )?;
                tx.execute("UPDATE recovery SET state='closed',token=NULL,deadline=NULL,updated=? WHERE job=?",params![now(),work.job])?;
            }
            Err(_) => {
                let retries: i64 = tx
                    .query_row(
                        "SELECT retries FROM result_outbox WHERE job=?",
                        [&work.job],
                        |r| r.get(0),
                    )
                    .optional()?
                    .unwrap_or(0);
                tx.execute(
                    "UPDATE rust_effects SET state='returned',updated=? WHERE id=?",
                    params![now(), work.effect],
                )?;
                if retries >= 2 {
                    if record.aggregate.state == JobState::Processing {
                        jobs::transition(&tx, &record, Event::PublicationFailed)?;
                    }
                    tx.execute("UPDATE jobs SET result=json_remove(COALESCE(result,'{}'),'$.artifact_ready') WHERE id=?",[&work.job])?;
                    open_incident(&tx, &record, "finalization_failed")?;
                    jobs::event(&tx, &record, "result_failed")?;
                } else {
                    tx.execute(
                        "UPDATE result_outbox SET retries=retries+1,next_try=? WHERE job=?",
                        params![now() + 5., work.job],
                    )?;
                }
            }
        }
        tx.commit()?;
        result
    }
    pub fn publication_unreadable(&mut self, work: &Work) -> Result<()> {
        let _ = self.publish_work(work, json!({}));
        Ok(())
    }
    pub fn cancelled(&self, work: &Work) -> Result<bool> {
        Ok(!work.hook && jobs::load(&self.db, &work.job)?.aggregate.cancel_requested)
    }
}
fn make_work(
    db: &rusqlite::Connection,
    config: &Config,
    bulk: &std::path::Path,
    record: &Record,
    stage: &str,
    publication: bool,
    hook: bool,
) -> Result<Work> {
    let attempt = record
        .aggregate
        .fence
        .attempt
        .as_ref()
        .ok_or_else(|| Error::conflict("Attempt not claimed"))?
        .as_str()
        .to_string();
    let (root, _) = crate::storage::output_store(
        config,
        bulk,
        record.spec["output_storage"].as_str().unwrap_or("data/0"),
        record.spec["_output_storage_id"].as_str(),
    )?;
    let mut folder = root
        .join("runs")
        .join(record.aggregate.id.as_str())
        .join(&attempt);
    let generation = if hook {
        db.query_row(
            "SELECT retries FROM hooks WHERE job=? AND attempt=? AND stage=?",
            params![record.aggregate.id.as_str(), attempt, stage],
            |r| r.get::<_, i64>(0),
        )?
    } else if stage == "finalize" {
        record.aggregate.fence.revision as i64
    } else {
        0
    };
    if hook {
        folder = folder
            .join("callbacks")
            .join(stage)
            .join(generation.to_string());
    }
    let effect = id();
    let work = Work {
        effect: effect.clone(),
        job: record.aggregate.id.as_str().into(),
        attempt,
        stage: stage.into(),
        epoch: record.aggregate.fence.epoch as i64,
        revision: record.aggregate.fence.revision as i64,
        spec: record.spec.clone(),
        folder,
        publication,
        hook,
        generation,
        result: record.result.clone().unwrap_or(Value::Null),
    };
    db.execute(
        "INSERT INTO rust_effects VALUES(?,?,?,?,?,'pending',?,NULL,?,?)",
        params![
            effect,
            work.job,
            work.attempt,
            stage,
            work.epoch,
            encode(&serde_json::to_value(&work)?)?,
            now(),
            now()
        ],
    )?;
    if !publication {
        db.execute("INSERT INTO stage_runs VALUES(?,?,?,NULL,NULL,'pending') ON CONFLICT(job,attempt,stage) DO UPDATE SET started=NULL,ended=NULL,state='pending'",params![work.job,work.attempt,stage])?;
    }
    Ok(work)
}
fn fence(db: &rusqlite::Connection, work: &Work) -> Result<Record> {
    let mut record = jobs::load(db, &work.job)?;
    if work.hook {
        let hook = rows(
            db,
            "SELECT * FROM hooks WHERE job=? AND attempt=? AND stage=?",
            &[&work.job, &work.attempt, &work.stage],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::conflict("Callback owner changed"))?;
        require(
            hook["state"] == "running" || hook["state"] == "complete" || hook["state"] == "failed",
            "hook",
            "Callback owner changed",
            409,
        )?;
        record.aggregate.fence.attempt = Some(loom_domain::AttemptId::new(&work.attempt)?);
        record.spec = work.spec.clone();
        return Ok(record);
    }
    require(
        record
            .aggregate
            .fence
            .attempt
            .as_ref()
            .is_some_and(|a| a.as_str() == work.attempt)
            && record.aggregate.fence.epoch as i64 == work.epoch
            && record.aggregate.fence.revision as i64 == work.revision,
        "attempt",
        "Stale execution observation",
        409,
    )?;
    Ok(record)
}
pub fn open_incident(db: &rusqlite::Connection, record: &Record, reason: &str) -> Result<()> {
    db.execute("INSERT INTO recovery(job,state,rounds,deadline,reason,updated) VALUES(?,'waiting',0,?,?,?) ON CONFLICT(job) DO UPDATE SET state=CASE WHEN rounds>=3 THEN 'exhausted' ELSE 'waiting' END,token=NULL,agent=NULL,deadline=excluded.deadline,reason=excluded.reason,updated=excluded.updated",params![record.aggregate.id.as_str(),now()+900.,reason,now()])?;
    Ok(())
}
pub(crate) fn enqueue_hook(db: &rusqlite::Connection, record: &Record, stage: &str) -> Result<()> {
    if record.spec["profile"].get(stage).is_none() {
        return Ok(());
    }
    db.execute("INSERT OR IGNORE INTO hooks(job,attempt,stage,state,spec_snapshot,result_snapshot,owner_snapshot,notify_snapshot) VALUES(?,?,?,'pending',?,?,?,?)",params![record.aggregate.id.as_str(),record.aggregate.fence.attempt.as_ref().map(|a|a.as_str()),stage,encode(&record.spec)?,record.result.as_ref().map(encode).transpose()?,record.owner,record.notify])?;
    Ok(())
}

impl Application {
    pub fn request_for(&self, work: &Work) -> Result<Value> {
        let record = jobs::load(&self.db, &work.job)?;
        require(
            work.hook
                || (record
                    .aggregate
                    .fence
                    .attempt
                    .as_ref()
                    .is_some_and(|a| a.as_str() == work.attempt)
                    && record.aggregate.fence.revision as i64 == work.revision),
            "attempt",
            "Stale request owner",
            409,
        )?;
        let profile = &work.spec["profile"];
        let mut params = work.spec["params"].clone();
        let mut inputs = serde_json::Map::new();
        for (name, prefix) in profile["inputs"].as_object().into_iter().flatten() {
            if work.hook {
                continue;
            }
            if work.stage == "prepare"
                && profile["prepare"]["inputs"]
                    .as_array()
                    .is_some_and(|a| !a.iter().any(|v| v == name))
            {
                continue;
            }
            let item = params[name].clone();
            let locator = if item["artifact"].is_object() {
                self.artifact_input(&item["artifact"], item["member"].as_str())?
            } else if item["job"].is_string() {
                let upstream = item["job"].as_str().unwrap();
                let entity = item["entity"]
                    .as_str()
                    .ok_or_else(|| Error::input("Upstream entity required"))?;
                let root = rows(
                    &self.db,
                    "SELECT request FROM job_workflows WHERE id=?",
                    &[&upstream],
                )?
                .into_iter()
                .next();
                let (upstream, entity) = if let Some(root) = root {
                    let graph: Value =
                        serde_json::from_str(root["request"].as_str().unwrap_or("{}"))?;
                    let alias = &graph["outputs"][entity];
                    let node = alias["node"]
                        .as_str()
                        .ok_or_else(|| Error::input("Unknown logical Job output"))?;
                    let child: String = self.db.query_row(
                        "SELECT job FROM workflow_nodes WHERE root=? AND node=?",
                        params![upstream, node],
                        |r| r.get(0),
                    )?;
                    (child, alias["entity"].as_str().unwrap_or("").to_string())
                } else {
                    (upstream.into(), entity.into())
                };
                let up = jobs::load(&self.db, &upstream)?;
                require(
                    loom_domain::dependency_available(
                        up.aggregate.state,
                        up.result
                            .as_ref()
                            .is_some_and(|r| r["artifact_ready"] == true),
                    ),
                    "input",
                    "Upstream output not available",
                    409,
                )?;
                let output = up
                    .result
                    .as_ref()
                    .and_then(|r| r["entities"].as_array())
                    .and_then(|a| a.iter().find(|e| e["id"] == entity))
                    .ok_or_else(|| Error::input("Upstream output not found"))?;
                if let Some(member) = item["member"].as_str() {
                    self.artifact_input(
                        &json!({"id":format!("{upstream}.{entity}"),"revision":1}),
                        Some(member),
                    )?
                } else {
                    let mut locator = output["locator"].clone();
                    locator["sha256"] = output["sha256"].clone();
                    locator
                }
            } else {
                item
            };
            let path = crate::storage::locator(&self.config, &self.bulk, &locator)?;
            let expected = locator["sha256"]
                .as_str()
                .ok_or_else(|| Error::input("Input hash required"))?;
            let selector = if locator["storage_id"] == self.config.storage_id {
                "data/0".to_string()
            } else {
                self.config
                    .data_stores
                    .iter()
                    .find(|(_, s)| s["storage_id"] == locator["storage_id"])
                    .map(|(k, _)| k.clone())
                    .ok_or_else(|| Error::input("Input store unknown"))?
            };
            let (root, _) = crate::storage::store(
                &self.config,
                &self.bulk,
                &selector,
                locator["storage_id"].as_str(),
            )?;
            let prefix = crate::storage::prefix(
                &root,
                prefix
                    .as_str()
                    .ok_or_else(|| Error::input("Input prefix required"))?,
            )?;
            require(
                path.starts_with(prefix),
                "input",
                "Input outside Profile prefix",
                403,
            )?;
            inputs.insert(
                name.clone(),
                json!({"path":path,"storage":selector,"ref":locator["ref"],"sha256":expected}),
            );
            params[name] = json!(format!("/inputs/{name}"));
        }
        let mut runner = if work.stage == "run" {
            profile["runtime"].clone()
        } else {
            profile[&work.stage].clone()
        };
        let mut tool_binaries = serde_json::Map::new();
        for (alias, pin) in profile["tool_pins"].as_object().into_iter().flatten() {
            if let Some(build) = pin.get("producer_build").filter(|b| !b.is_null()) {
                let locator = &build["binary"];
                let path = crate::storage::locator(&self.config, &self.bulk, locator)?;
                let selector = if locator["storage_id"] == self.config.storage_id {
                    "data/0".to_string()
                } else {
                    self.config
                        .data_stores
                        .iter()
                        .find(|(_, s)| s["storage_id"] == locator["storage_id"])
                        .map(|(k, _)| k.clone())
                        .ok_or_else(|| Error::input("Tool binary store unknown"))?
                };
                let name = format!("loom-tool-{alias}");
                require(
                    !inputs.contains_key(&name),
                    "input",
                    "Reserved Tool executable input alias",
                    400,
                )?;
                inputs.insert(name.clone(),json!({"path":path,"storage":selector,"ref":locator["ref"],"sha256":locator["sha256"]}));
                tool_binaries.insert(alias.clone(), json!(format!("/inputs/{name}")));
            }
        }
        if let Some(build) = runner.get("producer_build") {
            let locator = &build["binary"];
            let path = crate::storage::locator(&self.config, &self.bulk, locator)?;
            let selector = if locator["storage_id"] == self.config.storage_id {
                "data/0".to_string()
            } else {
                self.config
                    .data_stores
                    .iter()
                    .find(|(_, s)| s["storage_id"] == locator["storage_id"])
                    .map(|(k, _)| k.clone())
                    .ok_or_else(|| Error::input("Producer store unknown"))?
            };
            require(
                !inputs.contains_key("loom-producer"),
                "input",
                "Reserved producer input alias",
                400,
            )?;
            inputs.insert("loom-producer".into(),json!({"path":path,"storage":selector,"ref":locator["ref"],"sha256":locator["sha256"]}));
        }
        if work.spec["kind"] == "cpu.smoke" {
            runner = json!({"builtin":"cpu.smoke","lane":"cpu"});
        }
        Ok(
            json!({"schema":"loom-run-request/v1","job":work.job,"job_id":work.job,"attempt":work.attempt,"attempt_id":work.attempt,"stage":work.stage,"params":params,"output_dir":"/work","profile":{"revision":profile["revision"],"sha256":profile["sha256"]},"event_id":format!("{}.{}.{}",work.job,work.attempt,work.stage),"result":work.result,"execution":work.spec["execution"],"tools":profile["tool_pins"],"tool_binaries":tool_binaries,"steps":profile["steps"],"runner":runner,"verified_inputs":inputs,"component_path":profile["component_path"],"source_pins":profile["source_pins"]}),
        )
    }
}

pub fn lane_in_use(db: &rusqlite::Connection, lane: &str) -> Result<bool> {
    Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM rust_effects WHERE state IN ('pending','accepted','unknown') AND stage!='publish' AND CASE WHEN stage='run' THEN COALESCE(json_extract(request,'$.spec.profile.runtime.lane'),'cpu') ELSE 'cpu' END=?)",[lane],|r|r.get(0))?)
}
