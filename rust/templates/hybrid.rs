use loom_sdk::{Request,Producer};
fn main()->loom_sdk::Result<()> {
 let request=Request::from_args()?;
 let source=request.extra.get("component_path").and_then(|v|v.as_str()).ok_or("Component source mount required")?;
 let worker=std::path::Path::new(source).join("worker.py");
 let output=request.output_dir.clone();
 let mut producer=Producer::new(request)?;
 producer.progress("external-sdk","running")?;
 producer.run_worker("python3", &[worker.to_str().ok_or("Invalid worker path")?,output.to_str().ok_or("Invalid output path")?])?;
 // Replace this bounded receipt with outputs validated against the external SDK.
 let receipt=std::fs::read(output.join("worker-receipt.txt"))?;
 producer.write_artifact("receipt","receipt.txt",&receipt)?;
 producer.finish()?;
 Ok(())
}
