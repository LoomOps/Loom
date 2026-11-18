use serde_json::{json, Value};
pub fn dependency_ref(runtime: &Value, dep: &Value) -> Value {
    let mut origin = dep.get("origin").cloned().unwrap_or_else(|| {
        if dep["scope"] == "image" {
            json!({"type":"image","image":runtime["manifest"]["runtime"]["image"]})
        } else {
            json!({"type":"runtime","id":runtime["id"]})
        }
    });
    // Image OS inventories may declare only {type:image}. Bind that observation
    // to its immutable image; otherwise two images overwrite one current package
    // on every refresh and grow the authority log and search projection forever.
    if origin["type"] == "image" && !origin["image"].is_string() {
        if runtime["manifest"]["runtime"]["image"].is_string() {
            origin["image"] = runtime["manifest"]["runtime"]["image"].clone();
        } else {
            origin["runtime_id"] = runtime["id"].clone();
        }
    }
    json!({"name":dep["name"].as_str().unwrap_or("").to_lowercase(),"version":dep["version"],"scope":dep["scope"].as_str().unwrap_or("unrecorded"),"origin":origin})
}
