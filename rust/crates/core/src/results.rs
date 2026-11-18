//! Immutable output registration and additive historical current-availability evidence.
use crate::{
    auth::Actor, contracts::Target, error::require, jobs, storage, store::*, Error, Result,
};
use loom_domain::{Event, JobState};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
pub fn verify_target(db: &Connection, target: &Target, config: &Config) -> Result<()> {
    let v = &target.identity;
    let fields: &[&str] = match target.kind.as_str() {
        "job" => &["id"],
        "scenario" | "entity" | "protocol_artifact" => &["id", "revision"],
        "historical" => &["source", "kind", "id", "revision", "snapshot"],
        "profile" if v["kind"].is_string() => &["kind", "sha256"],
        "component" | "tool" | "profile" => &["owner", "id", "revision"],
        "runtime" => &["id", "revision", "sha256"],
        _ => &[],
    };
    require(
        v.as_object()
            .is_some_and(|o| o.keys().all(|k| fields.contains(&k.as_str())))
            && v.get("sha256").is_none_or(Value::is_string),
        "target",
        "Unexpected exact target identity fields",
        400,
    )?;
    let exists=match target.kind.as_str() {
  "scenario"=>db.query_row("SELECT EXISTS(SELECT 1 FROM scenario_revisions WHERE scenario_id=? AND revision=?)",params![v["id"].as_str(),v["revision"].as_i64()],|r|r.get(0))?,
  "job"=>{let id=v["id"].as_str().ok_or_else(||Error::input("Job target ID required"))?;db.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=? UNION SELECT 1 FROM job_workflows WHERE id=?)",params![id,id],|r|r.get(0))?},
  "entity"|"protocol_artifact"=>db.query_row("SELECT EXISTS(SELECT 1 FROM all_entity_revisions WHERE id=? AND revision=?)",params![v["id"].as_str(),v["revision"].as_i64()],|r|r.get(0))?,
  "historical"=>db.query_row("SELECT EXISTS(SELECT 1 FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?)",params![v["source"].as_str(),v["kind"].as_str(),v["id"].as_str(),v["revision"].as_str(),v["snapshot"].as_str()],|r|r.get(0))?,
  "profile" if v["kind"].is_string()=>{v["sha256"].is_string()&&crate::software::exact_profile(db,config,v).is_ok()},
  "component"|"tool"|"profile"=>{let table=match target.kind.as_str(){"component"=>"components","tool"=>"tools",_=>"execution_profiles"};db.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE owner=? AND id=? AND revision=?)"),params![v["owner"].as_str(),v["id"].as_str(),v["revision"].as_i64()],|r|r.get(0))?},
  "runtime"=>db.query_row("SELECT EXISTS(SELECT 1 FROM rust_runtime_observation o,json_each(o.value,'$.runtimes') r WHERE o.observed>? AND json_extract(r.value,'$.id')=? AND json_extract(r.value,'$.revision')=? AND (? IS NULL OR json_extract(r.value,'$.sha256')=?))",params![now()-30.,v["id"].as_str(),v["revision"].as_i64(),v["sha256"].as_str(),v["sha256"].as_str()],|r|r.get(0))?,
  _=>false,
 };
    require(exists, "target", "Exact target identity not found", 404)
}
// File bindings extend an exact historical record pin with a member selector.
// Task/lineage targets remain record identities and cannot carry these fields.
pub(crate) fn verify_historical_binding_target(
    db: &Connection,
    identity: &Value,
    config: &Config,
) -> Result<()> {
    let mut record = identity.clone();
    if let Some(fields) = record.as_object_mut() {
        for selector in ["file", "component"] {
            if let Some(value) = fields.remove(selector) {
                require(
                    value.is_string(),
                    "identity",
                    "Historical member selector must be a string",
                    400,
                )?;
            }
        }
    }
    verify_target(
        db,
        &Target {
            kind: "historical".into(),
            identity: record,
        },
        config,
    )
}
impl Application {
    // Display names are current curation, never a rewrite of an accepted source.
    pub(crate) fn output_label(&self, kind: &str, id: &str, recorded: &Value) -> Result<Value> {
        let overlay: Option<String> = self
            .db
            .query_row(
                "SELECT presentation FROM research_overlays WHERE kind=? AND id=?",
                params![kind, id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(overlay
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|p| p["label"].as_str().map(|s| json!(s)))
            .unwrap_or_else(|| recorded.clone()))
    }
    pub fn result_action(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        if matches!(
            action,
            "get_report_evidence"
                | "report_contracts"
                | "get_report_contract"
                | "register_report_contract"
                | "validate_report_contract"
                | "report_definitions"
                | "get_report_definition"
                | "register_report_definition"
                | "read_report"
        ) {
            return self.reporting_action(actor, action, args);
        }
        if matches!(
            action,
            "metadata_templates"
                | "get_metadata_template"
                | "register_metadata_template"
                | "validate_metadata"
                | "adopt_metadata"
        ) {
            return self.metadata_template_action(actor, action, args);
        }
        match action {
            "list" => self.results_listing(&args),
            "get" => {
                let id = args["id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Entity ID required"))?;
                let rev = args["revision"].as_i64().unwrap_or(1);
                let text: String = self.db.query_row(
                    "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
                    params![id, rev],
                    |r| r.get(0),
                )?;
                let mut value: Value = serde_json::from_str(&text)?;
                let display_label = self.output_label("entity", id, &value["label"])?;
                if display_label.is_string() {
                    value["label"] = display_label;
                }
                let mut projected = value.clone();
                projected["loom_registry"] = json!({"id":id,"revision":rev});
                crate::metadata_templates::attach(self, &mut projected)?;
                for key in [
                    "robot",
                    "research_summary",
                    "metadata_status",
                    "metadata_provenance",
                    "label",
                ] {
                    if let Some(v) = projected.get(key) {
                        value[key] = v.clone();
                    }
                }
                if value["file_set"]["entries"]
                    .as_u64()
                    .is_some_and(|n| n > 200)
                {
                    return Err(Error::new("paging_required",format!("Entity {id} revision {rev} has a large inventory. Use results.resolve_item with its exact entity reference, then results.item_files."),409));
                }
                if value["file_set"].is_object() {
                    value["components"] = json!(crate::file_set::Members::open(
                        self,
                        &value["file_set"],
                        None
                    )?
                    .map(|e| e.map(
                        |e| json!({"ref":e["entry_key"],"size":e["size"],"sha256":e["sha256"]})
                    ))
                    .collect::<Result<Vec<_>>>()?);
                    value["components_complete"] = json!(true);
                }
                value["locations"]=json!(rows(&self.db,"SELECT component,machine_id,storage_id,ref,status FROM all_entity_locations WHERE id=? AND revision=? ORDER BY component,ref",&[&id,&rev])?);
                if value["kind"] == "dataset" {
                    value["dataset_lineage"] = json!({"schema":"loom-dataset-lineage/v1","parents":crate::dataset_lineage::declared(self,&Target{kind:"entity".into(),identity:json!({"id":id,"revision":rev})})?});
                    value["loom_registry"] = json!({"id":id,"revision":rev});
                    crate::dataset_organization::attach_context(self, &mut value)?;
                    crate::dataset_organization::attach(self, &mut value)?;
                }
                let owner: String = self.db.query_row(
                    "SELECT owner FROM all_entity_revisions WHERE id=? AND revision=?",
                    params![id, rev],
                    |r| r.get(0),
                )?;
                self.decorate_owner("entity", id, &owner, value)
            }
            "history" => {
                let kind = args["kind"].as_str().unwrap_or("");
                let limit = args["limit"].as_i64().unwrap_or(100).clamp(1, 100);
                let offset = args["offset"].as_i64().unwrap_or(0).max(0);
                Ok(json!(rows(&self.db,"SELECT source,kind,id,revision,snapshot FROM history_records WHERE (?='' OR kind=?) ORDER BY kind,id,source,revision,snapshot LIMIT ? OFFSET ?",&[&kind,&kind,&limit,&offset])?))
            }
            "historical" => self.history_record(&args),
            "bind_historical_location" => self.bind_historical(actor, args),
            "reconnect" => self.reconnect(actor, args),
            "reindex" => {
                let id = args["id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Job ID required"))?;
                let record = jobs::load(&self.db, id)?;
                let attempt = args["attempt"]
                    .as_str()
                    .ok_or_else(|| Error::input("Attempt required"))?;
                require(
                    record
                        .aggregate
                        .fence
                        .attempt
                        .as_ref()
                        .is_some_and(|a| a.as_str() == attempt)
                        && matches!(
                            record.aggregate.state,
                            JobState::Complete | JobState::ResultFailed
                        ),
                    "attempt",
                    "Only current returned results may be reindexed",
                    409,
                )?;
                let tx = self.transaction()?;
                active(&tx)?;
                tx.execute(
                    "UPDATE jobs SET state='processing' WHERE id=? AND attempt=?",
                    params![id, attempt],
                )?;
                tx.execute("INSERT INTO result_outbox(job,attempt) VALUES(?,?) ON CONFLICT(job) DO UPDATE SET retries=0,next_try=0",params![id,attempt])?;
                tx.commit()?;
                Ok(json!({"id":id,"attempt":attempt,"state":"processing"}))
            }
            "refresh_metadata" => self.refresh_research_metadata(actor, args),
            "link_dataset" => self.link_dataset(actor, args),
            "organize_dataset" => self.organize_dataset(actor, args),
            "annotate" => {
                let id = args["id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Job ID required"))?;
                if !self.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM job_workflows WHERE id=?)",
                    [id],
                    |r| r.get::<_, bool>(0),
                )? {
                    jobs::load(&self.db, id)?;
                }
                let annotation = args["annotation_id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Annotation ID required"))?;
                let payload = json!({"id":annotation,"author":actor.id,"kind":"operator_note","reason":args["reason"],"evidence":args["evidence"]});
                require(
                    args["reason"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty() && s.len() <= 500)
                        && serde_json::to_vec(&payload)?.len() <= 8192,
                    "annotation",
                    "Annotation exceeds its structured evidence bound",
                    400,
                )?;
                let tx = self.transaction()?;
                let old: Option<String> = tx
                    .query_row(
                        "SELECT payload FROM job_annotations WHERE job=? AND id=?",
                        params![id, annotation],
                        |r| r.get(0),
                    )
                    .optional()?;
                let mut payload = payload;
                if let Some(old) = old {
                    let stored: Value = serde_json::from_str(&old)?;
                    if let Some(created) = stored.get("created") {
                        payload["created"] = created.clone();
                    }
                    require(
                        stored == payload,
                        "annotation",
                        "Annotation is immutable",
                        409,
                    )?;
                } else {
                    active(&tx)?;
                    payload["created"] = json!(now());
                    tx.execute(
                        "INSERT INTO job_annotations VALUES(?,?,?)",
                        params![id, annotation, encode(&payload)?],
                    )?;
                }
                tx.commit()?;
                Ok(payload)
            }
            "assign_owner" => {
                let id = args["id"]
                    .as_str()
                    .ok_or_else(|| Error::input("Entity ID required"))?;
                let fallback: String = self.db.query_row(
                    "SELECT owner FROM all_entity_revisions WHERE id=? ORDER BY revision DESC LIMIT 1",
                    [id],
                    |r| r.get(0),
                )?;
                self.assign_responsibility(
                    actor,
                    "entity",
                    id,
                    args["owner"]
                        .as_str()
                        .ok_or_else(|| Error::input("Owner required"))?,
                    &fallback,
                )
            }
            _ => Err(Error::input("Unknown result action")),
        }
    }
    fn bind_historical(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        if let Some(result) = replay(
            &self.db,
            actor,
            args["key"]
                .as_str()
                .ok_or_else(|| Error::input("Key required"))?,
            "historical.bind",
            &args,
        )? {
            return Ok(result);
        }
        let identity = args["identity"].clone();
        verify_historical_binding_target(&self.db, &identity, &self.config)?;
        let path = storage::locator(&self.config, &self.bulk, &args["locator"])?;
        let proof = self.verified_file(&path)?;
        let (size, sha) = (proof.size, proof.sha256.clone());
        require(
            args["sha256"] == sha && args["size"].as_u64() == Some(size),
            "hash",
            "Current bytes differ from submitted evidence",
            409,
        )?;
        validate_historical_digest(&self.db, &identity, &sha)?;
        let key = args["key"]
            .as_str()
            .ok_or_else(|| Error::input("Key required"))?;
        let tx = self.transaction()?;
        if let Some(r) = replay(&tx, actor, key, "historical.bind", &args)? {
            return Ok(r);
        }
        active(&tx)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM historical_bindings WHERE identity=? AND locator=?",
                params![encode(&identity)?, encode(&args["locator"])?],
                |r| r.get(0),
            )
            .optional()?;
        let id = existing.unwrap_or_else(id);
        tx.execute(
            "INSERT OR IGNORE INTO historical_bindings VALUES(?,?,?,?,?,?,?)",
            params![
                id,
                encode(&identity)?,
                encode(&args["locator"])?,
                sha,
                i64::try_from(size).map_err(|_| Error::input("File size exceeds storage range"))?,
                now(),
                actor.id
            ],
        )?;
        let result = json!({"id":id,"identity":identity,"locator":args["locator"],"sha256":sha,"size":size,"current_availability":"available","recorded_availability":"preserved"});
        receipt(&tx, actor, key, "historical.bind", &args, &result)?;
        proof.revalidate()?;
        tx.commit()?;
        Ok(result)
    }
    fn reconnect(&mut self, _actor: &Actor, args: Value) -> Result<Value> {
        let id = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Entity ID required"))?;
        let revision = args["revision"].as_i64().unwrap_or(1);
        let component = args["component"]
            .as_str()
            .ok_or_else(|| Error::input("Component ref required"))?;
        let reference = args["ref"]
            .as_str()
            .ok_or_else(|| Error::input("Location ref required"))?;
        let text: String = self.db.query_row(
            "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=?",
            params![id, revision],
            |r| r.get(0),
        )?;
        let manifest: Value = serde_json::from_str(&text)?;
        let expected = manifest["components"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["ref"] == component))
            .ok_or_else(|| Error::input("Unknown output component"))?;
        let (root, store) = storage::store(
            &self.config,
            &self.bulk,
            args["storage"].as_str().unwrap_or("data/0"),
            None,
        )?;
        let path = storage::relative(&root, reference)?;
        let proof = self.verified_file(&path)?;
        let (size, sha) = (proof.size, proof.sha256.clone());
        require(
            expected["sha256"] == sha && expected["size"].as_u64() == Some(size),
            "hash",
            "Reconnect bytes differ from immutable output",
            409,
        )?;
        let machine = self.config.machine_id.clone();
        let tx = self.transaction()?;
        active(&tx)?;
        tx.execute(
            "INSERT OR IGNORE INTO entity_locations VALUES(?,?,?,?,?,?,'available')",
            params![id, revision, component, machine, store, reference],
        )?;
        proof.revalidate()?;
        tx.commit()?;
        Ok(
            json!({"id":id,"revision":revision,"component":component,"ref":reference,"status":"available"}),
        )
    }
    /// Long reads/hashes happen before the fenced publication transaction.
    pub fn publish_result(
        &mut self,
        job: &str,
        attempt: &str,
        manifest: Value,
        folder: &std::path::Path,
    ) -> Result<Value> {
        self.publish_result_verified(job, attempt, manifest, folder, None)
    }
    pub(crate) fn publish_result_verified(
        &mut self,
        job: &str,
        attempt: &str,
        manifest: Value,
        folder: &std::path::Path,
        proofs: Option<&std::collections::BTreeMap<std::path::PathBuf, storage::VerifiedFile>>,
    ) -> Result<Value> {
        let row = jobs::load(&self.db, job)?;
        require(
            row.aggregate
                .fence
                .attempt
                .as_ref()
                .is_some_and(|a| a.as_str() == attempt)
                && row.aggregate.state == JobState::Processing,
            "attempt",
            "Stale publication owner",
            409,
        )?;
        if row.aggregate.cancel_requested {
            let tx = self.transaction()?;
            let current = jobs::load(&tx, job)?;
            require(
                current.aggregate.fence.matches(&row.aggregate.fence),
                "fence",
                "Publication owner changed",
                409,
            )?;
            jobs::transition(&tx, &current, Event::Publish)?;
            jobs::event(&tx, &current, "cancelled")?;
            tx.execute(
                "DELETE FROM result_outbox WHERE job=? AND attempt=?",
                params![job, attempt],
            )?;
            tx.commit()?;
            return Ok(json!({"id":job,"state":"cancelled"}));
        }
        require(
            manifest.as_object().is_some_and(|o| {
                o.len() == 2 && o.contains_key("entities") && o.contains_key("metrics")
            }),
            "manifest",
            "Manifest requires entities and metrics",
            400,
        )?;
        let entities = manifest["entities"]
            .as_array()
            .ok_or_else(|| Error::input("Result entities required"))?;
        require(
            !entities.is_empty() && entities.len() <= 1024,
            "manifest",
            "At least one and at most 1024 outputs required",
            400,
        )?;
        if row.spec["_reporting_required"] == true
            && entities
                .iter()
                .any(|e| matches!(e["kind"].as_str(), Some("model" | "dataset" | "experiment")))
        {
            require(row.spec["profile"]["reporting_policy"]==crate::reporting::POLICY,"report_profile_legacy","New scientific outputs require a frozen reporting contract; declare scientific outputs before submission",409)?;
        }
        let science_plan = crate::plans::scientific_plan(&self.db, job)?;
        let missing:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM workflow_nodes n JOIN job_workflows w ON w.id=n.root JOIN json_each(w.request,'$.outcome.deliverables') d WHERE n.job=? AND json_extract(d.value,'$.from.node')=n.node AND NOT EXISTS(SELECT 1 FROM json_each(?,'$.entities') e WHERE json_extract(e.value,'$.id')=json_extract(d.value,'$.from.entity') AND json_extract(e.value,'$.kind')=json_extract(d.value,'$.kind')))",params![job,encode(&manifest)?],|r|r.get(0))?;
        require(
            !missing,
            "deliverable",
            "Result is missing a promised output or its kind differs",
            409,
        )?;
        let typed = row.spec["kind"] != "cpu.smoke";
        validate_output_semantics(
            &manifest,
            if typed {
                Some(&row.spec["profile"])
            } else {
                None
            },
        )?;
        let mut prepared = vec![];
        let mut verified_files = vec![];
        let (root, store) = storage::output_store(
            &self.config,
            &self.bulk,
            row.spec["output_storage"].as_str().unwrap_or("data/0"),
            row.spec["_output_storage_id"].as_str(),
        )?;
        let mut ids = std::collections::BTreeSet::new();
        for entity in entities {
            let entity_id = entity["id"]
                .as_str()
                .ok_or_else(|| Error::input("Entity ID required"))?;
            require(
                ids.insert(entity_id.to_string()),
                "entity",
                "Duplicate entity ID",
                400,
            )?;
            let kind = entity["kind"]
                .as_str()
                .ok_or_else(|| Error::input("Entity kind required"))?;
            require(
                matches!(kind, "dataset" | "model" | "experiment" | "artifact"),
                "entity",
                "Unknown entity kind",
                400,
            )?;
            let path = storage::relative(
                folder,
                entity["path"]
                    .as_str()
                    .ok_or_else(|| Error::input("Entity path required"))?,
            )?;
            let verified = if let Some(proofs) = proofs {
                proofs
                    .get(&path)
                    .cloned()
                    .ok_or_else(|| Error::conflict("Output verification proof missing"))?
            } else {
                storage::VerifiedFile::read(&path)?
            };
            let (size, sha) = (verified.size, verified.sha256.clone());
            verified_files.push(verified);
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::input("Invalid entity path"))?;
            let ref_ = path
                .strip_prefix(&root)
                .map_err(|_| Error::input("Output outside registered root"))?
                .to_string_lossy()
                .to_string();
            if row.spec["kind"] != "cpu.smoke" {
                let expected = json!({"profile":row.spec["profile"]["revision"],"producer_sha256":row.spec["profile"]["sha256"]});
                require(
                    entity["provenance"] == expected,
                    "provenance",
                    "Producer provenance differs from accepted Profile",
                    409,
                )?;
            }
            let mut member_paths = std::collections::BTreeSet::new();
            for member in entity["components"].as_array().into_iter().flatten() {
                let member = member
                    .as_str()
                    .ok_or_else(|| Error::input("Output component must be a relative ref"))?;
                let item = storage::relative(path.parent().unwrap(), member)?;
                require(
                    item != path && member_paths.insert(item),
                    "components",
                    "Repeated output member",
                    400,
                )?;
            }
            if !entity["inventory"].is_object() && kind == "model" && name == "model.safetensors" {
                fn walk(
                    root: &std::path::Path,
                    dir: &std::path::Path,
                    out: &mut std::collections::BTreeSet<std::path::PathBuf>,
                ) -> Result<()> {
                    for e in std::fs::read_dir(dir)? {
                        let p = e?.path();
                        let m = p.symlink_metadata()?;
                        require(
                            !m.file_type().is_symlink(),
                            "model",
                            "Model member symlink denied",
                            409,
                        )?;
                        if m.is_dir() {
                            walk(root, &p, out)?;
                        } else {
                            let safe = storage::relative(
                                root,
                                p.strip_prefix(root)
                                    .unwrap()
                                    .to_str()
                                    .ok_or_else(|| Error::input("Invalid model member"))?,
                            )?;
                            out.insert(safe);
                        }
                    }
                    Ok(())
                }
                walk(
                    path.parent().unwrap(),
                    path.parent().unwrap(),
                    &mut member_paths,
                )?;
                member_paths.remove(&path);
            }
            let mut components = vec![json!({"ref":name,"size":size,"sha256":sha})];
            let mut locations = vec![(name.to_string(), ref_.clone())];
            for member in member_paths {
                let proof = if let Some(proofs) = proofs {
                    proofs.get(&member).cloned().ok_or_else(|| {
                        Error::conflict("Output member verification proof missing")
                    })?
                } else {
                    storage::VerifiedFile::read(&member)?
                };
                let name = member
                    .strip_prefix(path.parent().unwrap())
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                components.push(json!({"ref":name,"size":proof.size,"sha256":proof.sha256}));
                locations.push((
                    name,
                    member
                        .strip_prefix(&root)
                        .map_err(|_| Error::input("Output member outside store"))?
                        .to_string_lossy()
                        .to_string(),
                ));
                verified_files.push(proof);
            }
            let mut relations = vec![];
            for relation in entity["relations"].as_array().into_iter().flatten() {
                relations.push(json!({"kind":relation["kind"],"id":format!("{job}.{}",relation["entity_id"].as_str().unwrap_or("")),"revision":1}));
            }
            relations.extend(input_relations(&self.db, &row.spec["params"])?);
            let identity = format!("{job}.{entity_id}");
            for lineage in entity["lineage"].as_array().into_iter().flatten() {
                let input = lineage["input"].as_str().unwrap_or("");
                let matches = relations
                    .iter()
                    .filter(|r| r["kind"] == "consumes" && r["input"] == input)
                    .cloned()
                    .collect::<Vec<_>>();
                require(
                    matches.len() == 1,
                    "lineage_input",
                    "Dataset parent input must resolve to one exact accepted Dataset",
                    400,
                )?;
                let parent = &matches[0];
                let parent_kind: String = self.db.query_row(
                    "SELECT kind FROM all_entity_revisions WHERE id=? AND revision=?",
                    params![parent["id"].as_str(), parent["revision"].as_i64()],
                    |r| r.get(0),
                )?;
                require(
                    parent_kind == "dataset",
                    "lineage_kind",
                    "Data parent input must be a Dataset",
                    400,
                )?;
                relations.push(json!({"kind":"derived_from","input":input,"id":parent["id"],"revision":parent["revision"],"transformation":lineage["transformation"],"primary":lineage["primary"].as_bool().unwrap_or(false)}));
            }
            let mut published = json!({"id":identity,"revision":1,"kind":kind,"origin_machine":self.config.machine_id,"job":job,"attempt":attempt,"provenance":entity.get("provenance").cloned().unwrap_or(json!({})),"components":components,"relations":relations,"metadata_schema":row.spec["profile"]["metadata_schemas"].get(entity_id).or_else(||row.spec["profile"]["metadata_schemas"].get("*")).cloned(),"metrics":manifest["metrics"].as_array().unwrap().iter().filter(|m|m["entity_id"]==entity_id).collect::<Vec<_>>()});

            if entity["inventory"].is_object() {
                require(
                    entity.get("components").is_none_or(|v| {
                        v.as_array().is_some_and(|a| {
                            a.iter().all(|component| {
                                row.spec["profile"]["reporting_policy"] == crate::reporting::POLICY
                                    && entity["reporting"]["reports"].as_array().is_some_and(
                                        |reports| {
                                            reports.iter().any(|r| r["component"] == *component)
                                        },
                                    )
                                    || science_plan.as_ref().is_some_and(|plan| {
                                        plan.accepts_component(entity, component)
                                    })
                            })
                        })
                    }),
                    "inventory",
                    "Inventory outputs may add only declared analysis reports as inline members",
                    400,
                )?;
                let key =
                    crate::reconcile::inventory_key(path.parent().unwrap(), &entity["inventory"])?;
                let pin = if let Some(pin) = self.file_sets.get(&key) {
                    pin.clone()
                } else {
                    crate::file_set::seal(
                        self,
                        path.parent().unwrap(),
                        row.spec["output_storage"].as_str().unwrap_or("data/0"),
                        &entity["inventory"],
                    )?
                };
                published["file_set"] = pin;
                published["path"] = json!(name);
                published["components_complete"] = json!(false);
            }
            for key in ["metadata", "label", "description"] {
                if let Some(v) = entity.get(key) {
                    published[key] = v.clone();
                }
            }
            if let Some(pin) =
                crate::metadata_templates::validate_output(&row.spec["profile"], entity)?
            {
                published["metadata_template"] = pin;
                published["metadata"]["template_ref"] = published["metadata_template"].clone();
                if let Some(label) = published["label"].as_str() {
                    published["label"] = json!(crate::metadata_templates::robot_prefix(
                        &published["metadata"]["research"],
                        label
                    ));
                }
            }
            if let Some(organization) = entity.pointer("/metadata/research/organization") {
                let task = &row.spec["scenario_ref"];
                require(
                    task.is_object()
                        && (!organization["task_ref"].is_object()
                            || organization["task_ref"] == *task),
                    "organization_task",
                    "Dataset organization must use the accepted Job Task pin",
                    409,
                )?;
                published["scenario_ref"] = task.clone();
            }
            if let Some(reporting) = crate::reporting::validate_reports(
                &row.spec,
                entity,
                (job, attempt),
                path.parent().unwrap(),
                published["components"].as_array().unwrap(),
                &verified_files,
                &relations,
            )? {
                published["reporting"] = reporting;
            }
            if let Some(plan) = &science_plan {
                if let Some(science) = crate::plans::verify_output_science(
                    plan,
                    job,
                    attempt,
                    entity,
                    path.parent().unwrap(),
                    &verified_files,
                    &published,
                )? {
                    published["science"] = science;
                }
            }
            prepared.push((
                identity,
                kind.to_string(),
                published,
                ref_,
                locations,
                sha,
                i64::try_from(size).map_err(|_| Error::input("File size exceeds storage range"))?,
            ));
        }
        if let Some(outputs) = row.spec["profile"]["outputs"].as_object() {
            require(
                outputs.len() == prepared.len()
                    && entities
                        .iter()
                        .all(|e| outputs.get(e["id"].as_str().unwrap_or("")) == Some(&e["kind"])),
                "outputs",
                "Outputs differ from pinned Profile",
                409,
            )?;
        }
        let machine = self.config.machine_id.clone();
        let tx = self.transaction()?;
        return_allowed(&tx)?;
        let current = jobs::load(&tx, job)?;
        require(
            current.aggregate.fence.matches(&row.aggregate.fence),
            "fence",
            "Publication changed while verifying output",
            409,
        )?;
        if current.aggregate.cancel_requested {
            jobs::transition(&tx, &current, Event::Publish)?;
            jobs::event(&tx, &current, "cancelled")?;
            tx.execute(
                "DELETE FROM result_outbox WHERE job=? AND attempt=?",
                params![job, attempt],
            )?;
            tx.commit()?;
            return Ok(json!({"id":job,"state":"cancelled"}));
        }
        for proof in &verified_files {
            proof.revalidate()?;
        }
        if let Some(max) = row.spec["profile"]["limits"]["max_output_bytes"].as_u64() {
            require(
                verified_files.iter().map(|p| p.size).sum::<u64>() <= max,
                "limits",
                "Output exceeds accepted size limit",
                409,
            )?;
        }
        let published = rows(
            &tx,
            "SELECT id FROM all_entity_revisions WHERE job=?",
            &[&job],
        )?;
        require(
            published.is_empty()
                || published
                    .iter()
                    .map(|r| r["id"].as_str().unwrap_or(""))
                    .collect::<std::collections::BTreeSet<_>>()
                    == prepared
                        .iter()
                        .map(|(id, _, _, _, _, _, _)| id.as_str())
                        .collect(),
            "immutable",
            "Published output identities changed",
            409,
        )?;
        let mut outputs = vec![];
        for (identity, kind, record, reference, locations, sha, size) in prepared {
            let old: Option<String> = tx
                .query_row(
                    "SELECT manifest FROM all_entity_revisions WHERE id=? AND revision=1",
                    [&identity],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(old) = old {
                require(
                    serde_json::from_str::<Value>(&old)? == record,
                    "immutable",
                    "Published result differs from immutable revision",
                    409,
                )?;
            } else {
                tx.execute(
                    "INSERT INTO entity_revisions VALUES(?,1,?,?,?,?,?)",
                    params![
                        identity,
                        job,
                        kind,
                        row.owner,
                        row.submitter,
                        crate::compat::dump(&record)
                    ],
                )?;
                for (component, reference) in &locations {
                    tx.execute(
                        "INSERT INTO entity_locations VALUES(?,1,?,?,?,?,'available')",
                        params![identity, component, machine, store, reference],
                    )?;
                }
            }
            outputs.push(json!({"id":identity.strip_prefix(&format!("{job}.")).unwrap_or(&identity),"kind":kind,"sha256":sha,"size":size,"locator":{"machine_id":machine,"storage_id":store,"ref":reference}}));
        }
        let result = json!({"artifact_ready":true,"entities":outputs,"metrics":manifest.get("metrics").cloned().unwrap_or(json!([])),"execution":current.result.as_ref().and_then(|r|r.get("execution")).cloned().unwrap_or(json!({"process_group_returned":true})),"process_group_returned":true});
        jobs::transition(&tx, &current, Event::Publish)?;
        tx.execute(
            "UPDATE jobs SET result=? WHERE id=? AND attempt=?",
            params![encode(&result)?, job, attempt],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO attempt_results VALUES(?,?,?)",
            params![job, attempt, encode(&result)?],
        )?;
        tx.execute(
            "DELETE FROM result_outbox WHERE job=? AND attempt=?",
            params![job, attempt],
        )?;
        jobs::event(&tx, &current, "complete")?;
        tx.commit()?;
        Ok(result)
    }
}
fn validate_output_semantics(manifest: &Value, profile: Option<&Value>) -> Result<()> {
    let entities = manifest["entities"]
        .as_array()
        .ok_or_else(|| Error::input("Entities must be an array"))?;
    let metrics = manifest["metrics"]
        .as_array()
        .ok_or_else(|| Error::input("Metrics must be an array"))?;
    let declared: std::collections::BTreeMap<_, _> = entities
        .iter()
        .filter_map(|e| Some((e["id"].as_str()?, e["kind"].as_str()?)))
        .collect();
    for entity in entities {
        let id = entity["id"]
            .as_str()
            .ok_or_else(|| Error::input("Output ID required"))?;
        require(
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
            "entity",
            "Invalid output identity",
            400,
        )?;
        if let Some(profile) = profile {
            require(
                entity.as_object().is_some_and(|o| {
                    ["id", "kind", "path", "provenance", "relations"]
                        .iter()
                        .all(|k| o.contains_key(*k))
                        && o.keys().all(|k| {
                            [
                                "id",
                                "kind",
                                "path",
                                "provenance",
                                "relations",
                                "components",
                                "metadata",
                                "label",
                                "description",
                                "inventory",
                                "lineage",
                                "reporting",
                            ]
                            .contains(&k.as_str())
                        })
                }),
                "entity",
                "Invalid typed output fields",
                400,
            )?;
            let metadata = entity.get("metadata").cloned().unwrap_or(json!({}));
            require(
                metadata.is_object(),
                "metadata",
                "Output metadata must be an object",
                400,
            )?;
            let schema = profile["metadata_schemas"]
                .get(id)
                .or_else(|| profile["metadata_schemas"].get("*"))
                .cloned()
                .unwrap_or(json!({"type":"object"}));
            crate::contracts::validate(&schema, &metadata)?;
            crate::metadata_templates::validate_output(profile, entity)?;
            crate::reporting::validate_declaration(profile, entity)?;
            if let Some(organization) = metadata.pointer("/research/organization") {
                require(
                    entity["kind"] == "dataset",
                    "organization",
                    "Dataset organization belongs to Dataset outputs",
                    400,
                )?;
                crate::dataset_organization::validate_declaration(organization)?;
            }
            for field in ["label", "description"] {
                if let Some(value) = entity.get(field) {
                    require(
                        value.as_str().is_some_and(|s| s.len() <= 8192),
                        "entity",
                        "Invalid output text",
                        400,
                    )?;
                }
            }
        }
        for relation in entity["relations"].as_array().into_iter().flatten() {
            let target = relation["entity_id"].as_str().unwrap_or("");
            let expected = match relation["kind"].as_str() {
                Some("trained_on") => Some("dataset"),
                Some("evaluates") => Some("model"),
                Some("produces") => None,
                _ => return Err(Error::input("Unknown output relation")),
            };
            require(
                relation.as_object().is_some_and(|o| o.len() == 2)
                    && target != id
                    && declared.contains_key(target)
                    && expected.is_none_or(|kind| declared.get(target) == Some(&kind)),
                "relation",
                "Invalid typed output relation",
                400,
            )?;
        }
        if let Some(lineage) = entity.get("lineage") {
            require(
                entity["kind"] == "dataset" && lineage.as_array().is_some_and(|a| a.len() <= 16),
                "lineage",
                "Only Dataset outputs declare up to 16 data-parent inputs",
                400,
            )?;
            let mut seen = std::collections::BTreeSet::new();
            require(
                lineage
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|p| p["primary"] == true)
                    .count()
                    <= 1,
                "lineage_primary",
                "At most one primary browsing parent is allowed",
                400,
            )?;
            for parent in lineage.as_array().unwrap() {
                require(
                    parent.get("primary").is_none_or(Value::is_boolean),
                    "lineage",
                    "primary must be boolean",
                    400,
                )?;
                require(
                    parent.as_object().is_some_and(|o| {
                        o.keys()
                            .all(|k| ["input", "transformation", "primary"].contains(&k.as_str()))
                            && o.contains_key("input")
                            && o.contains_key("transformation")
                    }) && parent["input"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty() && seen.insert(s.to_string())),
                    "lineage",
                    "Each data-parent input must be named exactly once",
                    400,
                )?;
                crate::dataset_lineage::validate_transform(&parent["transformation"])?;
            }
        }
    }
    for metric in metrics {
        require(
            declared.contains_key(metric["entity_id"].as_str().unwrap_or(""))
                && metric["name"].as_str().is_some_and(|s| !s.is_empty()),
            "metric",
            "Metric output or name missing",
            400,
        )?;
        if profile.is_some() {
            require(
                metric.as_object().is_some_and(|o| {
                    o.len() == 6
                        && [
                            "name",
                            "entity_id",
                            "definition",
                            "unit",
                            "denominator",
                            "series",
                        ]
                        .iter()
                        .all(|k| o.contains_key(*k))
                }) && ["definition", "unit", "denominator"]
                    .iter()
                    .all(|k| metric[k].as_str().is_some_and(|s| !s.trim().is_empty())),
                "metric",
                "Typed metrics require definition, unit and denominator",
                400,
            )?;
            let points = metric["series"]
                .as_array()
                .ok_or_else(|| Error::input("Metric series required"))?;
            require(
                !points.is_empty() && points.len() <= 10000,
                "metric",
                "Metric series length outside limit",
                400,
            )?;
            let mut previous = -1;
            for point in points {
                let step = point["step"]
                    .as_i64()
                    .ok_or_else(|| Error::input("Metric step must be an integer"))?;
                require(
                    step > previous
                        && point.as_object().is_some_and(|o| o.len() == 2)
                        && point["value"].is_number(),
                    "metric",
                    "Invalid metric point",
                    400,
                )?;
                previous = step;
            }
        } else {
            require(
                metric["value"].is_number(),
                "metric",
                "Smoke metric must be numeric",
                400,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn validate_historical_digest(
    db: &Connection,
    identity: &Value,
    sha: &str,
) -> Result<()> {
    let body:String=db.query_row("SELECT body FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",params![identity["source"].as_str(),identity["kind"].as_str(),identity["id"].as_str(),identity["revision"].as_str(),identity["snapshot"].as_str()],|r|r.get(0))?;
    let body: Value = serde_json::from_str(&body)?;
    require(
        identity.as_object().is_some_and(|o| {
            o.keys().all(|k| {
                matches!(
                    k.as_str(),
                    "source" | "kind" | "id" | "revision" | "snapshot" | "file" | "component"
                )
            })
        }),
        "historical_identity",
        "Unknown historical identity selector",
        400,
    )?;
    let selected = identity
        .get("file")
        .or_else(|| identity.get("component"))
        .map(|v| {
            v.as_str()
                .ok_or_else(|| Error::input("File selector must be a string"))
        })
        .transpose()?;
    fn hashes(
        v: &Value,
        selected: Option<&str>,
        within: bool,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        match v {
            Value::Object(o) => {
                let chosen = within
                    || selected.is_some_and(|s| {
                        ["source_ref", "ref", "path", "file", "component"]
                            .iter()
                            .any(|k| o.get(*k).and_then(Value::as_str) == Some(s))
                    });
                if chosen || selected.is_none() {
                    if let Some(sha) = o.get("sha256").and_then(Value::as_str) {
                        out.insert(sha.into());
                    }
                }
                for (key, value) in o {
                    hashes(
                        value,
                        selected,
                        chosen || selected == Some(key.as_str()),
                        out,
                    )
                }
            }
            Value::Array(a) => {
                for value in a {
                    hashes(value, selected, within, out)
                }
            }
            _ => {}
        }
    }
    let mut known = std::collections::BTreeSet::new();
    hashes(&body, selected, false, &mut known);
    require(known.len()==1&&known.contains(sha),"historical_hash","Select an exact file with a matching original hash; explicit promotion/review required otherwise",409)?;
    Ok(())
}

pub(crate) fn input_relations(db: &Connection, params: &Value) -> Result<Vec<Value>> {
    let mut relations = vec![];
    for (input, value) in params.as_object().into_iter().flatten() {
        if let (Some(upstream), Some(output)) = (value["job"].as_str(), value["entity"].as_str()) {
            let parent = rows(
                db,
                "SELECT request FROM job_workflows WHERE id=?",
                &[&upstream],
            )?
            .into_iter()
            .next();
            let (upstream, output) = if let Some(parent) = parent {
                let request: Value =
                    serde_json::from_str(parent["request"].as_str().unwrap_or("{}"))?;
                let alias = &request["outputs"][output];
                let child: String = db.query_row(
                    "SELECT job FROM workflow_nodes WHERE root=? AND node=?",
                    params![upstream, alias["node"].as_str()],
                    |r| r.get(0),
                )?;
                (child, alias["entity"].as_str().unwrap_or("").to_string())
            } else {
                (upstream.into(), output.into())
            };
            let upstream_row = jobs::load(db, &upstream)?;
            relations.push(json!({"kind":"consumes","input":input,"id":format!("{upstream}.{output}"),"revision":1,"attempt":upstream_row.aggregate.fence.attempt.as_ref().map(|s|s.as_str())}));
        } else if value["artifact"].is_object() {
            relations.push(json!({"kind":"consumes","input":input,"id":value["artifact"]["id"],"revision":value["artifact"]["revision"],"member":value["member"]}));
        } else if value["ref"].is_string() && value["sha256"].is_string() {
            for hit in rows(db,"SELECT DISTINCT e.id,e.revision,e.manifest FROM all_entity_revisions e JOIN all_entity_locations l ON e.id=l.id AND e.revision=l.revision WHERE l.storage_id=? AND l.ref=? AND l.status='available'",&[&value["storage_id"].as_str(),&value["ref"].as_str()])? {let manifest:Value=serde_json::from_str(hit["manifest"].as_str().unwrap_or("{}"))?;if manifest["components"].as_array().is_some_and(|a|a.iter().any(|c|c["sha256"]==value["sha256"])){relations.push(json!({"kind":"consumes","input":input,"id":hit["id"],"revision":hit["revision"]}));}}
        }
    }

    Ok(relations)
}
