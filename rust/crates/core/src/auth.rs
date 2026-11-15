//! Authentication adapters produce grants; callers cannot forge application roles.
use crate::{error::require, store::Config, Error, Result};
use base64::Engine;
pub fn casefold(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        if let Ok(i) = crate::casefold::FOLDS.binary_search_by_key(&ch, |(c, _)| *c) {
            result.push_str(crate::casefold::FOLDS[i].1);
        } else {
            result.push(ch);
        }
    }
    result
}
use hmac::{Hmac, Mac};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Actor {
    pub id: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub notify: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    #[serde(default)]
    pub identity: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
impl Actor {
    pub fn require(&self, scope: &str) -> Result<()> {
        require(
            !self.id.is_empty() && self.scopes.iter().any(|v| v == scope),
            "forbidden",
            "Operation scope denied",
            403,
        )
    }
    pub fn public(&self) -> Value {
        json!({"id":self.id,"scopes":self.scopes,"owners":self.owners,"notify":self.notify,"credential_id":self.credential_id})
    }
}
fn equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |x, (a, b)| x | (a ^ b)) == 0
}
pub fn validate_current(db: &Connection, a: &Actor) -> Result<()> {
    if let Some(id) = &a.credential_id {
        let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM resident_tokens WHERE id=? AND revoked IS NULL AND owner=?)",params![id,a.id],|r|r.get(0))?;
        require(valid, "credential", "Credential revoked", 401)?;
    }
    Ok(())
}
pub fn authenticate(
    db: &Connection,
    config: &Config,
    headers: &BTreeMap<String, Vec<String>>,
    peer: &str,
    dev: bool,
) -> Result<Actor> {
    if headers.contains_key("origin") {
        return Err(Error::new("origin", "Cross-origin request denied", 403));
    }
    let one = |name: &str| -> Result<Option<&str>> {
        let Some(v) = headers.get(name) else {
            return Ok(None);
        };
        require(v.len() == 1, "identity", "Duplicate identity header", 403)?;
        Ok(v.first().map(String::as_str))
    };
    if let Some(header) = one("authorization")? {
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::new("credential", "Bearer token required", 401))?;
        let digest = crate::store::hash(token);
        for a in &config.actors {
            if a.token_sha256.as_ref().is_some_and(|s| equal(s, &digest)) {
                return Ok(a.clone());
            }
        }
        let dynamic:Option<(String,String)>=db.query_row("SELECT p.id,p.grant_json FROM credentials t JOIN principals p ON p.id=t.principal WHERE t.digest=? AND t.revoked IS NULL AND t.expires>?",params![digest,crate::store::now()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((id, grants)) = dynamic {
            let mut v: Value = serde_json::from_str(&grants)?;
            v["id"] = json!(id);
            return Ok(serde_json::from_value(v)?);
        }
        let resident: Option<(String, String, String)> = db
            .query_row(
                "SELECT id,owner,scopes FROM resident_tokens WHERE digest=? AND revoked IS NULL",
                [digest],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((id, owner, scopes)) = resident {
            return Ok(serde_json::from_value(
                json!({"id":owner,"scopes":serde_json::from_str::<Value>(&scopes)?,"owners":[owner],"notify":[owner],"credential_id":id}),
            )?);
        }
        return Err(Error::new(
            "credential",
            "Invalid or expired credential",
            401,
        ));
    }
    if let Some(assertion) = one("x-loom-dashboard-identity")? {
        let policy = config.extra.get("dashboard_auth").unwrap_or(&Value::Null);
        require(
            policy["enabled"] == true && assertion.len() <= 8192,
            "identity",
            "Invalid dashboard identity",
            403,
        )?;
        let (payload, signature) = assertion
            .split_once('.')
            .ok_or_else(|| Error::new("identity", "Invalid dashboard identity", 403))?;
        let path = policy["relay_key_file"]
            .as_str()
            .ok_or_else(|| Error::new("identity", "Dashboard trust unavailable", 503))?;
        let key = std::fs::read(path)?;
        let key = key.trim_ascii();
        require(
            key.len() >= 32,
            "identity",
            "Dashboard trust unavailable",
            503,
        )?;
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key)
            .map_err(|_| Error::input("Invalid trust key"))?;
        mac.update(payload.as_bytes());
        let expected = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        require(
            equal(&expected, signature),
            "identity",
            "Invalid dashboard identity",
            403,
        )?;
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| Error::input("Invalid identity encoding"))?;
        let claims: Value = serde_json::from_slice(&raw)?;
        let now = crate::store::now();
        let expiry = claims["exp"].as_f64().unwrap_or(0.);
        require(
            claims["aud"] == "loom-dashboard-relay" && expiry > now && expiry <= now + 60.,
            "identity",
            "Expired dashboard identity",
            403,
        )?;
        let mut person = claims["identity"].clone();
        require(
            person.is_object()
                && person["issuer"] == policy["issuer"]
                && person["subject"].as_str().is_some_and(|s| !s.is_empty()),
            "identity",
            "Invalid dashboard issuer",
            403,
        )?;
        let login = casefold(person["login"].as_str().unwrap_or("").trim());
        let link = policy["link_tailscale_email"] == true && !login.is_empty();
        let principal = if link {
            format!("ts-{}", &crate::store::hash(&login)[..24])
        } else {
            format!(
                "identity-{}",
                &crate::store::hash(format!(
                    "{}\0{}",
                    person["issuer"].as_str().unwrap_or(""),
                    person["subject"].as_str().unwrap_or("")
                ))[..24]
            )
        };
        let principal = db
            .query_row(
                "SELECT profile_id FROM auth_identities WHERE issuer=? AND subject=?",
                params![person["issuer"].as_str(), person["subject"].as_str()],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or(principal);
        person["provider"] = json!("dashboard");
        person["profile_id"] = json!(principal);
        let binding = config
            .extra
            .get("tailscale_auth")
            .and_then(|p| p["users"].get(&login))
            .and_then(Value::as_str);
        let mut actor = if let Some(binding) = binding {
            config
                .actors
                .iter()
                .find(|a| a.id == binding)
                .cloned()
                .ok_or_else(|| Error::new("identity", "Principal is not configured", 403))?
        } else {
            person_actor(&principal)
        };
        actor.identity = person;
        return Ok(actor);
    }
    let policy = config.extra.get("tailscale_auth").unwrap_or(&Value::Null);
    if policy["enabled"] == true
        && policy["trusted_proxies"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(peer)))
    {
        if let Some(login) = one("tailscale-user-login")? {
            require(
                !login.trim().is_empty() && login.len() <= 320,
                "identity",
                "Invalid Tailscale identity",
                403,
            )?;
            let login = casefold(login.trim());
            let binding = policy["users"]
                .as_object()
                .and_then(|o| o.iter().find(|(k, _)| casefold(k) == login))
                .and_then(|(_, v)| v.as_str());
            let mut a = if let Some(binding) = binding {
                config
                    .actors
                    .iter()
                    .find(|a| a.id == binding)
                    .cloned()
                    .ok_or_else(|| Error::new("identity", "Principal is not configured", 403))?
            } else {
                require(
                    policy["allow_new_users"] == true,
                    "identity",
                    "User is not registered",
                    403,
                )?;
                person_actor(&format!("ts-{}", &crate::store::hash(&login)[..24]))
            };
            a.identity = json!({"provider":"tailscale","login":login,"tailnet_role":null});
            return Ok(a);
        }
    }
    if dev {
        if let Some(a) = config.actors.iter().find(|a| a.id == "owner") {
            return Ok(a.clone());
        }
    }
    Err(Error::new("credential", "Bearer token required", 401))
}
pub fn person_actor(id: &str) -> Actor {
    Actor {
        id: id.into(),
        scopes: ["read", "submit", "cancel", "events"]
            .map(String::from)
            .to_vec(),
        owners: vec![id.into()],
        notify: vec![id.into()],
        identity: Value::Null,
        token_sha256: None,
        credential_id: None,
        extra: BTreeMap::new(),
    }
}
