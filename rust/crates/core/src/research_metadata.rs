//! Scientific browse facts from recorded metadata and verified registered JSON.
//! No database writes, arbitrary path following, code execution or result promotion.
use crate::{media, store::rows, Application};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const FILE_BYTES: u64 = 2 * 1024 * 1024;
const ENTITY_BYTES: u64 = 4 * 1024 * 1024;
const FILES: usize = 12;

struct Document {
    value: Value,
    source: Value,
}
struct Facts {
    values: BTreeMap<String, Value>,
    inputs: Vec<Value>,
}
impl Facts {
    fn add(
        &mut self,
        doc: &Document,
        id: &str,
        section: &str,
        label: &str,
        paths: &[&str],
        unit: Option<&str>,
    ) {
        for path in paths {
            let Some(value) = doc.value.pointer(path).filter(|v| {
                !v.is_null() && v.as_str() != Some("") && !v.as_array().is_some_and(Vec::is_empty)
            }) else {
                continue;
            };
            // Empty objects and operational instruction blobs are not display facts.
            if value.as_object().is_some_and(serde_json::Map::is_empty) {
                continue;
            }
            self.record(doc, id, section, label, value, path, unit);
            break;
        }
    }
    fn record(
        &mut self,
        doc: &Document,
        id: &str,
        section: &str,
        label: &str,
        value: &Value,
        path: &str,
        unit: Option<&str>,
    ) {
        let evidence = json!({"source":doc.source,"pointer":path});
        let entry=self.values.entry(id.into()).or_insert_with(||json!({"id":id,"section":section,"label":label,"value":value,"unit":unit,"status":"recorded","evidence":[]}));
        if entry["value"] != *value {
            if entry["alternatives"].is_null() {
                entry["alternatives"] = json!([])
            }
            let candidates = entry["alternatives"].as_array_mut().unwrap();
            if !candidates.iter().any(|v| v["value"] == *value) {
                candidates.push(json!({"value":value,"evidence":evidence}));
            }
            // Primary accepted record metadata wins over linked artifacts, but
            // disagreements stay explicit; two files never silently overwrite.
            if entry["evidence"][0]["source"]["kind"] != "record" {
                entry["status"] = json!("conflict");
            }
        } else if let Some(e) = entry["evidence"].as_array_mut() {
            e.push(evidence);
        }
    }
    fn inputs(&mut self, doc: &Document) {
        for path in ["/inputs", "/research/inputs", "/dataset_overview/inputs"] {
            if let Some(values) = doc.value.pointer(path).and_then(Value::as_array) {
                for input in values.iter().filter(|v| v.is_object()) {
                    let mut next = input.clone();
                    next["evidence"] = json!({"source":doc.source,"pointer":path});
                    let kind = next["kind"].as_str().unwrap_or("");
                    if kind.is_empty() {
                        continue;
                    }
                    if !self
                        .inputs
                        .iter()
                        .any(|old| old["kind"] == next["kind"] && old["label"] == next["label"])
                    {
                        self.inputs.push(next)
                    }
                }
                if !values.is_empty() {
                    return;
                }
            }
        }
        if let Some(hdf) = doc.value.get("hdf5").and_then(Value::as_object) {
            for kind in ["rgb", "depth", "state", "action"] {
                if hdf.get(kind).is_some_and(|v| !v.is_null())
                    && !self.inputs.iter().any(|v| v["kind"] == kind)
                {
                    self.inputs.push(json!({"kind":kind,"label":kind,"status":"present","evidence":{"source":doc.source,"pointer":format!("/hdf5/{kind}")}}));
                }
            }
        }
        // Published policy configs describe concrete feature shapes. Only model
        // documents use these; evaluated-model configs do not become dataset inputs.
        if let Some(features) = doc.value.get("input_features").and_then(Value::as_object) {
            for (name, feature) in features {
                let kind = match feature["type"].as_str() {
                    Some("VISUAL") => "rgb",
                    Some("STATE") => "state",
                    _ => continue,
                };
                if !self.inputs.iter().any(|v| v["label"] == *name) {
                    self.inputs.push(json!({"kind":kind,"label":name,"status":"present","shape":feature["shape"],"evidence":{"source":doc.source,"pointer":format!("/input_features/{name}")}}))
                }
            }
        }
    }
}

