//! System responsibility is a presentation boundary, never an authentication grant.
use crate::{auth::Actor, error::require, store::*, Result};
use serde_json::{json, Value};

pub(crate) fn require_admin(config: &Config, actor: &Actor) -> Result<()> {
    let administrator = actor.credential_id.is_none()
        && ((actor.identity["provider"] == "dashboard" && actor.identity["access_admin"] == true)
            || (actor.identity.is_null()
                && actor.token_sha256.is_some()
                && config.actors.iter().any(|a| {
                    a.id == "owner" && a.id == actor.id && a.token_sha256 == actor.token_sha256
                })));
    require(
        administrator,
        "system_owner",
        "System responsibility requires the installation administrator",
        403,
    )
}
pub(crate) fn person() -> Value {
    json!({"id":"system","provider":"system","display_name":"System"})
}
/// Read current attribution from authority, including while the search index lags.
/// Expressions and database prefixes are internal constants, never caller SQL.
pub(crate) fn native_sql(native: &str, database: &str) -> String {
    let f = |key: &str| format!("json_extract({native},'$.{key}')");
    let id = f("identity.id");
    let namespace = f("identity.owner");
    let profile = f("identity.kind");
    let owner = |kind: &str, key: &str| {
        format!("(SELECT owner FROM {database}item_owners WHERE kind='{kind}' AND id={key})")
    };
    let job = |key: &str| {
        format!("COALESCE({},(SELECT owner FROM {database}jobs WHERE id={key}),(SELECT owner FROM {database}job_workflows WHERE id={key}),'')='system'",owner("job",key))
    };
    let source=format!("(SELECT body FROM {database}catalog_sources WHERE kind='profile' AND json_extract(identity,'$.kind')={profile} LIMIT 1)");
    let ns = format!("COALESCE({namespace},json_extract({source},'$.origin.owner'),'unrecorded')");
    let pid = format!("COALESCE({id},json_extract({source},'$.origin.id'),{profile})");
    let pk = format!("({ns}||':'||{pid})");
    let software_key = format!("({namespace}||':'||{id})");
    format!("CASE {} WHEN 'job' THEN {} WHEN 'entity' THEN (COALESCE({},(SELECT owner FROM {database}all_entity_revisions WHERE id={id} AND revision={}),'')='system' OR EXISTS(SELECT 1 FROM {database}all_entity_revisions se WHERE se.id={id} AND se.revision={} AND {})) WHEN 'profile' THEN ({profile}='cpu.smoke' OR COALESCE(CASE WHEN json_extract({source},'$.origin.type')='component' THEN {} ELSE {} END,{ns})='system') WHEN 'component' THEN COALESCE({},{namespace},'')='system' WHEN 'tool' THEN COALESCE({},{namespace},'')='system' WHEN 'runtime' THEN COALESCE({},'')='system' WHEN 'historical' THEN COALESCE((SELECT owner FROM {database}item_owners WHERE kind=CASE WHEN {}='job_view' THEN 'job' ELSE 'entity' END AND id={id}),'')='system' ELSE 0 END", f("kind"),job(&id),owner("entity",&id),f("identity.revision"),f("identity.revision"),job("se.job"),owner("component",&pk),owner("profile",&pk),owner("component",&software_key),owner("tool",&software_key),owner("runtime",&format!("('host:'||{id})")),f("identity.kind"))
}
impl Application {
    pub fn system_native(&self, native: &Value) -> Result<bool> {
        let sql = format!(
            "SELECT COALESCE(({}),0) FROM (SELECT ? AS native)",
            native_sql("native", "")
        );
        Ok(self
            .db
            .prepare_cached(&sql)?
            .query_row([encode(native)?], |r| r.get(0))?)
    }
    pub fn system_job(&self, id: &str) -> Result<bool> {
        self.system_native(&json!({"kind":"job","identity":{"id":id}}))
    }
    pub(crate) fn dashboard_entity_is_system(&self, entity: &Value) -> Result<bool> {
        if matches!(
            entity["kind"].as_str(),
            Some("execution_profile" | "runtime")
        ) && entity["owner_kind"] == "system"
        {
            return Ok(true);
        }
        let native = if let Some(h) = entity.get("loom_history") {
            json!({"kind":"historical","identity":h})
        } else if let Some(p) = entity.get("loom_execution_profile") {
            json!({"kind":"profile","identity":{"kind":p["kind"]}})
        } else if let Some(r) = entity.get("loom_runtime") {
            json!({"kind":"runtime","identity":{"id":r["id"]}})
        } else if let Some(s) = entity.get("loom_extension") {
            json!({"kind":entity["kind"],"identity":{"owner":s["owner"],"id":s["id"]}})
        } else if entity["kind"] == "job" {
            json!({"kind":"job","identity":{"id":entity["id"]}})
        } else if let Some(r) = entity.get("loom_registry") {
            json!({"kind":"entity","identity":{"id":r["id"],"revision":r["revision"]}})
        } else {
            return Ok(entity["owner"] == "system"
                || entity["owner_kind"] == "system"
                || entity["owner_profile"]["id"] == "system");
        };
        self.system_native(&native)
    }
    pub(crate) fn dashboard_queue(&self) -> Result<Value> {
        self.dashboard_queue_with_detail(false)
    }
    pub(crate) fn dashboard_queue_with_detail(&self, compact: bool) -> Result<Value> {
        let mut queue = self.queue_snapshot(false, compact)?;
        for key in ["entries", "logical_entries"] {
            if let Some(entries) = queue[key].as_array_mut() {
                let mut visible = Vec::with_capacity(entries.len());
                for entry in std::mem::take(entries) {
                    if !self.system_job(entry["id"].as_str().unwrap_or(""))? {
                        visible.push(entry);
                    }
                }
                *entries = visible;
            }
        }
        Ok(queue)
    }
}
