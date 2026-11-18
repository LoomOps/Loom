use crate::{error::require, Error, Result};
use serde_json::Value;
use std::collections::BTreeSet;
pub fn primary(manifest: &Value, selection: Option<&Value>) -> Result<Vec<Value>> {
    let deps = manifest["dependencies"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let selectors = selection.or(manifest.get("primary_dependencies"));
    let mut result = vec![];
    if let Some(selectors) = selectors {
        let selectors = selectors
            .as_array()
            .ok_or_else(|| Error::input("Dependency selectors must be an array"))?;
        require(
            selectors.len() <= 8,
            "dependencies",
            "At most eight representative dependencies",
            400,
        )?;
        for selector in selectors {
            let found: Vec<_> = deps
                .iter()
                .filter(|d| {
                    d["name"].as_str().map(str::to_lowercase)
                        == selector["name"].as_str().map(str::to_lowercase)
                        && (selector.get("scope").is_none() || d["scope"] == selector["scope"])
                })
                .collect();
            let versions: BTreeSet<_> = found.iter().map(|d| d["version"].as_str()).collect();
            require(
                !found.is_empty() && versions.len() == 1,
                "dependencies",
                "Representative dependency missing or ambiguous; select its scope",
                409,
            )?;
            result.push(found[0].clone());
        }
    } else {
        for name in [
            "isaacsim",
            "isaaclab",
            "python",
            "isaac-python",
            "torch",
            "lerobot",
            "tensorflow",
            "mujoco",
        ] {
            let found: Vec<_> = deps
                .iter()
                .filter(|d| {
                    d["name"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(name))
                        && d["deprecated"] != true
                })
                .collect();
            if !found.is_empty()
                && found
                    .iter()
                    .map(|d| d["version"].as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == 1
            {
                result.push(found[0].clone());
            }
        }
    }
    Ok(result)
}
