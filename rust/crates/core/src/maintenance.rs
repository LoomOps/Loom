//! Durable, bounded maintenance commands. They never enter the scientific Job queue.
use crate::{auth::Actor, error::require, index_store, items, log_index, store::*, Error, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

impl Application {
    pub fn maintenance(&mut self, actor: &Actor, action: &str, args: &Value) -> Result<Value> {
        if action == "catalog_task" {
            return self.catalog_task(actor, args["id"].as_str().unwrap_or(""));
        }
        if action == "retry_catalog_task" {
            let id = args["id"]
                .as_str()
                .ok_or_else(|| Error::input("Task id required"))?;
            let row = rows(
                &self.db,
                "SELECT state,context,kind FROM catalog_tasks WHERE id=? AND actor=?",
                &[&id, &actor.id],
            )?
            .into_iter()
            .next()
            .ok_or_else(|| Error::new("task", "Maintenance task not found", 404))?;
            require(
                row["state"] == "failed",
                "task_state",
                "Only failed tasks may be retried",
                409,
            )?;
            require(
                row["context"] == self.maintenance_context()?,
                "task_stale",
                "Configured roots changed; create a new task",
                409,
            )?;
            require(row["kind"]!="rebuild" || !self.db.query_row("SELECT EXISTS(SELECT 1 FROM catalog_tasks WHERE kind='rebuild' AND state='pending')",[],|r|r.get::<_,bool>(0))?,"busy","Another rebuild is pending",409)?;
            let tx = self.transaction()?;
            tx.execute(
                "UPDATE catalog_tasks SET state='pending',error=NULL,updated=? WHERE id=?",
                params![now(), id],
            )?;
            tx.commit()?;
            return self.catalog_task(actor, id);
        }
        let key = args["key"]
            .as_str()
            .ok_or_else(|| Error::input("Stable maintenance key required"))?;
        let kind = if action == "rebuild_search" {
            "rebuild"
        } else if action == "prune_logs" {
            "retention"
        } else if action == "prune_search" {
            "cleanup"
        } else {
            return Err(Error::input("Unknown maintenance action"));
        };
        let old: Option<(String, String)> = self
            .db
            .query_row(
                "SELECT id,request FROM catalog_tasks WHERE actor=? AND request_key=?",
                params![actor.id, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((id, request)) = old {
            require(
                request == encode(args)?,
                "idempotency",
                "Maintenance key reused for a changed request",
                409,
            )?;
            return self.catalog_task(actor, &id);
        }
        let mut retained = vec![];
        if kind == "retention" {
            let streams = args["streams"]
                .as_array()
                .ok_or_else(|| Error::input("Explicit stream/generation selections required"))?;
            require(
                !streams.is_empty() && streams.len() <= 200,
                "retention",
                "Select at most 200 closed streams",
                400,
            )?;
            for v in streams {
                let (stream, work) = log_index::source(self, &v["stream_id"])?;
                require(
                    stream["source_state"] == "returned",
                    "retention_busy",
                    "Return the execution owner before expiring log bytes",
                    409,
                )?;
                let generation = v["source_generation"]
                    .as_str()
                    .ok_or_else(|| Error::input("Pin exact source generation"))?;
                let old: bool = self.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM log_retention WHERE stream=? AND generation=?)",
                    params![items::parse(&v["stream_id"])?, generation],
                    |r| r.get(0),
                )?;
                if old {
                    continue;
                }
                let (file, state) = log_index::read_generation(self, &work, Some(generation))?;
                require(
                    state["closed"] == true,
                    "retention_busy",
                    "Live log bytes cannot be expired",
                    409,
                )?;
                retained.push((
                    items::parse(&v["stream_id"])?,
                    generation.to_string(),
                    log_index::retained_length(&file, &state)?,
                ));
            }
        } else if kind == "rebuild" {
            require(!self.db.query_row("SELECT EXISTS(SELECT 1 FROM catalog_tasks WHERE kind='rebuild' AND state='pending')",[],|r|r.get::<_,bool>(0))?,"busy","One search generation rebuild is already pending",409)?;
        }
        let id = items::decimal(&items::allocate())?;
        let ctx = self.maintenance_context()?;
        let tx = self.transaction()?;
        active(&tx)?;
        for (stream, generation, end) in retained {
            tx.execute(
                "INSERT OR IGNORE INTO log_retention VALUES(?,?,0,?,?,?)",
                params![
                    stream,
                    generation,
                    i64::try_from(end).map_err(|_| Error::input("Log offset overflow"))?,
                    args["reason"].as_str().unwrap_or("explicit retention"),
                    now()
                ],
            )?;
        }
        if kind == "retention" {
            tx.execute(
                "UPDATE catalog_meta SET access_epoch=access_epoch+1 WHERE id=1",
                [],
            )?;
        }
        tx.execute(
            "INSERT INTO catalog_tasks VALUES(?,?,?,?,?,?,'pending','{}',NULL,NULL,?,?)",
            params![id, actor.id, key, kind, encode(args)?, ctx, now(), now()],
        )?;
        tx.commit()?;
        self.catalog_task(actor, &id)
    }
    fn catalog_task(&self, actor: &Actor, id: &str) -> Result<Value> {
        let row=rows(&self.db,"SELECT id,kind,state,progress,receipt,error,created,updated FROM catalog_tasks WHERE id=? AND actor=?",&[&id,&actor.id])?.into_iter().next().ok_or_else(||Error::new("task","Maintenance task not found",404))?;
        let mut value = row;
        for k in ["progress", "receipt"] {
            if let Some(s) = value[k].as_str() {
                value[k] = serde_json::from_str(s)?
            }
        }
        Ok(value)
    }
    pub fn maintenance_context(&self) -> Result<String> {
        Ok(hash(encode(&json!([
            self.config.machine_id,
            self.config.storage_id,
            self.config.data_stores,
            self.local,
            self.bulk,
            index_store::directory(&self.local, &self.config)
        ]))?))
    }
    pub fn pending_maintenance(&self) -> Result<Option<Value>> {
        Ok(rows(
            &self.db,
            "SELECT * FROM catalog_tasks WHERE state='pending' ORDER BY created,id LIMIT 1",
            &[],
        )?
        .into_iter()
        .next())
    }
    pub fn advance_maintenance(
        &mut self,
        id: &str,
        progress: &Value,
        error: Option<&str>,
    ) -> Result<Value> {
        let row = rows(
            &self.db,
            "SELECT * FROM catalog_tasks WHERE id=? AND state='pending'",
            &[&id],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::conflict("Maintenance task is no longer pending"))?;
        let error = if row["context"] != self.maintenance_context()? {
            Some("task_stale")
        } else {
            error
        };
        let mut complete = progress["complete"] == true;
        if complete && error.is_none() && row["kind"] == "rebuild" {
            complete = index_store::activate(self, id)?;
        }
        let tx = self.transaction()?;
        tx.execute("UPDATE catalog_tasks SET progress=?,state=?,receipt=?,error=?,updated=? WHERE id=? AND state='pending'",params![encode(progress)?,if error.is_some(){"failed"}else if complete{"complete"}else{"pending"},if complete{Some(encode(progress)?)}else{None},error,now(),id])?;
        tx.commit()?;
        Ok(json!({"id":id,"complete":complete}))
    }
}
