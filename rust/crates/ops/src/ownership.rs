//! Remote handoff authority is independent of the local writer lock. Grants never expire implicitly.
use loom_core::{
    error::require,
    storage,
    store::{hash, id},
    Error, Result,
};
use serde_json::{json, Value};
use std::{io::Read, path::Path};
pub struct Grant {
    client: reqwest::blocking::Client,
    origin: reqwest::Url,
    token: String,
    request: Value,
}
impl Grant {
    pub fn claim(settings: Value, state: &Path) -> Result<Self> {
        let origin = reqwest::Url::parse(
            settings["url"]
                .as_str()
                .ok_or_else(|| Error::input("Remote authority origin required"))?,
        )
        .map_err(|_| Error::input("Invalid authority origin"))?;
        require(
            origin.username().is_empty()
                && origin.password().is_none()
                && origin.path() == "/"
                && origin.query().is_none()
                && origin.fragment().is_none()
                && (origin.scheme() == "https"
                    || origin.scheme() == "http"
                        && matches!(
                            origin.host_str(),
                            Some("127.0.0.1" | "localhost" | "fence-authority")
                        )),
            "authority",
            "Remote authority requires a credential-free HTTPS origin",
            400,
        )?;
        let token = std::fs::read_to_string(
            settings["token_path"]
                .as_str()
                .ok_or_else(|| Error::input("Authority token file required"))?,
        )?;
        let token = token.trim().to_string();
        require(
            !token.is_empty() && !token.contains(['\r', '\n']),
            "authority",
            "Invalid authority credential",
            403,
        )?;
        let mut client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy();
        if let Some(ca) = settings["ca_file"].as_str() {
            client = client.add_root_certificate(
                reqwest::Certificate::from_pem(&std::fs::read(ca)?)
                    .map_err(|_| Error::input("Invalid authority CA"))?,
            );
        }
        let binding = hash(serde_json::to_vec(&settings)?);
        let path = state.join("remote-worker-session.json");
        let session = if path.exists() {
            let old: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
            require(
                old["binding"] == binding,
                "authority",
                "Authority settings changed; reconcile the retained worker session before cutover",
                409,
            )?;
            old["session"]
                .as_str()
                .ok_or_else(|| Error::input("Invalid retained session"))?
                .to_string()
        } else {
            let session = format!("{}{}", id(), id());
            storage::atomic(
                &path,
                &serde_json::to_vec(&json!({"binding":binding,"session":session}))?,
                0o600,
            )?;
            session
        };
        let grant = Self {
            client: client.build().map_err(|_| {
                Error::new("authority", "Cannot initialize authority TLS client", 503)
            })?,
            origin,
            token,
            request: json!({"epoch":settings["epoch"],"binding":settings["binding"],"session":session}),
        };
        grant.request("claim")?;
        Ok(grant)
    }
    pub fn request(&self, action: &str) -> Result<()> {
        require(
            ["claim", "check", "release"].contains(&action),
            "authority",
            "Invalid worker authority operation",
            400,
        )?;
        let endpoint = self
            .origin
            .join(action)
            .map_err(|_| Error::input("Invalid authority route"))?;
        let mut response = self
            .client
            .post(endpoint)
            .bearer_auth(&self.token)
            .json(&self.request)
            .send()
            .map_err(|_| {
                Error::new(
                    "authority_unavailable",
                    "Remote worker ownership is unconfirmed",
                    503,
                )
            })?;
        require(
            response.status().is_success(),
            "authority",
            "Remote authority rejected this worker epoch/session",
            409,
        )?;
        let mut bytes = vec![];
        response.by_ref().take(65537).read_to_end(&mut bytes)?;
        require(
            bytes.len() <= 65536,
            "authority",
            "Authority response exceeds bound",
            503,
        )?;
        let response: Value = serde_json::from_slice(&bytes)?;
        require(
            response["authorized"] == true && response["epoch"] == self.request["epoch"],
            "authority",
            "Authority did not confirm this epoch/session",
            409,
        )
    }
}
