-- Solutions are served as soon as their form checks out ('accepted'); nobody can re-solve
-- every one of them. The verifier re-solves a sample and marks those 'verified'; a client
-- whose sample fails is banned and its accepted solutions are set to 'rejected'.
-- 'submitted' is left for rows from before this change until the verifier has checked
-- their form. SQLite cannot change a CHECK constraint in place, so the table is rebuilt.
CREATE TABLE solutions_new (
  key TEXT PRIMARY KEY,                -- sha256 hex of kind, engine version and canonical input
  kind TEXT NOT NULL,
  engine_version INTEGER NOT NULL,
  input TEXT NOT NULL,                 -- canonical JSON the key was computed from
  result BLOB NOT NULL,                -- gzip of the result JSON
  result_size INTEGER NOT NULL,        -- uncompressed bytes
  status TEXT NOT NULL CHECK (status IN ('submitted', 'accepted', 'verified', 'rejected')),
  exploitability REAL,                 -- measured by the verifier, else what the stored spot reports
  submitter TEXT NOT NULL,             -- salted hash of the client IP
  submitted_at INTEGER NOT NULL,       -- unix seconds
  checked_at INTEGER,                  -- last time the verifier looked at the row
  check_note TEXT
) STRICT;

INSERT INTO solutions_new
  SELECT key, kind, engine_version, input, result, result_size, status, exploitability, submitter, submitted_at,
    checked_at, check_note
  FROM solutions;
DROP TABLE solutions;
ALTER TABLE solutions_new RENAME TO solutions;

CREATE INDEX solutions_pending ON solutions (status, submitted_at);
CREATE INDEX solutions_submitter ON solutions (submitter, status);

-- Clients whose sample failed: the API refuses their submissions.
CREATE TABLE banned_clients (
  submitter TEXT PRIMARY KEY,          -- salted hash of the client IP
  banned_at INTEGER NOT NULL,          -- unix seconds
  reason TEXT NOT NULL
) STRICT, WITHOUT ROWID;
