-- Spots a computer could not solve (POST /v1/work/fail). That client does not get the spot
-- again for a while; after a few failures (api.ts: MAX_FAILURES) the spot is parked and no
-- longer handed out, so one unsolvable spot cannot keep the community busy. ringside-queue
-- lists the parked spots for the operator.
ALTER TABLE work ADD COLUMN failures INTEGER NOT NULL DEFAULT 0;
ALTER TABLE work ADD COLUMN fail_note TEXT;          -- the last client's reason, shortened
ALTER TABLE work ADD COLUMN parked_at INTEGER;       -- unix seconds; not handed out while set

CREATE TABLE work_failures (
  key TEXT NOT NULL,                   -- work key
  submitter TEXT NOT NULL,             -- salted hash of the client IP
  failed_at INTEGER NOT NULL,          -- unix seconds, the client's last failure
  PRIMARY KEY (key, submitter)
) STRICT, WITHOUT ROWID;
