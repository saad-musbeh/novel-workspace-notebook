ALTER TABLE manuscript_scene ADD COLUMN deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN(0,1));
CREATE TABLE asset_transfer (id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project(id),asset_id TEXT NOT NULL,path TEXT NOT NULL,mime_type TEXT NOT NULL,total_size INTEGER NOT NULL CHECK(total_size>=0 AND total_size<=16777216),checksum TEXT NOT NULL,content BLOB NOT NULL DEFAULT X'',UNIQUE(project_id,asset_id));
CREATE TABLE sync_receipt (remote_epoch TEXT NOT NULL,revision_id TEXT NOT NULL REFERENCES revision_envelope(id),PRIMARY KEY(remote_epoch,revision_id));
PRAGMA user_version=3;
CREATE TABLE completed_asset_transfer (id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES project(id),asset_id TEXT NOT NULL);
