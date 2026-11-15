use loom_core::{Error, Result};
use loom_host::executor::{Config, Docker, Executor};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Default)]
struct State {
    container: Option<Value>,
    creates: usize,
    starts: usize,
    lost_create: bool,
    lost_start: bool,
    unknown: bool,
}
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<State>>);
impl Docker for Fake {
    fn call(&self, args: &[String]) -> Result<String> {
        let mut state = self.0.lock().unwrap();
        match args.first().map(String::as_str) {
            Some("create") => {
                state.creates += 1;
                let mut labels = serde_json::Map::new();
                for pair in args.windows(2) {
                    if pair[0] == "--label" {
                        let (k, v) = pair[1].split_once('=').unwrap();
                        labels.insert(k.into(), json!(v));
                    }
                }
                state.container = Some(
                    json!({"State":{"Status":"created","ExitCode":0},"Image":format!("sha256:{}","a".repeat(64)),"Config":{"Labels":labels}}),
                );
                if state.lost_create {
                    state.lost_create = false;
                    return Err(Error::new("lost", "Create reply lost", 503));
                }
                Ok("container".into())
            }
            Some("start") => {
                state.starts += 1;
                state.container.as_mut().unwrap()["State"]["Status"] = json!("running");
                if state.lost_start {
                    state.lost_start = false;
                    return Err(Error::new("lost", "Start reply lost", 503));
                }
                Ok("container".into())
            }
            Some("inspect") => {
                if state.unknown {
                    return Err(Error::new("lost", "Inspect unavailable", 503));
                }
                Ok(serde_json::to_string(&vec![state
                    .container
                    .clone()
                    .ok_or_else(|| Error::new("missing", "Container missing", 503))?])
                .unwrap())
            }
            Some("stop") => {
                state.container.as_mut().unwrap()["State"]["Status"] = json!("exited");
                Ok("container".into())
            }
            Some("logs") => Ok("one\ntwo\n".into()),
            _ => Err(Error::input("Unexpected Docker invocation")),
        }
    }
}
struct Fixture {
    root: PathBuf,
    executor: Executor<Fake>,
    fake: Fake,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("loom-host-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("data")).unwrap();
        std::fs::create_dir(root.join("components")).unwrap();
        let config:Config=serde_json::from_value(json!({"uid":1001,"gid":1001,"components":root.join("components"),"instances":{"one":{"uid":1001,"gid":1001,"token":"fixture-only","data":root.join("data"),"storage":{"data/0":root.join("data")},"runtimes":["cpu"],"gpu":true},"two":{"uid":1002,"gid":1002,"token":"other-only","data":root.join("data"),"runtimes":["cpu"],"gpu":true}},"runtimes":{"cpu":{"image":format!("sha256:{}","a".repeat(64)),"network":"none","cpus":1,"memory":"64m","pids":32}}})).unwrap();
        let fake = Fake::default();
        let executor = Executor::open(&root, config, fake.clone()).unwrap();
        Self {
            root,
            executor,
            fake,
        }
    }
    fn request(&self) -> Value {
        json!({"ticket":"job.attempt.run","job":"job","stage":"run","command":["/bin/true"],"runtime_id":"cpu","host_output":self.root.join("data")})
    }
    fn start(&mut self) -> Result<Value> {
        self.executor.handle("one", "start", self.request())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
fn lost_create_reply_reuses_exact_matching_launch() {
    let mut f = Fixture::new();
    f.fake.0.lock().unwrap().lost_create = true;
    let first = f.start().unwrap();
    let again = f.start().unwrap();
    assert_eq!(first["ticket"], again["ticket"]);
    let state = f.fake.0.lock().unwrap();
    assert_eq!((state.creates, state.starts), (1, 1));
}
#[test]
fn lost_start_reply_never_starts_twice() {
    let mut f = Fixture::new();
    f.fake.0.lock().unwrap().lost_start = true;
    assert!(f.start().is_err());
    assert_eq!(f.start().unwrap()["state"], "running");
    let state = f.fake.0.lock().unwrap();
    assert_eq!((state.creates, state.starts), (1, 1));
}
#[test]
fn unknown_status_keeps_gpu_ownership() {
    let mut f = Fixture::new();
    let mut request = f.request();
    request["lane"] = json!("gpu");
    let ticket = f.executor.handle("one", "start", request).unwrap()["ticket"].clone();
    f.fake.0.lock().unwrap().unknown = true;
    assert_eq!(
        f.executor
            .handle("one", "status", json!({"ticket":ticket}))
            .unwrap()["returned"],
        false
    );
    let mut second = f.request();
    second["ticket"] = json!("another");
    second["lane"] = json!("gpu");
    assert_eq!(
        f.executor.handle("two", "start", second).unwrap()["busy"],
        true
    );
}
#[test]
fn foreign_container_is_not_accepted_or_stopped() {
    let mut f = Fixture::new();
    let ticket = f.start().unwrap()["ticket"].clone();
    f.fake.0.lock().unwrap().container.as_mut().unwrap()["Config"]["Labels"]["loom.instance"] =
        json!("foreign");
    assert_eq!(
        f.executor
            .handle("one", "cancel", json!({"ticket":ticket}))
            .unwrap_err()
            .code,
        "container_identity"
    );
    assert_eq!(
        f.fake.0.lock().unwrap().container.as_ref().unwrap()["State"]["Status"],
        "running"
    );
}
#[test]
fn private_ticket_cannot_be_read_by_other_instance() {
    let mut f = Fixture::new();
    let ticket = f.start().unwrap()["ticket"].clone();
    assert_eq!(
        f.executor
            .handle("two", "status", json!({"ticket":ticket}))
            .unwrap_err()
            .status,
        404
    );
}
#[test]
fn ticket_reuse_with_changed_request_conflicts() {
    let mut f = Fixture::new();
    f.start().unwrap();
    let mut changed = f.request();
    changed["command"] = json!(["/bin/false"]);
    assert_eq!(
        f.executor
            .handle("one", "start", changed)
            .unwrap_err()
            .status,
        409
    );
    assert_eq!(f.fake.0.lock().unwrap().creates, 1);
}
#[test]
fn bind_option_injection_is_rejected_before_docker_create() {
    let mut f = Fixture::new();
    let path = f.root.join("data").join("evil,target=/etc");
    std::fs::create_dir_all(&path).unwrap();
    let mut request = f.request();
    request["host_output"] = json!(path);
    assert!(f.executor.handle("one", "start", request).is_err());
    assert_eq!(f.fake.0.lock().unwrap().creates, 0);
}
#[test]
fn peer_uid_and_token_must_both_match() {
    let f = Fixture::new();
    assert_eq!(
        f.executor.authenticate("fixture-only", 1001).unwrap(),
        "one"
    );
    assert!(f.executor.authenticate("fixture-only", 9999).is_err());
    assert!(f.executor.authenticate("wrong", 1001).is_err());
}
#[test]
fn cancel_intent_before_a_delayed_start_creates_a_durable_no_launch_tombstone() {
    let mut f = Fixture::new();
    let request = f.request();
    let cancelled = f.executor.handle("one", "cancel_intent", request).unwrap();
    assert_eq!(cancelled["not_started"], true);
    let replay = f.start().unwrap();
    assert_eq!(replay["returned"], true);
    assert_eq!(replay["exit_code"], 137);
    let counts = f.fake.0.lock().unwrap();
    assert_eq!((counts.creates, counts.starts), (0, 0));
}

#[test]
fn runtime_file_inventory_is_scoped_budgeted_and_checks_current_grants() {
    let f = Fixture::new();
    let dependency = f.root.join("dependencies");
    std::fs::create_dir(&dependency).unwrap();
    std::fs::create_dir(dependency.join("python")).unwrap();
    std::fs::write(dependency.join("python/a.txt"), "first").unwrap();
    std::fs::write(dependency.join("python/a-b.txt"), "second").unwrap();
    std::fs::create_dir(dependency.join("python/a")).unwrap();
    std::fs::write(dependency.join("python/a/deep.txt"), "third").unwrap();
    let mut config = f.executor.config.clone();
    config
        .instances
        .get_mut("one")
        .unwrap()
        .extra
        .insert("dependencies".into(), json!(dependency));
    let manifest = json!({"runtime":{"image":format!("sha256:{}","a".repeat(64))},"dependencies":[{"name":"Python","version":"3.12","path":dependency.join("python")}],"validation":{"status":"passed"}});
    let registered = loom_host::runtime::register(
        &f.root,
        &config,
        "inventory-runtime",
        manifest,
        &["one".into()],
    )
    .unwrap();
    let pin = json!({"id":"inventory-runtime","revision":registered["revision"],"sha256":registered["sha256"]});
    assert!(loom_host::runtime_files::register_inventory(
        &f.root,
        &config,
        "one",
        &json!({"runtime_ref":pin,"roots":[{"prefix":"."}]})
    )
    .is_err());
    assert!(loom_host::runtime_files::register_inventory(
        &f.root,
        &config,
        "one",
        &json!({"runtime_ref":pin,"roots":[{"prefix":"python"}],"max_bytes":1})
    )
    .is_err());
    let inventory=loom_host::runtime_files::register_inventory(&f.root,&config,"one",&json!({"runtime_ref":pin,"roots":[{"prefix":"python","dependency":{"name":"Python","version":"3.12"}}],"max_bytes":64})).unwrap();
    let next = json!({"id":"inventory-runtime","revision":inventory["revision"],"sha256":inventory["sha256"]});
    let grant = || loom_host::runtime_files::grant(&f.root, &config, "one", &next).unwrap();
    let page = grant()
        .call("runtime_file_page", &json!({"limit":2}))
        .unwrap();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert!(page["items"][0].get("proof").is_none());
    assert_eq!(page["items"][0]["entry_key"], "python/a-b.txt");
    let bytes = grant()
        .call(
            "runtime_file_read",
            &json!({"entry_key":"python/a.txt","limit":10}),
        )
        .unwrap();
    assert_eq!(bytes["sha256"], loom_core::store::hash("first"));
    assert!(loom_host::runtime_files::grant(&f.root, &config, "two", &next).is_err());
    std::fs::write(dependency.join("python/a.txt"), "mutated").unwrap();
    assert_eq!(
        grant()
            .call("runtime_file_read", &json!({"entry_key":"python/a.txt"}))
            .unwrap_err()
            .code,
        "artifact"
    );
}
