use voya_contracts::{AppSettingsV1, CURRENT_SCHEMA_VERSION};

use crate::{
    executor::{repository_constructors, run_query, RepositoryExecutor},
    AppStateRecord, DbError, Result,
};

/// Stores the IPC settings DTO verbatim as the on-disk settings payload.
///
/// `voya_contracts::AppSettingsV1` is therefore two contracts at once: the type
/// tauri-specta exports to `bindings.ts`, and the JSON layout of every installed
/// database's `app_settings.payload`. Because the contract and its nested
/// structs carry `deny_unknown_fields`, a field edited for UI reasons is a
/// storage-format change — removing or renaming one makes [`Self::load`] fail
/// for every existing install, which `AppServices::load_config` propagates into
/// `setup()` and turns into a launch failure.
///
/// Any change to `AppSettingsV1` must therefore either stay backwards
/// compatible (add fields with `#[serde(default)]`) or carry the removed or
/// renamed key in [`normalize_retired_keys`], which cleans up the stored JSON
/// before serde sees it.
/// The `persisted_settings_payload_from_an_earlier_build_still_loads` and
/// `settings_payload_with_retired_keys_still_loads` tests pin both halves
/// against checked-in fixtures, so an accidental break is caught here rather
/// than on a user's machine.
#[derive(Debug, Clone, Copy)]
pub struct SettingsRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(SettingsRepository);

impl<'executor> SettingsRepository<'executor> {
    pub async fn load(&self) -> Result<AppSettingsV1> {
        let row = run_query!(
            self.executor,
            sqlx::query_as::<_, (i64, String)>(
                "SELECT schema_version, payload FROM app_settings WHERE id = 1",
            ),
            fetch_optional
        )?;
        let Some((version, payload)) = row else {
            return Ok(AppSettingsV1::default());
        };
        if version != i64::from(CURRENT_SCHEMA_VERSION) {
            return Err(DbError::UnsupportedDatabaseSchema {
                path: "app_settings".into(),
                found: Some(version),
                expected: i64::from(CURRENT_SCHEMA_VERSION),
                manual_reset_command: settings_reset_command(),
            });
        }
        let settings = deserialize_payload(&payload)?;
        if settings.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(DbError::UnsupportedDatabaseSchema {
                path: "app_settings.payload".into(),
                found: Some(i64::from(settings.schema_version)),
                expected: i64::from(CURRENT_SCHEMA_VERSION),
                manual_reset_command: settings_reset_command(),
            });
        }
        Ok(settings)
    }

    pub async fn save(&self, settings: &AppSettingsV1) -> Result<()> {
        let payload = validated_payload(settings)?;
        save_settings(self.executor, &payload).await?;
        Ok(())
    }

    pub async fn save_with_state(
        &self,
        settings: &AppSettingsV1,
        state: &AppStateRecord,
    ) -> Result<()> {
        let payload = validated_payload(settings)?;
        match self.executor {
            RepositoryExecutor::Pool(pool) => {
                let mut transaction = pool.begin().await?;
                save_settings_on(&mut *transaction, &payload).await?;
                save_state_on(&mut *transaction, state).await?;
                transaction.commit().await?;
                Ok(())
            }
            RepositoryExecutor::Transaction(transaction) => {
                let mut transaction = transaction.lock().await;
                save_settings_on(&mut **transaction, &payload).await?;
                save_state_on(&mut **transaction, state).await
            }
        }
    }
}

/// Keys `AppSettingsV1::dns` used to have and no longer does.
///
/// sing-box 1.13 cannot express any of them (`hosts` falls back to
/// `/etc/hosts` even for an explicit empty list, and `option/dns.go` has no
/// stale-serving or parallel-query knob), so the three settings were dropped
/// rather than wired.
const RETIRED_DNS_KEYS: [&str; 3] = ["useSystemHosts", "serveStale", "parallelQuery"];

/// Keys `AppSettingsV1::speed_test` renamed, as `(stored, current)`.
///
/// `delayIntervalMs` always held seconds; only the name was wrong.
const RENAMED_SPEEDTEST_KEYS: [(&str, &str); 2] = [
    ("delayIntervalMs", "delayIntervalSeconds"),
    ("mixedConcurrency", "proxyDelayConcurrency"),
];

