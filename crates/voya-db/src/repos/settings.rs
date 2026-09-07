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
/// compatible (add fields with `#[serde(default)]`, never remove or rename) or
/// bump `CURRENT_SCHEMA_VERSION` with a migration. The
/// `persisted_settings_payload_from_an_earlier_build_still_loads` test pins the
/// current layout against a checked-in fixture so an accidental break is caught
/// here rather than on a user's machine.
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
        let settings =
            serde_json::from_str::<AppSettingsV1>(&payload).map_err(|source| DbError::Json {
                path: "app_settings.payload".into(),
                source,
            })?;
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
