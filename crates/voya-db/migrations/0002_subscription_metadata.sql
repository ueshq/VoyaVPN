ALTER TABLE subscriptions ADD COLUMN auto_update_interval_minutes INTEGER;

CREATE TABLE subscription_metadata (
    subscription_id TEXT PRIMARY KEY NOT NULL,
    upload_bytes INTEGER,
    download_bytes INTEGER,
    total_bytes INTEGER,
    expire_at INTEGER,
    last_update_at INTEGER,
    profile_title TEXT,
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(id) ON DELETE CASCADE
);
