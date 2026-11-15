//! External execution driver. It reports observations; it cannot choose Job states.
use crate::Service;
use loom_core::{
    error::require,
    execution::Work,
    storage,
    store::{hash, Config},
    Error, Result,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};
use tokio::{
    process::Command,
    time::{sleep, Duration},
};
async fn host(config: &Value, action: &str, value: Value) -> Result<Value> {
    let socket = PathBuf::from(
        config["socket"]
            .as_str()
            .ok_or_else(|| Error::input("Executor socket required"))?,
    );
    let credential = PathBuf::from(
        config["credential"]
            .as_str()
            .ok_or_else(|| Error::input("Executor credential required"))?,
    );
    let action = action.to_string();
    tokio::task::spawn_blocking(move || {
        loom_host::wire::call(&socket, &credential, &action, &value)
    })
    .await
    .map_err(|_| Error::new("executor", "Executor driver stopped", 503))?
}
pub async fn run(service: Service, config: Config, bulk: PathBuf, components: PathBuf) {
    let mut tasks: tokio::task::JoinSet<(String, Result<()>)> = tokio::task::JoinSet::new();
    let mut exclude = BTreeSet::new();
    let observer_service = service.clone();
    let observer_executor = config.extra.get("executor").cloned();
    let observer = tokio::spawn(async move {
        if let Some(executor) = observer_executor {
            while !observer_service.stopping.load(Ordering::Acquire) {
                if let Ok(mut value) = host(&executor, "list_runtimes", json!({})).await {
                    for item in value["runtimes"].as_array_mut().into_iter().flatten() {
                        if item["manifest_summary"] == true {
                            if let Ok(exact)=host(&executor,"resolve_runtime",json!({"runtime_ref":{"id":item["id"],"revision":item["revision"]}})).await{*item=exact;}
                        }
                    }
                    let _ = observer_service.runtimes(value).await;
                }
                sleep(Duration::from_secs(5)).await;
            }
        }
    });
    loop {
        while let Some(result) = tasks.try_join_next() {
            if let Ok((id, result)) = result {
                exclude.remove(&id);
                if let Err(error) = result {
                    eprintln!("Loom work observation: {}", error.code);
                }
            }
        }
        if service.stopping.load(Ordering::Acquire) && tasks.is_empty() {
            break;
        }
        if !service.stopping.load(Ordering::Acquire) && tasks.len() < 2 {
            match service.next(exclude.clone()).await {
                Ok(Some(work)) => {
                    exclude.insert(work.effect.clone());
                    let s = service.clone();
                    let c = config.clone();
                    let b = bulk.clone();
                    let p = components.clone();
                    tasks.spawn(async move {
                        let id = work.effect.clone();
                        (id, perform(s, c, b, p, work).await)
                    });
                }
                Ok(None) => {}
                Err(e) => eprintln!("Loom worker admission: {}", e.code),
            }
        }
        sleep(Duration::from_millis(250)).await;
    }
    observer.abort();
}
async fn perform(
    service: Service,
    config: Config,
    bulk: PathBuf,
    components: PathBuf,
    work: Work,
) -> Result<()> {
    let result = perform_inner(service.clone(), config, bulk, components, work.clone()).await;
    if result.is_err() && !work.publication {
        let sent = work
            .folder
            .join(format!(".effect-{}.json", work.effect))
            .exists()
            || work
                .folder
                .join(format!(".native-{}.json", work.effect))
                .exists();
        if !sent {
            let _=service.returned(work,1,json!({"returned":true,"not_started":true,"reason":"execution_preflight_failed"})).await;
        }
    }
    result
}
async fn perform_inner(
    service: Service,
    config: Config,
    bulk: PathBuf,
    components: PathBuf,
    work: Work,
) -> Result<()> {
    if work.publication {
        let bytes = tokio::fs::read(work.folder.join("result.json"))
            .await
            .map_err(Error::from);
        let manifest = bytes
            .and_then(|b| {
                require(
                    b.len() <= 8 * 1024 * 1024,
                    "manifest",
                    "Result manifest exceeds bound",
                    400,
                )?;
                Ok(serde_json::from_slice(&b)?)
            })
            .unwrap_or(Value::Null);
        let prepare_folder = work.folder.clone();
        let prepare_manifest = manifest.clone();
        let proofs =
            tokio::task::spawn_blocking(move || verify_outputs(&prepare_folder, &prepare_manifest))
                .await
                .map_err(|_| Error::new("verification", "Output verification task stopped", 503))?
                .unwrap_or_default();
        let _ = service.publish(work, manifest, proofs).await?;
        return Ok(());
    }
    let marker = work.folder.join(format!(".effect-{}.json", work.effect));
    let native_marker = work.folder.join(format!(".native-{}.json", work.effect));
    let already_sent = marker.exists() || native_marker.exists();
    if service.cancelled(work.clone()).await? {
        if marker.exists() {
            let frozen: Value = serde_json::from_slice(&tokio::fs::read(&marker).await?)?;
            let executor = config
                .extra
                .get("executor")
                .ok_or_else(|| Error::input("Executor configuration required"))?;
            let state = match host(executor, "cancel_intent", frozen["request"].clone()).await {
                Ok(state) => state,
                Err(error) => {
                    service.unknown(work).await?;
                    return Err(error);
                }
            };
            require(
                state["returned"] == true,
                "return",
                "Cancellation owner has not returned",
                503,
            )?;
            service
                .returned(
                    work,
                    state["exit_code"].as_i64().unwrap_or(137) as i32,
                    state,
                )
                .await?;
            return Ok(());
        } else if !native_marker.exists() {
            service
                .returned(
                    work,
                    137,
                    json!({"returned":true,"not_started":true,"reason":"cancelled_before_launch"}),
                )
                .await?;
            return Ok(());
        }
    }
    let mut request = service.request(work.clone()).await?;
    tokio::fs::create_dir_all(&work.folder).await?;
    let verified_request = request.clone();
    let verified_components = components.clone();
    let verification =
        tokio::task::spawn_blocking(move || verify_inputs(&verified_request, &verified_components))
            .await
            .map_err(|_| Error::new("verification", "Input verification task stopped", 503))?;
    if let Err(e) = verification {
        if already_sent {
            service.unknown(work).await?;
        } else {
            service.returned(work,1,json!({"returned":true,"not_started":true,"reason":"input_verification_failed"})).await?;
        }
        return Err(e);
    }
    if work.spec["kind"] == "cpu.smoke" {
        return run_builtin(service, work, &mut request).await;
    }
    let runner = request["runner"].clone();
    require(
        runner["executor"] == "container",
        "runtime",
        "An explicit container Runtime is required for user software",
        409,
    )?;
    let executor = config
        .extra
        .get("executor")
        .ok_or_else(|| Error::new("executor", "Container executor not configured", 503))?;
    let selector = work.spec["output_storage"].as_str().unwrap_or("data/0");
    let (output_root, _) = storage::store(
        &config,
        &bulk,
        selector,
        work.spec["_output_storage_id"].as_str(),
    )?;
    let host_root = if selector == "data/0" {
        executor["host_data"].as_str()
    } else {
        executor["storage"][selector].as_str()
    }
    .ok_or_else(|| Error::input("Output storage outside executor grant"))?;
    let relative = work
        .folder
        .strip_prefix(&output_root)
        .map_err(|_| Error::input("Output leaves storage"))?;
    let output = PathBuf::from(host_root).join(relative);
    let mut inputs = serde_json::Map::new();
    for (name, input) in request["verified_inputs"].as_object().into_iter().flatten() {
        let selector = input["storage"].as_str().unwrap_or("data/0");
        if selector == "data/0" {
            inputs.insert(
                name.clone(),
                json!(PathBuf::from(
                    executor["host_data"]
                        .as_str()
                        .ok_or_else(|| Error::input("Host data required"))?
                )
                .join(input["ref"].as_str().unwrap_or(""))),
            );
        } else {
            inputs.insert(name.clone(), json!({"storage":selector,"ref":input["ref"]}));
        }
    }
    request.as_object_mut().unwrap().remove("verified_inputs");
    request.as_object_mut().unwrap().remove("runner");
    request.as_object_mut().unwrap().remove("source_pins");
    request.as_object_mut().unwrap().remove("component_path");
    request["output_dir"] = json!("/work");
    let profile = &work.spec["profile"];
    let mut tools = serde_json::Map::new();
    let mut declared_tools = serde_json::Map::new();
    let host_components = PathBuf::from(
        executor["host_components"]
            .as_str()
            .unwrap_or(components.to_str().unwrap_or("")),
    );
    for (alias, pin) in profile["tool_pins"].as_object().into_iter().flatten() {
        let path = Path::new(
            pin["path"]
                .as_str()
                .ok_or_else(|| Error::input("Tool pin path required"))?,
        );
        let relative = path
            .strip_prefix(&components)
            .map_err(|_| Error::input("Tool pin outside configured root"))?;
        tools.insert(alias.clone(), json!(host_components.join(relative)));
        declared_tools.insert(alias.clone(), json!(format!("/tools/{alias}")));
    }
    request["tools"] = json!(declared_tools);
    if let Some(pin) = runner.get("runtime_pin") {
        request["runtime"] = pin.clone();
    }
    if profile["component_path"].is_string() {
        request["component_path"] = json!("/component");
    }
    let name = format!("{}-request.json", work.stage);
    storage::atomic(
        &work.folder.join(&name),
        &serde_json::to_vec(&request)?,
        0o600,
    )?;
    let argv = runner["command"]
        .as_array()
        .ok_or_else(|| Error::input("Stage command required"))?;
    let mut command = argv.clone();
    command.push(json!(format!("/work/{name}")));
    let mut payload = json!({"ticket":if work.hook||work.stage=="finalize"{format!("{}.{}.{}.r{}",work.job,work.attempt,work.stage,work.generation)}else{format!("{}.{}.{}",work.job,work.attempt,work.stage)},"job":work.job,"owner":work.spec["owner"],"stage":work.stage,"runtime_id":runner["runtime_id"],"lane":runner["lane"].as_str().unwrap_or("cpu"),"command":command,"env":runner.get("env").cloned().unwrap_or(json!({})),"host_output":output,"inputs":inputs,"tools":tools,"output_storage":selector});
    if let Some(component) = profile["component_path"].as_str() {
        let relative = Path::new(component)
            .strip_prefix(&components)
            .map_err(|_| Error::input("Component outside configured root"))?;
        payload["component"] = json!(host_components.join(relative));
    }
    for name in ["image", "resources", "storage"] {
        if let Some(v) = runner.get(name) {
            payload[name] = v.clone();
        }
    }
    if let Some(pin) = runner.get("runtime_pin") {
        payload["runtime_ref"] =
            json!({"id":pin["id"],"revision":pin["revision"],"sha256":pin["sha256"]});
        payload.as_object_mut().unwrap().remove("runtime_id");
        request["runtime"] = pin.clone();
    }
    let payload = if already_sent {
        let frozen: Value = serde_json::from_slice(&tokio::fs::read(&marker).await?)?;
        require(
            frozen["request_sha256"] == hash(serde_json::to_vec(&payload)?),
            "effect",
            "Frozen execution request changed",
            409,
        )?;
        frozen["request"].clone()
    } else {
        storage::atomic(
            &marker,
            &serde_json::to_vec(
                &json!({"request":payload,"request_sha256":hash(serde_json::to_vec(&payload)?)}),
            )?,
            0o600,
        )?;
        payload
    };
    let started = match host(executor, "start", payload).await {
        Ok(v) => v,
        Err(e) => {
            service.unknown(work).await?;
            return Err(e);
        }
    };
    if started["busy"] == true {
        if service.cancelled(work.clone()).await? {
            service.returned(work,137,json!({"returned":true,"not_started":true,"reason":"cancelled_while_waiting_for_executor"})).await?;
            return Ok(());
        }
        service.defer(work).await?;
        return Err(Error::new(
            "busy",
            "Executor resource is owned by another instance",
            503,
        ));
    }
    let ticket = started["ticket"]
        .as_str()
        .ok_or_else(|| Error::new("executor", "Missing accepted ticket", 503))?
        .to_string();
    service.accepted(work.clone(), started.clone()).await?;
    let timeout = work.spec["execution"]["timeout_seconds"]
        .as_u64()
        .or(profile["limits"]["timeout_seconds"].as_u64())
        .unwrap_or(30);
    let start = std::time::Instant::now();
    let mut state = started;
    loop {
        if state["returned"] == true {
            break;
        }
        let cancellation = service.cancelled(work.clone()).await?
            || service.stopping.load(Ordering::Acquire)
            || start.elapsed() > Duration::from_secs(timeout);
        state = match host(
            executor,
            if cancellation { "cancel" } else { "status" },
            json!({"ticket":ticket}),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                service.unknown(work).await?;
                return Err(e);
            }
        };
        if state["returned"] != true {
            sleep(Duration::from_millis(500)).await;
        }
    }
    let code = state["exit_code"]
        .as_i64()
        .ok_or_else(|| Error::new("return", "Missing returned exit code", 503))?
        as i32;
    if service.stopping.load(Ordering::Acquire) {
        state["interrupted"] = json!(true);
    }
    if start.elapsed() > Duration::from_secs(timeout) {
        state["reason"] = json!("timeout");
    }
    service.returned(work, code, state).await?;
    Ok(())
}
fn verify_inputs(request: &Value, _components: &Path) -> Result<()> {
    for (_, item) in request["verified_inputs"].as_object().into_iter().flatten() {
        let path = Path::new(
            item["path"]
                .as_str()
                .ok_or_else(|| Error::input("Input path required"))?,
        );
        let (_, sha) = storage::file_hash(path)?;
        require(
            item["sha256"] == sha,
            "input",
            "Input changed after acceptance",
            409,
        )?;
    }
    if let Some(path) = request["component_path"].as_str() {
        for (name, digest) in request["source_pins"].as_object().into_iter().flatten() {
            let path = storage::relative(Path::new(path), name)?;
            let (_, sha) = storage::file_hash(&path)?;
            require(
                digest == &json!(sha),
                "source",
                "Component revision changed",
                409,
            )?;
        }
    }
    for (_, pin) in request["tools"].as_object().into_iter().flatten() {
        if let Some(root) = pin["path"].as_str() {
            for (name, digest) in pin["source_pins"].as_object().into_iter().flatten() {
                let (_, sha) = storage::file_hash(&storage::relative(Path::new(root), name)?)?;
                require(
                    digest == &json!(sha),
                    "source",
                    "Tool revision changed",
                    409,
                )?;
            }
        }
    }
    Ok(())
}
async fn run_builtin(service: Service, work: Work, request: &mut Value) -> Result<()> {
    request["output_dir"] = json!(work.folder);
    request["params"] = work.spec["params"].clone();
    let file = work.folder.join(format!("{}-request.json", work.stage));
    storage::atomic(&file, &serde_json::to_vec(request)?, 0o600)?;
    let returned = work
        .folder
        .join(format!(".native-{}-return.json", work.effect));
    if returned.exists() {
        let value: Value = serde_json::from_slice(&tokio::fs::read(returned).await?)?;
        service
            .returned(
                work,
                value["exit_code"].as_i64().unwrap_or(137) as i32,
                value,
            )
            .await?;
        return Ok(());
    }
    let ownership = work.folder.join(format!(".native-{}.json", work.effect));
    if ownership.exists() {
        let previous: Value = serde_json::from_slice(&tokio::fs::read(&ownership).await?)?;
        let pid = if let Some(pid) = previous["pid"].as_i64() {
            pid as i32
        } else {
            service.unknown(work).await?;
            return Err(Error::new(
                "native_launch_unknown",
                "Native launch intent has no confirmed process identity; reconcile before retry",
                503,
            ));
        };
        let current_start = process_start(pid as u32);
        let same =
            current_start.is_some() && previous["start_ticks"].as_str() == current_start.as_deref();
        let mut exists = same && unsafe { libc::kill(-pid, 0) } == 0;
        if exists && service.cancelled(work.clone()).await? {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            for _ in 0..100 {
                if process_start(pid as u32).as_deref() != previous["start_ticks"].as_str() {
                    exists = false;
                    break;
                }
                sleep(Duration::from_millis(50)).await;
            }
        }
        if exists {
            service.unknown(work).await?;
            return Err(Error::new(
                "native_owner",
                "Existing native process group requires reconciliation",
                503,
            ));
        }
        let proof = json!({"returned":true,"exit_code":137,"interrupted":true,"source":"owned builtin process group absent"});
        service.returned(work, 137, proof).await?;
        return Ok(());
    }
    let binary = std::env::var_os("LOOM_SDK_BINARY")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("loom-sdk")))
        })
        .ok_or_else(|| Error::input("Packaged Rust producer required"))?;
    use std::os::unix::fs::OpenOptionsExt;
    loom_core::index_store::secure_dir(&service.state.join("log-spool"))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(loom_core::log_index::path(&service.state, &work))?;
    let mut command = Command::new(binary);
    command
        .arg(&file)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .kill_on_drop(true);
    use std::os::unix::process::CommandExt;
    command.as_std_mut().process_group(0);
    let parent = std::process::id() as i32;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                libc::_exit(137);
            }
            Ok(())
        });
    }
    storage::atomic(
        &ownership,
        &serde_json::to_vec(
            &json!({"state":"launching","effect":work.effect,"job":work.job,"attempt":work.attempt}),
        )?,
        0o600,
    )?;
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            storage::atomic(
                &returned,
                &serde_json::to_vec(&json!({"returned":true,"not_started":true,"exit_code":1}))?,
                0o600,
            )?;
            return Err(error.into());
        }
    };
    let pid = child
        .id()
        .ok_or_else(|| Error::new("process", "Missing builtin PID", 503))?;
    storage::atomic(
        &ownership,
        &serde_json::to_vec(
            &json!({"pid":pid,"start_ticks":process_start(pid),"effect":work.effect,"job":work.job,"attempt":work.attempt}),
        )?,
        0o600,
    )?;
    service
        .accepted(work.clone(), json!({"pid":pid,"backend":"rust-builtin"}))
        .await?;
    let timeout = work.spec["execution"]["timeout_seconds"]
        .as_u64()
        .unwrap_or(30);
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if service.cancelled(work.clone()).await?
            || service.stopping.load(Ordering::Acquire)
            || start.elapsed() > Duration::from_secs(timeout)
        {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            let _ = child.kill().await;
            break child.wait().await?;
        }
        sleep(Duration::from_millis(50)).await;
    };
    let code = status.code().unwrap_or(137);
    let proof = json!({"returned":true,"exit_code":code,"process_group_returned":true,"interrupted":service.stopping.load(Ordering::Acquire),"backend":"rust-builtin"});
    storage::atomic(&returned, &serde_json::to_vec(&proof)?, 0o600)?;
    storage::atomic(
        &loom_core::log_index::state_path(&service.state, &work),
        &serde_json::to_vec(&json!({"generation":work.effect,"closed":true,"gap":false}))?,
        0o600,
    )?;
    if work.stage == "run" {
        let bytes = tokio::fs::read(loom_core::log_index::path(&service.state, &work)).await?;
        storage::atomic(&work.folder.join("worker.log"), &bytes, 0o600)?;
    }
    service.returned(work, code, proof).await?;
    Ok(())
}

