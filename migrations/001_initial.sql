PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS documents (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  source_url TEXT NOT NULL,
  retrieved_at TEXT NOT NULL,
  plain_text TEXT NOT NULL,
  payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
  title, plain_text, content='documents', content_rowid='rowid', tokenize='unicode61'
);
CREATE TRIGGER IF NOT EXISTS documents_ai AFTER INSERT ON documents BEGIN
  INSERT INTO documents_fts(rowid,title,plain_text) VALUES(new.rowid,new.title,new.plain_text);
END;
CREATE TRIGGER IF NOT EXISTS documents_ad AFTER DELETE ON documents BEGIN
  INSERT INTO documents_fts(documents_fts,rowid,title,plain_text) VALUES('delete',old.rowid,old.title,old.plain_text);
END;
CREATE TRIGGER IF NOT EXISTS documents_au AFTER UPDATE ON documents BEGIN
  INSERT INTO documents_fts(documents_fts,rowid,title,plain_text) VALUES('delete',old.rowid,old.title,old.plain_text);
  INSERT INTO documents_fts(rowid,title,plain_text) VALUES(new.rowid,new.title,new.plain_text);
END;
CREATE TABLE IF NOT EXISTS libraries (
  name TEXT PRIMARY KEY,
  description TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS library_items (
  library TEXT NOT NULL REFERENCES libraries(name) ON DELETE CASCADE,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  added_by TEXT,
  added_at TEXT NOT NULL,
  PRIMARY KEY(library,document_id)
);
CREATE TABLE IF NOT EXISTS fetch_cache (
  cache_key TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  checked_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS annotations (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
  payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE TABLE IF NOT EXISTS jobs (
  id TEXT PRIMARY KEY,
  state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE INDEX IF NOT EXISTS jobs_by_state ON jobs(state,created_at);
CREATE INDEX IF NOT EXISTS documents_by_source ON documents(source_url,retrieved_at);
PRAGMA user_version = 1;
