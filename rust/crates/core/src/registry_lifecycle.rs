//! Reversible registry presentation overlays; never execution gates or physical deletion.
use crate::{auth::Actor, error::require, store::*, Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub kind: String,
    pub id: String,
    pub revision: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}
impl Target {
    fn validate(&self) -> Result<()> {
        require(
            matches!(
                self.kind.as_str(),
                "component"
                    | "tool"
                    | "profile"
                    | "runtime"
                    | "metadata_template"
                    | "report_contract"
                    | "report_definition"
            ) && !self.id.is_empty()
                && self.id.len() <= 2048,
            "registry_target",
            "Invalid registry kind or ID",
            400,
        )?;
        let software = matches!(self.kind.as_str(), "component" | "tool");
        require(
            if self.kind == "profile" {
                self.revision == 0
            } else {
                self.revision > 0
            },
            "registry_target",
            "Use zero for exact Profile kind, positive revision otherwise",
            400,
        )?;
        require(
            if software {
                self.owner
                    .as_ref()
                    .is_some_and(|v| !v.is_empty() && v.len() <= 2048)
            } else {
                self.owner.is_none()
            },
            "registry_target",
            "Owner namespace is required only for Component/Tool",
            400,
        )
    }
    pub fn key(&self) -> String {
        self.owner
            .as_ref()
            .map(|v| format!("{v}:{}", self.id))
            .unwrap_or_else(|| self.id.clone())
    }
}
pub fn target(kind: &str, id: &str, revision: i64, owner: Option<&str>) -> Target {
    Target {
        kind: kind.into(),
        id: id.into(),
        revision,
        owner: owner.map(str::to_owned),
    }
}
pub fn state(db: &Connection, t: &Target) -> Result<Value> {
    let v: Option<(String, String, f64)> = db
        .query_row(
            "SELECT value,actor,updated FROM runtime_curation WHERE kind=? AND id=? AND revision=?",
            params![t.kind, t.key(), t.revision],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((text, actor, updated)) = v {
        let v: Value = serde_json::from_str(&text)?;
        Ok(
            json!({"state":v["state"].as_str().unwrap_or("active"),"reason":v["lifecycle_reason"],"changed_by":v.get("lifecycle_changed_by").cloned().unwrap_or(json!(actor)),"changed_at":v.get("lifecycle_changed_at").cloned().unwrap_or(json!(updated)),"target":t}),
        )
    } else {
        Ok(json!({"state":"active","target":t}))
    }
}
/// Internal SQL expressions only. Used against current authority before paging.
pub fn native_state_sql(native: &str, database: &str) -> String {
    let f = |k: &str| format!("json_extract({native},'$.{k}')");
    let key=format!("CASE {} WHEN 'component' THEN ({}||':'||{}) WHEN 'tool' THEN ({}||':'||{}) WHEN 'profile' THEN {} ELSE {} END",f("kind"),f("identity.owner"),f("identity.id"),f("identity.owner"),f("identity.id"),f("identity.kind"),f("identity.id"));
    format!("COALESCE((SELECT json_extract(value,'$.state') FROM {database}runtime_curation WHERE kind={} AND id=({key}) AND revision=CASE WHEN {}='profile' THEN 0 ELSE {} END),'active')",f("kind"),f("kind"),f("identity.revision"))
}
pub fn record_state_sql(kind: &str, id: &str, revision: &str) -> String {
    format!("COALESCE((SELECT json_extract(value,'$.state') FROM runtime_curation WHERE kind='{kind}' AND id={id} AND revision={revision}),'active')")
}
pub fn listing_clause(args: &Value, expression: &str) -> String {
    if let Some(s @ ("active" | "archived" | "deprecated")) = args["lifecycle_state"].as_str() {
        format!("({expression})='{s}'")
    } else if args["include_archived"] == false {
        format!("({expression})!='archived'")
    } else {
        "1".into()
    }
}
fn builtin(v: &Value) -> bool {
    v["origin"] == "builtin"
        || v["origin_type"] == "builtin"
        || v["owner"] == "builtin"
        || v["owner"] == "built-in"
        || v["owner_kind"] == "built-in"
        || v["origin"] == "built-in"
        || v["origin"]["owner"] == "built-in"
}
pub fn dto_target(v: &Value) -> Option<Target> {
    let n = &v["native_ref"];
    if n.is_object() {
        let k = n["kind"].as_str()?;
        let i = &n["identity"];
        return match k {
            "component" | "tool" => Some(target(
                k,
                i["id"].as_str()?,
                i["revision"].as_i64()?,
                i["owner"].as_str(),
            )),
            "profile" => Some(target(k, i["kind"].as_str()?, 0, None)),
            "runtime" | "metadata_template" | "report_contract" | "report_definition" => {
                Some(target(k, i["id"].as_str()?, i["revision"].as_i64()?, None))
            }
            _ => None,
        };
    }
    if v["loom_extension"].is_object() {
        let x = &v["loom_extension"];
        return Some(target(
            v["kind"].as_str()?,
            x["id"].as_str()?,
            x["revision"].as_i64()?,
            x["owner"].as_str(),
        ));
    }
    if v["loom_runtime"].is_object() {
        let x = &v["loom_runtime"];
        return Some(target(
            "runtime",
            x["id"].as_str()?,
            x["revision"].as_i64()?,
            None,
        ));
    }
    if v["loom_execution_profile"].is_object() {
        return Some(target(
            "profile",
            v["loom_execution_profile"]["kind"].as_str()?,
            0,
            None,
        ));
    }
    if matches!(
        v["kind"].as_str(),
        Some("metadata_template" | "report_contract" | "report_definition")
    ) {
        return Some(target(
            v["kind"].as_str()?,
            v["id"].as_str()?,
            v["revision"].as_i64()?,
            None,
        ));
    }
    if v["manifest"].is_object() && v["curation"].is_object() && v["revision"].is_number() {
        return Some(target(
            "runtime",
            v["id"].as_str()?,
            v["revision"].as_i64()?,
            None,
        ));
    }
    if v["stages"].is_array() && v["origin"].is_object() && v["kind"].is_string() {
        return Some(target("profile", v["kind"].as_str()?, 0, None));
    }
    None
}
impl Application {
    pub fn decorate_registry(&self, v: &mut Value) -> Result<()> {
        if let Some(t) = dto_target(v) {
            let mut s = state(&self.db, &t)?;
            s["builtin"] = json!(
                builtin(v)
                    || t.owner.as_deref() == Some("built-in")
                    || (t.kind == "profile" && t.id == "cpu.smoke")
            );
            v["lifecycle"] = s;
        }
        match v {
            Value::Array(a) => {
                for x in a {
                    self.decorate_registry(x)?
                }
            }
            Value::Object(o) => {
                for (k, x) in o {
                    if matches!(
                        k.as_str(),
                        "entities" | "all_entities" | "items" | "runtimes"
                    ) {
                        self.decorate_registry(x)?
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn lifecycle_resource(&self, t: &Target) -> Result<Value> {
        t.validate()?;
        let mut v = match t.kind.as_str() {
            "component" | "tool" => {
                let table = if t.kind == "component" {
                    "components"
                } else {
                    "tools"
                };
                let body: Option<String> = self
                    .db
                    .query_row(
                        &format!(
                            "SELECT manifest FROM {table} WHERE owner=? AND id=? AND revision=?"
                        ),
                        params![t.owner, t.id, t.revision],
                        |r| r.get(0),
                    )
                    .optional()?;
                let body: Value = serde_json::from_str(&body.ok_or_else(|| {
                    Error::new("registry_missing", "Registry revision not found", 404)
                })?)?;
                let who = self.owner_projection(&t.kind, t.owner.as_deref().unwrap(), &t.id)?;
                json!({"owner":who["owner_profile"]["id"],"owner_kind":who["owner_kind"],"label":body["label"].as_str().unwrap_or(&t.id),"body":body})
            }
            "profile" => {
                let p = self
                    .profile_listing()?
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["kind"] == t.id)
                    .cloned()
                    .ok_or_else(|| Error::new("registry_missing", "Profile kind not found", 404))?;
                json!({"owner":p["owner_profile"]["id"].as_str().or_else(||p["origin"]["owner"].as_str()).unwrap_or("administrator"),"owner_kind":p["owner_kind"],"label":p["label"],"body":p,"protected":t.id=="cpu.smoke"})
            }
            "runtime" => {
                let p = self.runtime_inventory()?["runtimes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"] == t.id && r["revision"] == t.revision)
                    .cloned()
                    .ok_or_else(|| {
                        Error::new(
                            "registry_missing",
                            "Runtime revision outside instance grant",
                            404,
                        )
                    })?;
                json!({"owner":self.owner_projection("runtime","host",&t.id)?["owner_profile"]["id"],"label":t.id,"body":p})
            }
            "metadata_template" => {
                crate::metadata_templates::get(&self.db, &t.id, Some(t.revision as u64))?
            }
            _ => crate::reporting::get(&self.db, &t.kind, &t.id, Some(t.revision as u64))?,
        };
        v["builtin"] =
            json!(builtin(&v) || v["protected"] == true || t.owner.as_deref() == Some("built-in"));
        v["target"] = json!(t);
        v["lifecycle"] = state(&self.db, t)?;
        Ok(v)
    }
    pub(crate) fn lifecycle_manage(&self, actor: &Actor, t: &Target, v: &Value) -> bool {
        !self.read_only
            && actor.scopes.iter().any(|s| s == "submit")
            && v["builtin"] != true
            && (crate::system_ownership::require_admin(&self.config, actor).is_ok()
                || v["owner"] == actor.id
                || t.owner.as_deref() == Some(&actor.id))
    }
    pub fn inspect_lifecycle(&self, actor: &Actor, args: &Value) -> Result<Value> {
        require(
            matches!(
                args["state"].as_str(),
                Some("active" | "archived" | "deprecated")
            ),
            "registry_state",
            "Unknown lifecycle state",
            400,
        )?;
        let targets: Vec<Target> = crate::contracts::decode(args["targets"].clone())?;
        require(
            !targets.is_empty() && targets.len() <= 100,
            "registry_targets",
            "Select 1 to 100 exact registry targets",
            400,
        )?;
        let selected: BTreeSet<_> = targets.iter().cloned().collect();
        require(
            selected.len() == targets.len(),
            "registry_targets",
            "Duplicate target",
            400,
        )?;
        for t in &selected {
            t.validate()?;
        }
        let profiles = self.profile_listing()?;
        let mut candidates: Vec<(Target, Value)> = vec![];
        for p in profiles.as_array().unwrap() {
            candidates.push((
                target("profile", p["kind"].as_str().unwrap(), 0, None),
                p.clone(),
            ));
        }
        for (kind, table) in [("component", "components"), ("tool", "tools")] {
            for r in rows(
                &self.db,
                &format!(
                    "SELECT owner,id,revision,manifest FROM {table} ORDER BY owner,id,revision"
                ),
                &[],
            )? {
                candidates.push((
                    target(
                        kind,
                        r["id"].as_str().unwrap(),
                        r["revision"].as_i64().unwrap(),
                        r["owner"].as_str(),
                    ),
                    serde_json::from_str(r["manifest"].as_str().unwrap())?,
                ));
            }
        }
        for r in rows(&self.db,"SELECT kind,identity,body FROM catalog_sources WHERE kind IN('metadata_template','report_contract','report_definition') ORDER BY kind,identity",&[])?{let i:Value=serde_json::from_str(r["identity"].as_str().unwrap())?;candidates.push((target(r["kind"].as_str().unwrap(),i["id"].as_str().unwrap(),i["revision"].as_i64().unwrap(),None),serde_json::from_str(r["body"].as_str().unwrap())?));}
        for r in self.runtime_inventory()?["runtimes"]
            .as_array()
            .into_iter()
            .flatten()
        {
            candidates.push((
                target(
                    "runtime",
                    r["id"].as_str().unwrap(),
                    r["revision"].as_i64().unwrap(),
                    None,
                ),
                r["manifest"].clone(),
            ));
        }
        let jobs = rows(&self.db, "SELECT id,state,spec FROM jobs ORDER BY id", &[])?;
        let mut resources = vec![];
        let mut related = BTreeMap::new();
        let mut usage = BTreeMap::new();
        for t in &selected {
            let mut r = self.lifecycle_resource(t)?;
            r["can_manage"] = json!(self.lifecycle_manage(actor, t, &r));
            let mut refs = vec![];
            for (other, body) in &candidates {
                if other != t && references(body, t, "", &other.kind) {
                    refs.push(json!(other));
                    if !selected.contains(other) {
                        related.entry(other.clone()).or_insert_with(|| {
                            body["label"].as_str().unwrap_or(&other.id).to_owned()
                        });
                    }
                }
            }
            let mut dependencies = vec![];
            for (other, body) in &candidates {
                if other != t && references(&r["body"], other, "", &t.kind)
                    || other != t && references(&r["definition"], other, "", &t.kind)
                {
                    dependencies.push(json!(other));
                    if !selected.contains(other) {
                        related.entry(other.clone()).or_insert_with(|| {
                            body["label"].as_str().unwrap_or(&other.id).to_owned()
                        });
                    }
                }
            }
            r["dependencies"] = json!(dependencies);
            for j in &jobs {
                let spec: Value = serde_json::from_str(j["spec"].as_str().unwrap())?;
                let used = t.kind == "profile" && spec["kind"] == t.id
                    || references(&spec, t, "", "job")
                    || refs
                        .iter()
                        .any(|x| x["kind"] == "profile" && spec["kind"] == x["id"]);
                if used {
                    usage.insert(j["id"].as_str().unwrap().to_owned(),json!({"id":j["id"],"state":j["state"],"active":!matches!(j["state"].as_str(),Some("complete"|"failed"|"cancelled"))}));
                }
            }
            r.as_object_mut().unwrap().remove("body");
            r.as_object_mut().unwrap().remove("definition");
            r["references"] = json!(refs);
            r["history"]=json!(rows(&self.db,"SELECT previous,state,reason,actor,recorded FROM registry_lifecycle_events WHERE target=? ORDER BY seq DESC LIMIT 20",&[&encode(&json!(t))?])?);
            resources.push(r);
        }
        let mut related_resources = vec![];
        for (t, label) in related {
            let r = self.lifecycle_resource(&t)?;
            related_resources.push(json!({"target":t,"label":label,"lifecycle":r["lifecycle"],"builtin":r["builtin"],"can_manage":self.lifecycle_manage(actor,&t,&r)}));
        }
        let mut result = json!({"targets":resources,"related":related_resources,"jobs":usage.values().collect::<Vec<_>>(),"desired_state":args["state"],"reference_coverage":"registered definitions and frozen Job pins; artifact payload files are not scanned","cascade":false});
        result["digest"] = json!(hash(encode(&result)?));
        let related_count = result["related"].as_array().unwrap().len();
        let job_count = result["jobs"].as_array().unwrap().len();
        result["related_count"] = json!(related_count);
        result["related_truncated"] = json!(related_count > 100);
        result["job_count"] = json!(job_count);
        result["jobs_truncated"] = json!(job_count > 100);
        result["related"].as_array_mut().unwrap().truncate(100);
        result["jobs"].as_array_mut().unwrap().truncate(100);
        for r in result["targets"].as_array_mut().unwrap() {
            let n = r["references"].as_array().unwrap().len();
            let n_dependencies = r["dependencies"].as_array().unwrap().len();
            r["dependency_count"] = json!(n_dependencies);
            r["dependencies_truncated"] = json!(n_dependencies > 100);
            r["dependencies"].as_array_mut().unwrap().truncate(100);
            r["reference_count"] = json!(n);
            r["references_truncated"] = json!(n > 100);
            r["references"].as_array_mut().unwrap().truncate(100);
        }
        Ok(result)
    }
    pub fn set_lifecycle(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let key = args["key"].as_str().unwrap_or("");
        if let Some(v) = replay(&self.db, actor, key, "registry.lifecycle", &args)? {
            return Ok(v);
        }
        require(
            matches!(
                args["state"].as_str(),
                Some("active" | "archived" | "deprecated")
            ),
            "registry_state",
            "Unknown registry lifecycle state",
            400,
        )?;
        // Immutable borrow permits reference revalidation inside the immediate transaction.
        require(
            args["reason"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 2000)
                && !key.is_empty(),
            "registry_request",
            "Reason and request key required",
            400,
        )?;
        let tx = crate::store::UnitOfWork::begin(&self.db)?;
        active(&tx)?;
        if let Some(v) = replay(&tx, actor, key, "registry.lifecycle", &args)? {
            return Ok(v);
        }
        let preview = self.inspect_lifecycle(actor, &args)?;
        require(
            preview["digest"] == args["expected_digest"],
            "registry_preview_changed",
            "Registry state or references changed; inspect again",
            409,
        )?;
        for r in preview["targets"].as_array().unwrap() {
            require(
                r["builtin"] != true,
                "registry_builtin",
                "Built-in registrations cannot be archived or deprecated",
                403,
            )?;
            require(
                r["can_manage"] == true,
                "registry_owner",
                "Only the registration owner or installation administrator may change lifecycle",
                403,
            )?;
        }
        let mut changed = vec![];
        for r in preview["targets"].as_array().unwrap() {
            let t: Target = crate::contracts::decode(r["target"].clone())?;
            let mut v = self.curation(&t.kind, &t.key(), t.revision)?;
            let previous = v["state"].as_str().unwrap_or("active").to_owned();
            v["state"] = args["state"].clone();
            v["lifecycle_reason"] = args["reason"].clone();
            v["lifecycle_changed_by"] = json!(actor.id);
            v["lifecycle_changed_at"] = json!(now());
            if args["state"] != "active" {
                v["preferred"] = json!(false);
            }
            if previous != args["state"].as_str().unwrap() {
                tx.execute("INSERT INTO runtime_curation VALUES(?,?,?,?,?,?) ON CONFLICT(kind,id,revision) DO UPDATE SET value=excluded.value,actor=excluded.actor,updated=excluded.updated",params![t.kind,t.key(),t.revision,encode(&v)?,actor.id,now()])?;
                tx.execute("INSERT INTO registry_lifecycle_events(target,previous,state,reason,actor,recorded) VALUES(?,?,?,?,?,?)",params![encode(&json!(t))?,previous,args["state"].as_str(),args["reason"].as_str(),actor.id,now()])?;
                changed.push(json!(t));
            }
        }
        let result = json!({"state":args["state"],"changed":changed,"preserved":"definitions, hashes, source files, accepted pins, Job history and running execution"});
        receipt(&tx, actor, key, "registry.lifecycle", &args, &result)?;
        tx.commit()?;
        Ok(result)
    }
}
fn references(v: &Value, t: &Target, key: &str, source_kind: &str) -> bool {
    if let Some(o) = v.as_object() {
        let kind = match key {
            "component" | "component_ref" | "component_pin" => "component",
            "tools" | "tool_pins" | "tool_ref" | "tool_pin" => "tool",
            "runtime_pin" | "runtime_ref" | "environment" | "runtime" => "runtime",
            "metadata_template_pins" | "metadata_templates" | "template_ref" => "metadata_template",
            "reporting_pins" | "contracts" | "contract_ref" => "report_contract",
            "protocol_ref" | "definition_ref" | "primary_metric_ref" | "factor_refs"
            | "definitions" => "report_definition",
            "base_ref" => source_kind,
            _ => "",
        };
        if kind == t.kind
            && v["id"] == t.id
            && v["revision"] == t.revision
            && t.owner.as_ref().is_none_or(|owner| v["owner"] == *owner)
        {
            return true;
        }
        return o.iter().any(|(k, x)| {
            references(
                x,
                t,
                if matches!(
                    key,
                    "tools"
                        | "tool_pins"
                        | "metadata_template_pins"
                        | "metadata_templates"
                        | "reporting_pins"
                        | "contracts"
                ) && !o.contains_key("id")
                {
                    key
                } else {
                    k
                },
                source_kind,
            )
        });
    }
    v.as_array()
        .is_some_and(|a| a.iter().any(|x| references(x, t, key, source_kind)))
}

pub fn visible(args: &Value, lifecycle: &Value) -> bool {
    if let Some(s) = args["lifecycle_state"].as_str() {
        lifecycle["state"] == s
    } else {
        args["include_archived"] != false || lifecycle["state"] != "archived"
    }
}
impl Application {
    pub fn filter_registry(&self, value: &mut Value, args: &Value) -> Result<()> {
        self.decorate_registry(value)?;
        if let Some(a) = value.as_array_mut() {
            a.retain(|v| visible(args, &v["lifecycle"]));
        }
        if let Some(a) = value
            .as_object_mut()
            .and_then(|o| o.get_mut("runtimes"))
            .and_then(Value::as_array_mut)
        {
            a.retain(|v| visible(args, &v["lifecycle"]));
        }
        Ok(())
    }
}
