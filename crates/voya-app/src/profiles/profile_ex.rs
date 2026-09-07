use voya_core::ProfileExItem;
use voya_db::{Database, DatabaseSession, UnitOfWork};

use super::Result;

#[derive(Debug, Clone, Copy)]
pub struct ProfileExManager<'db> {
    database: DatabaseSession<'db>,
}

impl<'db> ProfileExManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
        Self::from_session(DatabaseSession::from_database(database))
    }

    #[must_use]
    pub fn new_in(unit_of_work: &'db UnitOfWork) -> Self {
        Self::from_session(DatabaseSession::from_unit_of_work(unit_of_work))
    }

    #[must_use]
    pub(crate) const fn from_session(database: DatabaseSession<'db>) -> Self {
        Self { database }
    }

    pub async fn init(&self) -> Result<u64> {
        Ok(self.database.profile_exs().delete_orphans().await?)
    }

    pub async fn ensure(&self, index_id: &str) -> Result<ProfileExItem> {
        Ok(self.database.profile_exs().ensure(index_id).await?)
    }

    pub async fn list(&self) -> Result<Vec<ProfileExItem>> {
        Ok(self.database.profile_exs().list().await?)
    }

    pub async fn get_max_sort(&self) -> Result<i32> {
        Ok(self.database.profile_exs().max_sort().await?)
    }

    pub async fn set_sort(&self, index_id: &str, sort: i32) -> Result<ProfileExItem> {
        self.update(index_id, |item| item.sort = sort).await
    }

    /// Applies a whole ordering as one transaction.
    ///
    /// Reordering used to loop over [`Self::set_sort`], which is a read-modify-
    /// write that autocommits per row; a subscription with hundreds of profiles
    /// therefore paid hundreds of fsynced commits for one user gesture. The
    /// repository writes only the `sort` column, which is all the read-modify-
    /// write ever changed here.
    pub async fn set_sort_many(&self, entries: &[(&str, i32)]) -> Result<()> {
        Ok(self.database.profile_exs().set_sort_many(entries).await?)
    }

    pub async fn set_test_delay(&self, index_id: &str, delay: i32) -> Result<ProfileExItem> {
        self.update(index_id, |item| item.delay = delay).await
    }

    pub async fn set_test_speed(&self, index_id: &str, speed: f64) -> Result<ProfileExItem> {
        self.update(index_id, |item| item.speed = speed).await
    }

    pub async fn set_test_message(
        &self,
        index_id: &str,
        message: impl Into<String>,
    ) -> Result<ProfileExItem> {
        let message = message.into();
        self.update(index_id, |item| item.message = Some(message))
            .await
    }

    pub async fn set_test_ip_info(
        &self,
        index_id: &str,
        ip_info: impl Into<String>,
    ) -> Result<ProfileExItem> {
        let ip_info = ip_info.into();
        self.update(index_id, |item| item.ip_info = Some(ip_info))
            .await
    }

    async fn update(
        &self,
        index_id: &str,
        update: impl FnOnce(&mut ProfileExItem),
    ) -> Result<ProfileExItem> {
        let mut item = self.ensure(index_id).await?;
        update(&mut item);
        self.database.profile_exs().upsert(&item).await?;

        Ok(item)
    }
}
