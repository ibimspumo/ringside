-- Key/value settings of the service; db/migrate.ts keeps the schema version here.
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT;

-- One row per solution key. A row is served only once a trusted verifier has set it to
-- 'verified'; a 'rejected' row may be replaced by a new submission.
CREATE TABLE solutions (
  key TEXT PRIMARY KEY,                -- sha256 hex of kind, engine version and canonical input
  kind TEXT NOT NULL,
  engine_version INTEGER NOT NULL,
  input TEXT NOT NULL,                 -- canonical JSON the key was computed from
  result BLOB NOT NULL,                -- gzip of the result JSON
  result_size INTEGER NOT NULL,        -- uncompressed bytes
  status TEXT NOT NULL CHECK (status IN ('submitted', 'verified', 'rejected')),
  exploitability REAL,                 -- measured by the verifier, in the kind's unit
  submitter TEXT NOT NULL,             -- salted hash of the client IP
  submitted_at INTEGER NOT NULL,       -- unix seconds
  checked_at INTEGER,
  check_note TEXT
) STRICT;

CREATE INDEX solutions_pending ON solutions (status, submitted_at);

-- Every submission attempt that passed validation, including duplicates; no result bodies.
CREATE TABLE submissions (
  id INTEGER PRIMARY KEY,
  key TEXT NOT NULL,
  kind TEXT NOT NULL,
  submitter TEXT NOT NULL,
  received_at INTEGER NOT NULL,
  size INTEGER NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('new', 'replaced', 'pending', 'verified'))
) STRICT;

CREATE INDEX submissions_key ON submissions (key);
CREATE INDEX submissions_received ON submissions (received_at);

-- Fixed-window request counters; bucket is "<rule>:<salted client hash>" or "<rule>:global".
CREATE TABLE rate_limits (
  bucket TEXT NOT NULL,
  window_start INTEGER NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (bucket, window_start)
) STRICT, WITHOUT ROWID;

CREATE INDEX rate_limits_window ON rate_limits (window_start);

-- Parser failure reports: one hand the client anonymized before sending, never other data.
CREATE TABLE reports (
  id INTEGER PRIMARY KEY,
  received_at INTEGER NOT NULL,
  app_version TEXT NOT NULL,
  error TEXT NOT NULL,
  hand TEXT NOT NULL,
  submitter TEXT NOT NULL
) STRICT;

CREATE INDEX reports_received ON reports (received_at);
