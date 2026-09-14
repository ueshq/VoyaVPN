use sqlx::SqliteConnection;
use voya_contracts::{AppSettingsV1, CURRENT_SCHEMA_VERSION};

use crate::{
    executor::{repository_constructors, run_query, with_connection, RepositoryExecutor},
    AppStateRecord, DbError, Result,
};

/// Stores the current IPC settings DTO verbatim. Database initialization rejects
/// historical baselines; this repository strictly reads the current payload and
/// never converts retired keys. Initialization only normalizes the documented
/// retired keys and values (see [`normalize_retired_settings_value`]) in
/// otherwise valid current settings. Missing settings use the current defaults.
#[derive(Debug, Clone, Copy)]
pub struct SettingsRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(SettingsRepository);

impl<'executor> SettingsRepository<'executor> {
    pub async fn load(&self) -> Result<AppSettingsV1> {
        Ok(self.load_stored().await?.unwrap_or_default())
    }

    /// The stored settings, or `None` on a database that never saved any.
    pub async fn load_stored(&self) -> Result<Option<AppSettingsV1>> {
        let row = run_query!(
            self.executor,
            sqlx::query_as::<_, (i64, String)>(
                "SELECT schema_version, payload FROM app_settings WHERE id = 1",
            ),
            fetch_optional
        )?;
        let Some((version, payload)) = row else {
            return Ok(None);
        };
        if version != i64::from(CURRENT_SCHEMA_VERSION) {
            return Err(schema_mismatch("app_settings", version));
        }
        let settings: AppSettingsV1 = serde_json::from_str(&payload).map_err(payload_error)?;
        if settings.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(schema_mismatch(
                "app_settings.payload",
                i64::from(settings.schema_version),
            ));
        }
        Ok(Some(settings))
    }

    pub async fn save(&self, settings: &AppSettingsV1) -> Result<()> {
        let payload = validated_payload(settings)?;
        run_query!(self.executor, settings_upsert_query(&payload), execute)?;
        Ok(())
    }

    pub async fn save_with_state(
        &self,
        settings: &AppSettingsV1,
        state: &AppStateRecord,
    ) -> Result<()> {
        let payload = validated_payload(settings)?;
        with_connection(
            self.executor,
            &(payload.as_str(), state),
            |connection, (payload, state)| {
                Box::pin(async move {
                    settings_upsert_query(payload)
                        .execute(&mut *connection)
                        .await?;
                    save_state_on(connection, state).await
                })
            },
        )
        .await
    }
}

fn payload_error(source: serde_json::Error) -> DbError {
    DbError::Json {
        path: "app_settings.payload".into(),
        source,
    }
}

/// Keys retired from the settings payload, as JSON object paths.
const RETIRED_SETTINGS_KEYS: &[&[&str]] = &[
    &["behavior", "statistics"],
    &["behavior", "realtimeSpeed"],
    &["network", "systemProxy", "advancedProtocol"],
    &["network", "systemProxy", "customPacPath"],
    &["network", "systemProxy", "customScriptPath"],
    &["routing", "singboxDomainStrategy"],
    &["grpc"],
];

/// Keys retired from every element of a settings array: (array path, key).
const RETIRED_SETTINGS_ELEMENT_KEYS: &[(&[&str], &str)] = &[(&["network", "inbounds"], "protocol")];

/// Keys added after the baseline: (parent path, key, JSON value an older
/// payload implies). Inserted only when absent.
const ADDED_SETTINGS_DEFAULTS: &[(&[&str], &str, &str)] = &[
    (&["behavior"], "autoCheckIp", "false"),
    (&["behavior"], "autoCreateSubscriptionGroup", "true"),
    (&["behavior"], "closeAction", "\"minimizeToTray\""),
    (&["behavior"], "startMinimized", "false"),
];

/// Retired enum values and the current value each one maps onto.
const RETIRED_SETTINGS_VALUES: &[(&[&str], &str, &str)] = &[
    (&["proxy", "trafficMode"], "direct", "rule"),
    (&["network", "systemProxy", "mode"], "pac", "forcedChange"),
];