/// A stable vocabulary shared by historical records and new producers. Each
/// value keeps its JSON pointer, registered file identity and immutable digest.
pub fn summarize(entity: &Value, documents: &[(Value, Value)]) -> Value {
    let kind = entity["kind"].as_str().unwrap_or("");
    let mut facts = Facts {
        values: BTreeMap::new(),
        inputs: vec![],
    };
    // Historical typed fields may be top-level; accepted metadata/properties
    // override them in the effective record without discarding that context.
    let mut record = entity.clone();
    if !record.is_object() {
        record = json!({});
    }
    if let Some(properties) = entity["properties"].as_object() {
        record.as_object_mut().unwrap().extend(properties.clone());
    }
    if let Some(m) = entity["metadata"].as_object() {
        record.as_object_mut().unwrap().extend(m.clone());
    }
    for key in ["inputs", "protocol", "dataset_overview"] {
        if let Some(v) = entity.get(key) {
            if !v.is_null() {
                record[key] = v.clone()
            }
        }
    }
    let mut docs = vec![Document {
        value: record,
        source: json!({"kind":"record","id":entity["id"],"revision":entity["loom_registry"].get("revision").or_else(||entity["loom_history"].get("revision"))}),
    }];
    docs.extend(documents.iter().map(|(value, source)| Document {
        value: value.clone(),
        source: source.clone(),
    }));
    for d in &docs {
        macro_rules! field {
            ($id:expr,$section:expr,$label:expr,$paths:expr,$unit:expr) => {
                facts.add(d, $id, $section, $label, $paths, $unit)
            };
        }
        field!(
            "environment",
            "context",
            "Environment",
            &[
                "/research/environment",
                "/environment/type",
                "/environment_type",
                "/evaluation_environment",
                "/simulator"
            ],
            None
        );
        field!(
            "robot",
            "context",
            "Robot",
            &[
                "/research/robot",
                "/robot/name",
                "/robot_name",
                "/robot_type"
            ],
            None
        );
        field!(
            "physics_hz",
            "physics",
            "Physics",
            &[
                "/research/physics/physics_hz",
                "/physics_hz",
                "/control/physics_Hz",
                "/physical_contract/physics_hz",
                "/native_pins/physics_hz",
                "/control_policy/physics_hz",
                "/clock/clock_hz/physics"
            ],
            Some("Hz")
        );
        field!(
            "physics_dt",
            "physics",
            "Physics timestep",
            &[
                "/research/physics/dt",
                "/native_pins/physics_dt",
                "/physics/dt"
            ],
            Some("s")
        );
        field!(
            "control_hz",
            "physics",
            "Control",
            &[
                "/research/physics/control_hz",
                "/control_hz",
                "/control/control_Hz",
                "/physical_contract/control_hz",
                "/native_pins/control_clock_hz",
                "/sampling_clocks_hz/command",
                "/control_policy/overwrite_hz",
                "/clock/clock_hz/command"
            ],
            Some("Hz")
        );
        field!(
            "rgb_hz",
            "physics",
            "RGB sampling",
            &[
                "/research/physics/rgb_hz",
                "/sampling_clocks_hz/RGB",
                "/native_pins/RGB_hz",
                "/clock/clock_hz/rgb"
            ],
            Some("Hz")
        );
        field!(
            "state_hz",
            "physics",
            "State sampling",
            &[
                "/research/physics/state_hz",
                "/sampling_clocks_hz/state",
                "/native_pins/state_hz"
            ],
            Some("Hz")
        );
        field!(
            "robot_gravity_disabled",
            "physics",
            "Robot gravity disabled",
            &[
                "/research/physics/robot_gravity_disabled",
                "/native_pins/robot_gravity_disabled"
            ],
            None
        );
        field!(
            "collision",
            "physics",
            "Collision",
            &[
                "/research/physics/collision",
                "/native_pins/collision",
                "/physics/collision"
            ],
            None
        );
        field!(
            "payload_mass",
            "physics",
            "Payload mass",
            &[
                "/research/physics/payload_mass_kg",
                "/native_pins/fixed_mass_properties/mass"
            ],
            Some("kg")
        );
        field!(
            "friction",
            "physics",
            "Friction",
            &[
                "/research/physics/friction",
                "/physics/friction",
                "/physics/material/friction"
            ],
            None
        );
        field!(
            "controller",
            "physics",
            "Controller",
            &[
                "/research/physics/controller",
                "/physical_contract/controller",
                "/native_pins/controller_revision"
            ],
            None
        );
        field!(
            "annotation",
            "annotations",
            "Annotation",
            &[
                "/research/annotation/method",
                "/annotation_method",
                "/annotation/method",
                "/annotations/method"
            ],
            None
        );
        field!(
            "annotation_source",
            "annotations",
            "Annotation source",
            &[
                "/research/annotation/source",
                "/annotation/source",
                "/observer_identity"
            ],
            None
        );
        field!(
            "instruction_mode",
            "instructions",
            "Instruction",
            &[
                "/research/instruction/mode",
                "/instruction_mode",
                "/language/mode"
            ],
            None
        );
        field!(
            "instruction_source",
            "instructions",
            "Instruction source",
            &[
                "/research/instruction/source",
                "/dynamic_instruction_source",
                "/instruction_source"
            ],
            None
        );
        field!(
            "calibrated",
            "observations",
            "Camera calibrated",
            &[
                "/research/observations/calibrated",
                "/camera_profile/calibrated",
                "/native_pins/RGB_profile/calibrated"
            ],
            None
        );
        field!(
            "sensor_noise",
            "observations",
            "Simulated depth noise",
            &["/camera_profile/depth/simulated_sensor_noise"],
            None
        );
        if kind == "dataset" {
            facts.inputs(d);
            field!(
                "annotation_source",
                "annotations",
                "Annotation source",
                &["/physical_contract/teacher"],
                None
            );
            field!(
                "count",
                "collection",
                "Count",
                &[
                    "/dataset_overview/count",
                    "/research/collection/count",
                    "/totals/adopted",
                    "/episodes",
                    "/UIDs",
                    "/count"
                ],
                None
            );
            field!(
                "count_unit",
                "collection",
                "Unit",
                &["/dataset_overview/count_unit", "/research/collection/unit"],
                None
            );
            field!(
                "frames",
                "collection",
                "Frames",
                &["/dataset_overview/frames", "/frames"],
                None
            );
            field!(
                "split",
                "collection",
                "Split",
                &[
                    "/research/collection/split",
                    "/split_clips",
                    "/splits",
                    "/dataset_overview/splits",
                    "/split"
                ],
                None
            );
            field!(
                "source_demos",
                "collection",
                "Source demonstrations",
                &[
                    "/research/collection/source_demos",
                    "/source_human_parent_count",
                    "/source_parent_count"
                ],
                None
            );
            field!(
                "collection_method",
                "collection",
                "Generation",
                &[
                    "/research/collection/method",
                    "/generation_method",
                    "/method"
                ],
                None
            );
            field!(
                "observation_schema",
                "observations",
                "Observations",
                &["/research/observations/schema", "/hdf5"],
                None
            );
        }
        if kind == "dataset" {
            // A hash-bound product report can record snapshot populations without
            // an episode count. Preserve its actual units, never invent a total.
            field!(
                "train_snapshots",
                "collection",
                "Train snapshots",
                &[
                    "/research/collection/train_snapshots",
                    "/model/train_snapshots"
                ],
                None
            );
            field!(
                "validation_snapshots",
                "collection",
                "Validation snapshots",
                &[
                    "/research/collection/validation_snapshots",
                    "/model/validation_snapshots"
                ],
                None
            );
            field!(
                "test_frames_read",
                "collection",
                "Test frames read",
                &["/model/test_frames_read_for_fit_or_validation"],
                None
            );
        }
        if matches!(kind, "model" | "experiment" | "run") {
            // Training reports are outputs too. These remain loss/error facts,
            // separate from closed-loop policy success and process exit status.
            field!(
                "selected_step",
                "training",
                "Selected step",
                &[
                    "/research/model/selected_step",
                    "/selected_step",
                    "/selection/selected_step"
                ],
                Some("updates")
            );
            field!(
                "optimizer_updates",
                "training",
                "Training steps",
                &[
                    "/research/model/optimizer_updates",
                    "/optimizer_updates",
                    "/selection/training_updates"
                ],
                Some("updates")
            );
            field!(
                "selection",
                "training",
                "Checkpoint selection",
                &[
                    "/research/model/selection",
                    "/selected_by",
                    "/selection/criterion"
                ],
                None
            );
            field!(
                "train_snapshots",
                "training",
                "Train snapshots",
                &["/model/train_snapshots"],
                None
            );
            field!(
                "validation_snapshots",
                "training",
                "Validation snapshots",
                &["/model/validation_snapshots"],
                None
            );
            field!(
                "selection_validation_loss",
                "validation",
                "Selection validation loss",
                &["/selection/validation_loss"],
                None
            );
            field!(
                "test_action_mae",
                "validation",
                "Test action MAE",
                &["/result/action_metrics/all_phases/mae_radians"],
                Some("rad")
            );
            field!(
                "test_action_rmse",
                "validation",
                "Test action RMSE",
                &["/result/action_metrics/all_phases/rmse_radians"],
                Some("rad")
            );
            field!(
                "train_frames",
                "training",
                "Train frames",
                &[
                    "/research/model/train_frames",
                    "/train_frames",
                    "/prepare/frames/train"
                ],
                None
            );
            field!(
                "validation_frames",
                "training",
                "Validation frames",
                &[
                    "/research/model/validation_frames",
                    "/validation_frames",
                    "/prepare/frames/validation"
                ],
                None
            );
        }
        if kind == "model" {
            facts.inputs(d);
            if d.value["split"] == "test" {
                field!(
                    "test_action_mse",
                    "validation",
                    "Test action MSE",
                    &["/all/normalized_MSE"],
                    Some("normalized")
                );
            }

            field!(
                "model_role",
                "model",
                "Model type",
                &["/research/model/role", "/model_role"],
                None
            );
            field!(
                "architecture",
                "model",
                "Architecture",
                &[
                    "/research/model/architecture",
                    "/architecture",
                    "/model_family",
                    "/family",
                    "/type"
                ],
                None
            );
            field!(
                "initialization",
                "training",
                "Initialization",
                &[
                    "/research/model/initialization",
                    "/initialization",
                    "/model/initialization"
                ],
                None
            );
            field!(
                "training_method",
                "training",
                "Training",
                &[
                    "/research/model/training_method",
                    "/method",
                    "/training_method"
                ],
                None
            );
            field!(
                "base_model",
                "training",
                "Base model",
                &[
                    "/research/model/base_model",
                    "/public_model",
                    "/pretrained_checkpoint",
                    "/base_identity/model_id"
                ],
                None
            );
            field!(
                "frozen_base",
                "training",
                "Frozen base",
                &["/research/model/frozen_base", "/frozen_base"],
                None
            );
            field!(
                "selected_step",
                "training",
                "Selected step",
                &["/research/model/selected_step", "/selected_step"],
                Some("updates")
            );
            field!(
                "optimizer_updates",
                "training",
                "Training steps",
                &["/research/model/optimizer_updates", "/optimizer_updates"],
                Some("updates")
            );
            field!(
                "selection",
                "training",
                "Checkpoint selection",
                &[
                    "/research/model/selection",
                    "/selected_by",
                    "/selection/metric",
                    "/ensemble_validation/selection_data",
                    "/model/selection"
                ],
                None
            );
            field!(
                "train_frames",
                "training",
                "Train frames",
                &["/research/model/train_frames", "/train_frames"],
                None
            );
            field!(
                "validation_frames",
                "training",
                "Validation frames",
                &["/research/model/validation_frames", "/validation_frames"],
                None
            );
            field!(
                "test_frames_read",
                "training",
                "Test frames read",
                &[
                    "/research/model/test_frames_read",
                    "/test_frames_read",
                    "/ensemble_validation/test_frames_read",
                    "/model/test_frames_read_for_fit_or_validation"
                ],
                None
            );
            field!(
                "action_chunk",
                "model",
                "Action chunk",
                &["/research/model/action_chunk", "/chunk_size"],
                Some("steps")
            );
            field!(
                "executed_actions",
                "model",
                "Executed actions",
                &["/research/model/executed_actions", "/n_action_steps"],
                Some("steps")
            );
            field!(
                "outputs",
                "model",
                "Prediction",
                &[
                    "/research/model/outputs",
                    "/architecture_details/output_semantics",
                    "/output_schema",
                    "/output_features"
                ],
                None
            );
            field!(
                "validation_mean_error",
                "validation",
                "Validation point error",
                &[
                    "/research/model/validation_mean_error_m",
                    "/validation_mean_point_error_m",
                    "/ensemble_validation/mean_point_error_m",
                    "/ensemble_validation/mean_point_m",
                    "/validation/error_distributions/point_m/mean"
                ],
                Some("m")
            );
            field!(
                "validation_p95_error",
                "validation",
                "Validation P95 error",
                &[
                    "/research/model/validation_p95_error_m",
                    "/validation_point_p95_m",
                    "/ensemble_validation/point_p95_m",
                    "/validation/error_distributions/point_m/p95"
                ],
                Some("m")
            );
        }
        if matches!(kind, "experiment" | "run") {
            // Explicit per-condition task outcomes are a comparison, not one
            // pooled headline. Do not mistake passed/failed checks for trials.
            for pointer in ["/research/evaluation/conditions", "/conditions"] {
                let Some(conditions) = d.value.pointer(pointer).and_then(Value::as_array) else {
                    continue;
                };
                let outcomes: Vec<Value> = conditions
                    .iter()
                    .filter_map(|row| {
                        let name = row["condition"].as_str()?;
                        let valid = row["valid"].as_u64()?;
                        let mut value = json!({"condition":name,"valid":valid});
                        let mut measured = false;
                        for key in ["strict_success", "functional_success"] {
                            if let Some(count) = row[key].as_u64().filter(|n| *n <= valid) {
                                value[key] = json!(count);
                                measured = true;
                            }
                        }
                        if let Some(planned) = row["planned"].as_u64().filter(|n| *n >= valid) {
                            value["planned"] = json!(planned);
                        }
                        measured.then_some(value)
                    })
                    .collect();
                if !outcomes.is_empty() {
                    facts.record(
                        d,
                        "condition_outcomes",
                        "evaluation",
                        "Condition outcomes",
                        &json!(outcomes),
                        pointer,
                        None,
                    );
                    break;
                }
            }
            field!("episodes", "population", "Episodes", &["/episodes"], None);
            field!("frames", "population", "Frames", &["/frames"], None);
            field!(
                "outcome",
                "evaluation",
                "Outcome",
                &["/research/evaluation/outcome", "/outcome"],
                None
            );
            field!(
                "terminal_reason",
                "evaluation",
                "Termination reason",
                &["/terminal/reason"],
                None
            );
            field!("case_id", "population", "Case", &["/case/id"], None);
            field!(
                "observer_hz",
                "physics",
                "Observer sampling",
                &["/clock/clock_hz/observer"],
                Some("Hz")
            );
            field!(
                "policy_hz",
                "physics",
                "Policy actions",
                &["/clock/clock_hz/policy_action"],
                Some("Hz")
            );
            field!(
                "validation_mean_error",
                "validation",
                "Validation point error",
                &["/validation/error_distributions/point_m/mean"],
                Some("m")
            );
            field!(
                "validation_p95_error",
                "validation",
                "Validation P95 error",
                &["/validation/error_distributions/point_m/p95"],
                Some("m")
            );
            // Standalone distribution reports do not imply a split from a name.
            field!(
                "point_mean_error",
                "validation",
                "Point error",
                &["/error_distributions/point_m/mean"],
                Some("m")
            );
            field!(
                "point_p95_error",
                "validation",
                "P95 point error",
                &["/error_distributions/point_m/p95"],
                Some("m")
            );
            field!(
                "closed_loop_success_measured",
                "evaluation",
                "Closed-loop success measured",
                &[
                    "/closed_loop_success_measured",
                    "/validation/closed_loop_success_measured"
                ],
                None
            );
            field!(
                "evaluation_mode",
                "evaluation",
                "Evaluation",
                &[
                    "/research/evaluation/mode",
                    "/evaluation_scope",
                    "/evaluation_mode",
                    "/evaluation_type",
                    "/performed_scope"
                ],
                None
            );
            field!(
                "offline_only",
                "evaluation",
                "Offline only",
                &["/offline_only"],
                None
            );
            field!(
                "physical_success_measured",
                "evaluation",
                "Physical success measured",
                &["/physical_success_measured"],
                None
            );
            field!(
                "evaluator",
                "evaluation",
                "Evaluator",
                &[
                    "/research/evaluation/evaluator",
                    "/common_observer",
                    "/observer_model_id",
                    "/evaluator",
                    "/observer_identity"
                ],
                None
            );
            field!(
                "cohort",
                "evaluation",
                "Cohort",
                &["/research/evaluation/cohort", "/cohort", "/population"],
                None
            );
            field!(
                "split",
                "evaluation",
                "Split",
                &["/research/evaluation/split", "/split", "/case/split"],
                None
            );
            field!(
                "planned",
                "population",
                "Planned",
                &[
                    "/research/evaluation/planned",
                    "/planned_cases",
                    "/counts/planned",
                    "/planned"
                ],
                Some("cases")
            );
            field!(
                "valid",
                "population",
                "Valid",
                &[
                    "/research/evaluation/valid",
                    "/valid_cases",
                    "/counts/valid",
                    "/valid"
                ],
                Some("cases")
            );
            field!(
                "infra",
                "population",
                "Infrastructure failures",
                &["/research/evaluation/infra", "/counts/infra", "/infra"],
                Some("cases")
            );
            field!(
                "pending",
                "population",
                "Pending",
                &[
                    "/research/evaluation/pending",
                    "/counts/pending",
                    "/pending"
                ],
                Some("cases")
            );
            field!(
                "success_criteria",
                "scoring",
                "Success criteria",
                &[
                    "/research/evaluation/success_criteria",
                    "/success_criteria",
                    "/protocol/scoring/strict",
                    "/physical_contract/positive_BC_eligibility"
                ],
                None
            );
            for (id, label) in [
                ("distance_tolerance_mm", "Position tolerance"),
                ("angle_tolerance_deg", "Angle tolerance"),
                ("heading_tolerance_deg", "Heading tolerance"),
                ("continuous_s", "Continuous hold"),
                ("active_hold_s", "Hold window"),
            ] {
                let paths = [
                    format!("/research/evaluation/{id}"),
                    format!("/protocol/{id}"),
                    format!("/protocol/scoring/{id}"),
                    format!("/goal/{id}"),
                ];
                let unit = if id.ends_with("mm") {
                    "mm"
                } else if id.ends_with("deg") {
                    "°"
                } else {
                    "s"
                };
                facts.add(
                    d,
                    id,
                    "scoring",
                    label,
                    &paths.iter().map(String::as_str).collect::<Vec<_>>(),
                    Some(unit),
                );
            }
            field!(
                "hold_seconds",
                "scoring",
                "Continuous hold",
                &["/physical_contract/stable_hold_seconds"],
                Some("s")
            );
            field!(
                "hold_center_drift_m",
                "scoring",
                "Hold center drift",
                &["/physical_contract/relative_center_drift_limit_m"],
                Some("m")
            );
            field!(
                "height_tolerance_m",
                "scoring",
                "Height tolerance",
                &["/physical_contract/height_tolerance_m"],
                Some("m")
            );
            field!(
                "rotation_tolerance_deg",
                "scoring",
                "Hold rotation drift",
                &["/physical_contract/relative_rotation_limit_deg"],
                Some("°")
            );
        }
    }
    if kind == "dataset"
        && entity["dataset_overview"]["frames"].is_null()
        && entity["dataset_overview"]["frames_missing_reason"] == "not_applicable"
    {
        facts.values.remove("frames");
    }
    json!({"schema":"loom-research-summary/v1","kind":kind,"facts":facts.values.into_values().collect::<Vec<_>>(),"inputs":facts.inputs})
}

