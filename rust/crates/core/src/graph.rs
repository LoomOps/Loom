//! Revision-qualified software/research relations with bounded pages and no lineage guesses.
use crate::{auth::Actor, error::require, index_store, store::*, Application, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

fn edge(relation: &str, kind: &str, identity: Value) -> Value {
    json!({"relation":relation,"native_ref":{"kind":kind,"identity":identity}})
}
impl Application {
    pub fn item_graph(&self, actor: &Actor, args: &Value) -> Result<Value> {
        let detail = self.get_item(args)?;
        let native = &detail["native_ref"];
        let id = &native["identity"];
        let head = self.catalog_head()?;
        let (key, access) = index_store::context(self, actor)?;
        let mut query = args.clone();
        query.as_object_mut().unwrap().remove("cursor");
        let fingerprint = hash(encode(&query)?);
        let previous = args["cursor"]
            .as_str()
            .map(|s| index_store::cursor(s, &key))
            .transpose()?;
        if let Some(c) = &previous {
            require(
                c["query"] == fingerprint
                    && c["access"] == access
                    && c["head"] == head
                    && c["revision"] == detail["revision_id"]
                    && c["expires"].as_f64().is_some_and(|t| t > now()),
                "cursor_expired",
                "Relation source snapshot changed; refresh the page",
                409,
            )?;
        }
        let offset = previous
            .as_ref()
            .and_then(|c| c["offset"].as_u64())
            .unwrap_or(0) as usize;
        let lane = previous
            .as_ref()
            .and_then(|c| c["lane"].as_u64())
            .unwrap_or(0);
        let after = previous
            .as_ref()
            .and_then(|c| c["after"].as_str())
            .unwrap_or("");
        let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as usize;
        let inverse_software = matches!(
            native["kind"].as_str(),
            Some("component" | "tool" | "runtime" | "dependency")
        );
        let index = if inverse_software {
            index_store::read(self)
                .ok()
                .map(|db| -> Result<_> {
                    let (generation, version): (String, i64) =
                        db.query_row("SELECT generation,version FROM meta WHERE id=1", [], |r| {
                            Ok((r.get(0)?, r.get(1)?))
                        })?;
                    if let Some(c) = &previous {
                        if !c["index_generation"].is_null() {
                            require(
                                c["index_generation"] == generation,
                                "cursor_expired",
                                "Relation index changed",
                                409,
                            )?;
                        }
                    }
                    let snapshot = previous
                        .as_ref()
                        .and_then(|c| c["index_snapshot"].as_i64())
                        .unwrap_or(version);
                    Ok((db, generation, snapshot))
                })
                .transpose()?
        } else {
            None
        };
        let mut declared = vec![];
        if detail["scenario_ref"].is_object() {
            declared.push(edge("scenario", "scenario", detail["scenario_ref"].clone()));
        }
        if let Some(job) = detail["job"].as_str().filter(|s| !s.is_empty()) {
            declared.push(edge("produced_by", "job", json!({"id":job})));
        }
        if let Some(root) = detail["workflow_root"].as_str() {
            declared.push(edge("workflow", "job", json!({"id":root})));
        }
        if detail["kind"] == "file" {
            declared.push(json!({"relation":"member_of","item_id":detail["parent_item_id"],"revision_id":detail["owner_revision_id"]}));
        }
        if native["kind"] == "job" {
            let spec: Option<String> = self
                .db
                .query_row(
                    "SELECT spec FROM jobs WHERE id=?",
                    [id["id"].as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(text) = spec {
                let spec: Value = serde_json::from_str(&text)?;
                if spec["profile"]["sha256"].is_string() {
                    declared.push(edge(
                        "profile",
                        "profile",
                        json!({"kind":spec["kind"],"sha256":spec["profile"]["sha256"]}),
                    ));
                }
            }
        }
        let mut profile_kind = None;
        let mut definition = Value::Null;
        if native["kind"] == "entity" {
            let text:Option<String>=self.db.query_row("SELECT json_extract(manifest,'$.relations') FROM all_entity_revisions WHERE id=? AND revision=?",params![id["id"].as_str(),id["revision"].as_i64()],|r|r.get(0))?;
            if let Some(text) = text {
                let values: Value = serde_json::from_str(&text)?;
                for r in values.as_array().into_iter().flatten() {
                    let mut v = edge(
                        r["kind"].as_str().unwrap_or("related"),
                        "entity",
                        json!({"id":r["id"],"revision":r["revision"]}),
                    );
                    for field in ["input", "member"] {
                        if let Some(value) = r.get(field) {
                            v[field] = value.clone();
                        }
                    }
                    declared.push(v);
                }
            }
        } else if native["kind"] == "profile" {
            if let Some(kind) = id["kind"].as_str() {
                definition = crate::software::exact_profile(&self.db, &self.config, id)?;
                profile_kind = Some(kind.to_string());
            } else {
                let text: String = self.db.query_row(
                    "SELECT profile FROM execution_profiles WHERE owner=? AND id=? AND revision=?",
                    params![
                        id["owner"].as_str(),
                        id["id"].as_str(),
                        id["revision"].as_i64()
                    ],
                    |r| r.get(0),
                )?;
                definition = serde_json::from_str(&text)?;
                profile_kind = definition["revision"].as_str().map(String::from);
            }
            if let Some(path) = definition["component_path"].as_str() {
                if let Ok(path) = std::path::Path::new(path).strip_prefix(&self.components) {
                    let pieces = path
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().to_string())
                        .collect::<Vec<_>>();
                    if pieces.len() == 4 && pieces[2] == "revisions" {
                        if let Ok(rev) = pieces[3].parse::<i64>() {
                            declared.push(edge(
                                "component",
                                "component",
                                json!({"owner":pieces[0],"id":pieces[1],"revision":rev}),
                            ));
                        }
                    }
                }
            }
            for (alias, pin) in definition["tool_pins"].as_object().into_iter().flatten() {
                let mut e = edge(
                    "tool",
                    "tool",
                    json!({"owner":pin["owner"],"id":pin["id"],"revision":pin["revision"]}),
                );
                e["alias"] = json!(alias);
                declared.push(e);
            }
            for stage in crate::runtime::STAGES {
                let runner = &definition[stage];
                if let Some(pin) = runner
                    .get("runtime_pin")
                    .or_else(|| runner.get("runtime_ref"))
                {
                    let mut e = edge(
                        "runtime",
                        "runtime",
                        json!({"id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"]}),
                    );
                    e["stage"] = json!(stage);
                    declared.push(e);
                }
            }
        } else if native["kind"] == "runtime" {
            let text:Option<String>=self.db.query_row("SELECT r.value FROM rust_runtime_observation o,json_each(o.value,'$.runtimes') r WHERE json_extract(r.value,'$.id')=? AND json_extract(r.value,'$.revision')=?",params![id["id"].as_str(),id["revision"].as_i64()],|r|r.get(0)).optional()?;
            if let Some(text) = text {
                let runtime: Value = serde_json::from_str(&text)?;
                for dep in runtime["manifest"]["dependencies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    if args["include_support"] != true
                        && !crate::runtime::primary(&runtime["manifest"], None)?
                            .iter()
                            .any(|d| d["name"] == dep["name"] && d["version"] == dep["version"])
                    {
                        continue;
                    }
                    declared.push(edge(
                        "dependency",
                        "dependency",
                        crate::items::dependency_ref(&runtime, dep),
                    ));
                }
            }
        } else if native["kind"] == "dependency" {
            // Relations are read from the accepted observation, never inferred from a filename.
            for row in rows(
                &self.db,
                "SELECT r.value FROM rust_runtime_observation o,json_each(o.value,'$.runtimes') r",
                &[],
            )? {
                let runtime: Value = serde_json::from_str(row["value"].as_str().unwrap_or("{}"))?;
                if runtime["manifest"]["dependencies"]
                    .as_array()
                    .is_some_and(|deps| {
                        deps.iter()
                            .any(|d| crate::items::dependency_ref(&runtime, d) == *id)
                    })
                {
                    declared.push(edge("runtime","runtime",json!({"id":runtime["id"],"revision":runtime["revision"],"sha256":runtime["sha256"]})));
                }
            }
        }
        declared.sort_by_key(|e| encode(e).unwrap_or_default());
        declared.dedup();
        let mut values = vec![];
        let mut next = None;
        if lane == 0 {
            for e in declared.iter().skip(offset).take(limit) {
                values.push(e.clone());
            }
            if offset + values.len() < declared.len() {
                next = Some(json!({"lane":0,"offset":offset+values.len(),"after":""}));
            }
        }
        let mut inverse = vec![];
        if args["direction"] != "outgoing" && next.is_none() && values.len() < limit {
            let left = limit - values.len();
            let last = if lane == 0 { "" } else { after };
            if native["kind"] == "job" {
                for row in rows(&self.db,"SELECT node,job FROM workflow_nodes WHERE root=? AND node>? ORDER BY node LIMIT ?",&[&id["id"].as_str(),&last,&((left+1)as i64)])?{inverse.push((row["node"].as_str().unwrap_or("").to_string(),edge("step","job",json!({"id":row["job"]}))));}
            } else if let Some(kind) = profile_kind {
                for row in rows(&self.db,"SELECT id FROM jobs WHERE json_extract(spec,'$.kind')=? AND (? IS NULL OR json_extract(spec,'$.profile.sha256')=?) AND id>? ORDER BY id LIMIT ?",&[&kind,&definition["sha256"].as_str(),&definition["sha256"].as_str(),&last,&((left+1)as i64)])?{inverse.push((row["id"].as_str().unwrap_or("").to_string(),edge("used_by","job",json!({"id":row["id"]}))));}
            } else if native["kind"] == "scenario" {
                for row in rows(&self.db,"SELECT id,target_kind,target,relation FROM scenario_links WHERE scenario_id=? AND revision=? AND superseded_by IS NULL AND id>? ORDER BY id LIMIT ?",&[&id["id"].as_str(),&id["revision"].as_i64(),&last,&((left+1)as i64)])?{let target:Value=serde_json::from_str(row["target"].as_str().unwrap_or("{}"))?;inverse.push((row["id"].as_str().unwrap_or("").to_string(),edge(row["relation"].as_str().unwrap_or("context"),row["target_kind"].as_str().unwrap_or("entity"),target)));}
            } else if matches!(
                native["kind"].as_str(),
                Some("component" | "tool" | "runtime" | "dependency")
            ) {
                if let Some((db, _generation, snapshot)) = &index {
                    let snapshot = *snapshot;
                    let after = if last.is_empty() {
                        vec![]
                    } else {
                        crate::items::parse(&json!(last))?
                    };
                    let mut q=db.prepare("SELECT DISTINCT e.source FROM relation_docs e JOIN authority.catalog_aliases a ON a.kind=e.target_kind AND a.identity=e.target_identity WHERE a.item=? AND e.valid_from<=? AND (e.valid_to IS NULL OR e.valid_to>?) AND e.source>? ORDER BY e.source LIMIT ?")?;
                    let ids = q
                        .query_map(
                            params![
                                crate::items::parse(&detail["item_id"])?,
                                snapshot,
                                snapshot,
                                after,
                                (left + 1) as i64
                            ],
                            |r| r.get::<_, Vec<u8>>(0),
                        )?
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    for id in ids {
                        let item_id = crate::items::decimal(&id)?;
                        if let Ok(item) = self.get_item(&json!({"item_id":item_id})) {
                            inverse.push((item_id,json!({"relation":"used_by","native_ref":item["native_ref"],"item_id":item["item_id"],"revision_id":item["revision_id"],"label":item["label"]})));
                        }
                    }
                }
            } else {
                let native_kind = native["kind"].as_str().unwrap_or("");
                let target = encode(id)?;
                for row in rows(&self.db,"SELECT id,scenario_id,revision,relation FROM scenario_links WHERE target_kind=? AND target=? AND superseded_by IS NULL AND id>? ORDER BY id LIMIT ?",&[&native_kind,&target,&last,&((left+1)as i64)])?{inverse.push((row["id"].as_str().unwrap_or("").to_string(),edge(row["relation"].as_str().unwrap_or("context"),"scenario",json!({"id":row["scenario_id"],"revision":row["revision"]}))));}
            }
            let more = inverse.len() > left;
            let mut last_key = last.to_string();
            for (key, edge) in inverse.into_iter().take(left) {
                last_key = key;
                values.push(edge);
            }
            if more {
                next = Some(json!({"lane":1,"offset":0,"after":last_key}));
            }
        }
        let mut partial = inverse_software && index.is_none();
        for e in &mut values {
            if e["item_id"].is_string() {
                continue;
            }
            match self.resolve_item(&json!({"native_ref":e["native_ref"]})) {
                Ok(r) => {
                    e["item_id"] = r["item_id"].clone();
                    e["revision_id"] = r["revision_id"].clone();
                    if let Ok(target) = self
                        .get_item(&json!({"item_id":r["item_id"],"revision_id":r["revision_id"]}))
                    {
                        e["label"] = target["label"].clone();
                    }
                }
                Err(_) => partial = true,
            }
        }
        let next_cursor = next
            .map(|mut v| {
                if let Some((_, generation, snapshot)) = &index {
                    v["index_generation"] = json!(generation);
                    v["index_snapshot"] = json!(snapshot);
                }
                v["query"] = json!(fingerprint);
                v["access"] = json!(access);
                v["head"] = json!(head);
                v["revision"] = detail["revision_id"].clone();
                v["expires"] = json!(previous
                    .as_ref()
                    .and_then(|c| c["expires"].as_f64())
                    .unwrap_or(now() + 300.));
                index_store::sign(&v, &key)
            })
            .transpose()?;
        Ok(
            json!({"items":values,"next_cursor":next_cursor,"partial":partial,"warnings":if partial{json!(["related_item_adoption_pending"])}else{json!([])}}),
        )
    }
}

/// Outgoing declarations only. Reverse traversal is a derived join, not a second authority.
pub fn index_relations(app: &Application, item: &Value) -> Result<Vec<Value>> {
    let mut args = json!({"item_id":item["item_id"],"revision_id":item["revision_id"],"direction":"outgoing","include_support":true,"limit":200});
    let actor = crate::auth::person_actor("catalog-indexer");
    let mut edges = vec![];
    loop {
        let page = app.item_graph(&actor, &args)?;
        edges.extend(page["items"].as_array().cloned().unwrap_or_default());
        require(
            edges.len() <= 16384,
            "relation_budget",
            "Declared relation budget exceeded; use versioned grouped records",
            413,
        )?;
        if !page["next_cursor"].is_string() {
            break;
        }
        args["cursor"] = page["next_cursor"].clone();
    }
    Ok(edges)
}
