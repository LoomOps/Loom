CREATE TABLE IF NOT EXISTS rust_authority(id INTEGER PRIMARY KEY CHECK(id=1),epoch INTEGER NOT NULL,minimum_writer INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS rust_receipts(actor TEXT NOT NULL,key TEXT NOT NULL,operation TEXT NOT NULL,request TEXT NOT NULL,result TEXT NOT NULL,created REAL NOT NULL,PRIMARY KEY(actor,key));
CREATE TABLE IF NOT EXISTS rust_effects(id TEXT PRIMARY KEY,job TEXT,attempt TEXT,stage TEXT,epoch INTEGER NOT NULL,state TEXT NOT NULL,request TEXT NOT NULL,evidence TEXT,created REAL NOT NULL,updated REAL NOT NULL);
CREATE INDEX IF NOT EXISTS rust_effects_active_job ON rust_effects(job,attempt,epoch) WHERE state IN ('pending','accepted','unknown');
CREATE TABLE IF NOT EXISTS scenarios(id TEXT PRIMARY KEY,label TEXT NOT NULL,current_revision INTEGER NOT NULL,archived INTEGER NOT NULL DEFAULT 0,created REAL NOT NULL,created_by TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS scenario_revisions(scenario_id TEXT NOT NULL,revision INTEGER NOT NULL,definition TEXT NOT NULL,sha256 TEXT NOT NULL,created REAL NOT NULL,created_by TEXT NOT NULL,reason TEXT NOT NULL,PRIMARY KEY(scenario_id,revision),FOREIGN KEY(scenario_id) REFERENCES scenarios(id));
CREATE TABLE IF NOT EXISTS scenario_links(id TEXT PRIMARY KEY,scenario_id TEXT NOT NULL,revision INTEGER NOT NULL,target_kind TEXT NOT NULL,target TEXT NOT NULL,relation TEXT NOT NULL,provenance TEXT NOT NULL,created REAL NOT NULL,created_by TEXT NOT NULL,superseded_by TEXT,reason TEXT,FOREIGN KEY(scenario_id,revision) REFERENCES scenario_revisions(scenario_id,revision));
CREATE INDEX IF NOT EXISTS scenario_links_lookup ON scenario_links(scenario_id,revision,target_kind);
CREATE TABLE IF NOT EXISTS historical_bindings(id TEXT PRIMARY KEY,identity TEXT NOT NULL,locator TEXT NOT NULL,sha256 TEXT NOT NULL,size INTEGER NOT NULL,created REAL NOT NULL,created_by TEXT NOT NULL,UNIQUE(identity,locator));
CREATE TABLE IF NOT EXISTS rust_job_requests(job TEXT PRIMARY KEY,actor TEXT NOT NULL,key TEXT NOT NULL,request TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 0,epoch INTEGER NOT NULL,UNIQUE(actor,key));
CREATE TABLE IF NOT EXISTS rust_annotations(kind TEXT NOT NULL,id TEXT NOT NULL,body TEXT NOT NULL,updated REAL NOT NULL,PRIMARY KEY(kind,id));

CREATE TABLE IF NOT EXISTS rust_node_events(job TEXT,attempt TEXT,code TEXT,recorded REAL,root TEXT,PRIMARY KEY(job,attempt,code));

CREATE TABLE IF NOT EXISTS rust_recovery_receipts(job TEXT NOT NULL,actor TEXT NOT NULL,lease_hash TEXT NOT NULL,request TEXT NOT NULL,result TEXT NOT NULL,PRIMARY KEY(job,actor,lease_hash));

CREATE TABLE IF NOT EXISTS rust_runtime_observation(id INTEGER PRIMARY KEY CHECK(id=1),value TEXT NOT NULL,observed REAL NOT NULL);

CREATE TABLE IF NOT EXISTS rust_job_fences(job TEXT PRIMARY KEY,revision INTEGER NOT NULL,epoch INTEGER NOT NULL);

CREATE TABLE IF NOT EXISTS rust_attempt_starts(attempt TEXT PRIMARY KEY,accepted REAL);

CREATE TABLE IF NOT EXISTS rust_projection_version(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL);
INSERT OR IGNORE INTO rust_projection_version VALUES(1,1);
CREATE TRIGGER IF NOT EXISTS rust_projection_jobs_insert AFTER INSERT ON jobs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_jobs_update AFTER UPDATE ON jobs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_jobs_delete AFTER DELETE ON jobs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_attempts_insert AFTER INSERT ON attempts BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_attempts_update AFTER UPDATE ON attempts BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_attempts_delete AFTER DELETE ON attempts BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_stage_runs_insert AFTER INSERT ON stage_runs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_stage_runs_update AFTER UPDATE ON stage_runs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_stage_runs_delete AFTER DELETE ON stage_runs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_workflow_nodes_insert AFTER INSERT ON workflow_nodes BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_workflow_nodes_update AFTER UPDATE ON workflow_nodes BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_workflow_nodes_delete AFTER DELETE ON workflow_nodes BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_job_workflows_insert AFTER INSERT ON job_workflows BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_job_workflows_update AFTER UPDATE ON job_workflows BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_job_workflows_delete AFTER DELETE ON job_workflows BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_recovery_insert AFTER INSERT ON recovery BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_recovery_update AFTER UPDATE ON recovery BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_recovery_delete AFTER DELETE ON recovery BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_dispatch_control_insert AFTER INSERT ON dispatch_control BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_dispatch_control_update AFTER UPDATE ON dispatch_control BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_dispatch_control_delete AFTER DELETE ON dispatch_control BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_node_control_insert AFTER INSERT ON node_control BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_node_control_update AFTER UPDATE ON node_control BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_node_control_delete AFTER DELETE ON node_control BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_entity_revisions_insert AFTER INSERT ON entity_revisions BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_entity_revisions_update AFTER UPDATE ON entity_revisions BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_entity_revisions_delete AFTER DELETE ON entity_revisions BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_entity_locations_insert AFTER INSERT ON entity_locations BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_entity_locations_update AFTER UPDATE ON entity_locations BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_entity_locations_delete AFTER DELETE ON entity_locations BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_components_insert AFTER INSERT ON components BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_components_update AFTER UPDATE ON components BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_components_delete AFTER DELETE ON components BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_tools_insert AFTER INSERT ON tools BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_tools_update AFTER UPDATE ON tools BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_tools_delete AFTER DELETE ON tools BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_execution_profiles_insert AFTER INSERT ON execution_profiles BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_execution_profiles_update AFTER UPDATE ON execution_profiles BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_execution_profiles_delete AFTER DELETE ON execution_profiles BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_software_metadata_insert AFTER INSERT ON software_metadata BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_software_metadata_update AFTER UPDATE ON software_metadata BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_software_metadata_delete AFTER DELETE ON software_metadata BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_runtime_curation_insert AFTER INSERT ON runtime_curation BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_runtime_curation_update AFTER UPDATE ON runtime_curation BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_runtime_curation_delete AFTER DELETE ON runtime_curation BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_research_overlays_insert AFTER INSERT ON research_overlays BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_research_overlays_update AFTER UPDATE ON research_overlays BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_research_overlays_delete AFTER DELETE ON research_overlays BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_item_owners_insert AFTER INSERT ON item_owners BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_item_owners_update AFTER UPDATE ON item_owners BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_item_owners_delete AFTER DELETE ON item_owners BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_history_records_insert AFTER INSERT ON history_records BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_history_records_update AFTER UPDATE ON history_records BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_history_records_delete AFTER DELETE ON history_records BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenario_revisions_insert AFTER INSERT ON scenario_revisions BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenario_revisions_update AFTER UPDATE ON scenario_revisions BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenario_revisions_delete AFTER DELETE ON scenario_revisions BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenarios_insert AFTER INSERT ON scenarios BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenarios_update AFTER UPDATE ON scenarios BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenarios_delete AFTER DELETE ON scenarios BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenario_links_insert AFTER INSERT ON scenario_links BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenario_links_update AFTER UPDATE ON scenario_links BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_scenario_links_delete AFTER DELETE ON scenario_links BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_historical_bindings_insert AFTER INSERT ON historical_bindings BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_historical_bindings_update AFTER UPDATE ON historical_bindings BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_historical_bindings_delete AFTER DELETE ON historical_bindings BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_rust_attempt_starts_insert AFTER INSERT ON rust_attempt_starts BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_rust_attempt_starts_update AFTER UPDATE ON rust_attempt_starts BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_rust_attempt_starts_delete AFTER DELETE ON rust_attempt_starts BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_runtime_insert AFTER INSERT ON rust_runtime_observation BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_runtime_update AFTER UPDATE ON rust_runtime_observation WHEN OLD.value IS NOT NEW.value BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_person_insert AFTER INSERT ON user_profiles BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_person_update AFTER UPDATE ON user_profiles WHEN OLD.display_name IS NOT NEW.display_name OR OLD.login IS NOT NEW.login BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_annotations_insert AFTER INSERT ON job_annotations BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;

CREATE TRIGGER IF NOT EXISTS rust_projection_metadata_insert AFTER INSERT ON rust_annotations WHEN NEW.kind='research_metadata' BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_metadata_update AFTER UPDATE ON rust_annotations WHEN NEW.kind='research_metadata' BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS rust_projection_metadata_delete AFTER DELETE ON rust_annotations WHEN OLD.kind='research_metadata' BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; END;
CREATE TABLE IF NOT EXISTS registry_lifecycle_events(seq INTEGER PRIMARY KEY AUTOINCREMENT,target TEXT NOT NULL,previous TEXT NOT NULL,state TEXT NOT NULL,reason TEXT NOT NULL,actor TEXT NOT NULL,recorded REAL NOT NULL);

CREATE INDEX IF NOT EXISTS registry_lifecycle_target_history ON registry_lifecycle_events(target,seq DESC);

CREATE TABLE IF NOT EXISTS rust_plan_revisions(
 root TEXT NOT NULL REFERENCES job_workflows(id), revision INTEGER NOT NULL,
 actor TEXT NOT NULL, key TEXT NOT NULL, request TEXT NOT NULL, response TEXT NOT NULL,
 recorded REAL NOT NULL, PRIMARY KEY(root,revision), UNIQUE(root,actor,key)
);

CREATE TABLE IF NOT EXISTS research_jobs(id TEXT PRIMARY KEY REFERENCES job_workflows(id));
CREATE TABLE IF NOT EXISTS unit_pauses(
    job TEXT NOT NULL, attempt TEXT NOT NULL, state TEXT NOT NULL, requested REAL NOT NULL,
    requested_by TEXT NOT NULL, reason TEXT NOT NULL, checkpoint TEXT, resume_attempt TEXT,
    PRIMARY KEY(job,attempt)
);
CREATE INDEX IF NOT EXISTS unit_pauses_resume ON unit_pauses(job,resume_attempt);
CREATE TABLE IF NOT EXISTS execution_tuning(
    key TEXT PRIMARY KEY, settings TEXT NOT NULL, evidence TEXT NOT NULL,
    job TEXT NOT NULL, attempt TEXT NOT NULL, recorded REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS output_edit_claims(
 target TEXT PRIMARY KEY, root TEXT NOT NULL UNIQUE REFERENCES research_jobs(id),
 base INTEGER NOT NULL, acquired REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS execution_units(
 id TEXT PRIMARY KEY REFERENCES jobs(id),
 job_id TEXT REFERENCES research_jobs(id),
 name TEXT NOT NULL,
 label TEXT NOT NULL,
 phase TEXT NOT NULL CHECK(phase IN ('prepare','run','finalize')),
 UNIQUE(job_id,name)
);
CREATE VIEW IF NOT EXISTS execution_memberships AS
 SELECT rowid,root,node,label,job FROM workflow_nodes
 UNION ALL
 SELECT rowid,job_id,name,label,id FROM execution_units WHERE job_id IS NOT NULL;

-- Retrospective classification overlays. Exact source records remain authoritative.
CREATE TABLE IF NOT EXISTS reviewed_work(
 id TEXT PRIMARY KEY,
 entry_type TEXT NOT NULL CHECK(entry_type IN ('job','unit')),
 job_id TEXT,
 visible INTEGER NOT NULL CHECK(visible IN (0,1)),
 reason TEXT NOT NULL,
 CHECK(job_id IS NULL OR (entry_type='unit' AND id<>job_id))
);
CREATE TABLE IF NOT EXISTS reviewed_outputs(
 source TEXT NOT NULL, kind TEXT NOT NULL, id TEXT NOT NULL,
 revision TEXT NOT NULL, snapshot TEXT NOT NULL,
 catalog_class TEXT NOT NULL CHECK(catalog_class IN ('regular','reference')),
 reason TEXT NOT NULL,
 PRIMARY KEY(source,kind,id,revision,snapshot)
);
CREATE TABLE IF NOT EXISTS reviewed_clock(id INTEGER PRIMARY KEY CHECK(id=1),version INTEGER NOT NULL);
INSERT OR IGNORE INTO reviewed_clock VALUES(1,0);
CREATE TRIGGER IF NOT EXISTS reviewed_work_insert AFTER INSERT ON reviewed_work BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; UPDATE reviewed_clock SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS reviewed_work_update AFTER UPDATE ON reviewed_work BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; UPDATE reviewed_clock SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS reviewed_work_delete AFTER DELETE ON reviewed_work BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; UPDATE reviewed_clock SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS reviewed_outputs_insert AFTER INSERT ON reviewed_outputs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; UPDATE reviewed_clock SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS reviewed_outputs_update AFTER UPDATE ON reviewed_outputs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; UPDATE reviewed_clock SET version=version+1 WHERE id=1; END;
CREATE TRIGGER IF NOT EXISTS reviewed_outputs_delete AFTER DELETE ON reviewed_outputs BEGIN UPDATE rust_projection_version SET version=version+1 WHERE id=1; UPDATE reviewed_clock SET version=version+1 WHERE id=1; END;
