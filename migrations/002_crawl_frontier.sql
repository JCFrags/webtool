CREATE TABLE IF NOT EXISTS crawl_runs (
  job_id TEXT PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
  sitemaps_seeded INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS crawl_frontier (
  job_id TEXT NOT NULL REFERENCES crawl_runs(job_id) ON DELETE CASCADE,
  ordinal INTEGER NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('page','sitemap')),
  url TEXT NOT NULL,
  depth INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('pending','active','saved','failed','excluded','interrupted')),
  attempts INTEGER NOT NULL DEFAULT 0,
  interruptions INTEGER NOT NULL DEFAULT 0,
  document_id TEXT REFERENCES documents(id),
  reason TEXT,
  PRIMARY KEY(job_id,kind,url),
  UNIQUE(job_id,ordinal)
);
CREATE INDEX IF NOT EXISTS crawl_frontier_state ON crawl_frontier(job_id,kind,state,depth,ordinal);
PRAGMA user_version = 2;