/// Runs only after the database baseline has been validated. Keep the narrow
/// upgrade at the persistence boundary so IPC never accepts retired settings.
pub(crate) async fn normalize_retired_settings(pool: &sqlx::SqlitePool) -> Result<()> {
    let candidate = sqlx::query_as::<_, (String,)>(
        "SELECT payload FROM app_settings WHERE id = 1 AND schema_version = ?",
    )
    .bind(i64::from(CURRENT_SCHEMA_VERSION))
    .fetch_optional(pool)
    .await?;
    let Some((original,)) = candidate else {
        return Ok(());
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&original) else {
        return Ok(());
    };
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        != Some(u64::from(CURRENT_SCHEMA_VERSION))
        || !normalize_retired_settings_value(&mut value)
    {
        return Ok(());
    }
    // Leave malformed or otherwise unknown settings untouched for the strict
    // loader to reject: the conversion must never silently discard fields it
    // does not know about.
    let Ok(settings) = serde_json::from_value::<AppSettingsV1>(value) else {
        return Ok(());
    };
    let normalized = serde_json::to_string(&settings).map_err(payload_error)?;
    sqlx::query(
        "UPDATE app_settings SET payload = ? WHERE id = 1 AND schema_version = ? AND payload = ?",
    )
    .bind(normalized)
    .bind(i64::from(CURRENT_SCHEMA_VERSION))
    .bind(original)
    .execute(pool)
    .await?;
    Ok(())
}

/// Applies every retired-key and retired-value conversion to a settings
/// payload. Returns whether anything changed.
pub(crate) fn normalize_retired_settings_value(value: &mut serde_json::Value) -> bool {
    let mut changed = false;
    for path in RETIRED_SETTINGS_KEYS {
        if let Some((key, parents)) = path.split_last() {
            if let Some(object) = json_at(value, parents).and_then(serde_json::Value::as_object_mut)
            {
                changed |= object.remove(*key).is_some();
            }
        }
    }
    for (array_path, key) in RETIRED_SETTINGS_ELEMENT_KEYS {
        if let Some(elements) = json_at(value, array_path).and_then(serde_json::Value::as_array_mut)
        {
            for element in elements {
                if let Some(object) = element.as_object_mut() {
                    changed |= object.remove(*key).is_some();
                }
            }
        }
    }
    // The boolean fragment switch became a mode: enabled meant TLS record
    // fragmentation, which is what the generator emitted for it.
    if let Some(core) = json_at(value, &["core"]).and_then(serde_json::Value::as_object_mut) {
        if let Some(enabled) = core.remove("fragmentEnabled") {
            let mode = if enabled.as_bool() == Some(true) {
                "record"
            } else {
                "off"
            };
            core.entry("tlsFragment")
                .or_insert_with(|| serde_json::Value::String(mode.to_string()));
            core.entry("fragmentFallbackDelayMs")
                .or_insert_with(|| serde_json::Value::from(500));
            changed = true;
        }
    }
    for (parents, key, default) in ADDED_SETTINGS_DEFAULTS {
        if let Some(object) = json_at(value, parents).and_then(serde_json::Value::as_object_mut) {
            if !object.contains_key(*key) {
                if let Ok(default) = serde_json::from_str::<serde_json::Value>(default) {
                    object.insert((*key).to_string(), default);
                    changed = true;
                }
            }
        }
    }
    for (path, retired, current) in RETIRED_SETTINGS_VALUES {
        if let Some(slot) = json_at(value, path) {
            if slot.as_str() == Some(retired) {
                *slot = serde_json::Value::String((*current).to_string());
                changed = true;
            }
        }
    }
    changed
}

fn json_at<'value>(
    value: &'value mut serde_json::Value,
    path: &[&str],
) -> Option<&'value mut serde_json::Value> {
    path.iter()
        .try_fold(value, |current, key| current.get_mut(*key))
}

fn validated_payload(settings: &AppSettingsV1) -> Result<String> {
    if settings.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(schema_mismatch(
            "app_settings.payload",
            i64::from(settings.schema_version),
        ));
    }
    serde_json::to_string(settings).map_err(payload_error)
}

/// A stored or submitted settings version this build does not read.
fn schema_mismatch(path: &str, found: i64) -> DbError {
    DbError::UnsupportedDatabaseSchema {
        path: path.into(),
        found: Some(found),
        expected: i64::from(CURRENT_SCHEMA_VERSION),
        manual_reset_command:
            "remove the Voya database file reported at startup, then restart VoyaVPN".to_string(),
    }
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

async fn save_state_on(connection: &mut SqliteConnection, state: &AppStateRecord) -> Result<()> {
    sqlx::query("UPDATE app_state SET active_profile_id = ?, active_routing_id = ?, active_group_id = ? WHERE id = 1")
        .bind(state.active_profile_id.as_deref())
        .bind(state.active_routing_id.as_deref())
        .bind(state.active_group_id.as_deref())
        .execute(connection)
        .await?;
    Ok(())
}