const RETIRED_SPEEDTEST_KEYS: [&str; 2] = ["downloadUrl", "udpTarget"];

/// Reads a stored payload into the current contract.
///
/// `AppSettingsV1` and every struct nested in it deny unknown fields, so a row
/// written before a field was removed or renamed would fail to deserialize and
/// take `setup()` down with it. Normalizing the stored JSON first — dropping
/// the retired keys, moving the renamed ones onto their current name and their
/// stored value — keeps those rows loading while leaving `deny_unknown_fields`
/// strict about keys that were never part of the contract. The first save after
/// an upgrade rewrites the row in the current shape.
fn deserialize_payload(payload: &str) -> Result<AppSettingsV1> {
    let mut value = serde_json::from_str::<serde_json::Value>(payload).map_err(payload_error)?;
    normalize_retired_keys(&mut value);
    serde_json::from_value(value).map_err(payload_error)
}

fn payload_error(source: serde_json::Error) -> DbError {
    DbError::Json {
        path: "app_settings.payload".into(),
        source,
    }
}

fn normalize_retired_keys(value: &mut serde_json::Value) {
    // Retired top-level settings are discarded only at the persistence boundary.
    if let Some(settings) = value.as_object_mut() {
        settings.remove("sources");
        settings.remove("shortcuts");
    }

    if let Some(dns) = value
        .get_mut("dns")
        .and_then(serde_json::Value::as_object_mut)
    {
        for key in RETIRED_DNS_KEYS {
            dns.remove(key);
        }
    }

    if let Some(speedtest) = value
        .get_mut("speedTest")
        .and_then(serde_json::Value::as_object_mut)
    {
        for key in RETIRED_SPEEDTEST_KEYS {
            speedtest.remove(key);
        }
        for (stored, current) in RENAMED_SPEEDTEST_KEYS {
            if let Some(stored_value) = speedtest.remove(stored) {
                // A payload already carrying the current key wins: only a row
                // that predates the rename should be filled in from the old one.
                speedtest.entry(current).or_insert(stored_value);
            }
        }
    }
}

fn validated_payload(settings: &AppSettingsV1) -> Result<String> {
    if settings.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(DbError::UnsupportedDatabaseSchema {
            path: "app_settings.payload".into(),
            found: Some(i64::from(settings.schema_version)),
            expected: i64::from(CURRENT_SCHEMA_VERSION),
            manual_reset_command: settings_reset_command(),
        });
    }
    let payload = serde_json::to_string(settings).map_err(|source| DbError::Json {
        path: "app_settings.payload".into(),
        source,
    })?;
    Ok(payload)
}

fn settings_reset_command() -> String {
    "remove the Voya database file reported at startup, then restart VoyaVPN".to_string()
}

async fn save_settings(executor: RepositoryExecutor<'_>, payload: &str) -> Result<()> {
    run_query!(executor, settings_upsert_query(payload), execute)?;
    Ok(())
}

fn settings_upsert_query(
    payload: &str,
) -> sqlx::query::Query<'_, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    sqlx::query(
        r#"
            INSERT INTO app_settings (id, schema_version, payload)
            VALUES (1, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                schema_version = excluded.schema_version,
                payload = excluded.payload
            "#,
    )
    .bind(i64::from(CURRENT_SCHEMA_VERSION))
    .bind(payload)
}

async fn save_settings_on<'executor, E>(executor: E, payload: &str) -> Result<()>
where
    E: sqlx::Executor<'executor, Database = sqlx::Sqlite>,
{
    settings_upsert_query(payload).execute(executor).await?;
    Ok(())
}

async fn save_state_on<'executor, E>(executor: E, state: &AppStateRecord) -> Result<()>
where
    E: sqlx::Executor<'executor, Database = sqlx::Sqlite>,
{
    sqlx::query("UPDATE app_state SET active_profile_id = ?, active_routing_id = ? WHERE id = 1")
        .bind(state.active_profile_id.as_deref())
        .bind(state.active_routing_id.as_deref())
        .execute(executor)
        .await?;
    Ok(())
}
