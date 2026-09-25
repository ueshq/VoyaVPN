use sqlx::SqliteConnection;
use voya_contracts::{AppSettingsV1, CURRENT_SCHEMA_VERSION};

use crate::{
    executor::{repository_constructors, run_query, with_connection, RepositoryExecutor},
    AppStateRecord, DbError, Result,
};

/// Stores the current IPC settings DTO verbatim. Database initialization rejects
/// historical baselines, and this repository strictly reads the current payload:
/// nothing converts retired keys. Missing settings use the current defaults.
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
