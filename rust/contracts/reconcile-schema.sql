-- Explicit native schema 7. Scientific originals and specialized IDs stay intact.
CREATE TABLE IF NOT EXISTS reconcile_plans(id TEXT PRIMARY KEY,actor TEXT NOT NULL,request_key TEXT NOT NULL,request TEXT NOT NULL,contract TEXT NOT NULL,source_context TEXT NOT NULL,state TEXT NOT NULL,validation TEXT,receipt TEXT,created REAL NOT NULL,UNIQUE(actor,request_key));
CREATE TABLE IF NOT EXISTS asset_imports(id TEXT NOT NULL,revision INTEGER NOT NULL,source TEXT NOT NULL UNIQUE,recorded REAL NOT NULL,provenance TEXT NOT NULL,PRIMARY KEY(id,revision));
CREATE TABLE IF NOT EXISTS file_bindings(file BLOB NOT NULL,locator TEXT NOT NULL,evidence TEXT NOT NULL,PRIMARY KEY(file,locator));
CREATE TABLE IF NOT EXISTS log_retention(stream BLOB NOT NULL,generation TEXT NOT NULL,start INTEGER NOT NULL,end INTEGER NOT NULL,reason TEXT NOT NULL,created REAL NOT NULL,PRIMARY KEY(stream,generation,start));
CREATE TABLE IF NOT EXISTS catalog_tasks(id TEXT PRIMARY KEY,actor TEXT NOT NULL,request_key TEXT NOT NULL,kind TEXT NOT NULL,request TEXT NOT NULL,context TEXT NOT NULL,state TEXT NOT NULL,progress TEXT NOT NULL,receipt TEXT,error TEXT,created REAL NOT NULL,updated REAL NOT NULL,UNIQUE(actor,request_key));
CREATE TABLE IF NOT EXISTS asset_revisions(id TEXT NOT NULL,revision INTEGER NOT NULL,job TEXT,kind TEXT NOT NULL,owner TEXT NOT NULL,submitter TEXT NOT NULL,manifest TEXT NOT NULL,PRIMARY KEY(id,revision));
CREATE TABLE IF NOT EXISTS asset_locations(id TEXT NOT NULL,revision INTEGER NOT NULL,component TEXT NOT NULL,machine_id TEXT NOT NULL,storage_id TEXT NOT NULL,ref TEXT NOT NULL,status TEXT NOT NULL,PRIMARY KEY(id,revision,component,machine_id,storage_id,ref));
CREATE VIEW IF NOT EXISTS all_entity_revisions AS SELECT * FROM entity_revisions UNION ALL SELECT * FROM asset_revisions;
CREATE VIEW IF NOT EXISTS all_entity_locations AS SELECT * FROM entity_locations UNION ALL SELECT * FROM asset_locations;
CREATE INDEX IF NOT EXISTS execution_profile_kind ON execution_profiles(json_extract(profile,'$.revision'));
CREATE INDEX IF NOT EXISTS job_profile_kind ON jobs(json_extract(spec,'$.kind'));
