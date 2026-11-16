use axum::{
    body::{to_bytes, Body},
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    response::Response,
    Router,
};
use loom_core::{
    http::{HttpReply, HttpRequest},
    Application, Config, Error, Result,
};
use loom_ops::{worker, Service};
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::Ordering};
#[derive(Clone)]
struct Web {
    service: Service,
    dev: bool,
}
async fn request(
    State(state): State<Web>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    let result = async {
        let bytes = to_bytes(body, 2 * 1024 * 1024)
            .await
            .map_err(|_| Error::input("Request body exceeds bound"))?;
        let body = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes)?
        };
        let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in parts.headers.keys() {
            let values = parts
                .headers
                .get_all(name)
                .iter()
                .map(|v| {
                    v.to_str()
                        .map(String::from)
                        .map_err(|_| Error::input("Invalid header"))
                })
                .collect::<Result<Vec<_>>>()?;
            headers.insert(name.as_str().into(), values);
        }
        state
            .service
            .http(HttpRequest {
                method: parts.method.to_string(),
                path: parts.uri.to_string(),
                headers,
                peer: peer.ip().to_string(),
                body,
                dev: state.dev,
            })
            .await
    }
    .await;
    let reply = match result {
        Ok(r) => r,
        Err(e) => HttpReply::json(e.status, json!({"error":e.message,"code":e.code})),
    };
    let mut response = Response::new(Body::from(reply.body));
    *response.status_mut() =
        StatusCode::from_u16(reply.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(reply.content_type),
    );
    response.headers_mut().insert(
        "x-loom-revision",
        header::HeaderValue::from_str(
            loom_core::onboarding::identity()["revision"]
                .as_str()
                .expect("onboarding revision"),
        )
        .expect("hexadecimal revision"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        header::HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    response
}
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("loom-ops {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--guide-bundle") {
        use std::io::Write;
        let actor = loom_core::auth::person_actor("documentation");
        std::io::stdout().write_all(&loom_core::onboarding::bundle(&actor)?)?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--contracts") {
        println!(
            "{}",
            serde_json::to_string(&loom_core::contracts::actions())?
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--build-info") {
        println!("{}", loom_core::onboarding::identity());
        return Ok(());
    }
    let value = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    let config = PathBuf::from(
        value("--config")
            .map(String::from)
            .or_else(|| std::env::var("LOOM_API_CONFIG").ok())
            .ok_or_else(|| Error::input("--config required"))?,
    );
    let local = PathBuf::from(
        value("--state")
            .map(String::from)
            .or_else(|| std::env::var("LOOM_STATE").ok())
            .ok_or_else(|| Error::input("--state required"))?,
    );
    let data = PathBuf::from(
        value("--data")
            .map(String::from)
            .or_else(|| std::env::var("LOOM_DATA").ok())
            .ok_or_else(|| Error::input("--data required"))?,
    );
    let components = PathBuf::from(
        value("--components")
            .map(String::from)
            .or_else(|| std::env::var("LOOM_COMPONENTS").ok())
            .ok_or_else(|| Error::input("--components required"))?,
    );
    let cfg = Config::read(&config)?;
    if let Ok(worker_config) = std::env::var("LOOM_WORKER_CONFIG") {
        let worker = Config::read(std::path::Path::new(&worker_config))?;
        let mut api = serde_json::to_value(&cfg)?;
        let mut worker = serde_json::to_value(worker)?;
        api.as_object_mut().unwrap().remove("actors");
        worker.as_object_mut().unwrap().remove("actors");
        loom_core::error::require(
            api == worker,
            "config_parity",
            "API and worker configuration differ; reconcile before starting Rust authority",
            409,
        )?;
    }
    let _writer = loom_core::store::WriterLock::acquire(&local)?;
    let application = if args.iter().any(|a| a == "--read-only") {
        Application::open_read_only(&local, &data, &components, cfg.clone())?
    } else {
        Application::open(&local, &data, &components, cfg.clone())?
    };
    let grant = if !args.iter().any(|a| a == "--read-only") {
        if let Some(settings) = cfg.extra.get("remote_fence").cloned() {
            let state = local.clone();
            Some(
                tokio::task::spawn_blocking(move || {
                    loom_ops::ownership::Grant::claim(settings, &state)
                })
                .await
                .map_err(|_| Error::new("authority", "Authority driver stopped", 503))??,
            )
        } else {
            None
        }
    } else {
        None
    };
    let service = Service::start(application);
    let updates = if !args.iter().any(|a| a == "--read-only") {
        let policy = loom_ops::updates::Policy::read(
            loom_core::onboarding::identity()["build_id"]
                .as_str()
                .unwrap_or("unversioned")
                .into(),
        )?;
        Some(tokio::spawn(loom_ops::updates::run(
            local.clone(),
            policy,
            service.stopping.clone(),
        )))
    } else {
        None
    };
    let worker = if args.iter().any(|a| a == "--read-only") {
        None
    } else {
        Some(tokio::spawn(worker::run(
            service.clone(),
            cfg.clone(),
            data,
            components,
        )))
    };
    let dev = std::env::var("LOOM_DEV_NO_AUTH").as_deref() == Ok("1");
    let address = value("--listen")
        .unwrap_or("127.0.0.1:8080")
        .parse::<std::net::SocketAddr>()
        .map_err(|_| Error::input("Invalid listen address"))?;
    let app = Router::new().fallback(request).with_state(Web {
        service: service.clone(),
        dev,
    });
    let shutdown = service.stopping.clone();
    let signal = async move {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("SIGTERM handler");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
        shutdown.store(true, Ordering::Release);
    };
    let tls_handle = axum_server::Handle::new();
    let tls = if let (Ok(cert), Ok(key)) = (
        std::env::var("LOOM_TLS_CERT"),
        std::env::var("LOOM_TLS_KEY"),
    ) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let tls = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert, key).await?;
        let address = value("--tls-listen")
            .unwrap_or("0.0.0.0:8443")
            .parse::<std::net::SocketAddr>()
            .map_err(|_| Error::input("Invalid TLS address"))?;
        let listener = std::net::TcpListener::bind(address)?;
        let tls_app = app.clone();
        let handle = tls_handle.clone();
        Some(tokio::spawn(async move {
            axum_server::from_tcp_rustls(listener, tls)?
                .handle(handle)
                .serve(tls_app.into_make_service_with_connect_info::<std::net::SocketAddr>())
                .await
        }))
    } else {
        None
    };
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        signal.await;
        tls_handle.graceful_shutdown(Some(std::time::Duration::from_secs(20)));
    })
    .await?;
    if let Some(tls) = tls {
        tls.await
            .map_err(|_| Error::new("tls", "TLS listener task stopped", 503))??;
    }
    service.stopping.store(true, Ordering::Release);
    if let Some(worker) = worker {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(30), worker).await;
    }
    if let Some(updates) = updates {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(6), updates).await;
    }
    if let Some(grant) = grant {
        if service.drained().await? {
            tokio::task::spawn_blocking(move || grant.request("release"))
                .await
                .map_err(|_| Error::new("authority", "Authority release unconfirmed", 503))??;
        }
    }
    Ok(())
}
