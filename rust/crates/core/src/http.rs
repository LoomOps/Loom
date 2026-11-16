//! Bounded route adapter. Public routes invoke the same application operations as MCP.
use crate::{
    auth::{self, Actor},
    error::require,
    onboarding,
    store::*,
    Error, Result,
};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: BTreeMap<String, Vec<String>>,
    pub peer: String,
    pub body: Value,
    pub dev: bool,
}
#[derive(Debug)]
pub struct HttpReply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}
impl HttpReply {
    pub fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()),
        }
    }
}
impl Application {
    pub fn http(&mut self, r: HttpRequest) -> Result<HttpReply> {
        let path = r.path.split('?').next().unwrap_or(&r.path);
        if path == "/" && r.method == "GET" {
            return Ok(HttpReply::json(
                200,
                json!({"service":"Loom Ops API","mcp":"/mcp","status":"/api/status","engine":"rust"}),
            ));
        }
        let mut actor = auth::authenticate(&self.db, &self.config, &r.headers, &r.peer, r.dev)?;
        require(
            !self.read_only
                || r.method == "GET"
                || matches!(
                    path,
                    "/api/actions" | "/mcp" | "/api/history/list" | "/api/history/get"
                ),
            "read_only",
            "This instance is read-only",
            409,
        )?;
        if !self.read_only && !self.query_snapshot {
            self.register_person(&mut actor)?;
        }
        if path == "/mcp" {
            require(
                r.method == "POST",
                "method",
                "Stateless Streamable HTTP requires POST",
                405,
            )?;
            let accepted = r
                .headers
                .get("accept")
                .and_then(|a| a.first())
                .map(String::as_str)
                .unwrap_or("*/*");
            require(
                accepted.contains("application/json")
                    || accepted.contains("text/event-stream")
                    || accepted.contains("*/*"),
                "accept",
                "Accept application/json or text/event-stream",
                406,
            )?;
            if let Some(v) = r
                .headers
                .get("mcp-protocol-version")
                .and_then(|v| v.first())
            {
                require(
                    ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&v.as_str()),
                    "protocol",
                    "Unsupported MCP version",
                    400,
                )?;
            }
            let Some(reply) = self.mcp(&actor, r.body)? else {
                return Ok(HttpReply {
                    status: 202,
                    content_type: "application/json",
                    body: vec![],
                });
            };
            if accepted.contains("text/event-stream")
                && !accepted.contains("application/json")
                && !accepted.contains("*/*")
            {
                return Ok(HttpReply {
                    status: 200,
                    content_type: "text/event-stream",
                    body: format!(
                        "event: message\ndata: {}\n\n",
                        serde_json::to_string(&reply)?
                    )
                    .into_bytes(),
                });
            }
            return Ok(HttpReply::json(200, reply));
        }
        if path == "/api/actions" && r.method == "POST" {
            let value = self.execute(
                &actor,
                r.body["group"]
                    .as_str()
                    .ok_or_else(|| Error::input("Group required"))?,
                r.body["action"]
                    .as_str()
                    .ok_or_else(|| Error::input("Action required"))?,
                r.body.get("arguments").cloned().unwrap_or(json!({})),
            )?;
            return Ok(HttpReply::json(200, value));
        }
        if path.starts_with("/api/admin/tokens") {
            let value = self.admin_tokens(&actor, &r.method, path, &r.body)?;
            return Ok(HttpReply::json(200, value));
        }
        if path == "/api/guide/bundle" {
            actor.require("read")?;
            return Ok(HttpReply {
                status: 200,
                content_type: "application/zip",
                body: onboarding::bundle(&actor)?,
            });
        }
        if path == "/api/guide" || path.starts_with("/api/guide/") {
            actor.require("read")?;
            let args = if path == "/api/guide" {
                json!({})
            } else {
                json!({"id":path.trim_start_matches("/api/guide/")})
            };
            return Ok(HttpReply::json(200, onboarding::read(&args)?));
        }
        if matches!(
            path,
            "/api/dashboard/catalog"
                | "/api/dashboard/bootstrap"
                | "/api/dashboard/revision"
                | "/api/dashboard/resources"
        ) {
            actor.require("read")?;
            let value = if path.ends_with("revision") {
                json!({"revision":self.dashboard_revision()?})
            } else if path.ends_with("resources") {
                self.resources()?
            } else {
                let mut catalog = if path.ends_with("bootstrap") {
                    self.dashboard_bootstrap()?
                } else {
                    self.dashboard_catalog()?
                };
                let write = !self.read_only && actor.scopes.iter().any(|s| s == "submit");
                crate::query::queue_permissions(&mut catalog["queue"], write);
                catalog["capabilities"]["curate"] =
                    json!(!self.read_only && actor.scopes.iter().any(|s| s == "submit"));
                if path.ends_with("revision") {
                    json!({"revision":catalog["revision"]})
                } else {
                    catalog
                }
            };
            return Ok(HttpReply::json(200, value));
        }
        if path == "/api/events/delivery" && r.method == "POST" {
            require(
                !self.read_only,
                "read_only",
                "This instance is read-only",
                409,
            )?;
            return Ok(HttpReply::json(200, self.record_delivery(&actor, r.body)?));
        }
        let query = query_arguments(r.path.split_once('?').map(|(_, q)| q).unwrap_or(""))?;
        if r.method == "GET"
            && path.starts_with("/api/jobs/")
            && !matches!(
                path,
                "/api/jobs/contract" | "/api/jobs/queue" | "/api/jobs/profiles"
            )
        {
            return Ok(HttpReply::json(
                200,
                self.execute(
                    &actor,
                    "jobs",
                    "get",
                    json!({"id":decode_uri(path.trim_start_matches("/api/jobs/"))?}),
                )?,
            ));
        }
        if r.method == "GET" && path.starts_with("/api/registry/") {
            return Ok(HttpReply::json(200,self.execute(&actor,"results","get",json!({"id":decode_uri(path.trim_start_matches("/api/registry/"))?,"revision":query["revision"].as_i64().unwrap_or(1)}))?));
        }
        if let Some(rest) = path.strip_prefix("/api/items/") {
            if rest != "search" && rest != "resolve" {
                let (identity, suffix) = rest.split_once('/').unwrap_or((rest, ""));
                require(
                    r.method == if suffix == "files" { "POST" } else { "GET" },
                    "method",
                    "Unsupported Item method",
                    405,
                )?;
                let action = match suffix {
                    "" => "get_item",
                    "files" => "item_files",
                    "relations" => "item_relations",
                    _ => return Err(Error::input("Unknown Item path")),
                };
                let mut args = if r.method != "GET" && r.body.is_object() {
                    r.body.clone()
                } else {
                    query.clone()
                };
                args["item_id"] = json!(decode_uri(identity)?);
                return Ok(HttpReply::json(
                    200,
                    self.execute(&actor, "results", action, args)?,
                ));
            }
        }
        if let Some(rest) = path.strip_prefix("/api/files/") {
            let (identity, suffix) = rest.split_once('/').unwrap_or((rest, ""));
            require(
                r.method == if suffix == "read" { "POST" } else { "GET" },
                "method",
                "Unsupported file method",
                405,
            )?;
            let action = if suffix == "read" {
                "read_file"
            } else if suffix.is_empty() {
                "get_file"
            } else {
                return Err(Error::input("Unknown file path"));
            };
            let mut args = if r.method != "GET" && r.body.is_object() {
                r.body.clone()
            } else {
                query.clone()
            };
            args["file_item_id"] = json!(decode_uri(identity)?);
            return Ok(HttpReply::json(
                200,
                self.execute(&actor, "results", action, args)?,
            ));
        }
        let route = match (r.method.as_str(), path) {
            ("POST", "/api/logs/streams") => ("jobs", "log_streams"),
            ("POST", "/api/logs/read") => ("jobs", "read_log"),
            ("POST", "/api/logs/search") => ("jobs", "search_logs"),
            ("GET", "/api/dashboard/summary") => ("system", "dashboard_summary"),
            ("POST", "/api/items/search") => ("results", "search_items"),
            ("POST", "/api/items/resolve") => ("results", "resolve_item"),
            ("GET", "/api/search/status") => ("system", "search_status"),
            ("GET", "/api/status") => ("system", "status"),
            ("GET", "/api/jobs/contract") => ("system", "contract"),
            ("GET", "/api/jobs/queue") => ("jobs", "queue"),
            ("GET", "/api/jobs/profiles") => ("components", "profiles"),
            ("GET", "/api/jobs") => ("jobs", "list"),
            ("POST", "/api/jobs") => ("jobs", "submit"),
            ("POST", "/api/jobs/workflow") => ("jobs", "submit_workflow"),
            ("POST", "/api/jobs/cancel") => ("jobs", "cancel"),
            ("POST", "/api/research/curate") => ("jobs", "curate"),
            ("GET", "/api/events") => ("events", "list"),
            ("POST", "/api/events/ack") => ("events", "ack"),
            ("GET", "/api/registry") => ("results", "list"),
            ("GET", "/api/runtimes") => ("components", "runtimes"),
            ("GET", "/api/docker") => ("system", "docker"),
            ("GET", "/api/ssh") => ("system", "ssh"),
            ("GET", "/api/updates") => ("system", "updates"),
            ("POST", "/api/updates/check") => ("system", "check_updates"),
            ("POST", "/api/history/list") => ("results", "history"),
            ("POST", "/api/history/get") => ("results", "historical"),
            ("GET", "/api/principal") => return Ok(HttpReply::json(200, actor.public())),
            _ => return Err(Error::new("route", "Unknown HTTP route", 404)),
        };
        let value = self.execute(
            &actor,
            route.0,
            route.1,
            if r.body.is_null() { query } else { r.body },
        )?;
        Ok(HttpReply::json(200, value))
    }
    pub fn mcp(&mut self, actor: &Actor, body: Value) -> Result<Option<Value>> {
        let id = body.get("id").cloned().unwrap_or(Value::Null);
        if !body.is_object()
            || body["jsonrpc"] != "2.0"
            || !body["method"].is_string()
            || !(id.is_null() || id.is_string() || id.is_number())
            || body.get("params").is_some_and(|v| !v.is_object())
        {
            return Ok(Some(
                json!({"jsonrpc":"2.0","id":if id.is_string()||id.is_number(){id}else{Value::Null},"error":{"code":-32600,"message":"Invalid JSON-RPC request"}}),
            ));
        }
        let method = body["method"].as_str().unwrap();
        if body.get("id").is_none() {
            return Ok(None);
        }
        let result: Result<Value> = (|| match method {
            "initialize" => {
                let version = body["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or("2025-03-26");
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&version) {
                    version
                } else {
                    "2025-06-18"
                };
                Ok(
                    json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"loom-ops","title":"Loom Use","version":env!("CARGO_PKG_VERSION")},"instructions":onboarding::MCP_INSTRUCTIONS}),
                )
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools":onboarding::tools(actor)})),
            "tools/call" => {
                let group = body["params"]["name"]
                    .as_str()
                    .ok_or_else(|| Error::input("Tool group required"))?;
                let call = &body["params"]["arguments"];
                let action = call["action"]
                    .as_str()
                    .ok_or_else(|| Error::input("Action required"))?;
                let value = self.execute(
                    actor,
                    group,
                    action,
                    call.get("arguments").cloned().unwrap_or(json!({})),
                );
                match value {
                    Ok(v) => {
                        Ok(json!({"content":[{"type":"text","text":serde_json::to_string(&v)?}]}))
                    }
                    Err(e) => {
                        let incident = self.incident(&format!("{group}.{action}"), &e);
                        Ok(
                            json!({"isError":true,"content":[{"type":"text","text":e.message}],"incident_id":incident,"structuredContent":{"error":e.code,"incident_id":incident}}),
                        )
                    }
                }
            }
            _ => Err(Error::new("rpc_method", "Unknown method", 400)),
        })();
        let mut result: Value = match result {
            Ok(value) => value,
            Err(error) => {
                return Ok(Some(
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":if error.code=="rpc_method"{-32601}else{-32602},"message":error.message}}),
                ))
            }
        };
        let identity = onboarding::identity();
        if method == "tools/call" {
            if let Some(content) = result["content"].as_array_mut() {
                content.push(
                    json!({"type":"text","text":format!("Loom {} | contract {} | guide {}",
                    identity["build_id"].as_str().unwrap_or("unversioned"),
                    &identity["contract_revision"].as_str().expect("contract digest")[..12],
                    &identity["guide_revision"].as_str().expect("guide digest")[..12])}),
                );
            }
        }
        result["_meta"] = json!({"loom":identity});
        Ok(Some(json!({"jsonrpc":"2.0","id":id,"result":result})))
    }
    fn register_person(&mut self, actor: &mut Actor) -> Result<()> {
        if !matches!(
            actor.identity["provider"].as_str(),
            Some("dashboard" | "tailscale")
        ) {
            return Ok(());
        }
        let identity = actor.identity["profile_id"]
            .as_str()
            .unwrap_or(&actor.id)
            .to_string();
        let login = actor.identity["login"].as_str().unwrap_or(&identity);
        let name = actor.identity["name"].as_str().unwrap_or(login);
        self.db.execute("INSERT INTO user_profiles VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET display_name=excluded.display_name,updated=excluded.updated",params![identity,actor.identity["provider"].as_str(),login,name,now()])?;
        if let (Some(issuer), Some(subject)) = (
            actor.identity["issuer"].as_str(),
            actor.identity["subject"].as_str(),
        ) {
            self.db.execute(
                "INSERT OR IGNORE INTO auth_identities VALUES(?,?,?)",
                params![issuer, subject, identity],
            )?;
        }
        actor.extra.insert("profile_id".into(), json!(identity));
        Ok(())
    }
    fn admin_tokens(
        &mut self,
        actor: &Actor,
        method: &str,
        path: &str,
        args: &Value,
    ) -> Result<Value> {
        require(
            actor.identity["provider"] == "dashboard" && actor.identity["access_admin"] == true,
            "admin",
            "Dashboard Admin required",
            403,
        )?;
        let owners = rows(
            &self.db,
            "SELECT id,login,display_name FROM user_profiles ORDER BY login",
            &[],
        )?;
        let logins = actor.identity["token_owner_logins"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let eligible: Vec<_> = owners
            .iter()
            .filter(|p| {
                p["id"] == actor.id
                    || logins.iter().any(|l| {
                        l.as_str().is_some_and(|l| {
                            p["login"].as_str().is_some_and(|p| {
                                auth::casefold(l.trim()) == auth::casefold(p.trim())
                            })
                        })
                    })
            })
            .cloned()
            .collect();
        let mut eligible = eligible;
        for admin in actor.identity["token_owner_admins"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(profile) = self
                .db
                .query_row(
                    "SELECT profile_id FROM auth_identities WHERE issuer=? AND subject=?",
                    params![admin["issuer"].as_str(), admin["subject"].as_str()],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
            {
                if let Some(owner) = owners.iter().find(|p| p["id"] == profile) {
                    if !eligible.contains(owner) {
                        eligible.push(owner.clone());
                    }
                }
            }
        }
        if method == "GET" {
            let mut tokens=rows(&self.db,"SELECT id,label,owner,scopes,created,created_by,revoked,revoked_by FROM resident_tokens ORDER BY created DESC",&[])?;
            for token in &mut tokens {
                token["scopes"] = serde_json::from_str(token["scopes"].as_str().unwrap_or("[]"))?;
                token["expires"] = Value::Null;
            }
            return Ok(
                json!({"tokens":tokens,"owners":eligible,"owner_profiles":owners,"expires":null}),
            );
        }
        require(
            method == "POST",
            "method",
            "Token operations require POST",
            405,
        )?;
        if path.ends_with("/revoke") {
            require(
                args.as_object()
                    .is_some_and(|o| o.len() == 1 && o["id"].is_string()),
                "token",
                "Provide token id",
                400,
            )?;
            require(
                self.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM resident_tokens WHERE id=?)",
                    [args["id"].as_str()],
                    |r| r.get(0),
                )?,
                "token",
                "Token not found",
                404,
            )?;
            let _changed = self.db.execute(
                "UPDATE resident_tokens SET revoked=?,revoked_by=? WHERE id=? AND revoked IS NULL",
                params![now(), actor.id, args["id"].as_str()],
            )?;
            return Ok(json!({"id":args["id"],"revoked":true}));
        }
        require(
            args.as_object().is_some_and(|o| {
                o.len() == 3
                    && ["label", "owner", "scopes"]
                        .iter()
                        .all(|k| o.contains_key(*k))
            }),
            "token",
            "Provide label, owner and scopes",
            400,
        )?;
        let owner = args["owner"]
            .as_str()
            .ok_or_else(|| Error::input("Owner required"))?;
        require(
            eligible.iter().any(|o| o["id"] == owner),
            "owner",
            "Select approved member/Admin",
            403,
        )?;
        let label = args["label"]
            .as_str()
            .ok_or_else(|| Error::input("Label required"))?;
        require(
            !label.trim().is_empty() && label.len() <= 80 && !label.chars().any(char::is_control),
            "label",
            "Invalid token label",
            400,
        )?;
        let scopes = args["scopes"]
            .as_array()
            .ok_or_else(|| Error::input("Scopes required"))?;
        require(
            !scopes.is_empty()
                && scopes.iter().all(|v| {
                    v.as_str()
                        .is_some_and(|s| ["read", "submit", "cancel", "events"].contains(&s))
                }),
            "scopes",
            "Invalid token scopes",
            400,
        )?;
        let token = format!("loom_{}{}", id(), id());
        let identity = id();
        self.db.execute(
            "INSERT INTO resident_tokens VALUES(?,?,?,?,?,?,?,NULL,NULL)",
            params![
                identity,
                hash(&token),
                label.trim(),
                owner,
                encode(&json!(scopes))?,
                now(),
                actor.id
            ],
        )?;
        Ok(json!({"credential_id":identity,"token":token,"owner":owner,"expires":null}))
    }
}

