//! Release-built bounded CPU/installation producer; no Python runtime required.
use loom_sdk::{Producer, Request};

fn main() -> loom_sdk::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--version") {
        println!("loom-sdk {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let installation = args.get(1).map(String::as_str) == Some("--installation");
    let r = Request::from_args()?;
    let mut p = Producer::new(r)?;
    if p.request.stage == "prepare" {
        p.write_artifact("prepared", "prepared.txt", b"ready")?;
        return Ok(());
    }
    if installation {
        if std::fs::read(p.request.output_dir.join("prepared.txt"))? != b"ready" {
            return Err("Preparation receipt missing".into());
        }
        p.write_artifact("proof", "installation.txt", b"Loom installation smoke\n")?;
        return p.finish();
    }
    let seconds = p.request.params["seconds"].as_u64().unwrap_or(0);
    if seconds > 20 {
        return Err("Bounded CPU smoke accepts at most 20 seconds".into());
    }
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < seconds as f64 {
        p.check_cancelled()?;
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    p.progress("run", "running")?;
    if p.request.params["fail"] == true {
        return Err("Requested bounded smoke failure".into());
    }
    p.write_artifact("observation", "observation.json", b"{\"synthetic\":true}\n")?;
    p.metric(serde_json::json!({"name":"observations","entity_id":"observation","value":1}))?;
    p.finish()
}
