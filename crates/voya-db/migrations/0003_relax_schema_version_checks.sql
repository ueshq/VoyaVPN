-- Relax the pinned schema-version CHECK constraints.
--
-- 0001 declared `CHECK (version = 1)` on schema_metadata and
-- `CHECK (schema_version = 1)` on app_settings, which made any future version
-- bump require a table rebuild instead of a one-line migration. SQLite cannot
-- alter a CHECK constraint in place, so both single-row tables are rebuilt here.
-- Neither table is referenced by a foreign key, so the rebuild is safe with
-- foreign key enforcement enabled.

CREATE TABLE schema_metadata_relaxed (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    version INTEGER NOT NULL CHECK (version >= 1)
);

INSERT INTO schema_metadata_relaxed (id, version)
SELECT id, version FROM schema_metadata;

DROP TABLE schema_metadata;

ALTER TABLE schema_metadata_relaxed RENAME TO schema_metadata;

CREATE TABLE app_settings_relaxed (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
);

INSERT INTO app_settings_relaxed (id, schema_version, payload)
SELECT id, schema_version, payload FROM app_settings;

DROP TABLE app_settings;

ALTER TABLE app_settings_relaxed RENAME TO app_settings;
