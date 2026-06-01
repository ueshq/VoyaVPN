use voya_core::ProfileExItem;
use voya_db::Database;

use super::Result;

#[derive(Debug, Clone, Copy)]
pub struct ProfileExManager<'db> {
    database: &'db Database,
}

impl<'db> ProfileExManager<'db> {
    #[must_use]
    pub fn new(database: &'db Database) -> Self {
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

    pub async fn get_sort(&self, index_id: &str) -> Result<i32> {
        Ok(self
            .database
            .profile_exs()
            .get(index_id)
            .await?
            .map_or(0, |item| item.sort))
    }

    pub async fn get_max_sort(&self) -> Result<i32> {
        Ok(self.database.profile_exs().max_sort().await?)
    }

    pub async fn set_sort(&self, index_id: &str, sort: i32) -> Result<ProfileExItem> {
        self.update(index_id, |item| item.sort = sort).await
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
