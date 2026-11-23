//! Typed inspect/plan/validate/apply. Agents propose meaning; this module owns comparison,
//! compatibility and evidence. Reconciliation never fabricates execution history.
use crate::{auth::Actor, error::require, file_set, items, storage, store::*, Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};

pub type InventoryInput = (PathBuf, String, Value);
pub type ValidationInputs = (Vec<PathBuf>, Vec<InventoryInput>);
pub const CONTRACT: &str = "loom-reconcile/v1";
#[derive(Default)]
pub struct PreparedFiles {
    pub files: BTreeMap<PathBuf, storage::VerifiedFile>,
    pub inventories: BTreeMap<String, Value>,
}
pub fn inventory_key(base: &std::path::Path, declaration: &Value) -> Result<String> {
    Ok(hash(encode(&json!([base, declaration]))?))
}
fn context(app: &Application) -> Result<String> {
    Ok(hash(encode(&json!([
        crate::onboarding::manifest()["contract_revision"],
        app.config.machine_id,
        app.config.storage_id,
        app.config.data_stores,
        app.bulk,
        app.components
    ]))?))
}
fn field<'a>(v: &'a Value, name: &str) -> Result<&'a str> {
    v[name]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 2048)
        .ok_or_else(|| Error::input(format!("Missing or invalid {name}")))
}
fn import_catalog_class(definition: &Value) -> Option<&str> {
    matches!(
        definition["kind"].as_str(),
        Some("dataset" | "model" | "experiment")
    )
    .then(|| {
        definition["catalog_class"].as_str().unwrap_or_else(|| {
            let presentation = &definition["presentation"];
            if presentation["scope"].as_str().unwrap_or("research") == "research"
                && presentation["role"].as_str().unwrap_or("main") == "main"
            {
                "regular"
            } else {
                "reference"
            }
        })
    })
}

