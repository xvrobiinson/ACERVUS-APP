-- Acervus Finance schema v10: the linked business's role may be any of the
-- 0.16 roles. Version 7 only allowed admin, collector and viewer, so 0.16.0
-- could not store an Owner, Manager or Accountant and the whole cloud copy
-- rolled back. SQLite can't change a CHECK constraint in place, so the table
-- is rebuilt with its row copied unchanged. 'viewer' stays allowed: rows
-- written by 0.15 keep their value (read as Accountant) and a cloud rolled
-- back to 0.15 still sends it.

CREATE TABLE sync_state_v10 (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    business_id      TEXT NOT NULL,
    business_name    TEXT NOT NULL,
    timezone         TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    role             TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'manager', 'accountant', 'collector', 'viewer')),
    -- "Today" in the business's time zone when the copy was made.
    today            TEXT NOT NULL CHECK (today GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    settings_version INTEGER NOT NULL DEFAULT 0,
    -- The business language in the cloud (on this PC the language is the
    -- person's own choice; settings sent back keep the cloud's).
    language         TEXT NOT NULL DEFAULT 'es',
    applied_at       TEXT NOT NULL
) STRICT;

INSERT INTO sync_state_v10 (id, business_id, business_name, timezone, user_id, role, today, settings_version, language, applied_at)
SELECT id, business_id, business_name, timezone, user_id, role, today, settings_version, language, applied_at FROM sync_state;

DROP TABLE sync_state;
ALTER TABLE sync_state_v10 RENAME TO sync_state;