fn verify_outputs(
    folder: &Path,
    manifest: &Value,
) -> Result<std::collections::BTreeMap<PathBuf, storage::VerifiedFile>> {
    let mut paths = BTreeSet::new();
    for entity in manifest["entities"].as_array().into_iter().flatten() {
        let file = storage::relative(
            folder,
            entity["path"]
                .as_str()
                .ok_or_else(|| Error::input("Output path required"))?,
        )?;
        paths.insert(file.clone());
        for member in entity["components"].as_array().into_iter().flatten() {
            paths.insert(storage::relative(
                file.parent().unwrap(),
                member
                    .as_str()
                    .ok_or_else(|| Error::input("Output member required"))?,
            )?);
        }
        if entity["kind"] == "model"
            && file.file_name().and_then(|n| n.to_str()) == Some("model.safetensors")
        {
            fn walk(root: &Path, dir: &Path, out: &mut BTreeSet<PathBuf>) -> Result<()> {
                for entry in std::fs::read_dir(dir)? {
                    let path = entry?.path();
                    let metadata = path.symlink_metadata()?;
                    require(
                        !metadata.file_type().is_symlink(),
                        "model",
                        "Model member symlink denied",
                        409,
                    )?;
                    if metadata.is_dir() {
                        walk(root, &path, out)?;
                    } else {
                        out.insert(storage::relative(
                            root,
                            path.strip_prefix(root)
                                .unwrap()
                                .to_str()
                                .ok_or_else(|| Error::input("Invalid member name"))?,
                        )?);
                        require(
                            out.len() <= 10000,
                            "model",
                            "Model member limit exceeded",
                            400,
                        )?;
                    }
                }
                Ok(())
            }
            walk(file.parent().unwrap(), file.parent().unwrap(), &mut paths)?;
        }
    }
    paths
        .into_iter()
        .map(|path| Ok((path.clone(), storage::VerifiedFile::read(&path)?)))
        .collect()
}

fn process_start(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = stat.rsplit_once(')')?;
    let mut values = fields.split_whitespace();
    if matches!(values.next(), Some("Z" | "X")) {
        return None;
    }
    values.nth(18).map(String::from)
}
