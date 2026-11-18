-- The "Mitrechnen" queue: spots without a solution that computers of the community solve.
-- Sources: catalog spots and push/fold gaps the operator seeds (ringside-queue), and
-- spots clients asked for (POST /v1/work/request). A row is handed out with a lease and
-- handed out again once the lease has run out; it is skipped while its key has a
-- submitted or verified solution, and the verifier deletes it once that is verified.
CREATE TABLE work (
  key TEXT PRIMARY KEY,                -- solution key of kind, engine version and input
  kind TEXT NOT NULL,
  engine_version INTEGER NOT NULL,
  input TEXT NOT NULL,                 -- canonical JSON
  source TEXT NOT NULL CHECK (source IN ('catalog', 'preflop', 'request')),
  priority INTEGER NOT NULL,           -- higher first; each client request adds one
  created_at INTEGER NOT NULL,         -- unix seconds
  lease_until INTEGER,                 -- unix seconds; free again afterwards
  leased_to TEXT,                      -- salted hash of the client IP
  leases INTEGER NOT NULL DEFAULT 0    -- how often it was handed out
) STRICT;

CREATE INDEX work_open ON work (kind, priority DESC, leases, created_at);
CREATE INDEX work_leased_to ON work (leased_to, lease_until);
