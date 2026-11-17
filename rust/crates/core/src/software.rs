//! Immutable source/build revisions and exact composition. Registration never executes source.
use crate::{auth::Actor, compat, contracts, error::require, storage, store::*, Error, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 2048;

fn source_text(path: &Path, remaining: &mut usize) -> Result<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    storage::open_regular(path)?
        .take(*remaining as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= *remaining,
        "package",
        "Package exceeds source limit",
        400,
    )?;
    *remaining -= bytes.len();
    String::from_utf8(bytes).map_err(|_| Error::input("Source must be UTF-8 text"))
}
fn ident(v: &str) -> Result<()> {
    require(
        !v.is_empty()
            && v.len() <= 81
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            && v.as_bytes()[0].is_ascii_alphanumeric(),
        "identity",
        "Invalid software identity",
        400,
    )
}
pub fn validate_files(files: &Value) -> Result<()> {
    let files = files
        .as_object()
        .ok_or_else(|| Error::input("Source files must be an object"))?;
    require(
        !files.is_empty() && files.len() <= MAX_SOURCE_FILES,
        "package",
        "Source files required",
        400,
    )?;
    let mut total = 0;
    for (path, data) in files {
        let p = Path::new(path);
        require(
            !path.is_empty()
                && !p.is_absolute()
                && p.components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
                && p.to_str() == Some(path),
            "path",
            "Invalid package path",
            400,
        )?;
        let text = data
            .as_str()
            .ok_or_else(|| Error::input("Source content must be UTF-8 text"))?;
        total += text.len();
        require(
            total <= MAX_SOURCE_BYTES,
            "package",
            "Package exceeds source limit",
            400,
        )?;
        for parent in p.ancestors().skip(1) {
            if let Some(name) = parent.to_str() {
                require(
                    !files.contains_key(name),
                    "path",
                    "Conflicting package paths",
                    400,
                )?;
            }
        }
    }
    Ok(())
}
pub fn validate_package(args: &Value) -> Result<Value> {
    validate_package_inner(args, false)
}
fn validate_package_inner(args: &Value, legacy_copy: bool) -> Result<Value> {
    validate_files(&args["files"])?;
    let manifest = &args["manifest"];
    require(manifest.is_object(), "package", "Manifest required", 400)?;
    let files = args["files"].as_object().unwrap();
    let mut languages = vec![];
    if files.keys().any(|p| p.ends_with(".rs")) {
        languages.push("rust");
    }
    if files.keys().any(|p| p.ends_with(".py")) {
        languages.push("python");
    }
    if languages.contains(&"python") && !legacy_copy {
        require(
            manifest.get("implementation").is_some(),
            "implementation",
            "New Python workers must declare their external SDK integration",
            400,
        )?;
    }
    if let Some(implementation) = manifest.get("implementation") {
        require(
            implementation.is_object(),
            "implementation",
            "Invalid implementation metadata",
            400,
        )?;
        let language = implementation["language"].as_str().unwrap_or("");
        require(
            matches!(language, "rust" | "python" | "hybrid"),
            "implementation",
            "Unknown implementation language",
            400,
        )?;
        require(
            !languages.contains(&"python")
                || matches!(language, "python" | "hybrid")
                || legacy_copy,
            "implementation",
            "Python source requires an explicit external SDK boundary",
            400,
        )?;
        if language == "python" || language == "hybrid" {
            let workers = implementation["python_integrations"]
                .as_array()
                .ok_or_else(|| {
                    Error::input("Python integrations must name external dependencies")
                })?;
            require(
                !workers.is_empty()
                    && workers.iter().all(|w| {
                        w["package"].as_str().is_some_and(|s| !s.is_empty())
                            && w["reason"].as_str().is_some_and(|s| !s.trim().is_empty())
                            && w["stage"].as_str().is_some()
                    }),
                "implementation",
                "Python integration requires package, stage and reason",
                400,
            )?;
        }
    }
    let source_pins: BTreeMap<_, _> = files
        .iter()
        .map(|(p, v)| (p.clone(), hash(v.as_str().unwrap())))
        .collect();
    Ok(
        json!({"valid":true,"languages":languages,"source_pins":source_pins,"source_sha256":hash(compat::seal(&json!(source_pins))),"build_executed":false}),
    )
}
pub fn validate_profile(profile: &Value) -> Result<()> {
    require(
        profile.is_object(),
        "profile",
        "Profile object required",
        400,
    )?;
    for field in ["params_schema", "runtime", "inputs", "limits"] {
        require(
            profile[field].is_object(),
            "profile",
            "Profile requires schema/runtime/inputs/limits",
            400,
        )?;
    }
    require(
        profile
            .get("outputs")
            .is_some_and(|v| v.is_null() || v.is_object()),
        "profile",
        "Profile outputs required",
        400,
    )?;
    contracts::check_schema(&profile["params_schema"])?;
    for (_, schema) in profile["metadata_schemas"]
        .as_object()
        .into_iter()
        .flatten()
    {
        contracts::check_schema(schema)?;
    }
    for (name, prefix) in profile["inputs"].as_object().unwrap() {
        ident(name)?;
        let prefix = prefix
            .as_str()
            .ok_or_else(|| Error::input("Input prefix must be a string"))?;
        require(
            !Path::new(prefix).is_absolute()
                && !Path::new(prefix)
                    .components()
                    .any(|p| matches!(p, std::path::Component::ParentDir)),
            "input",
            "Input prefix must stay relative",
            400,
        )?;
    }
    for stage in [
        "runtime",
        "prepare",
        "finalize",
        "on_complete",
        "on_error",
        "on_cancel",
    ] {
        if let Some(runner) = profile.get(stage) {
            let argv = runner["command"]
                .as_array()
                .ok_or_else(|| Error::input("Stage command must be argv"))?;
            require(
                !argv.is_empty()
                    && argv.iter().all(|v| {
                        v.as_str()
                            .is_some_and(|s| !s.is_empty() && !s.contains('\0'))
                    }),
                "profile",
                "Invalid stage command",
                400,
            )?;
            let lane = runner["lane"].as_str().unwrap_or("cpu");
            require(
                matches!(lane, "cpu" | "gpu") && (matches!(stage, "runtime") || lane == "cpu"),
                "profile",
                "Lifecycle stages require CPU",
                400,
            )?;
            if let Some(env) = runner.get("env") {
                require(
                    env.as_object()
                        .is_some_and(|o| o.values().all(Value::is_string)),
                    "profile",
                    "Stage environment must contain strings",
                    400,
                )?;
            }
        }
    }
    for field in ["timeout_seconds", "max_attempts"] {
        require(
            profile["limits"][field].as_u64().is_some_and(|n| {
                n > 0
                    && n <= if field == "max_attempts" {
                        100
                    } else {
                        31_536_000
                    }
            }),
            "limits",
            "Invalid execution limit",
            400,
        )?;
    }
    if let Some(plan) = profile.get("steps") {
        crate::progress::validate_plan(plan)?;
    }
    require(
        profile["inputs"]
            .as_object()
            .unwrap()
            .keys()
            .all(|k| k != "loom-producer" && !k.starts_with("loom-tool-")),
        "input",
        "Reserved producer input alias",
        400,
    )?;
    Ok(())
}
fn seal(mut p: Value) -> Result<Value> {
    validate_profile(&p)?;
    p.as_object_mut().unwrap().remove("sha256");
    p["sha256"] = json!(hash(compat::seal(&p)));
    Ok(p)
}
/// Reconstruct an accepted immutable Profile definition; never substitute a newer hash.
pub fn exact_profile(db: &Connection, config: &Config, reference: &Value) -> Result<Value> {
    if reference["owner"].is_string() {
        let text: String = db.query_row(
            "SELECT profile FROM execution_profiles WHERE owner=? AND id=? AND revision=?",
            params![
                reference["owner"].as_str(),
                reference["id"].as_str(),
                reference["revision"].as_i64()
            ],
            |r| r.get(0),
        )?;
        return Ok(serde_json::from_str(&text)?);
    }
    if reference["kind"] == "cpu.smoke" {
        return Ok(
            json!({"revision":"cpu.smoke","builtin":true,"inputs":{},"outputs":{"observation":"artifact"}}),
        );
    }
    let kind = reference["kind"]
        .as_str()
        .ok_or_else(|| Error::input("Profile kind required"))?;
    let digest = reference["sha256"].as_str();
    if let Some(p) = config.profiles.get(kind) {
        if digest.is_none_or(|d| p["sha256"] == d) {
            return Ok(p.clone());
        }
    }
    let text:Option<String>=db.query_row("SELECT value FROM (SELECT p.value value FROM components c,json_each(c.profiles) p WHERE p.key=?1 UNION ALL SELECT profile FROM execution_profiles WHERE json_extract(profile,'$.revision')=?1 UNION ALL SELECT json_extract(spec,'$.profile') FROM jobs WHERE json_extract(spec,'$.kind')=?1) WHERE ?2 IS NULL OR json_extract(value,'$.sha256')=?2 LIMIT 1",params![kind,digest],|r|r.get(0)).optional()?;
    text.map(|s| serde_json::from_str::<Value>(&s))
        .transpose()?
        .ok_or_else(|| {
            Error::new(
                "profile_revision",
                "Accepted Profile definition unavailable",
                404,
            )
        })
}
pub fn profile(db: &Connection, config: &Config, kind: &str) -> Result<Option<Value>> {
    if let Some(p) = config.profiles.get(kind) {
        return Ok(Some(p.clone()));
    }
    if let Some(text)=db.query_row("SELECT profile FROM execution_profiles WHERE json_extract(profile,'$.revision')=? ORDER BY revision DESC LIMIT 1",[kind],|r|r.get::<_,String>(0)).optional()? {return Ok(Some(serde_json::from_str(&text)?))}
    let cache: Option<String> = if crate::items::installed(db)? {
        db.query_row(
            "SELECT body FROM catalog_sources WHERE kind='profile' AND identity=?",
            [encode(&json!({"kind":kind}))?],
            |r| r.get(0),
        )
        .optional()?
    } else {
        None
    };
    if let Some(cache) = cache {
        let value: Value = serde_json::from_str(&cache)?;
        let parent = &value["component_ref"];
        if parent.is_object() {
            let text: String = db.query_row(
                "SELECT profiles FROM components WHERE owner=? AND id=? AND revision=?",
                params![
                    parent["owner"].as_str(),
                    parent["id"].as_str(),
                    parent["revision"].as_i64()
                ],
                |r| r.get(0),
            )?;
            let profiles: Value = serde_json::from_str(&text)?;
            return Ok(profiles.get(kind).cloned());
        }
    }
    for row in rows(db, "SELECT profiles FROM components", &[])? {
        let p: Value = serde_json::from_str(row["profiles"].as_str().unwrap_or("{}"))?;
        if let Some(p) = p.get(kind) {
            return Ok(Some(p.clone()));
        }
    }
    Ok(None)
}
pub fn bind_tools(db: &Connection, definition: &mut Value, owner: &str, root: &Path) -> Result<()> {
    let references = definition
        .as_object_mut()
        .ok_or_else(|| Error::input("Profile object required"))?
        .remove("tools")
        .unwrap_or(json!({}));
    let mut pins = serde_json::Map::new();
    for (alias, reference) in references
        .as_object()
        .ok_or_else(|| Error::input("Tool references must be an object"))?
    {
        ident(alias)?;
        let namespace = reference["owner"].as_str().unwrap_or(owner);
        let identity = reference["id"]
            .as_str()
            .ok_or_else(|| Error::input("Tool ID required"))?;
        let revision = reference["revision"]
            .as_i64()
            .ok_or_else(|| Error::input("Exact Tool revision required"))?;
        let row = rows(
            db,
            "SELECT * FROM tools WHERE owner=? AND id=? AND revision=?",
            &[&namespace, &identity, &revision],
        )?
        .into_iter()
        .next()
        .ok_or_else(|| Error::input("Tool revision not found"))?;
        let manifest: Value = serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;
        pins.insert(alias.clone(),json!({"owner":namespace,"id":identity,"revision":revision,"sha256":row["sha256"],"path":root.join(".tools").join(namespace).join(identity).join("revisions").join(revision.to_string()),"source_pins":manifest["source_pins"],"producer_build":manifest["build_receipt"]}));
    }
    if !pins.is_empty() {
        definition["tool_pins"] = json!(pins);
    }
    Ok(())
}
impl Application {
    pub fn software_action(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        let owner = args["owner"].as_str().unwrap_or(&actor.id).to_string();
        let identity = args["id"].as_str().unwrap_or("").to_string();
        if owner == "system"
            && matches!(
                action,
                "register" | "register_tool" | "register_profile" | "write"
            )
        {
            crate::system_ownership::require_admin(&self.config, actor)?;
        }
        match action {
            "list" | "list_tools" => {
                let table = if action == "list" {
                    "components"
                } else {
                    "tools"
                };
                let mut items=rows(&self.db,&format!("SELECT t.* FROM {table} t WHERE revision=(SELECT max(revision) FROM {table} x WHERE x.owner=t.owner AND x.id=t.id) ORDER BY owner,id"),&[])?;
                if args.get("owner").is_some() {
                    items.retain(|r| r["owner"] == owner)
                }
                for row in &mut items {
                    for field in ["manifest", "profiles"] {
                        if let Some(s) = row[field].as_str() {
                            row[field] = serde_json::from_str(s)?;
                        }
                    }
                    row["origin"] = json!(if row["owner"] == "built-in" {
                        "built-in"
                    } else {
                        "user"
                    });
                }
                for row in &mut items {
                    let t = crate::registry_lifecycle::target(
                        if action == "list" {
                            "component"
                        } else {
                            "tool"
                        },
                        row["id"].as_str().unwrap(),
                        row["revision"].as_i64().unwrap(),
                        row["owner"].as_str(),
                    );
                    row["lifecycle"] = crate::registry_lifecycle::state(&self.db, &t)?;
                    row["lifecycle"]["builtin"] = json!(row["owner"] == "built-in");
                }
                items.retain(|v| crate::registry_lifecycle::visible(&args, &v["lifecycle"]));
                Ok(json!(items))
            }
            "get" | "get_tool" | "get_profile" => {
                let table = match action {
                    "get" => "components",
                    "get_tool" => "tools",
                    _ => "execution_profiles",
                };
                let revision = args["revision"].as_i64().unwrap_or(i64::MAX);
                let mut row=rows(&self.db,&format!("SELECT * FROM {table} WHERE owner=? AND id=? AND (revision=? OR ?=9223372036854775807) ORDER BY revision DESC LIMIT 1"),&[&owner,&identity,&revision,&revision])?.into_iter().next().ok_or_else(||Error::new("software","Revision not found",404))?;
                for field in ["manifest", "profiles", "profile"] {
                    if let Some(s) = row[field].as_str() {
                        row[field] = serde_json::from_str(s)?;
                    }
                }
                let t = if action == "get_profile" {
                    crate::registry_lifecycle::target(
                        "profile",
                        row["profile"]["revision"].as_str().unwrap_or(&identity),
                        0,
                        None,
                    )
                } else {
                    crate::registry_lifecycle::target(
                        if action == "get" { "component" } else { "tool" },
                        &identity,
                        row["revision"].as_i64().unwrap(),
                        Some(&owner),
                    )
                };
                row["lifecycle"] = crate::registry_lifecycle::state(&self.db, &t)?;
                Ok(row)
            }
            "register" | "register_tool" => self.register_source(actor, action, args),
            "write" => {
                ident(&owner)?;
                ident(&identity)?;
                let base = self.components.join(&owner).join(&identity).join("src");
                fs::create_dir_all(&base)?;
                let p = storage::relative(
                    &base,
                    args["path"]
                        .as_str()
                        .ok_or_else(|| Error::input("Path required"))?,
                )?;
                let text = args["content"]
                    .as_str()
                    .ok_or_else(|| Error::input("Content required"))?;
                active(&self.db)?;
                storage::atomic(&p, text.as_bytes(), 0o600)?;
                Ok(json!({"owner":owner,"id":identity,"path":args["path"],"bytes":text.len()}))
            }
            "register_profile" => self.register_profile(actor, args),
            "profiles" => {
                let mut v = self.profile_listing()?;
                self.filter_registry(&mut v, &args)?;
                Ok(v)
            }
            "runtimes" | "get_runtime" | "dependencies" | "get_dependency" => {
                let mut v = self.runtime_query(action, args.clone())?;
                if matches!(action, "runtimes" | "get_runtime") {
                    self.filter_registry(&mut v, &args)?;
                }
                Ok(v)
            }
            "inspect_profile" | "compare_profiles" => self.inspect_profiles(action, args),
            "annotate" | "curate" | "assign_owner" => self.curate_software(actor, action, args),
            _ => Err(Error::input("Unknown software action")),
        }
    }
    fn register_source(&mut self, actor: &Actor, action: &str, args: Value) -> Result<Value> {
        let owner = args["owner"].as_str().unwrap_or(&actor.id).to_string();
        let identity = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Software ID required"))?
            .to_string();
        ident(&owner)?;
        ident(&identity)?;
        let mut manifest = args["manifest"].clone();
        let mut files = args["files"].clone();
        let legacy_copy = action == "register_tool" && args.get("component").is_some();
        if legacy_copy {
            require(
                files.is_null() || files == json!({}),
                "tool",
                "Select files or an exact legacy Component",
                400,
            )?;
            let reference = &args["component"];
            let namespace = reference["owner"].as_str().unwrap_or(&owner);
            let source_id = reference["id"]
                .as_str()
                .ok_or_else(|| Error::input("Component identity required"))?;
            let revision = reference["revision"]
                .as_i64()
                .ok_or_else(|| Error::input("Component revision required"))?;
            ident(namespace)?;
            ident(source_id)?;
            let row = rows(
                &self.db,
                "SELECT manifest,profiles FROM components WHERE owner=? AND id=? AND revision=?",
                &[&namespace, &source_id, &revision],
            )?
            .into_iter()
            .next()
            .ok_or_else(|| Error::input("Component revision not found"))?;
            let source_manifest: Value =
                serde_json::from_str(row["manifest"].as_str().unwrap_or("{}"))?;
            let profiles: Value = serde_json::from_str(row["profiles"].as_str().unwrap_or("{}"))?;
            let pins = source_manifest
                .get("source_pins")
                .or_else(|| {
                    profiles
                        .as_object()
                        .into_iter()
                        .flatten()
                        .find_map(|(_, p)| p.get("source_pins"))
                })
                .ok_or_else(|| Error::conflict("Legacy source pins are unrecorded"))?;
            let folder = self
                .components
                .join(namespace)
                .join(source_id)
                .join("revisions")
                .join(revision.to_string());
            let mut copied = serde_json::Map::new();
            let mut remaining = MAX_SOURCE_BYTES;
            require(
                pins.as_object()
                    .is_some_and(|p| p.len() <= MAX_SOURCE_FILES),
                "package",
                "Source file limit exceeded",
                400,
            )?;
            for (name, digest) in pins
                .as_object()
                .ok_or_else(|| Error::input("Invalid source pins"))?
            {
                let path = storage::relative(&folder, name)?;
                let text = source_text(&path, &mut remaining)?;
                require(
                    *digest == hash(&text),
                    "source",
                    "Legacy Component bytes changed",
                    409,
                )?;
                copied.insert(name.clone(), json!(text));
            }
            files = json!(copied);
            manifest["origin"] =
                json!({"kind":"component","owner":namespace,"id":source_id,"revision":revision});
        }
        if files.is_null() || files == json!({}) {
            let source = self.components.join(&owner).join(&identity).join("src");
            let mut found = serde_json::Map::new();
            fn read(
                root: &Path,
                dir: &Path,
                out: &mut serde_json::Map<String, Value>,
                remaining: &mut usize,
            ) -> Result<()> {
                for p in fs::read_dir(dir)? {
                    let p = p?.path();
                    require(
                        !p.symlink_metadata()?.file_type().is_symlink(),
                        "path",
                        "Source symlink denied",
                        400,
                    )?;
                    if p.is_dir() {
                        read(root, &p, out, remaining)?;
                    } else {
                        require(
                            out.len() < MAX_SOURCE_FILES,
                            "package",
                            "Source file limit exceeded",
                            400,
                        )?;
                        out.insert(
                            p.strip_prefix(root).unwrap().to_string_lossy().into(),
                            json!(source_text(&p, remaining)?),
                        );
                    }
                }
                Ok(())
            }
            let mut remaining = MAX_SOURCE_BYTES;
            read(&source, &source, &mut found, &mut remaining)?;
            files = json!(found);
        }
        let report =
            validate_package_inner(&json!({"manifest":manifest,"files":files}), legacy_copy)?;
        let build = self.verify_build(&manifest, &report["source_sha256"])?;
        if action == "register_tool" {
            require(
                [
                    "profiles",
                    "runtime",
                    "prepare",
                    "finalize",
                    "on_complete",
                    "on_error",
                    "on_cancel",
                ]
                .iter()
                .all(|k| manifest.get(k).is_none()),
                "tool",
                "Tools do not own Job lifecycle stages",
                400,
            )?;
        }
        manifest["source_pins"] = report["source_pins"].clone();
        let manifest_name = if action == "register_tool" {
            "tool.json"
        } else {
            "component.json"
        };
        require(
            files.get(manifest_name).is_none(),
            "package",
            "Reserved manifest source path",
            400,
        )?;
        let mut snapshot = files.as_object().unwrap().clone();
        snapshot.insert(
            manifest_name.into(),
            json!(serde_json::to_string_pretty(&manifest)?),
        );
        let stage = storage::stage(&self.components, &snapshot)?;
        let table = if action == "register_tool" {
            "tools"
        } else {
            "components"
        };
        let base = if action == "register_tool" {
            self.components.join(".tools")
        } else {
            self.components.clone()
        };
        let catalog = self.runtime_inventory()?;
        let config = self.config.clone();
        let component_root = self.components.clone();
        let result = (|| -> Result<Value> {
            let tx = self.transaction()?;
            active(&tx)?;
            if action == "register_tool" {
                let digest = hash(compat::seal(&manifest));
                if let Some(row) = rows(
                    &tx,
                    "SELECT * FROM tools WHERE owner=? AND id=? ORDER BY revision DESC LIMIT 1",
                    &[&owner, &identity],
                )?
                .into_iter()
                .next()
                {
                    if row["sha256"] == digest {
                        let revision = row["revision"].as_i64().unwrap();
                        let folder = base
                            .join(&owner)
                            .join(&identity)
                            .join("revisions")
                            .join(revision.to_string());
                        storage::publish(&stage, &folder)?;
                        tx.commit()?;
                        return Ok(
                            json!({"owner":owner,"id":identity,"revision":revision,"sha256":digest,"manifest":manifest,"unchanged":true}),
                        );
                    }
                }
            }
            let revision: i64 = tx.query_row(
                &format!("SELECT COALESCE(max(revision),0)+1 FROM {table} WHERE owner=? AND id=?"),
                params![owner, identity],
                |r| r.get(0),
            )?;
            let folder = base
                .join(&owner)
                .join(&identity)
                .join("revisions")
                .join(revision.to_string());
            let mut profiles = serde_json::Map::new();
            if action == "register" {
                let definitions = manifest["profiles"]
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                for (name, mut p) in definitions {
                    ident(&name)?;
                    require(
                        [
                            "tool_pins",
                            "metadata_policy",
                            "metadata_template_pins",
                            "reporting_pins",
                            "reporting_policy",
                            "source_pins",
                            "component_path",
                            "sha256",
                            "revision",
                            "owners",
                            "submitters",
                        ]
                        .iter()
                        .all(|key| p.get(key).is_none()),
                        "profile",
                        "Execution pins are server assigned",
                        400,
                    )?;
                    if p.get("tools").is_none() {
                        if let Some(tools) = manifest.get("tools") {
                            p["tools"] = tools.clone();
                        }
                    }
                    let kind = format!("{owner}.{identity}.{name}.r{revision}");
                    require(
                        profile(&tx, &config, &kind)?.is_none(),
                        "profile",
                        "Profile identity conflict",
                        409,
                    )?;
                    p["schema"] = json!("loom-execution/v2");
                    p["revision"] = json!(kind);
                    p["owners"] = json!([owner]);
                    p["submitters"] = json!([actor.id, owner]);
                    p["component_path"] = json!(folder);
                    p["source_pins"] = manifest["source_pins"].clone();
                    bind_tools(&tx, &mut p, &owner, &component_root)?;
                    crate::runtime::bind(&catalog, &mut p)?;
                    crate::authoring::bind_build(&mut p, build.as_ref())?;
                    crate::runtime::require_dependencies(&manifest, &p)?;
                    verify_tool_requirements(&tx, &p)?;
                    crate::metadata_templates::bind_profile(&tx, &mut p)?;
                    crate::reporting::bind_profile(&tx, &mut p)?;
                    profiles.insert(kind, seal(p)?);
                }
            }
            storage::publish(&stage, &folder)?;
            if action == "register_tool" {
                let sha = hash(compat::seal(&manifest));
                tx.execute(
                    "INSERT INTO tools VALUES(?,?,?,?,?)",
                    params![owner, identity, revision, encode(&manifest)?, sha],
                )?;
                tx.commit()?;
                Ok(
                    json!({"owner":owner,"id":identity,"revision":revision,"sha256":sha,"manifest":manifest}),
                )
            } else {
                tx.execute(
                    "INSERT INTO components VALUES(?,?,?,?,?)",
                    params![
                        owner,
                        identity,
                        revision,
                        encode(&manifest)?,
                        encode(&json!(profiles))?
                    ],
                )?;
                tx.commit()?;
                Ok(
                    json!({"owner":owner,"id":identity,"revision":revision,"profiles":profiles.keys().collect::<Vec<_>>()}),
                )
            }
        })();
        if stage.exists() {
            let _ = fs::remove_dir_all(stage);
        }
        result
    }
    fn register_profile(&mut self, actor: &Actor, args: Value) -> Result<Value> {
        let owner = args["owner"].as_str().unwrap_or(&actor.id).to_string();
        let identity = args["id"]
            .as_str()
            .ok_or_else(|| Error::input("Profile ID required"))?
            .to_string();
        ident(&owner)?;
        ident(&identity)?;
        let mut p = args["profile"].clone();
        require(
            [
                "sha256",
                "revision",
                "source_pins",
                "tool_pins",
                "metadata_policy",
                "metadata_template_pins",
                "reporting_pins",
                "reporting_policy",
                "component_path",
                "owners",
                "submitters",
            ]
            .iter()
            .all(|k| p.get(k).is_none()),
            "profile",
            "Pins are assigned by Loom",
            400,
        )?;
        let root = self.components.clone();
        let catalog = self.runtime_inventory()?;
        let tx = self.transaction()?;
        active(&tx)?;
        let revision: i64 = tx.query_row(
            "SELECT COALESCE(max(revision),0)+1 FROM execution_profiles WHERE owner=? AND id=?",
            params![owner, identity],
            |r| r.get(0),
        )?;
        bind_tools(&tx, &mut p, &owner, &root)?;
        crate::runtime::bind(&catalog, &mut p)?;
        if let Some(c) = args.get("component") {
            let namespace = c["owner"].as_str().unwrap_or(&owner);
            let id = c["id"]
                .as_str()
                .ok_or_else(|| Error::input("Component ID required"))?;
            let revision = c["revision"]
                .as_i64()
                .ok_or_else(|| Error::input("Component revision required"))?;
            let manifest: String = tx.query_row(
                "SELECT manifest FROM components WHERE owner=? AND id=? AND revision=?",
                params![namespace, id, revision],
                |r| r.get(0),
            )?;
            let manifest: Value = serde_json::from_str(&manifest)?;
            crate::authoring::bind_build(&mut p, manifest.get("build_receipt"))?;
            crate::runtime::require_dependencies(&manifest, &p)?;
            p["source_pins"] = manifest["source_pins"].clone();
            p["component_path"] = json!(root
                .join(namespace)
                .join(id)
                .join("revisions")
                .join(revision.to_string()));
        }
        verify_tool_requirements(&tx, &p)?;
        let kind = format!("{owner}.{identity}.r{revision}");
        p["schema"] = json!("loom-execution/v2");
        p["revision"] = json!(kind);
        p["owners"] = json!([owner]);
        p["submitters"] = json!([actor.id, owner]);
        crate::metadata_templates::bind_profile(&tx, &mut p)?;
        crate::reporting::bind_profile(&tx, &mut p)?;
        p = seal(p)?;
        tx.execute(
            "INSERT INTO execution_profiles VALUES(?,?,?,?)",
            params![owner, identity, revision, encode(&p)?],
        )?;
        tx.commit()?;
        Ok(
            json!({"owner":owner,"id":identity,"revision":revision,"kind":kind,"sha256":p["sha256"]}),
        )
    }
    fn runtime_query(&self, action: &str, args: Value) -> Result<Value> {
        let catalog = self.runtime_inventory()?;
        match action {
            "runtimes" => Ok(catalog),
            "dependencies" => Ok(catalog["dependency_catalog"].clone()),
            "get_runtime" => catalog["runtimes"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|r| r["id"] == args["id"] && r["revision"] == args["revision"])
                .cloned()
                .ok_or_else(|| Error::new("runtime", "Runtime outside instance grant", 404)),
            _ => catalog["dependency_catalog"]["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|d| d["id"] == args["id"])
                .cloned()
                .ok_or_else(|| Error::new("dependency", "Dependency outside instance grant", 404)),
        }
    }
}
pub fn scaffold(args: &Value) -> Result<Value> {
    let name = args["name"]
        .as_str()
        .ok_or_else(|| Error::input("Package name required"))?;
    ident(name)?;
    let template = args["template"].as_str().unwrap_or("tool");
    let cargo=format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nloom-sdk = {{ path = \"sdk/loom-sdk\" }}\n");
    let source = if template == "hybrid" {
        include_str!("../../../templates/hybrid.rs")
    } else {
        include_str!("../../../templates/runner.rs")
    };
    let profile = json!({"params_schema":{"type":"object","additionalProperties":false},"runtime":{"executor":"container","lane":"cpu","runtime_ref":{"id":"select-runtime","revision":1},"command":["@producer"]},"inputs":{},"outputs":{"receipt":"artifact"},"limits":{"timeout_seconds":120,"max_attempts":1}});
    let mut manifest = json!({"implementation":{"language":if template=="hybrid"{"hybrid"}else{"rust"},"python_integrations":if template=="hybrid"{json!([{"stage":"run","package":"replace-with-external-sdk","reason":"replace-with-concrete-integration"}])}else{json!([])}}});
    if template != "tool" {
        manifest["profiles"] = json!({"runner":profile});
    }
    let mut files = json!({"Cargo.toml":cargo,"src/main.rs":source,"sdk/loom-sdk/Cargo.toml":include_str!("../../../templates/sdk-Cargo.toml"),"sdk/loom-sdk/src/lib.rs":include_str!("../../sdk/src/lib.rs")});
    if template == "hybrid" {
        files["worker.py"] = json!(include_str!("../../../templates/worker.py"));
    }
    Ok(
        json!({"template":template,"language":"rust","producer_protocol":"loom-producer/v1","files":files,"manifest":manifest,"draft_profile":profile,"build_executed":false}),
    )
}

fn verify_tool_requirements(db: &Connection, p: &Value) -> Result<()> {
    for (_, pin) in p["tool_pins"].as_object().into_iter().flatten() {
        let text: String = db.query_row(
            "SELECT manifest FROM tools WHERE owner=? AND id=? AND revision=?",
            params![
                pin["owner"].as_str(),
                pin["id"].as_str(),
                pin["revision"].as_i64()
            ],
            |r| r.get(0),
        )?;
        crate::runtime::require_dependencies(&serde_json::from_str(&text)?, p)?;
    }
    Ok(())
}
