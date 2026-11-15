use loom_host::{
    executor::{Config, DockerCli, Executor},
    observe, runtime, wire,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str){Some("observe")=>{let result=observe::collect()?;if let Some(path)=args.get(2){loom_core::storage::atomic(std::path::Path::new(path),serde_json::to_string(&result)?.as_bytes(),0o600)?;}else{println!("{}",result);}},Some("runtime-register")=>{let root=std::path::Path::new(args.get(2).ok_or("Executor root required")?);let config:Config=serde_json::from_slice(&std::fs::read(root.join("config.json"))?)?;let id=args.get(3).ok_or("Runtime ID required")?;let manifest:serde_json::Value=serde_json::from_slice(&std::fs::read(args.get(4).ok_or("Manifest file required")?)?)?;let instances=args.iter().skip(5).cloned().collect::<Vec<_>>();println!("{}",runtime::register(root,&config,id,manifest,&instances)?);},Some("serve")=>{let root=std::path::Path::new(args.get(2).ok_or("Executor root required")?);let config:Config=serde_json::from_slice(&std::fs::read(root.join("config.json"))?)?;wire::serve(Executor::open(root,config,DockerCli)?)?;},Some("--version")=>println!("loom-host {}",env!("CARGO_PKG_VERSION")),_=>return Err("Usage: loom-host serve ROOT | observe [FILE] | runtime-register ROOT ID MANIFEST INSTANCE...".into())}
    Ok(())
}
