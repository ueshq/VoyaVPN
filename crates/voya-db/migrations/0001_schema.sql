-- The VoyaVPN database schema: one baseline, no upgrade path. Startup accepts
-- an empty database or one whose single migration record carries this file's
-- version and checksum; anything else is refused and offered a reset.
--
-- Editing this file therefore ALWAYS means renaming its version prefix. An
-- in-place edit keeps the version and changes the checksum, so every existing
-- database is refused at startup. Once 1.0 ships this baseline is frozen and
-- later schema changes become incremental migrations on top of it (ADR 0001).
--
-- `created_at` columns are set by SQLite on insert and never written by an
-- upsert, so they record when a row first appeared.

CREATE TABLE app_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    payload TEXT NOT NULL
);

CREATE TABLE app_state (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    active_profile_id TEXT,
    active_routing_id TEXT,
    active_group_id TEXT,
    FOREIGN KEY (active_profile_id) REFERENCES profile_items(index_id) ON DELETE SET NULL,
    FOREIGN KEY (active_routing_id) REFERENCES routing_items(id) ON DELETE SET NULL,
    FOREIGN KEY (active_group_id) REFERENCES policy_groups(id) ON DELETE SET NULL,
    CHECK (active_profile_id IS NULL OR active_group_id IS NULL)
);

CREATE TABLE policy_group_members (
    group_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (group_id, profile_id),
    UNIQUE (group_id, position),
    FOREIGN KEY (group_id) REFERENCES policy_groups(id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id) REFERENCES profile_items(index_id) ON DELETE CASCADE
);

CREATE TABLE policy_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    strategy TEXT NOT NULL CHECK (strategy IN ('selector', 'urltest', 'fallback')),
    source_subscription_id TEXT,
    auto_created INTEGER NOT NULL DEFAULT 0 CHECK (auto_created IN (0, 1)),
    selected_profile_id TEXT,
    test_url TEXT,
    interval_seconds INTEGER,
    tolerance_ms INTEGER,
    sort INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (source_subscription_id) REFERENCES subscriptions(id) ON DELETE SET NULL,
    FOREIGN KEY (selected_profile_id) REFERENCES profile_items(index_id) ON DELETE SET NULL
);

-- What was last measured through a node. A missing row means "not measured";
-- the node's list position lives on `profile_items`.
CREATE TABLE profile_ex_items (
    index_id TEXT PRIMARY KEY NOT NULL,
    delay INTEGER NOT NULL DEFAULT 0,
    message TEXT,
    ip_info TEXT,
    country_code TEXT,
    FOREIGN KEY (index_id) REFERENCES profile_items(index_id) ON DELETE CASCADE
);

-- `config_type` is derived from the protocol blob's tag, so the two cannot
-- disagree, and a protocol that is not JSON with a `kind` cannot be stored.
CREATE TABLE profile_items (
    index_id TEXT PRIMARY KEY NOT NULL,
    config_type TEXT GENERATED ALWAYS AS (json_extract(protocol, '$.kind')) STORED NOT NULL,
    subscription_id TEXT,
    display_log INTEGER NOT NULL DEFAULT 1 CHECK (display_log IN (0, 1)),
    remarks TEXT NOT NULL DEFAULT '',
    protocol TEXT NOT NULL,
    transport TEXT,
    tls TEXT,
    sort INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
);

CREATE TABLE routing_items (
    id TEXT PRIMARY KEY NOT NULL,
    remarks TEXT NOT NULL DEFAULT '',
    rule_set TEXT NOT NULL DEFAULT '[]',
    sort INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE self_host (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    payload TEXT NOT NULL
);

-- `day_number` is the local calendar day the `today_*` counters belong to.
CREATE TABLE server_stat_items (
    index_id TEXT PRIMARY KEY NOT NULL,
    total_up INTEGER NOT NULL DEFAULT 0,
    total_down INTEGER NOT NULL DEFAULT 0,
    today_up INTEGER NOT NULL DEFAULT 0,
    today_down INTEGER NOT NULL DEFAULT 0,
    day_number INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (index_id) REFERENCES profile_items(index_id) ON DELETE CASCADE
);

CREATE TABLE subscription_metadata (
    subscription_id TEXT PRIMARY KEY NOT NULL,
    upload_bytes INTEGER,
    download_bytes INTEGER,
    total_bytes INTEGER,
    expire_at INTEGER,
    last_update_at INTEGER,
    last_attempt_at INTEGER,
    last_attempt_failed INTEGER CHECK (last_attempt_failed IS NULL OR last_attempt_failed IN (0, 1)),
    last_attempt_error TEXT,
    profile_title TEXT,
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
);

-- `url` is deliberately not unique: two subscriptions may fetch the same URL
-- with different filters.
CREATE TABLE subscriptions (
    id TEXT PRIMARY KEY NOT NULL,
    remarks TEXT NOT NULL DEFAULT '',
    url TEXT NOT NULL DEFAULT '',
    more_url TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    user_agent TEXT NOT NULL DEFAULT '',
    sort INTEGER NOT NULL DEFAULT 0,
    filter TEXT,
    convert_target TEXT,
    auto_update_interval_minutes INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Only the tables that grow with the user's node list are indexed beyond
-- their keys; the others hold a handful of rows that a scan reads faster.
CREATE INDEX idx_policy_group_members_profile ON policy_group_members (profile_id);

CREATE INDEX idx_profile_items_sort ON profile_items (sort, index_id);

CREATE INDEX idx_profile_items_subscription_id ON profile_items (subscription_id);

-- The only place measurements are cleared: a node whose connection changed is
-- not the node they were taken on.
CREATE TRIGGER clear_country_on_connection_change
AFTER UPDATE OF protocol, transport, tls ON profile_items
WHEN OLD.protocol IS NOT NEW.protocol OR OLD.transport IS NOT NEW.transport OR OLD.tls IS NOT NEW.tls
BEGIN
    UPDATE profile_ex_items SET country_code = NULL, delay = 0, message = NULL, ip_info = NULL WHERE index_id = NEW.index_id;
END;

INSERT INTO app_state (id, active_profile_id, active_routing_id, active_group_id) VALUES (1, NULL, NULL, NULL);