fn decode_uri(value: &str) -> Result<String> {
    let mut bytes = vec![];
    let mut iter = value.bytes();
    while let Some(c) = iter.next() {
        if c == b'%' {
            let a = iter
                .next()
                .ok_or_else(|| Error::input("Invalid URI encoding"))?;
            let b = iter
                .next()
                .ok_or_else(|| Error::input("Invalid URI encoding"))?;
            let h = std::str::from_utf8(&[a, b])
                .map_err(|_| Error::input("Invalid URI encoding"))?
                .to_string();
            bytes.push(
                u8::from_str_radix(&h, 16).map_err(|_| Error::input("Invalid URI encoding"))?,
            );
        } else {
            bytes.push(if c == b'+' { b' ' } else { c });
        }
    }
    String::from_utf8(bytes).map_err(|_| Error::input("Invalid URI text"))
}
fn query_arguments(query: &str) -> Result<Value> {
    require(query.len() <= 8192, "query", "Query too long", 400)?;
    let mut args = serde_json::Map::new();
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| Error::input("Invalid query"))?;
        let key = decode_uri(k)?;
        require(
            !args.contains_key(&key),
            "query",
            "Repeated query argument",
            400,
        )?;
        let v = decode_uri(v)?;
        let value = if ["offset", "limit", "after", "revision"].contains(&key.as_str()) {
            json!(v
                .parse::<i64>()
                .map_err(|_| Error::input("Invalid numeric query"))?)
        } else if key == "include_consolidated" {
            match v.as_str() {
                "true" => json!(true),
                "false" => json!(false),
                _ => return Err(Error::input("Invalid boolean query")),
            }
        } else {
            json!(v)
        };
        args.insert(key, value);
    }
    Ok(json!(args))
}

