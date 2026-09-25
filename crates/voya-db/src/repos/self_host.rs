use voya_contracts::SelfHostRecord;

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
    pub async fn load(&self) -> Result<SelfHostRecord> {
        let payload = run_query!(
            self.executor,
            sqlx::query_scalar::<_, String>("SELECT payload FROM self_host WHERE id = 1"),
            fetch_optional
        )?;
        match payload {
            Some(payload) => serde_json::from_str(&payload).map_err(payload_error),
            None => Ok(SelfHostRecord::default()),
        }
    }

    pub async fn save(&self, record: &SelfHostRecord) -> Result<()> {
        let payload = serde_json::to_string(record).map_err(payload_error)?;
        run_query!(
            self.executor,
            sqlx::query(
                r#"
                    INSERT INTO self_host (id, payload)
                    VALUES (1, ?)
                    ON CONFLICT(id) DO UPDATE SET payload = excluded.payload
                    "#,
            )
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
