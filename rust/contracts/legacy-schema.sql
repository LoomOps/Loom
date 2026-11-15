CREATE TABLE IF NOT EXISTS attempt_results(job TEXT,attempt TEXT,result TEXT NOT NULL,PRIMARY KEY(job,attempt));
CREATE TABLE IF NOT EXISTS attempts(
            id TEXT PRIMARY KEY, job TEXT NOT NULL, state TEXT NOT NULL,
            started REAL NOT NULL, ended REAL);
CREATE TABLE IF NOT EXISTS auth_identities(
      issuer TEXT NOT NULL, subject TEXT NOT NULL, profile_id TEXT NOT NULL,
      PRIMARY KEY(issuer,subject));
CREATE TABLE IF NOT EXISTS components(owner TEXT,id TEXT,revision INTEGER,
        manifest TEXT,profiles TEXT,PRIMARY KEY(owner,id,revision));
CREATE TABLE IF NOT EXISTS credentials(
      id TEXT PRIMARY KEY, principal TEXT NOT NULL REFERENCES principals(id),
      digest TEXT NOT NULL UNIQUE, expires REAL NOT NULL, revoked REAL);
CREATE TABLE IF NOT EXISTS delivery(
      event INTEGER PRIMARY KEY REFERENCES events(seq), state TEXT NOT NULL,
      receipt TEXT, version INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS delivery_history(
      event INTEGER NOT NULL REFERENCES events(seq), version INTEGER NOT NULL,
      state TEXT NOT NULL, receipt TEXT NOT NULL, recorded REAL NOT NULL,
      PRIMARY KEY(event,version));
CREATE TABLE IF NOT EXISTS dispatch_control(id INTEGER PRIMARY KEY CHECK(id=1),paused INTEGER NOT NULL,token TEXT,reason TEXT,recorded REAL);
CREATE TABLE IF NOT EXISTS entity_locations(
      id TEXT NOT NULL, revision INTEGER NOT NULL, component TEXT NOT NULL,
      machine_id TEXT NOT NULL, storage_id TEXT NOT NULL, ref TEXT NOT NULL,
      status TEXT NOT NULL, PRIMARY KEY(id,revision,component,machine_id,storage_id,ref));
CREATE TABLE IF NOT EXISTS entity_revisions(
      id TEXT NOT NULL, revision INTEGER NOT NULL, job TEXT NOT NULL,
      kind TEXT NOT NULL, owner TEXT NOT NULL, submitter TEXT NOT NULL,
      manifest TEXT NOT NULL, PRIMARY KEY(id,revision));
CREATE TABLE IF NOT EXISTS events(
            seq INTEGER PRIMARY KEY, job TEXT NOT NULL, target TEXT NOT NULL,
            code TEXT NOT NULL, created REAL NOT NULL, ack INTEGER NOT NULL DEFAULT 0, attempt TEXT);
CREATE TABLE IF NOT EXISTS execution_profiles(owner TEXT,id TEXT,revision INTEGER,profile TEXT,PRIMARY KEY(owner,id,revision));
CREATE TABLE IF NOT EXISTS finalizer_receipts(job TEXT,attempt TEXT,completed REAL NOT NULL,PRIMARY KEY(job,attempt));
CREATE TABLE IF NOT EXISTS history_imports(
        id TEXT PRIMARY KEY, manifest_sha256 TEXT NOT NULL, imported_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS history_records(
        source TEXT NOT NULL, kind TEXT NOT NULL, id TEXT NOT NULL,
        revision TEXT NOT NULL, snapshot TEXT NOT NULL,
        body TEXT NOT NULL, PRIMARY KEY(source,kind,id,revision,snapshot));
CREATE TABLE IF NOT EXISTS hooks(job TEXT,attempt TEXT,stage TEXT,state TEXT,
      retries INTEGER NOT NULL DEFAULT 0,next_try REAL NOT NULL DEFAULT 0,error TEXT, spec_snapshot TEXT, result_snapshot TEXT, owner_snapshot TEXT, notify_snapshot TEXT,
      PRIMARY KEY(job,attempt,stage));
CREATE TABLE IF NOT EXISTS item_owners(
      kind TEXT NOT NULL, id TEXT NOT NULL, owner TEXT NOT NULL,
      updated REAL NOT NULL, updated_by TEXT NOT NULL, PRIMARY KEY(kind,id));
CREATE TABLE IF NOT EXISTS job_annotations(
      job TEXT NOT NULL, id TEXT NOT NULL, payload TEXT NOT NULL, PRIMARY KEY(job,id));
CREATE TABLE IF NOT EXISTS job_revisions(
      job TEXT NOT NULL, revision INTEGER NOT NULL, spec TEXT NOT NULL,
      reason TEXT NOT NULL, created REAL NOT NULL, PRIMARY KEY(job,revision));
CREATE TABLE IF NOT EXISTS job_workflows(id TEXT PRIMARY KEY,submitter TEXT,key TEXT,request TEXT NOT NULL,owner TEXT NOT NULL,notify TEXT NOT NULL,created REAL NOT NULL,state TEXT NOT NULL, observed_errors TEXT NOT NULL DEFAULT '[]',UNIQUE(submitter,key));
CREATE TABLE IF NOT EXISTS jobs(
            id TEXT PRIMARY KEY, submitter TEXT NOT NULL, idem TEXT NOT NULL,
            spec TEXT NOT NULL, owner TEXT NOT NULL, notify TEXT NOT NULL,
            state TEXT NOT NULL, attempt TEXT, created REAL NOT NULL,
            result TEXT, cancel INTEGER NOT NULL DEFAULT 0, available_at REAL NOT NULL DEFAULT 0, queue_order REAL, queue_held INTEGER NOT NULL DEFAULT 0,
            UNIQUE(submitter,idem));
CREATE TABLE IF NOT EXISTS node_control(id INTEGER PRIMARY KEY CHECK(id=1), mode TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS ownership_history(
      seq INTEGER PRIMARY KEY, kind TEXT NOT NULL, id TEXT NOT NULL,
      previous_owner TEXT, owner TEXT NOT NULL, changed_by TEXT NOT NULL, changed REAL NOT NULL);
CREATE TABLE IF NOT EXISTS principals(
      id TEXT PRIMARY KEY, kind TEXT NOT NULL, grant_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS queue_history(
 seq INTEGER PRIMARY KEY,job TEXT NOT NULL,actor TEXT NOT NULL,action TEXT NOT NULL,
 recorded REAL NOT NULL,reason TEXT,before_order TEXT NOT NULL,after_order TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS recovery(
      job TEXT PRIMARY KEY, state TEXT NOT NULL, rounds INTEGER NOT NULL DEFAULT 0,
      agent TEXT, token TEXT, deadline REAL, hard_deadline REAL,
      reason TEXT, updated REAL NOT NULL);
CREATE TABLE IF NOT EXISTS recovery_history(
      seq INTEGER PRIMARY KEY, job TEXT NOT NULL, action TEXT NOT NULL,
      agent TEXT, round INTEGER NOT NULL, recorded REAL NOT NULL, detail TEXT);
CREATE TABLE IF NOT EXISTS research_batches(actor TEXT,key TEXT,request TEXT NOT NULL,created REAL NOT NULL,PRIMARY KEY(actor,key));
CREATE TABLE IF NOT EXISTS research_overlays(kind TEXT,id TEXT,presentation TEXT NOT NULL,actor TEXT NOT NULL,reason TEXT NOT NULL,updated REAL NOT NULL,PRIMARY KEY(kind,id));
CREATE TABLE IF NOT EXISTS resident_tokens(
      id TEXT PRIMARY KEY, digest TEXT NOT NULL UNIQUE, label TEXT NOT NULL,
      owner TEXT NOT NULL, scopes TEXT NOT NULL, created REAL NOT NULL,
      created_by TEXT NOT NULL, revoked REAL, revoked_by TEXT);
CREATE TABLE IF NOT EXISTS result_outbox(
            job TEXT PRIMARY KEY, attempt TEXT NOT NULL,
            retries INTEGER NOT NULL DEFAULT 0, next_try REAL NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS runtime_curation(kind TEXT,id TEXT,revision INTEGER,value TEXT,actor TEXT,updated REAL,
 PRIMARY KEY(kind,id,revision));
CREATE TABLE IF NOT EXISTS software_metadata(
        kind TEXT,owner TEXT,id TEXT,revision INTEGER,value TEXT,actor TEXT,updated REAL,
        PRIMARY KEY(kind,owner,id,revision));
CREATE TABLE IF NOT EXISTS stage_runs(job TEXT,attempt TEXT,stage TEXT,started REAL,ended REAL,state TEXT,PRIMARY KEY(job,attempt,stage));
CREATE TABLE IF NOT EXISTS tools(owner TEXT,id TEXT,revision INTEGER,manifest TEXT,sha256 TEXT,PRIMARY KEY(owner,id,revision));
CREATE TABLE IF NOT EXISTS user_profiles(
      id TEXT PRIMARY KEY, provider TEXT NOT NULL, login TEXT NOT NULL,
      display_name TEXT NOT NULL, updated REAL NOT NULL);
CREATE TABLE IF NOT EXISTS workflow_nodes(root TEXT,node TEXT,label TEXT,job TEXT UNIQUE NOT NULL,PRIMARY KEY(root,node));
CREATE UNIQUE INDEX IF NOT EXISTS event_attempt_code ON events(job,attempt,code) WHERE attempt IS NOT NULL;
CREATE INDEX IF NOT EXISTS history_search ON history_records(kind,id,source);
CREATE INDEX IF NOT EXISTS jobs_ready_order ON jobs(state,created);