/// Enrichment is bounded to registered JSON artifacts of this exact record. It
/// never follows a path found in a JSON document or recursively opens archives.
pub fn enrich(app: &Application, entity: &mut Value) {
    if !matches!(
        entity["kind"].as_str(),
        Some("model" | "dataset" | "experiment" | "run")
    ) {
        return;
    }
    let mut files = entity["loom_files"].as_array().cloned().unwrap_or_default();
    // A sibling scientific report may describe a unique product of this exact
    // publication, but only an explicit artifact hash binding admits its facts.
    let own_keys: std::collections::BTreeSet<_> = files
        .iter()
        .filter_map(|f| f["id"].as_str().map(String::from))
        .collect();
    if entity["loom_registry"].is_object()
        && matches!(entity["kind"].as_str(), Some("model" | "dataset"))
    {
        let job = entity["loom_job"]["id"].as_str();
        let attempt = entity["attempt"].as_str();
        let kind = entity["kind"].as_str();
        let unique: i64=app.db.query_row("SELECT count(*) FROM all_entity_revisions WHERE job=? AND kind=? AND json_extract(manifest,'$.attempt')=?",rusqlite::params![job,kind,attempt],|r|r.get(0)).unwrap_or(0);
        if unique == 1 {
            if let Ok(siblings)=rows(&app.db,"SELECT id,revision,manifest FROM all_entity_revisions WHERE job=? AND json_extract(manifest,'$.attempt')=? AND kind IN ('artifact','experiment') ORDER BY id LIMIT 16",&[&job,&attempt]) {
                for row in siblings {
                    let manifest=serde_json::from_str::<Value>(row["manifest"].as_str().unwrap_or("{}"));
                    let locations=rows(&app.db,"SELECT * FROM all_entity_locations WHERE id=? AND revision=?",&[&row["id"].as_str(),&row["revision"].as_i64()]);
                    if let (Ok(m),Ok(l))=(manifest,locations){let (f,_)=media::file_dtos(&json!({"id":row["id"],"revision":row["revision"]}),&m,&l,&app.config);files.extend(f.as_array().into_iter().flatten().cloned());}
                }
            }
        }
    }
    // Small summaries before inventories; no model checkpoint or HDF5 is opened.
    files.sort_by_key(|f| f["size"].as_u64().unwrap_or(u64::MAX));
    let mut used = 0;
    let mut docs = vec![];
    let mut sources = vec![];
    for f in files
        .iter()
        .filter(|f| {
            f["source_ref"]
                .as_str()
                .is_some_and(|s| s.ends_with(".json"))
        })
        .take(FILES)
    {
        let size = f["size"].as_u64().unwrap_or(u64::MAX);
        if size > FILE_BYTES || used + size > ENTITY_BYTES {
            continue;
        }
        used += size;
        let Some(key) = f["id"].as_str() else {
            continue;
        };
        let mut source =
            json!({"kind":"file","ref":f["source_ref"],"media_id":key,"status":"unavailable"});
        if let Ok(request) = app.registered_media(key, Some(FILE_BYTES)) {
            source["sha256"] = json!(request.sha256);
            if let Ok(reply) = media::read(request) {
                if let Ok(value) = serde_json::from_slice::<Value>(&reply.body) {
                    if value.is_object() {
                        let own = own_keys.contains(key);
                        let bound = entity["components"].as_array().is_some_and(|components| {
                            !components.is_empty()
                                && components.iter().all(|c| {
                                    value["artifacts"][c["ref"].as_str().unwrap_or("")]["sha256"]
                                        == c["sha256"]
                                        && c["sha256"].is_string()
                                })
                        });
                        if !own && !bound {
                            continue;
                        }
                        source["status"] = json!("verified");
                        if !own {
                            source["binding"] = json!({"kind":"publication_artifact_sha256","target":entity["loom_registry"]});
                        }
                        docs.push((value, source.clone()));
                    }
                }
            }
        }
        sources.push(source);
    }
    let mut summary = summarize(entity, &docs);
    summary["sources"] = json!(sources);
    entity["research_summary"] = summary;
}

