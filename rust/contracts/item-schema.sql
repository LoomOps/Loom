-- Common authority is small. File/segment membership lives in sealed manifests.
CREATE TABLE IF NOT EXISTS catalog_meta(id INTEGER PRIMARY KEY CHECK(id=1),origin BLOB NOT NULL,cursor_key BLOB NOT NULL,access_epoch INTEGER NOT NULL DEFAULT 1);
CREATE TABLE IF NOT EXISTS catalog_items(id BLOB PRIMARY KEY CHECK(length(id)=16),kind TEXT NOT NULL,native_kind TEXT NOT NULL,native_key TEXT NOT NULL,current_revision BLOB,created REAL NOT NULL,deleted INTEGER NOT NULL DEFAULT 0,UNIQUE(native_kind,native_key));
CREATE TABLE IF NOT EXISTS catalog_revisions(id BLOB PRIMARY KEY CHECK(length(id)=16),item BLOB NOT NULL,source_key TEXT NOT NULL,native_ref TEXT NOT NULL,body TEXT NOT NULL,UNIQUE(item,source_key));
CREATE TABLE IF NOT EXISTS catalog_aliases(kind TEXT NOT NULL,identity TEXT NOT NULL,item BLOB NOT NULL,revision BLOB,PRIMARY KEY(kind,identity));
CREATE TABLE IF NOT EXISTS catalog_changes(seq INTEGER PRIMARY KEY AUTOINCREMENT,item BLOB NOT NULL,revision BLOB,operation TEXT NOT NULL,body TEXT NOT NULL,created REAL NOT NULL);
CREATE TABLE IF NOT EXISTS catalog_dirty(seq INTEGER PRIMARY KEY AUTOINCREMENT,topic TEXT NOT NULL,identity TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS catalog_backfill(topic TEXT PRIMARY KEY,position INTEGER NOT NULL DEFAULT 0,complete INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS catalog_sources(kind TEXT NOT NULL,identity TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(kind,identity));
CREATE TABLE IF NOT EXISTS catalog_manifests(revision BLOB PRIMARY KEY,item BLOB NOT NULL,descriptor TEXT NOT NULL,sha256 TEXT NOT NULL,entries INTEGER NOT NULL,created REAL NOT NULL);
CREATE INDEX IF NOT EXISTS catalog_revisions_item ON catalog_revisions(item);
CREATE INDEX IF NOT EXISTS catalog_changes_item ON catalog_changes(item,seq);
