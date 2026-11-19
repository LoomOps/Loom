//! Preserved read models never enter native execution tables or acquire resources.
use crate::{store::*, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
impl Application {
    pub fn imported_job(&self, id: &str) -> Result<Option<Value>> {
        let text:Option<(String,String,String,String)>=self.db.query_row("SELECT body,source,revision,snapshot FROM history_records WHERE id=? AND kind='job_view' AND source IN ('queue','loom-ops') AND NOT EXISTS(SELECT 1 FROM jobs WHERE jobs.id=history_records.id) ORDER BY rowid DESC LIMIT 1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((text, source, revision, snapshot)) = text else {
            return Ok(None);
        };
        let body: Value = serde_json::from_str(&text)?;
        let entity = &body["entity"];
        let mut record=body.get("record").filter(|r|r.is_object()).cloned().unwrap_or_else(||json!({"id":id,"label":entity["label"],"state":entity["properties"]["state"],"created":entity.get("created").or_else(||entity["properties"].get("created_at")),"properties":entity["properties"],"steps":entity["steps"]}));
        record["category"] = record["spec"]
            .get("category")
            .or_else(|| entity["properties"].get("category"))
            .cloned()
            .unwrap_or(json!("other"));
        record["execution_mode"] = record["spec"]
            .get("execution_mode")
            .or_else(|| entity["properties"].get("execution_mode"))
            .cloned()
            .unwrap_or(Value::Null);
        record["migration"] = json!({"read_only":true,"source":source,"snapshot":snapshot});
        record["loom_history"] = json!({"source":source,"kind":"job_view","id":id,"revision":revision,"snapshot":snapshot});
        record["presentation"] = entity
            .get("presentation")
            .cloned()
            .unwrap_or(json!({"scope":"research","role":"main"}));
        let owner = record["owner"].as_str().unwrap_or("").to_string();
        Ok(Some(self.decorate_owner("job", id, &owner, record)?))
    }
    pub fn imported_entities(&self) -> Result<Vec<Value>> {
        self.imported_entities_selected(None)
    }
    pub fn imported_entities_selected(&self, selected: Option<&str>) -> Result<Vec<Value>> {
        let mut entities = vec![];
        for row in rows(&self.db,"SELECT h.* FROM history_records h WHERE (? OR h.id IN(SELECT value FROM json_each(?))) AND h.rowid IN(SELECT max(rowid) FROM history_records WHERE kind='job_view' AND source IN ('queue','loom-ops') GROUP BY id) AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.id=h.id) ORDER BY h.id",&[&selected.is_none(),&selected.unwrap_or("[]")])? {
   let body:Value=serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;let Some(mut entity)=body.get("entity").filter(|v|v.is_object()).cloned() else{continue};
   entity["kind"]=json!("job");entity["loom_history"]=json!({"source":row["source"],"kind":row["kind"],"id":row["id"],"revision":row["revision"],"snapshot":row["snapshot"]});entity["migration"]=json!({"read_only":true,"source":row["source"],"snapshot":row["snapshot"]});let owner=entity["owner"].as_str().or_else(||entity["properties"]["owner"].as_str()).unwrap_or("").to_string();let id=row["id"].as_str().unwrap_or("");entities.push(self.decorate_owner("job",id,&owner,entity)?);
  }
        Ok(entities)
    }
    pub fn results_listing(&self, args: &Value) -> Result<Value> {
        let view = args["view"].as_str().unwrap_or("research");
        let mut out = vec![];
        let mut native = std::collections::BTreeSet::new();
        for mut row in rows(&self.db,"SELECT e.id,e.revision,e.job,e.kind,e.owner,e.submitter,e.manifest,j.spec FROM all_entity_revisions e LEFT JOIN jobs j ON j.id=e.job ORDER BY e.id,e.revision",&[])?{
   let manifest:Value=serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;let spec:Value=serde_json::from_str(row["spec"].as_str().unwrap_or("{}"))?;
   native.insert(row["id"].as_str().unwrap_or("").to_string());
   let overlay:Option<String>=self.db.query_row("SELECT presentation FROM research_overlays WHERE (kind='entity' AND id=?) OR (kind='job' AND id=?) ORDER BY kind LIMIT 1",params![row["id"].as_str(),row["job"].as_str()],|r|r.get(0)).optional()?;
   let mut presentation=manifest.get("presentation").or_else(||spec.get("presentation")).cloned().unwrap_or(json!({"scope":"research","role":"main"}));if let Some(overlay)=overlay{presentation.as_object_mut().unwrap().extend(serde_json::from_str::<Value>(&overlay)?.as_object().unwrap().clone());}
   row.as_object_mut().unwrap().remove("manifest");row.as_object_mut().unwrap().remove("spec");row["record_type"]=json!("native");row["detail_action"]=json!("results.get");row["label"]=self.output_label("entity",row["id"].as_str().unwrap_or(""),manifest.get("label").unwrap_or(&row["id"]))?;row["presentation"]=presentation;
   if view=="all"||row["presentation"]["scope"]==view{let id=row["id"].as_str().unwrap_or("").to_string();let owner=row["owner"].as_str().unwrap_or("").to_string();out.push(self.decorate_owner("entity",&id,&owner,row)?)}
  }
        let snapshot:Option<String>=self.db.query_row("SELECT snapshot FROM history_records WHERE source='catalog' ORDER BY rowid DESC LIMIT 1",[],|r|r.get(0)).optional()?;
        if let Some(snapshot) = snapshot {
            for row in rows(&self.db,"SELECT * FROM history_records WHERE source='catalog' AND snapshot=? AND kind IN ('model','dataset','run','experiment','artifact') ORDER BY rowid DESC",&[&snapshot])?{
   let id=row["id"].as_str().unwrap_or("");if !native.insert(id.into()){continue}let body:Value=serde_json::from_str(row["body"].as_str().unwrap_or("{}"))?;let original=&body["original"];
   let presentation=original.get("presentation").cloned().unwrap_or(json!({"scope":"research","role":"main"}));
   if view=="all"||presentation["scope"]==view{out.push(json!({"id":id,"revision":row["revision"],"kind":row["kind"],"label":self.output_label(if row["kind"]=="run"{"experiment"}else{row["kind"].as_str().unwrap_or("")},id,original.get("label").unwrap_or(&row["id"]))?,"record_type":"historical","history_ref":{"source":row["source"],"kind":row["kind"],"id":id,"revision":row["revision"],"snapshot":snapshot},"detail_action":"results.historical","presentation":presentation}));}
  }
        }
        Ok(json!(out))
    }
}

impl Application {
    pub fn history_record(&self, args: &Value) -> Result<Value> {
        let mut record=rows(&self.db,"SELECT * FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND (? IS NULL OR snapshot=?) ORDER BY rowid DESC LIMIT 1",&[&args["source"].as_str(),&args["kind"].as_str(),&args["id"].as_str(),&args["revision"].as_str(),&args["snapshot"].as_str(),&args["snapshot"].as_str()])?.into_iter().next().ok_or_else(||crate::Error::new("history","Historical identity not found",404))?;
        record["body"] = serde_json::from_str(record["body"].as_str().unwrap_or("{}"))?;
        if let Some(class) = self.reviewed_output_class(
            record["source"].as_str().unwrap_or(""),
            record["kind"].as_str().unwrap_or(""),
            record["id"].as_str().unwrap_or(""),
            record["revision"].as_str().unwrap_or(""),
            record["snapshot"].as_str().unwrap_or(""),
        )? {
            record["catalog_class"] = json!(class);
        }
        let mut bindings=rows(&self.db,"SELECT identity,locator,sha256,size,created,created_by FROM historical_bindings WHERE json_extract(identity,'$.source')=? AND json_extract(identity,'$.kind')=? AND json_extract(identity,'$.id')=? AND json_extract(identity,'$.revision')=? AND json_extract(identity,'$.snapshot')=?",&[&record["source"].as_str(),&record["kind"].as_str(),&record["id"].as_str(),&record["revision"].as_str(),&record["snapshot"].as_str()])?;
        for binding in &mut bindings {
            for key in ["identity", "locator"] {
                binding[key] = serde_json::from_str(binding[key].as_str().unwrap_or("{}"))?;
            }
            binding["current_availability"] = self.location_availability(
                &binding["locator"],
                &binding["size"],
                &binding["sha256"],
            );
        }
        let mut availability = vec![];
        for location in record["body"]["locations"].as_array().into_iter().flatten() {
            let locator = json!({"machine_id":location["machine_id"],"storage_id":location["storage_id"],"ref":location["target_ref"]});
            availability.push(json!({"source_ref":location["source_ref"],"recorded_status":location["status"],"status":self.location_availability(&locator,&location["size"],&location["sha256"])}));
        }
        record["current_bindings"] = json!(bindings);
        record["current_availability"] =
            json!({"checked_at":now(),"scope":"registered_copies_only","locations":availability});
        Ok(record)
    }
    fn location_availability(&self, locator: &Value, size: &Value, sha: &Value) -> Value {
        let result = (|| -> crate::Result<&str> {
            let path = crate::storage::locator(&self.config, &self.bulk, locator)?;
            let meta = std::fs::symlink_metadata(&path)?;
            if !meta.is_file() || size.as_u64().is_some_and(|s| s != meta.len()) {
                return Ok("mismatch");
            }
            if meta.len() <= 1024 * 1024 && sha.is_string() {
                return Ok(
                    if crate::storage::file_hash(&path)?.1 == sha.as_str().unwrap() {
                        "verified"
                    } else {
                        "mismatch"
                    },
                );
            }
            Ok("present_unverified")
        })();
        json!(match result {
            Ok(status) => status,
            Err(error) => match error.code.as_str() {
                "filesystem_permission" => "permission_denied",
                "filesystem_missing" => "missing",
                "filesystem" | "io_timeout" => "unavailable",
                _ => "not_connected",
            },
        })
    }
}

impl Application {
    pub fn historical_files(&self, identity: &Value) -> Result<(Value, Vec<Value>)> {
        let text:String=self.db.query_row("SELECT body FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",params![identity["source"].as_str(),identity["kind"].as_str(),identity["id"].as_str(),identity["revision"].as_str(),identity["snapshot"].as_str()],|r|r.get(0))?;
        let body: Value = serde_json::from_str(&text)?;
        let mut components = vec![];
        let mut locations = vec![];
        for location in body["locations"].as_array().into_iter().flatten() {
            let component = location["source_ref"].as_str().unwrap_or("");
            if component.is_empty() {
                continue;
            }
            components
                .push(json!({"ref":component,"size":location["size"],"sha256":location["sha256"]}));
            locations.push(json!({"component":component,"machine_id":location["machine_id"],"storage_id":location["storage_id"],"ref":location["target_ref"].as_str().unwrap_or(component),"status":location["status"]}));
        }
        for binding in rows(&self.db,"SELECT identity,locator,size,sha256 FROM historical_bindings WHERE json_extract(identity,'$.source')=? AND json_extract(identity,'$.kind')=? AND json_extract(identity,'$.id')=? AND json_extract(identity,'$.revision')=? AND json_extract(identity,'$.snapshot')=?",&[&identity["source"].as_str(),&identity["kind"].as_str(),&identity["id"].as_str(),&identity["revision"].as_str(),&identity["snapshot"].as_str()])?{
   let selector:Value=serde_json::from_str(binding["identity"].as_str().unwrap_or("{}"))?;let locator:Value=serde_json::from_str(binding["locator"].as_str().unwrap_or("{}"))?;
   let component=selector.get("file").or_else(||selector.get("component")).and_then(Value::as_str).map(String::from).or_else(||components.iter().find(|c|c["sha256"]==binding["sha256"]).and_then(|c|c["ref"].as_str()).map(String::from)).unwrap_or_else(||"bound-file".into());
   if !components.iter().any(|c|c["ref"]==component){components.push(json!({"ref":component,"size":binding["size"],"sha256":binding["sha256"]}));}
   locations.push(json!({"component":component,"machine_id":locator["machine_id"],"storage_id":locator["storage_id"],"ref":locator["ref"],"status":"available"}));
  }
        Ok((json!({"components":components}), locations))
    }
}