/// A private in-process read context carries configuration, never a mutation RPC.
#[derive(Clone)]
pub struct ReadContext {
    config: Config,
    local: std::path::PathBuf,
    bulk: std::path::PathBuf,
    components: std::path::PathBuf,
    read_only: bool,
    cache: std::sync::Arc<std::sync::Mutex<Option<(String, Value)>>>,
    pub files: Vec<std::path::PathBuf>,
    pub inventories: Vec<(std::path::PathBuf, String, Value)>,
}
fn request_action(r: &HttpRequest) -> Option<(&str, &str, Value)> {
    let path = r.path.split('?').next()?;
    if path == "/api/actions" {
        Some((
            r.body["group"].as_str()?,
            r.body["action"].as_str()?,
            r.body.get("arguments").cloned().unwrap_or(json!({})),
        ))
    } else if path == "/mcp" && r.body["method"] == "tools/call" {
        let p = &r.body["params"];
        Some((
            p["name"].as_str()?,
            p["arguments"]["action"].as_str()?,
            p["arguments"]
                .get("arguments")
                .cloned()
                .unwrap_or(json!({})),
        ))
    } else if let Some((group, action)) = match path {
        "/api/items/search" => Some(("results", "search_items")),
        "/api/items/resolve" => Some(("results", "resolve_item")),
        "/api/logs/streams" => Some(("jobs", "log_streams")),
        "/api/logs/read" => Some(("jobs", "read_log")),
        "/api/logs/search" => Some(("jobs", "search_logs")),
        _ => None,
    } {
        Some((group, action, r.body.clone()))
    } else if let Some(rest) = path.strip_prefix("/api/items/") {
        let (id, suffix) = rest.split_once('/').unwrap_or((rest, ""));
        let action = match suffix {
            "" => "get_item",
            "files" => "item_files",
            "relations" => "item_relations",
            _ => return None,
        };
        let mut args = r.body.clone();
        if !args.is_object() {
            args = json!({});
        }
        args["item_id"] = json!(decode_uri(id).ok()?);
        Some(("results", action, args))
    } else if let Some(rest) = path.strip_prefix("/api/files/") {
        let (id, suffix) = rest.split_once('/').unwrap_or((rest, ""));
        let action = match suffix {
            "" => "get_file",
            "read" => "read_file",
            _ => return None,
        };
        let mut args = r.body.clone();
        if !args.is_object() {
            args = json!({});
        }
        args["file_item_id"] = json!(decode_uri(id).ok()?);
        Some(("results", action, args))
    } else {
        None
    }
}
pub fn is_read(r: &HttpRequest) -> bool {
    if r.method == "GET" {
        return !r.path.starts_with("/api/dashboard/media/");
    }
    if let Some((group, action, _)) = request_action(r) {
        return action != "check_updates"
            && crate::contracts::action(group, action).is_ok_and(|c| c.scope == "read");
    }
    matches!(r.path.as_str(), "/api/history/list" | "/api/history/get")
        || r.path == "/mcp"
            && matches!(
                r.body["method"].as_str(),
                Some("initialize" | "tools/list" | "notifications/initialized")
            )
}
pub fn is_heavy_read(r: &HttpRequest) -> bool {
    (r.path.starts_with("/api/dashboard/")
        && !matches!(
            r.path.as_str(),
            "/api/dashboard/resources" | "/api/dashboard/revision"
        ))
        || r.path.starts_with("/api/items/")
        || r.path.starts_with("/api/files/")
        || r.path.starts_with("/api/logs/")
        || request_action(r).is_some_and(|(group, action, _)| {
            matches!(
                (group, action),
                (
                    "results",
                    "search_items" | "item_files" | "read_file" | "get_file"
                ) | ("jobs", "logs" | "read_log" | "search_logs")
                    | (
                        "system",
                        "dashboard_page"
                            | "dashboard_entity"
                            | "dashboard_activity"
                            | "dashboard_summary"
                    )
            )
        })
}
pub fn needs_file_proofs(r: &HttpRequest) -> bool {
    request_action(r).is_some_and(|(group, action, args)| {
        group == "results"
            && matches!(
                action,
                "bind_historical_location" | "reconnect" | "reconcile_validate" | "reconcile_apply"
            )
            || group == "components"
                && matches!(action, "register" | "register_tool")
                && args["manifest"]["build_receipt"].is_object()
    })
}
impl Application {
    pub fn read_context(&mut self, r: &HttpRequest) -> Result<ReadContext> {
        if r.path != "/" {
            let mut actor = auth::authenticate(&self.db, &self.config, &r.headers, &r.peer, r.dev)?;
            if !self.read_only {
                self.register_person(&mut actor)?;
            }
        }
        let mut files = vec![];
        let mut inventories = vec![];
        if let Some((group, action, args)) = request_action(r).filter(|_| needs_file_proofs(r)) {
            let contract = crate::contracts::action(group, action)?;
            let actor = auth::authenticate(&self.db, &self.config, &r.headers, &r.peer, r.dev)?;
            actor.require(&contract.scope)?;
            crate::contracts::validate(&contract.schema, &args)?;
            if group == "results"
                && action == "bind_historical_location"
                && crate::store::replay(
                    &self.db,
                    &actor,
                    args["key"].as_str().unwrap_or(""),
                    "historical.bind",
                    &args,
                )?
                .is_none()
            {
                files.push(crate::storage::locator(
                    &self.config,
                    &self.bulk,
                    &args["locator"],
                )?);
            }
            if group == "results" && matches!(action, "reconcile_validate" | "reconcile_apply") {
                (files, inventories) =
                    self.reconciliation_inputs(&actor, args["plan_id"].as_str().unwrap_or(""))?;
            }
            if group == "results" && action == "reconnect" {
                let (root, _) = crate::storage::store(
                    &self.config,
                    &self.bulk,
                    args["storage"].as_str().unwrap_or("data/0"),
                    None,
                )?;
                files.push(crate::storage::relative(
                    &root,
                    args["ref"]
                        .as_str()
                        .ok_or_else(|| Error::input("Location reference required"))?,
                )?);
            }
            if group == "components" && args["manifest"]["build_receipt"].is_object() {
                files.push(crate::storage::locator(
                    &self.config,
                    &self.bulk,
                    &args["manifest"]["build_receipt"]["binary"],
                )?);
            }
        }
        Ok(ReadContext {
            config: self.config.clone(),
            local: self.local.clone(),
            bulk: self.bulk.clone(),
            components: self.components.clone(),
            read_only: self.read_only,
            cache: self.catalog_cache.clone(),
            files,
            inventories,
        })
    }
    pub fn http_verified(
        &mut self,
        r: HttpRequest,
        proofs: crate::reconcile::PreparedFiles,
    ) -> Result<HttpReply> {
        self.file_proofs = proofs.files;
        self.file_sets = proofs.inventories;
        let result = self.http(r);
        self.file_proofs.clear();
        self.file_sets.clear();
        result
    }
    pub fn verified_file(&self, path: &std::path::Path) -> Result<crate::storage::VerifiedFile> {
        if let Some(proof) = self.file_proofs.get(path) {
            proof.revalidate()?;
            Ok(proof.clone())
        } else {
            crate::storage::VerifiedFile::read(path)
        }
    }
}
impl ReadContext {
    pub fn prepare_files(&self) -> Result<crate::reconcile::PreparedFiles> {
        let mut prepared = crate::reconcile::PreparedFiles::default();
        for path in &self.files {
            prepared
                .files
                .insert(path.clone(), crate::storage::VerifiedFile::read(path)?);
        }
        if !self.inventories.is_empty() {
            let app = Application::open_read_only(
                &self.local,
                &self.bulk,
                &self.components,
                self.config.clone(),
            )?;
            for (base, selector, declaration) in &self.inventories {
                let key = crate::reconcile::inventory_key(base, declaration)?;
                prepared.inventories.insert(
                    key,
                    crate::file_set::seal(&app, base, selector, declaration)?,
                );
            }
        }
        Ok(prepared)
    }
    pub fn http(self, r: HttpRequest) -> Result<HttpReply> {
        self.http_with_logs(r, None)
    }
    pub fn live_file_request(&self, r: &HttpRequest) -> Result<Option<(Value, Value)>> {
        let Some(("results", "read_file", args)) = request_action(r) else {
            return Ok(None);
        };
        let Some(executor) = self.config.extra.get("executor") else {
            return Ok(None);
        };
        let app = Application::open_read_only(
            &self.local,
            &self.bulk,
            &self.components,
            self.config.clone(),
        )?;
        let actor = auth::authenticate(&app.db, &app.config, &r.headers, &r.peer, r.dev)?;
        actor.require("read")?;
        crate::contracts::validate(
            &crate::contracts::action("results", "read_file")?.schema,
            &args,
        )?;
        let file = app.get_file(&args)?;
        if crate::inventory::open_copy(&app, &file).is_ok() {
            return Ok(None);
        }
        let Some(copy) = file["copies"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["type"] == "runtime")
        else {
            return Ok(None);
        };
        Ok(Some((
            executor.clone(),
            json!({"runtime_ref":copy["runtime_ref"],"entry_key":copy["entry_key"],"inventory_sha256":copy["inventory_sha256"],"offset":args["offset"].as_str().unwrap_or("0"),"limit":args["limit"].as_u64().unwrap_or(65536).min(262144)}),
        )))
    }
    pub fn live_log_request(&self, r: &HttpRequest) -> Result<Option<(Value, Value)>> {
        let Some(("jobs", action @ ("logs" | "read_log"), args)) = request_action(r) else {
            return Ok(None);
        };
        let Some(executor) = self.config.extra.get("executor") else {
            return Ok(None);
        };
        let app = Application::open_read_only(
            &self.local,
            &self.bulk,
            &self.components,
            self.config.clone(),
        )?;
        let actor = auth::authenticate(&app.db, &app.config, &r.headers, &r.peer, r.dev)?;
        actor.require("read")?;
        crate::contracts::validate(&crate::contracts::action("jobs", action)?.schema, &args)?;
        if action == "read_log" {
            let (_, work) = crate::log_index::source(&app, &args["stream_id"])?;
            let state = crate::log_index::read_state(&app, &work)?;
            crate::log_index::retained(
                &app,
                &work,
                args["source_generation"]
                    .as_str()
                    .or(state["generation"].as_str())
                    .unwrap_or(&work.effect),
            )?;
            if crate::log_index::read_generation(&app, &work, args["source_generation"].as_str())
                .is_ok_and(|(_, s)| s["closed"] == true)
            {
                return Ok(None);
            }
            let evidence: Option<String> = app
                .db
                .query_row(
                    "SELECT evidence FROM rust_effects WHERE id=?",
                    [&work.effect],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            let Some(evidence) = evidence else {
                return Ok(None);
            };
            let value: Value = serde_json::from_str(&evidence)?;
            let Some(ticket) = value["ticket"].as_str() else {
                return Ok(None);
            };
            let offset = args["offset"]
                .as_str()
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| Error::input("Invalid byte offset"))?;
            return Ok(Some((
                executor.clone(),
                json!({"ticket":ticket,"id":work.job,"attempt":work.attempt,"offset":offset,"limit":args["limit"].as_u64().unwrap_or(65536).min(262144),"tail":args["tail"]==true,"source_generation":args["source_generation"]}),
            )));
        }
        let job = crate::jobs::load(&app.db, args["id"].as_str().unwrap_or(""))?;
        let attempt = args["attempt"]
            .as_str()
            .or(job.aggregate.fence.attempt.as_ref().map(|a| a.as_str()))
            .ok_or_else(|| Error::input("Attempt required"))?;
        app.require_log_attempt(job.aggregate.id.as_str(), attempt)?;
        crate::log_index::legacy_guard(&app, job.aggregate.id.as_str(), attempt)?;
        let evidence: Option<String> = app.db.query_row("SELECT evidence FROM rust_effects WHERE job=? AND attempt=? AND stage='run' AND state IN ('accepted','unknown','returned') ORDER BY created DESC LIMIT 1",params![job.aggregate.id.as_str(),attempt],|row|row.get(0)).optional()?.flatten();
        let Some(evidence) = evidence else {
            return Ok(None);
        };
        let value: Value = serde_json::from_str(&evidence)?;
        let Some(ticket) = value["ticket"].as_str() else {
            return Ok(None);
        };
        Ok(Some((
            executor.clone(),
            json!({"ticket":ticket,"id":job.aggregate.id.as_str(),"attempt":attempt,"offset":args["offset"].as_u64().unwrap_or(0),"limit":args["limit"].as_u64().unwrap_or(65536).min(65536)}),
        )))
    }
    pub fn http_with_logs(self, r: HttpRequest, logs: Option<Result<Value>>) -> Result<HttpReply> {
        self.http_with_sources(r, logs, None)
    }
    pub fn http_with_file(self, r: HttpRequest, file: Result<Value>) -> Result<HttpReply> {
        self.http_with_sources(r, None, Some(file))
    }
    fn http_with_sources(
        self,
        r: HttpRequest,
        logs: Option<Result<Value>>,
        file: Option<Result<Value>>,
    ) -> Result<HttpReply> {
        let mut app =
            Application::open_read_only(&self.local, &self.bulk, &self.components, self.config)?;
        app.query_snapshot = true;
        app.live_logs = logs;
        app.live_file = file;
        app.read_only = self.read_only;
        app.catalog_cache = self.cache;
        app.db.execute_batch("BEGIN")?;
        let result = app.http(r);
        let _ = app.db.execute_batch("ROLLBACK");
        result
    }
}
