use voya_contracts::{SelfHostRecordV1, CURRENT_SCHEMA_VERSION};

use crate::{
    executor::{repository_constructors, run_query, RepositoryExecutor},
    DbError, Result,
};

/// Stores the self-hosted node — its settings and minted credentials — as one
/// strict JSON row. A database that never enabled the node has no row, which
/// reads as the default record.
#[derive(Debug, Clone, Copy)]
pub struct SelfHostRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(SelfHostRepository);

impl<'executor> SelfHostRepository<'executor> {
    pub async fn load(&self) -> Result<SelfHostRecordV1> {
        let row = run_query!(
            self.executor,
            sqlx::query_as::<_, (i64, String)>(
                "SELECT schema_version, payload FROM self_host WHERE id = 1",
            ),
            fetch_optional
        )?;
        let Some((version, payload)) = row else {
            return Ok(SelfHostRecordV1::default());
        };
        if version != i64::from(CURRENT_SCHEMA_VERSION) {
            return Err(schema_mismatch("self_host", version));
        }
        let record: SelfHostRecordV1 = serde_json::from_str(&payload).map_err(payload_error)?;
        if record.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(schema_mismatch(
                "self_host.payload",
                i64::from(record.schema_version),
            ));
        }
        Ok(record)
    }

    pub async fn save(&self, record: &SelfHostRecordV1) -> Result<()> {
        if record.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(schema_mismatch(
                "self_host.payload",
                i64::from(record.schema_version),
            ));
        }
        let payload = serde_json::to_string(record).map_err(payload_error)?;
        run_query!(
            self.executor,
            sqlx::query(
                r#"
                    INSERT INTO self_host (id, schema_version, payload)
                    VALUES (1, ?, ?)
                    ON CONFLICT(id) DO UPDATE SET
                        schema_version = excluded.schema_version,
                        payload = excluded.payload
                    "#,
            )
            .bind(i64::from(CURRENT_SCHEMA_VERSION))
            .bind(payload.as_str()),
            execute
        )?;
        Ok(())
    }
}

fn payload_error(source: serde_json::Error) -> DbError {
    DbError::Json {
        path: "self_host.payload".into(),
        source,
    }
}

fn schema_mismatch(path: &str, found: i64) -> DbError {
    DbError::UnsupportedDatabaseSchema {
        path: path.into(),
        found: Some(found),
        expected: i64::from(CURRENT_SCHEMA_VERSION),
        manual_reset_command:
            "remove the Voya database file reported at startup, then restart VoyaVPN".to_string(),
    }
}
