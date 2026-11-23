//! Reuse measured workload settings only under matching execution conditions.
use crate::{error::require, store::*, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::Path;

pub(crate) fn validate_profile(profile: &Value) -> Result<()> {
    let Some(tuning) = profile["runtime"].get("performance_tuning") else {
        return Ok(());
    };
    let mut names = BTreeSet::new();
    require(
        profile["runtime"]["lane"] == "gpu" && profile["runtime"]["runtime_ref"].is_object()
            && tuning.as_object().is_some_and(|fields| fields.len() == 3)
            && tuning["max_seconds"].as_u64().is_some_and(|seconds| (1..=300).contains(&seconds))
            && tuning["workload_params"].as_array().is_some_and(|keys| keys.len() <= 32
                && keys.iter().all(|key| key.as_str().is_some_and(|name| names.insert(name)
                    && profile["params_schema"]["properties"].get(name).is_some())))
            && tuning["settings_schema"]["type"] == "object"
            && tuning["settings_schema"]["additionalProperties"] == false,
        "performance_tuning", "Declare an exact GPU Runtime, a bounded tuning budget, workload params and strict settings schema",
        400,
    )?;
    crate::contracts::check_schema(&tuning["settings_schema"])
}

pub(crate) fn request(
    db: &Connection,
    config: &Config,
    observation_path: &Path,
    spec: &Value,
) -> Result<Value> {
    let Some(tuning) = spec["profile"]["runtime"].get("performance_tuning") else {
        return Ok(Value::Null);
    };
    if spec["gpu_access"]["mode"] == "shared" {
        return Ok(Value::Null);
    }
    let observation = crate::file_set::bounded(observation_path, 64 * 1024)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .unwrap_or_default();
    // The current host telemetry does not expose the assigned device identity.
    // Reuse only when there is exactly one observed GPU and its capacity is known.
    let device = observation["data"]
        .as_array()
        .filter(|gpus| gpus.len() == 1)
        .filter(|_| {
            observation["status"] == "ok" && crate::catalog::observation_fresh(&observation, now())
        })
        .and_then(|gpus| gpus.first())
        .filter(|gpu| {
            gpu["name"].is_string()
                && gpu["driver_version"].is_string()
                && gpu["total_MiB"]
                    .as_f64()
                    .is_some_and(|capacity| capacity > 0.)
        });
    let mut workload = Map::new();
    for name in tuning["workload_params"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        workload.insert(name.to_owned(), spec["params"][name].clone());
    }
    let runtime = spec["profile"]["runtime"]["runtime_pin"]["sha256"].as_str();
    let key = device.zip(runtime).map(|(gpu, runtime)| hash(crate::compat::seal(&json!({
        "machine":config.machine_id,"gpu":{"name":gpu["name"],"driver":gpu["driver_version"],"memory_mib":gpu["total_MiB"]},
        "profile":spec["profile"]["sha256"],"runtime":runtime,
        "gpu_access":spec["gpu_access"],"scenario":spec["scenario_ref"],"storage":spec["_output_storage_id"],
        "workload":workload
    }))));
    let cached = if let Some(key) = &key {
        db.query_row("SELECT settings,job,attempt FROM execution_tuning WHERE key=?", [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
            .optional()?.map(|(settings, job, attempt)| Ok::<_, crate::Error>(
                json!({"settings":serde_json::from_str::<Value>(&settings)?,"job":job,"attempt":attempt})
            )).transpose()?.unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    Ok(
        json!({"schema":"loom-performance-tuning/v1","key":key,"cached":cached,
        "capacity_mib":device.map(|gpu| gpu["total_MiB"].clone()),
        "max_seconds":tuning["max_seconds"],"report":"/work/performance-tuning.json"}),
    )
}

pub(crate) fn record(
    db: &Connection,
    work: &crate::execution::Work,
    report: &Value,
) -> Result<Option<Value>> {
    let Some(key) = work.performance_tuning["key"].as_str() else {
        return Ok(None);
    };
    let schema = &work.spec["profile"]["runtime"]["performance_tuning"]["settings_schema"];
    let Some(samples) = report["samples"].as_array() else {
        return Ok(None);
    };
    let capacity = work.performance_tuning["capacity_mib"]
        .as_f64()
        .unwrap_or(0.);
    let seconds = work.performance_tuning["max_seconds"]
        .as_f64()
        .unwrap_or(0.);
    if report["schema"] != "loom-performance-tuning/v1"
        || report["key"] != key
        || samples.is_empty()
        || samples.len() > 32
        || !report["elapsed_seconds"]
            .as_f64()
            .is_some_and(|elapsed| elapsed > 0. && elapsed <= seconds)
    {
        return Ok(None);
    }
    let winner = samples
        .iter()
        .filter_map(|sample| {
            let proven = sample["equivalent"] == true
                && sample["peak_vram_mib"]
                    .as_f64()
                    .is_some_and(|peak| peak > 0. && peak < capacity)
                && crate::contracts::validate(schema, &sample["settings"]).is_ok();
            let valid = sample["valid"].as_u64().filter(|n| *n > 0)?;
            let measured = sample["seconds"]
                .as_f64()
                .filter(|s| *s > 0. && *s <= seconds)?;
            let throughput = valid as f64 / measured;
            (proven && throughput.is_finite()).then_some((throughput, &sample["settings"]))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0));
    let Some((throughput, settings)) = winner else {
        return Ok(None);
    };
    if (work.performance_tuning["cached"].is_object()
        && work.performance_tuning["cached"]["settings"] != *settings)
        || report["applied"] != *settings
    {
        return Ok(None);
    }
    db.execute(
        "INSERT OR IGNORE INTO execution_tuning VALUES(?,?,?,?,?,?)",
        params![
            key,
            encode(settings)?,
            encode(report)?,
            work.job,
            work.attempt,
            now()
        ],
    )?;
    Ok(Some(
        json!({"key":key,"settings":settings,"valid_per_second":throughput}),
    ))
}