fn snapshot_target(entity: &Value) -> Option<crate::contracts::Target> {
    let (kind, identity) = if entity["loom_registry"].is_object() {
        ("entity", entity["loom_registry"].clone())
    } else if entity["loom_history"].is_object() {
        ("historical", entity["loom_history"].clone())
    } else {
        return None;
    };
    Some(crate::contracts::Target {
        kind: kind.into(),
        identity,
    })
}
fn snapshot_key(target: &crate::contracts::Target) -> crate::Result<String> {
    Ok(crate::store::hash(serde_json::to_vec(target)?))
}
fn source_identity(
    app: &Application,
    target: &crate::contracts::Target,
) -> crate::Result<(String, String)> {
    use rusqlite::params;
    crate::results::verify_target(&app.db, target, &app.config)?;
    let v = &target.identity;
    let (body,kind):(String,String)=match target.kind.as_str() {
        "entity"=>app.db.query_row("SELECT manifest,kind FROM all_entity_revisions WHERE id=? AND revision=?",params![v["id"].as_str(),v["revision"].as_i64()],|r|Ok((r.get(0)?,r.get(1)?)))?,
        "historical"=>app.db.query_row("SELECT body,kind FROM history_records WHERE source=? AND kind=? AND id=? AND revision=? AND snapshot=?",params![v["source"].as_str(),v["kind"].as_str(),v["id"].as_str(),v["revision"].as_str(),v["snapshot"].as_str()],|r|Ok((r.get(0)?,r.get(1)?)))?,
        _=>return Err(crate::Error::input("Exact entity or historical research target required"))
    };
    crate::error::require(
        matches!(kind.as_str(), "dataset" | "model" | "experiment" | "run"),
        "metadata_kind",
        "Target has no scientific metadata projection",
        400,
    )?;
    Ok((crate::store::hash(body), kind))
}
/// Retain recovered facts on the exact original revision. This is an annotation,
/// never a replacement of an accepted manifest or a historical source body.
pub fn attach_materialized(app: &Application, entity: &mut Value) -> crate::Result<()> {
    use rusqlite::OptionalExtension;
    let Some(target) = snapshot_target(entity) else {
        return Ok(());
    };
    let old: Option<String> = app
        .db
        .query_row(
            "SELECT body FROM rust_annotations WHERE kind='research_metadata' AND id=?",
            [snapshot_key(&target)?],
            |r| r.get(0),
        )
        .optional()?;
    let Some(old) = old else {
        return Ok(());
    };
    let saved: Value = serde_json::from_str(&old)?;
    let (sha, _) = source_identity(app, &target)?;
    if saved["source_sha256"] != sha || saved["extractor_version"] != 1 {
        return Ok(());
    }
    if !entity["research_summary"].is_object() {
        entity["research_summary"] = summarize(entity, &[]);
    }
    let summary = &mut entity["research_summary"];
    for key in ["facts", "inputs", "sources"] {
        let mut current = summary[key].as_array().cloned().unwrap_or_default();
        for item in saved["summary"][key].as_array().into_iter().flatten() {
            let duplicate = current.iter().any(|old| match key {
                "facts" => old["id"] == item["id"],
                "inputs" => old["kind"] == item["kind"] && old["label"] == item["label"],
                _ => old["media_id"] == item["media_id"] && old["sha256"] == item["sha256"],
            });
            if !duplicate {
                current.push(item.clone());
            }
        }
        summary[key] = json!(current);
    }
    summary["materialized"] = json!({"source_sha256":sha,"captured_at":saved["captured_at"],"captured_by":saved["captured_by"]});
    Ok(())
}
impl Application {
    pub fn refresh_research_metadata(
        &mut self,
        actor: &crate::auth::Actor,
        args: Value,
    ) -> crate::Result<Value> {
        use crate::{error::require, store::*};
        let target: crate::contracts::Target = crate::contracts::decode(args["target"].clone())?;
        let (source_sha, kind) = source_identity(self, &target)?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(true);
        if !dry_run {
            require(
                args["expected_source_sha256"] == source_sha,
                "source_changed",
                "Source digest changed or preview digest missing",
                409,
            )?;
            if let Some(receipt) = replay(
                &self.db,
                actor,
                args["key"]
                    .as_str()
                    .ok_or_else(|| crate::Error::input("Request key required"))?,
                "research.metadata",
                &args,
            )? {
                return Ok(receipt);
            }
        }
        let selected = json!({"native_ref":target,"kind":kind});
        let value = self.dashboard_catalog_selected(Some(&[selected]))?;
        let mut entity = value["entities"]
            .as_array()
            .and_then(|a| {
                a.iter().find(|e| {
                    snapshot_target(e)
                        .is_some_and(|t| t.kind == target.kind && t.identity == target.identity)
                })
            })
            .cloned()
            .ok_or_else(|| crate::Error::input("Target projection unavailable"))?;
        // Recompute from accepted metadata and currently verified sources. A
        // previous snapshot cannot masquerade as newly verified evidence.
        enrich(self, &mut entity);
        let summary = &entity["research_summary"];
        let facts = summary["facts"].as_array().map(Vec::len).unwrap_or(0);
        let inputs = summary["inputs"].as_array().map(Vec::len).unwrap_or(0);
        let mut receipt = json!({"target":target,"source_sha256":source_sha,"facts":facts,"inputs":inputs,"dry_run":dry_run,"stored":false});
        if dry_run {
            return Ok(receipt);
        }
        require(
            facts + inputs > 0,
            "metadata_unavailable",
            "No recorded scientific facts or typed inputs could be recovered",
            409,
        )?;
        let snapshot = json!({"schema":"loom-materialized-research/v1","extractor_version":1,"target":target,"source_sha256":source_sha,"summary":summary,"captured_at":now(),"captured_by":actor.id,"reason":args["reason"]});
        require(
            serde_json::to_vec(&snapshot)?.len() <= 1024 * 1024,
            "metadata_size",
            "Recovered metadata exceeds one MiB",
            400,
        )?;
        let tx = self.transaction()?;
        active(&tx)?;
        tx.execute("INSERT INTO rust_annotations VALUES('research_metadata',?,?,?) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body,updated=excluded.updated",rusqlite::params![snapshot_key(&target)?,encode(&snapshot)?,now()])?;
        receipt["stored"] = json!(true);
        crate::store::receipt(
            &tx,
            actor,
            args["key"].as_str().unwrap(),
            "research.metadata",
            &args,
            &receipt,
        )?;
        tx.commit()?;
        Ok(receipt)
    }
}