fn import_task(db: &Connection, definition: &Value) -> Result<Option<Value>> {
    let reference = definition.get("scenario_ref");
    require(
        reference.is_some() || import_catalog_class(definition).is_none(),
        "task_required",
        "Pin a Task revision for a Dataset, Policy or Experiment import",
        400,
    )?;
    let task = reference
        .map(|value| crate::scenario::pin(db, &serde_json::from_value(value.clone())?))
        .transpose()?;
    if definition["kind"] == "dataset" {
        if let Some(organization) = definition.pointer("/metadata/research/organization") {
            crate::dataset_organization::validate_declaration(organization)?;
            crate::dataset_organization::validate_task(
                organization,
                task.as_ref().unwrap_or(&Value::Null),
            )?;
        }
    }
    let inputs: &[(&str, &str)] = match definition["kind"].as_str() {
        Some("model") => &[("trained_on", "dataset")],
        Some("experiment") => &[("evaluated_on", "dataset"), ("evaluated_model", "model")],
        _ => &[],
    };
    for &(relation, kind) in inputs {
        let mut pins = definition["relations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| r["kind"] == relation)
            .peekable();
        require(
            pins.peek().is_some(),
            "catalog_input_required",
            &format!("Catalog import requires an exact {kind} {relation} relation"),
            400,
        )?;
        for pin in pins {
            crate::reporting::validate_catalog_input(
                db,
                pin,
                kind,
                &definition["scenario_ref"]["id"],
            )?;
        }
    }
    Ok(task)
}
fn operations(v: &Value) -> Result<&Vec<Value>> {
    let a = v["operations"]
        .as_array()
        .ok_or_else(|| Error::input("Explicit operations required"))?;
    require(
        !a.is_empty() && a.len() <= 200 && encode(v)?.len() <= 262144,
        "reconcile_limit",
        "Use explicit bounded scopes of at most 200 operations",
        413,
    )?;
    Ok(a)
}
fn normalize(v: &Value) -> Result<Value> {
    let mut v = v.clone();
    if v["schema"] == "loom-asset/v1" {
        require(
            v["definition"].is_null(),
            "source_schema",
            "Legacy adapter requires type/name/properties fields",
            400,
        )?;
        v["definition"] = json!({"kind":v["type"],"label":v["name"],"metadata":v.get("properties").cloned().unwrap_or(json!({})),"primary":v["primary"],"members":v.get("members").cloned().unwrap_or(json!([]))});
        for k in ["type", "name", "properties", "primary", "members"] {
            v.as_object_mut().unwrap().remove(k);
        }
        for k in ["scenario_ref", "catalog_class", "relations"] {
            if let Some(value) = v.as_object_mut().unwrap().remove(k) {
                v["definition"][k] = value;
            }
        }
        v["schema"] = json!("loom-asset/v2");
    }
    match v["operation"].as_str().unwrap_or("") {
        "import" => {
            require(
                matches!(
                    v["schema"].as_str(),
                    Some("loom-asset/v2" | "loom-asset/v3")
                ),
                "source_schema",
                "Unsupported source schema; supply a documented adapter or current definition",
                400,
            )?;
            let d = &v["definition"];
            require(
                matches!(
                    d["kind"].as_str(),
                    Some("dataset" | "model" | "experiment" | "artifact")
                ),
                "asset_kind",
                "Explicit research kind required",
                400,
            )?;
            field(d, "label")?;
            crate::contracts::validate(
                &json!({"type":"object","properties":{"kind":{"type":"string"},"label":{"type":"string","maxLength":512},"description":{"type":"string","maxLength":4096},"metadata":{"type":"object"},"primary":{"type":"object"},"members":{"type":"array","maxItems":200,"items":{"type":"object"}},"inventory":{"type":"object"},"catalog_class":{"enum":["regular","reference"]},"scenario_ref":{"type":"object","properties":{"id":{"type":"string","minLength":1,"maxLength":2048},"revision":{"type":"integer","minimum":1}},"required":["id","revision"],"additionalProperties":false},"relations":{"type":"array","maxItems":200},"presentation":{"type":"object"}},"required":["kind","label","primary"],"additionalProperties":false}),
                d,
            )?;
            let source = &v["source"];
            for name in ["system", "id", "revision"] {
                field(source, name)?;
            }
            require(
                encode(source)?.len() <= 65536,
                "provenance",
                "Source evidence exceeds bound",
                413,
            )?;
            require(
                d.get("inventory").is_none()
                    || d.get("members")
                        .is_none_or(|m| m.as_array().is_some_and(Vec::is_empty)),
                "inventory",
                "Use a streamed inventory or small members, not both",
                400,
            )?;
        }
        "bind_file" => {
            items::parse(&v["file_item_id"])?;
            require(
                v["locator"].is_object(),
                "locator",
                "Explicit registered locator required",
                400,
            )?;
        }
        "bind_historical" => {
            require(
                v["identity"].is_object() && v["locator"].is_object(),
                "historical_identity",
                "Exact historical selector and locator required",
                400,
            )?;
        }
        "adopt" => {
            require(
                v["native_ref"].is_object(),
                "native_ref",
                "Exact native reference required",
                400,
            )?;
        }
        _ => {
            return Err(Error::input(
                "Supported operations: import, bind_file, bind_historical, adopt",
            ))
        }
    }
    Ok(v)
}
fn import_identity(op: &Value) -> Result<(String, String)> {
    let source = &op["source"];
    let identity = json!({"system":source["system"],"id":source["id"],"revision":source["revision"],"snapshot":source["snapshot"]});
    let key = encode(&identity)?;
    Ok((format!("import.{}", &hash(&key)[..32]), key))
}
impl Application {
    pub fn reconciliation(&mut self, actor: &Actor, action: &str, args: &Value) -> Result<Value> {
        match action {
            "reconcile_contract" => Ok(
                json!({"schema":CONTRACT,"asset_schema":"loom-asset/v3","adapters":["loom-asset/v1","loom-asset/v2","loom-asset/v3"],"operations":["import","bind_file","bind_historical","adopt"],"max_operations":200,"inline_members_max":200,"producer_inventory":"loom-output-inventory/v1","catalog_import":{"kinds":["dataset","model","experiment"],"class_field":"definition.catalog_class","classes":["regular","reference"],"task_field":"definition.scenario_ref","task_reference":{"required":["id","revision"]},"required_for":["regular","reference"],"input_relations":{"model":{"trained_on":"dataset"},"experiment":{"evaluated_on":"dataset","evaluated_model":"model"}}},"roots":"configured registered stores; Runtime payloads require protected host grants","stages":["plan","validate","apply","verify"],"scientific_validation":"declared metadata is not a scientific acceptance claim"}),
            ),
            "reconcile_plan" => {
                let key = field(args, "key")?;
                let proposal = &args["proposal"];
                require(
                    proposal["schema"] == CONTRACT,
                    "source_schema",
                    "Unknown reconciliation contract",
                    400,
                )?;
                let normalized: Vec<Value> = operations(proposal)?
                    .iter()
                    .map(normalize)
                    .collect::<Result<_>>()?;
                let mut imports = BTreeMap::new();
                for op in &normalized {
                    if op["operation"] == "import" {
                        let (_, source) = import_identity(op)?;
                        if let Some(previous) = imports.insert(source, op) {
                            require(
                                previous == op,
                                "source_conflict",
                                "One source identity has conflicting operations in this plan",
                                409,
                            )?;
                        }
                        let d = &op["definition"];
                        if op["schema"] == "loom-asset/v3"
                            && matches!(
                                d["kind"].as_str(),
                                Some("dataset" | "model" | "experiment")
                            )
                        {
                            let template = crate::metadata_templates::resolve(
                                &self.db,
                                &d["metadata"]["template_ref"],
                            )?;
                            crate::metadata_templates::validate(
                                &template,
                                &d["metadata"]["research"],
                                d["kind"].as_str().unwrap(),
                            )?;
                        }
                        let mut names = std::collections::BTreeSet::new();
                        for member in d["members"].as_array().into_iter().flatten() {
                            let name = field(member, "key")?;
                            require(
                                !std::path::Path::new(name).is_absolute()
                                    && std::path::Path::new(name)
                                        .components()
                                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                                "member",
                                "Member key must be relative",
                                400,
                            )?;
                            require(
                                names.insert(name),
                                "member_conflict",
                                "Duplicate member key",
                                409,
                            )?;
                        }
                        if let Some(name) = d["primary"]["ref"]
                            .as_str()
                            .and_then(|r| std::path::Path::new(r).file_name())
                            .and_then(|n| n.to_str())
                        {
                            require(
                                !names.contains(name),
                                "member_conflict",
                                "Primary key must not be repeated in members",
                                409,
                            )?;
                        }
                    }
                }
                let request = json!({"schema":CONTRACT,"operations":normalized});
                let old: Option<(String, String)> = self
                    .db
                    .query_row(
                        "SELECT id,request FROM reconcile_plans WHERE actor=? AND request_key=?",
                        params![actor.id, key],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                if let Some((id, old)) = old {
                    require(
                        old == encode(&request)?,
                        "idempotency",
                        "Plan key was reused with another request",
                        409,
                    )?;
                    return self.reconcile_get(actor, &json!({"plan_id":id}));
                }
                let id = items::decimal(&items::allocate())?;
                let source_context = context(self)?;
                // Fail unknown targets and ambiguous sources before accepting a plan.
                for op in &normalized {
                    if op["operation"] == "import"
                        && op["schema"] == "loom-asset/v3"
                        && op["definition"]["kind"] == "dataset"
                    {
                        crate::metadata_templates::require_dataset_split(
                            &op["definition"]["metadata"]["research"],
                        )?;
                    }
                    self.reconcile_compare(op)?;
                }
                let tx = self.transaction()?;
                active(&tx)?;
                tx.execute(
                    "INSERT INTO reconcile_plans VALUES(?,?,?,?,?,?,'planned',NULL,NULL,?)",
                    params![
                        id,
                        actor.id,
                        key,
                        encode(&request)?,
                        CONTRACT,
                        source_context,
                        now()
                    ],
                )?;
                tx.commit()?;
                self.reconcile_get(actor, &json!({"plan_id":id}))
            }
            "reconcile_get" => self.reconcile_get(actor, args),
            "reconcile_validate" => self.reconcile_validate(actor, args),
            "reconcile_apply" => self.reconcile_apply(actor, args),
            "reconcile_inspect" => {
                let page=self.search_items(actor,&json!({"kinds":args.get("kinds").cloned().unwrap_or(json!([])),"view":"all","role":"all","limit":args["limit"].as_u64().unwrap_or(50),"cursor":args.get("cursor").cloned().unwrap_or(Value::Null)}))?;
                Ok(
                    json!({"items":page["items"],"next_cursor":page["next_cursor"],"partial":page["partial"],"warnings":page["warnings"],"scope":"registered metadata only; propose exact locators separately","scan_performed":false}),
                )
            }
            _ => Err(Error::input("Unknown reconciliation action")),
        }
    }
    fn reconcile_get(&self, actor: &Actor, args: &Value) -> Result<Value> {
        let row=rows(&self.db,"SELECT id,actor,request,state,receipt,validation,created FROM reconcile_plans WHERE id=?",&[&field(args,"plan_id")?])?.into_iter().next().ok_or_else(||Error::new("plan","Plan not found",404))?;
        require(
            row["actor"] == actor.id,
            "plan_scope",
            "Plan belongs to another actor",
            403,
        )?;
        let request: Value = serde_json::from_str(row["request"].as_str().unwrap_or("{}"))?;
        let validation = row["validation"]
            .as_str()
            .map(serde_json::from_str::<Value>)
            .transpose()?;
        Ok(
            json!({"plan_id":row["id"],"state":row["state"],"proposal":request,"created":row["created"],"validation":validation.map(|v|json!({"status":"passed","files":v["file_count"],"bytes":v["bytes"],"verified_at":v["verified_at"]})),"receipt":row["receipt"].as_str().map(serde_json::from_str::<Value>).transpose()?}),
        )
    }
    fn plan(&self, actor: &Actor, id: &str) -> Result<Value> {
        let row = rows(
            &self.db,
            "SELECT * FROM reconcile_plans WHERE id=? AND actor=?",
            &[&id, &actor.id],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::new("plan", "Plan not found", 404))?;
        require(
            row["state"] == "applied"
                || (row["contract"] == CONTRACT && row["source_context"] == context(self)?),
            "plan_stale",
            "Contract or configured grants changed; create a fresh plan",
            409,
        )?;
        Ok(row)
    }
    fn reconcile_compare(&self, op: &Value) -> Result<Value> {
        match op["operation"].as_str().unwrap_or("") {
            "import" => {
                if op["source"]["native_ref"].is_object() {
                    let reference = &op["source"]["native_ref"];
                    let known = self.resolve_item(&json!({"native_ref":reference})).is_ok()
                        || crate::results::verify_target(
                            &self.db,
                            &crate::contracts::Target {
                                kind: reference["kind"].as_str().unwrap_or("").into(),
                                identity: reference["identity"].clone(),
                            },
                            &self.config,
                        )
                        .is_ok();
                    require(!known,"already_registered","Use adopt or an additive binding for an existing native source; importing it would duplicate the asset",409)?;
                }
                let (id, source) = import_identity(op)?;
                let old: Option<String> = self
                    .db
                    .query_row(
                        "SELECT provenance FROM asset_imports WHERE source=?",
                        [source],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(old) = old {
                    require(
                        serde_json::from_str::<Value>(&old)? == *op,
                        "source_conflict",
                        "Source identity was already imported with different evidence",
                        409,
                    )?;
                    Ok(json!({"status":"unchanged","id":id}))
                } else {
                    import_task(&self.db, &op["definition"])?;
                    let exists: bool = self.db.query_row(
                        "SELECT EXISTS(SELECT 1 FROM all_entity_revisions WHERE id=?)",
                        [&id],
                        |r| r.get(0),
                    )?;
                    require(
                        !exists,
                        "source_conflict",
                        "Target identity is already occupied",
                        409,
                    )?;
                    Ok(json!({"status":"import","id":id}))
                }
            }
            "bind_file" => {
                let file = self.get_file(&json!({"file_item_id":op["file_item_id"]}))?;
                Ok(json!({"status":"bind","file":file}))
            }
            "bind_historical" => {
                crate::results::verify_historical_binding_target(
                    &self.db,
                    &op["identity"],
                    &self.config,
                )?;
                Ok(json!({"status":"bind_historical"}))
            }
            "adopt" => {
                let target = crate::contracts::Target {
                    kind: field(&op["native_ref"], "kind")?.into(),
                    identity: op["native_ref"]["identity"].clone(),
                };
                if self
                    .resolve_item(&json!({"native_ref":op["native_ref"]}))
                    .is_err()
                {
                    crate::results::verify_target(&self.db, &target, &self.config)?;
                    require(target.kind != "profile" || target.identity["owner"].is_string(), "adoption_pending", "Exact frozen Profile adoption requires its source cache; retry after catalog observation", 409)?;
                }
                Ok(json!({"status":"adopt"}))
            }
            _ => Err(Error::input("Invalid operation")),
        }
    }
    /// Resolves only the explicit bounded candidate set. Service hashing is outside the writer.
    pub fn reconciliation_inputs(&self, actor: &Actor, id: &str) -> Result<ValidationInputs> {
        let row = self.plan(actor, id)?;
        if row["state"] == "applied" {
            return Ok((vec![], vec![]));
        }
        let request: Value = serde_json::from_str(row["request"].as_str().unwrap())?;
        let mut files = vec![];
        let mut inventories = vec![];
        for op in operations(&request)? {
            if self.reconcile_compare(op)?["status"] == "unchanged" {
                continue;
            }
            if op["operation"] == "import" {
                let d = &op["definition"];
                let primary = storage::locator(&self.config, &self.bulk, &d["primary"])?;
                files.push(primary.clone());
                for member in d["members"].as_array().into_iter().flatten() {
                    files.push(storage::locator(
                        &self.config,
                        &self.bulk,
                        &member["locator"],
                    )?);
                }
                if d["inventory"].is_object() {
                    let selector = self.locator_selector(&d["primary"])?;
                    inventories.push((
                        primary.parent().unwrap().into(),
                        selector,
                        d["inventory"].clone(),
                    ));
                }
            } else if matches!(
                op["operation"].as_str(),
                Some("bind_file" | "bind_historical")
            ) {
                files.push(storage::locator(&self.config, &self.bulk, &op["locator"])?);
            }
        }
        require(
            files.len() <= 400,
            "reconcile_limit",
            "Use smaller validation batches",
            413,
        )?;
        Ok((files, inventories))
    }
    pub fn locator_selector(&self, locator: &Value) -> Result<String> {
        if locator["storage_id"] == self.config.storage_id {
            Ok("data/0".into())
        } else {
            self.config
                .data_stores
                .iter()
                .find(|(_, s)| s["storage_id"] == locator["storage_id"])
                .map(|(k, _)| k.clone())
                .ok_or_else(|| Error::input("Store identity is not granted"))
        }
    }
    fn reconcile_validate(&mut self, actor: &Actor, args: &Value) -> Result<Value> {
        let id = field(args, "plan_id")?;
        let row = self.plan(actor, id)?;
        if row["state"] == "applied" {
            return self.reconcile_get(actor, args);
        }
        let request: Value = serde_json::from_str(row["request"].as_str().unwrap())?;
        let (paths, inventories) = self.reconciliation_inputs(actor, id)?;
        let mut proofs = vec![];
        let mut total = 0u64;
        for path in paths {
            let proof = self.verified_file(&path)?;
            total = total
                .checked_add(proof.size)
                .ok_or_else(|| Error::input("Byte overflow"))?;
            proofs.push(proof);
        }
        let mut sets = BTreeMap::new();
        for (base, selector, declaration) in inventories {
            let key = inventory_key(&base, &declaration)?;
            let pin = if let Some(p) = self.file_sets.get(&key) {
                p.clone()
            } else {
                file_set::seal(self, &base, &selector, &declaration)?
            };
            sets.insert(key, pin);
        }
        for op in operations(&request)? {
            let comparison = self.reconcile_compare(op)?;
            if op["operation"] == "bind_file" {
                let file = &comparison["file"];
                let path = storage::locator(&self.config, &self.bulk, &op["locator"])?;
                let proof = proofs
                    .iter()
                    .find(|p| p.path == path)
                    .ok_or_else(|| Error::conflict("Missing file proof"))?;
                require(
                    file["sha256"] == proof.sha256 && file["size"].as_u64() == Some(proof.size),
                    "content_changed",
                    "Candidate differs from immutable file",
                    409,
                )?;
            }
            if op["operation"] == "bind_historical" {
                let path = storage::locator(&self.config, &self.bulk, &op["locator"])?;
                let proof = proofs
                    .iter()
                    .find(|p| p.path == path)
                    .ok_or_else(|| Error::conflict("Binding proof missing"))?;
                crate::results::validate_historical_digest(
                    &self.db,
                    &op["identity"],
                    &proof.sha256,
                )?;
            }
            if op["operation"] == "import"
                && comparison["status"] != "unchanged"
                && op["definition"]["inventory"].is_object()
            {
                let d = &op["definition"];
                let path = storage::locator(&self.config, &self.bulk, &d["primary"])?;
                let proof = proofs
                    .iter()
                    .find(|p| p.path == path)
                    .ok_or_else(|| Error::conflict("Primary proof missing"))?;
                let pin = &sets[&inventory_key(path.parent().unwrap(), &d["inventory"])?];
                let member = crate::file_set::lookup(
                    self,
                    pin,
                    path.file_name()
                        .and_then(|v| v.to_str())
                        .ok_or_else(|| Error::input("Primary filename missing"))?,
                )?;
                require(
                    member["sha256"] == proof.sha256 && member["size"].as_u64() == Some(proof.size),
                    "inventory",
                    "Declared primary does not match the inventory",
                    409,
                )?;
            }
            if op["operation"] == "import" && comparison["status"] != "unchanged" {
                let d = &op["definition"];
                for locator in std::iter::once(&d["primary"]).chain(
                    d["members"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|m| &m["locator"]),
                ) {
                    let path = storage::locator(&self.config, &self.bulk, locator)?;
                    let proof = proofs
                        .iter()
                        .find(|p| p.path == path)
                        .ok_or_else(|| Error::conflict("Source proof missing"))?;
                    if let Some(expected) = locator.get("sha256") {
                        require(
                            expected == &proof.sha256,
                            "content_changed",
                            "Source digest differs from declared original",
                            409,
                        )?;
                    }
                    if let Some(expected) = locator.get("size") {
                        require(
                            expected.as_u64() == Some(proof.size),
                            "content_changed",
                            "Source size differs from declared original",
                            409,
                        )?;
                    }
                }
                import_task(&self.db, d)?;
                for member in d["members"].as_array().into_iter().flatten() {
                    field(member, "key")?;
                }
            }
        }
        let validation = json!({"proofs":proofs,"inventories":sets,"file_count":proofs.len(),"bytes":total,"verified_at":now()});
        let tx = self.transaction()?;
        active(&tx)?;
        tx.execute("UPDATE reconcile_plans SET state='validated',validation=? WHERE id=? AND state!='applied'",params![encode(&validation)?,id])?;
        tx.commit()?;
        self.reconcile_get(actor, args)
    }
    fn reconcile_apply(&mut self, actor: &Actor, args: &Value) -> Result<Value> {
        let id = field(args, "plan_id")?;
        let row = self.plan(actor, id)?;
        if row["state"] == "applied" {
            return self.reconcile_get(actor, args);
        }
        require(
            row["state"] == "validated",
            "validation_required",
            "Validate the exact plan before apply",
            409,
        )?;
        let validation: Value = serde_json::from_str(
            row["validation"]
                .as_str()
                .ok_or_else(|| Error::input("Validation missing"))?,
        )?;
        let proofs: Vec<storage::VerifiedFile> =
            serde_json::from_value(validation["proofs"].clone())?;
        let request: Value = serde_json::from_str(row["request"].as_str().unwrap())?;
        for (key, pin) in validation["inventories"].as_object().into_iter().flatten() {
            if let Some(current) = self.file_sets.get(key) {
                require(
                    current == pin,
                    "content_changed",
                    "Inventory changed since validation",
                    409,
                )?;
            } else {
                // Native HTTP supplies off-lane proofs; direct/offline callers perform the same check.
                file_set::revalidate(self, pin)?;
            }
        }
        let mut receipts = vec![];
        // Proofs and grant paths are compared again immediately before the bounded commit.
        for proof in &proofs {
            proof.revalidate()?;
        }
        for op in operations(&request)? {
            self.reconcile_compare(op)?;
            if matches!(
                op["operation"].as_str(),
                Some("bind_file" | "bind_historical")
            ) {
                storage::locator(&self.config, &self.bulk, &op["locator"])?;
            }
        }
        let mut bound_files = BTreeMap::new();
        for op in operations(&request)? {
            if op["operation"] == "bind_file" {
                let file = self.get_file(&json!({"file_item_id":op["file_item_id"]}))?;
                let owner=self.get_item(&json!({"item_id":file["parent_item_id"],"revision_id":file["owner_revision_id"]}))?;
                bound_files.insert(
                    field(op, "file_item_id")?.to_string(),
                    (file, owner["native_ref"].clone()),
                );
            }
        }
        let adopted = operations(&request)?
            .iter()
            .filter(|op| op["operation"] == "adopt")
            .map(|op| {
                encode(&op["native_ref"]).map(|key| {
                    (
                        key,
                        self.resolve_item(&json!({"native_ref":op["native_ref"]}))
                            .is_ok(),
                    )
                })
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let config = self.config.clone();
        let bulk = self.bulk.clone();
        let machine = config.machine_id.clone();
        let tx = self.transaction()?;
        active(&tx)?;
        for op in operations(&request)? {
            match op["operation"].as_str().unwrap_or("") {
                "import" => {
                    let (identity, source) = import_identity(op)?;
                    let exists: bool = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM asset_imports WHERE source=?)",
                        [&source],
                        |r| r.get(0),
                    )?;
                    if exists {
                        receipts.push(json!({"operation":"import","id":identity,"revision":1,"changed":false}));
                        continue;
                    }
                    let d = &op["definition"];
                    let primary = storage::locator(&config, &bulk, &d["primary"])?;
                    let proof = proofs
                        .iter()
                        .find(|p| p.path == primary)
                        .ok_or_else(|| Error::conflict("Import proof missing"))?;
                    let name = primary
                        .file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(|| Error::input("Primary name required"))?;
                    let mut components =
                        vec![json!({"ref":name,"sha256":proof.sha256,"size":proof.size})];
                    for member in d["members"].as_array().into_iter().flatten() {
                        let path = storage::locator(&config, &bulk, &member["locator"])?;
                        let proof = proofs
                            .iter()
                            .find(|p| p.path == path)
                            .ok_or_else(|| Error::conflict("Member proof missing"))?;
                        components.push(
                            json!({"ref":member["key"],"sha256":proof.sha256,"size":proof.size}),
                        );
                    }
                    let task = import_task(&tx, d)?;
                    let mut record = json!({"id":identity,"revision":1,"kind":d["kind"],"label":d["label"],"description":d["description"].as_str().unwrap_or(""),"metadata":d.get("metadata").cloned().unwrap_or(json!({})),"components":components,"path":name,"relations":d.get("relations").cloned().unwrap_or(json!([])),"scenario_ref":task,"origin":{"type":"import","source":op["source"],"adapter":op["schema"],"plan_id":id},"presentation":d.get("presentation").cloned().unwrap_or(json!({"scope":"research","role":"main"})),"validation":{"level":"structural_and_byte_integrity","scientific_acceptance":"unverified"}});
                    if let Some(class) = import_catalog_class(d) {
                        record["catalog_class"] = json!(class);
                    }
                    if op["schema"] == "loom-asset/v3"
                        && matches!(d["kind"].as_str(), Some("dataset" | "model" | "experiment"))
                    {
                        let template = crate::metadata_templates::resolve(
                            &tx,
                            &d["metadata"]["template_ref"],
                        )?;
                        crate::metadata_templates::validate(
                            &template,
                            &d["metadata"]["research"],
                            d["kind"].as_str().unwrap(),
                        )?;
                        record["metadata_template"] =
                            crate::metadata_templates::reference(&template);
                        record["label"] = json!(crate::metadata_templates::robot_prefix(
                            &d["metadata"]["research"],
                            d["label"].as_str().unwrap()
                        ));
                    }
                    if d["inventory"].is_object() {
                        record["file_set"] = validation["inventories"]
                            [inventory_key(primary.parent().unwrap(), &d["inventory"])?.as_str()]
                        .clone();
                    }
                    tx.execute(
                        "INSERT INTO asset_revisions VALUES(?,1,NULL,?,?,?,?)",
                        params![
                            identity,
                            d["kind"].as_str(),
                            actor.id,
                            actor.id,
                            encode(&record)?
                        ],
                    )?;
                    tx.execute(
                        "INSERT INTO asset_locations VALUES(?,1,?,?,?,?,'available')",
                        params![
                            identity,
                            name,
                            machine,
                            d["primary"]["storage_id"].as_str(),
                            d["primary"]["ref"].as_str()
                        ],
                    )?;
                    for member in d["members"].as_array().into_iter().flatten() {
                        tx.execute(
                            "INSERT INTO asset_locations VALUES(?,1,?,?,?,?,'available')",
                            params![
                                identity,
                                member["key"].as_str(),
                                machine,
                                member["locator"]["storage_id"].as_str(),
                                member["locator"]["ref"].as_str()
                            ],
                        )?;
                    }
                    tx.execute(
                        "INSERT INTO asset_imports VALUES(?,1,?,?,?)",
                        params![identity, source, now(), encode(op)?],
                    )?;
                    receipts.push(
                        json!({"operation":"import","id":identity,"revision":1,"changed":true}),
                    );
                }
                "bind_file" => {
                    let locator = op["locator"].clone();
                    tx.execute("INSERT OR IGNORE INTO file_bindings VALUES(?,?,?)",params![items::parse(&op["file_item_id"])?,encode(&locator)?,encode(&json!({"plan_id":id,"actor":actor.id,"verified_at":validation["verified_at"],"proof":proofs.iter().find(|p|storage::locator(&config,&bulk,&locator).ok().as_ref()==Some(&p.path))}))?])?;
                    let binding_changed = tx.changes() > 0;
                    if let Some((file, native)) = bound_files.get(field(op, "file_item_id")?) {
                        if native["kind"] == "entity" {
                            tx.execute("INSERT OR IGNORE INTO entity_locations VALUES(?,?,?,?,?,?,'available')",params![native["identity"]["id"].as_str(),native["identity"]["revision"].as_i64(),file["entry_key"].as_str(),locator["machine_id"].as_str(),locator["storage_id"].as_str(),locator["ref"].as_str()])?;
                        }
                    }
                    receipts.push(json!({"operation":"bind_file","file_item_id":op["file_item_id"],"changed":binding_changed}));
                }
                "bind_historical" => {
                    let identity = op["identity"].clone();
                    let path = storage::locator(&config, &bulk, &op["locator"])?;
                    let proof = proofs
                        .iter()
                        .find(|p| p.path == path)
                        .ok_or_else(|| Error::conflict("Binding proof missing"))?;
                    crate::results::validate_historical_digest(&tx, &identity, &proof.sha256)?;
                    let binding_id = crate::store::id();
                    tx.execute(
                        "INSERT OR IGNORE INTO historical_bindings VALUES(?,?,?,?,?,?,?)",
                        params![
                            binding_id,
                            encode(&identity)?,
                            encode(&op["locator"])?,
                            proof.sha256,
                            i64::try_from(proof.size)
                                .map_err(|_| Error::input("File too large"))?,
                            now(),
                            actor.id
                        ],
                    )?;
                    receipts.push(json!({"operation":"bind_historical","identity":identity,"changed":tx.changes()>0}));
                }
                "adopt" => {
                    receipts.push(
                        json!({"operation":"adopt","native_ref":op["native_ref"],"changed":!adopted[&encode(&op["native_ref"])?]}),
                    );
                }
                _ => return Err(Error::input("Invalid operation")),
            }
        }
        // Adopt only the exact requested source; never manufacture a scientific Job.
        for op in operations(&request)? {
            if op["operation"] == "adopt" {
                let native = &op["native_ref"];
                if adopted[&encode(native)?] {
                    continue;
                }
                let identity = &native["identity"];
                let topic = match native["kind"].as_str().unwrap_or("") {
                    "job" => "jobs",
                    "entity" | "protocol_artifact" => {
                        if tx.query_row("SELECT EXISTS(SELECT 1 FROM asset_revisions WHERE id=? AND revision=?)",params![identity["id"].as_str(),identity["revision"].as_i64()],|r|r.get::<_,bool>(0))? {"asset_revisions"} else {"entity_revisions"}
                    }
                    "historical" => "history_records",
                    "component" => "components",
                    "tool" => "tools",
                    "profile" => "execution_profiles",
                    "scenario" => "scenario_revisions",
                    _ => {
                        return Err(Error::input(
                            "Source requires protected observation adoption",
                        ))
                    }
                };
                let mut key = identity.clone();
                if topic == "scenario_revisions" {
                    key["scenario_id"] = key["id"].clone();
                    key.as_object_mut().unwrap().remove("id");
                }
                items::adopt(&tx, topic, &key)?;
            }
        }
        let receipt = json!({"plan_id":id,"operations":receipts,"applied_at":now(),"created_jobs":0,"scientific_acceptance":"not_inferred"});
        tx.execute(
            "UPDATE reconcile_plans SET state='applied',receipt=? WHERE id=?",
            params![encode(&receipt)?, id],
        )?;
        tx.commit()?;
        self.reconcile_get(actor, args)
    }
}
