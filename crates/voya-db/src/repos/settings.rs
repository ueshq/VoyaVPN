use sqlx::SqliteConnection;
use voya_contracts::AppSettings;

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
    pub async fn load(&self) -> Result<AppSettings> {
        Ok(self.load_stored().await?.unwrap_or_default())
    }

    /// The stored settings, or `None` on a database that never saved any.
    pub async fn load_stored(&self) -> Result<Option<AppSettings>> {
        let payload = run_query!(
            self.executor,
            sqlx::query_scalar::<_, String>("SELECT payload FROM app_settings WHERE id = 1"),
            fetch_optional
        )?;
        payload
            .map(|payload| serde_json::from_str(&payload).map_err(payload_error))
            .transpose()
    }

    pub async fn save(&self, settings: &AppSettings) -> Result<()> {
        let payload = settings_payload(settings)?;
        run_query!(self.executor, settings_upsert_query(&payload), execute)?;
        Ok(())
    }

    pub async fn save_with_state(
        &self,
        settings: &AppSettings,
        state: &AppStateRecord,
    ) -> Result<()> {
        let payload = settings_payload(settings)?;
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

fn settings_payload(settings: &AppSettings) -> Result<String> {
    serde_json::to_string(settings).map_err(payload_error)
}

fn settings_upsert_query(
    payload: &str,
) -> sqlx::query::Query<'_, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    sqlx::query(
        r#"
            INSERT INTO app_settings (id, payload)
            VALUES (1, ?)
            ON CONFLICT(id) DO UPDATE SET payload = excluded.payload
            "#,
    )
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
