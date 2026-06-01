//! SQLite and JSON persistence boundary for VoyaVPN.
//!
//! This crate owns the fresh schema, repository mapping, and the only place
//! where typed domain blobs become SQLite `TEXT`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sqlx::{
    migrate::MigrateError,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
    Acquire, Row, SqlitePool,
};
use thiserror::Error;
use voya_core::{
    AppConfig, ConfigType, CoreType, DnsItem, ProfileExItem, ProfileItem, RoutingItem,
    ServerStatItem, SubItem,
};

pub mod blob {
    use serde::{de::DeserializeOwned, Serialize};
    use thiserror::Error;
    use voya_core::{ProtocolExtraItem, RulesItem, TransportExtraItem};

    #[derive(Debug, Error)]
    pub enum BlobError {
        #[error("failed to serialize {type_name}: {source}")]
        Serialize {
            type_name: &'static str,
            #[source]
            source: serde_json::Error,
        },
        #[error("failed to deserialize {type_name}: {source}")]
        Deserialize {
            type_name: &'static str,
            #[source]
            source: serde_json::Error,
        },
    }

    pub fn protocol_extra_to_text(value: &ProtocolExtraItem) -> Result<String, BlobError> {
        to_text("ProtocolExtraItem", value)
    }

    pub fn protocol_extra_from_text(value: &str) -> Result<ProtocolExtraItem, BlobError> {
        from_text_or_default("ProtocolExtraItem", value)
    }

    pub fn transport_extra_to_text(value: &TransportExtraItem) -> Result<String, BlobError> {
        to_text("TransportExtraItem", value)
    }

    pub fn transport_extra_from_text(value: &str) -> Result<TransportExtraItem, BlobError> {
        from_text_or_default("TransportExtraItem", value)
    }

    pub fn rules_to_text(value: &[RulesItem]) -> Result<String, BlobError> {
        to_text("RulesItem[]", value)
    }

    pub fn rules_from_text(value: &str) -> Result<Vec<RulesItem>, BlobError> {
        if value.trim().is_empty() {
            return Ok(Vec::new());
        }

        serde_json::from_str(value).map_err(|source| BlobError::Deserialize {
            type_name: "RulesItem[]",
            source,
        })
    }

    fn to_text<T>(type_name: &'static str, value: &T) -> Result<String, BlobError>
    where
        T: Serialize + ?Sized,
    {
        serde_json::to_string(value).map_err(|source| BlobError::Serialize { type_name, source })
    }

    fn from_text_or_default<T>(type_name: &'static str, value: &str) -> Result<T, BlobError>
    where
        T: DeserializeOwned + Default,
    {
        if value.trim().is_empty() {
            return Ok(T::default());
        }

        serde_json::from_str(value).map_err(|source| BlobError::Deserialize { type_name, source })
    }

    #[cfg(test)]
    mod tests {
        use voya_core::{MultipleLoad, ProtocolExtraItem, TransportExtraItem};

        use super::*;

        #[test]
        fn protocol_and_transport_extras_are_text_only_at_blob_boundary() {
            let proto = ProtocolExtraItem {
                flow: Some("xtls-rprx-vision".to_string()),
                multiple_load: Some(MultipleLoad::RoundRobin),
                ..ProtocolExtraItem::default()
            };
            let transport = TransportExtraItem {
                host: Some("example.com".to_string()),
                path: Some("/ws".to_string()),
                ..TransportExtraItem::default()
            };

            let proto_text = protocol_extra_to_text(&proto).unwrap();
            let transport_text = transport_extra_to_text(&transport).unwrap();

            assert_eq!(
                proto_text,
                r#"{"Flow":"xtls-rprx-vision","MultipleLoad":3}"#
            );
            assert_eq!(transport_text, r#"{"Host":"example.com","Path":"/ws"}"#);
            assert_eq!(protocol_extra_from_text(&proto_text).unwrap(), proto);
            assert_eq!(
                transport_extra_from_text(&transport_text).unwrap(),
                transport
            );
            assert_eq!(
                protocol_extra_from_text("").unwrap(),
                ProtocolExtraItem::default()
            );
        }
    }
}

pub const DATABASE_NAME: &str = "voyavpn.sqlite";

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] MigrateError),
    #[error(transparent)]
    Blob(#[from] blob::BlobError),
    #[error("invalid {enum_name} discriminant {value} in database")]
    InvalidEnum { enum_name: &'static str, value: i32 },
    #[error("filesystem error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON config error at {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
    path: Option<PathBuf>,
}

impl Database {
    pub async fn connect(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| DbError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        MIGRATOR.run(&pool).await?;

        Ok(Self {
            pool,
            path: Some(path.to_path_buf()),
        })
    }

    pub async fn connect_in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        MIGRATOR.run(&pool).await?;

        Ok(Self { pool, path: None })
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub async fn backup_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| DbError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        if path.exists() {
            fs::remove_file(path).map_err(|source| DbError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        }

        let target = path.to_string_lossy().into_owned();
        sqlx::query("VACUUM INTO ?")
            .bind(target)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn replace_from_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let source = path.to_string_lossy().into_owned();
        let mut conn = self.pool.acquire().await?;

        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await?;
        sqlx::query("ATTACH DATABASE ? AS backup")
            .bind(source)
            .execute(&mut *conn)
            .await?;

        let copy_result = async {
            let mut tx = conn.begin().await?;

            for statement in [
                "DELETE FROM main.server_stat_items",
                "DELETE FROM main.profile_ex_items",
                "DELETE FROM main.profile_items",
                "DELETE FROM main.subscriptions",
                "DELETE FROM main.routing_items",
                "DELETE FROM main.dns_items",
                "DELETE FROM main.full_config_template_items",
                "INSERT INTO main.profile_items SELECT * FROM backup.profile_items",
                "INSERT INTO main.profile_ex_items SELECT * FROM backup.profile_ex_items",
                "INSERT INTO main.server_stat_items SELECT * FROM backup.server_stat_items",
                "INSERT INTO main.subscriptions SELECT * FROM backup.subscriptions",
                "INSERT INTO main.routing_items SELECT * FROM backup.routing_items",
                "INSERT INTO main.dns_items SELECT * FROM backup.dns_items",
                "INSERT INTO main.full_config_template_items SELECT * FROM backup.full_config_template_items",
            ] {
                sqlx::query(statement).execute(&mut *tx).await?;
            }

            tx.commit().await?;
            Result::<()>::Ok(())
        }
        .await;

        let detach_result = sqlx::query("DETACH DATABASE backup")
            .execute(&mut *conn)
            .await;
        let _ = sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await;

        copy_result?;
        detach_result?;

        Ok(())
    }

    #[must_use]
    pub fn profiles(&self) -> ProfileRepository<'_> {
        ProfileRepository::new(&self.pool)
    }

    #[must_use]
    pub fn profile_exs(&self) -> ProfileExRepository<'_> {
        ProfileExRepository::new(&self.pool)
    }

    #[must_use]
    pub fn server_stats(&self) -> ServerStatRepository<'_> {
        ServerStatRepository::new(&self.pool)
    }

    #[must_use]
    pub fn subscriptions(&self) -> SubscriptionRepository<'_> {
        SubscriptionRepository::new(&self.pool)
    }

    #[must_use]
    pub fn routings(&self) -> RoutingRepository<'_> {
        RoutingRepository::new(&self.pool)
    }

    #[must_use]
    pub fn dns(&self) -> DnsRepository<'_> {
        DnsRepository::new(&self.pool)
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileRepository<'pool> {
    pool: &'pool SqlitePool,
}

impl<'pool> ProfileRepository<'pool> {
    #[must_use]
    pub fn new(pool: &'pool SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, item: &ProfileItem) -> Result<()> {
        let protocol_extra = blob::protocol_extra_to_text(&item.protocol_extra)?;
        let transport_extra = blob::transport_extra_to_text(&item.transport_extra)?;
        let core_type = item.core_type.map(CoreType::as_i32);

        sqlx::query(
            r#"
            INSERT INTO profile_items (
                index_id, config_type, core_type, config_version, subid, is_sub,
                pre_socks_port, display_log, remarks, address, port, password,
                username, network, stream_security, allow_insecure, sni, alpn,
                fingerprint, public_key, short_id, spider_x, mldsa65_verify,
                mux_enabled, cert, cert_sha, ech_config_list, finalmask,
                protocol_extra, transport_extra
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            ON CONFLICT(index_id) DO UPDATE SET
                config_type = excluded.config_type,
                core_type = excluded.core_type,
                config_version = excluded.config_version,
                subid = excluded.subid,
                is_sub = excluded.is_sub,
                pre_socks_port = excluded.pre_socks_port,
                display_log = excluded.display_log,
                remarks = excluded.remarks,
                address = excluded.address,
                port = excluded.port,
                password = excluded.password,
                username = excluded.username,
                network = excluded.network,
                stream_security = excluded.stream_security,
                allow_insecure = excluded.allow_insecure,
                sni = excluded.sni,
                alpn = excluded.alpn,
                fingerprint = excluded.fingerprint,
                public_key = excluded.public_key,
                short_id = excluded.short_id,
                spider_x = excluded.spider_x,
                mldsa65_verify = excluded.mldsa65_verify,
                mux_enabled = excluded.mux_enabled,
                cert = excluded.cert,
                cert_sha = excluded.cert_sha,
                ech_config_list = excluded.ech_config_list,
                finalmask = excluded.finalmask,
                protocol_extra = excluded.protocol_extra,
                transport_extra = excluded.transport_extra
            "#,
        )
        .bind(&item.index_id)
        .bind(item.config_type.as_i32())
        .bind(core_type)
        .bind(item.config_version)
        .bind(&item.subid)
        .bind(item.is_sub)
        .bind(item.pre_socks_port)
        .bind(item.display_log)
        .bind(&item.remarks)
        .bind(&item.address)
        .bind(item.port)
        .bind(&item.password)
        .bind(&item.username)
        .bind(&item.network)
        .bind(&item.stream_security)
        .bind(&item.allow_insecure)
        .bind(&item.sni)
        .bind(&item.alpn)
        .bind(&item.fingerprint)
        .bind(&item.public_key)
        .bind(&item.short_id)
        .bind(&item.spider_x)
        .bind(&item.mldsa65_verify)
        .bind(item.mux_enabled)
        .bind(&item.cert)
        .bind(&item.cert_sha)
        .bind(&item.ech_config_list)
        .bind(&item.finalmask)
        .bind(protocol_extra)
        .bind(transport_extra)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_with_profile_ex(
        &self,
        item: &ProfileItem,
        profile_ex: &ProfileExItem,
    ) -> Result<()> {
        self.upsert(item).await?;
        ProfileExRepository::new(self.pool).upsert(profile_ex).await
    }

    pub async fn get(&self, index_id: &str) -> Result<Option<ProfileItem>> {
        let row = sqlx::query("SELECT * FROM profile_items WHERE index_id = ?")
            .bind(index_id)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_profile).transpose()
    }

    pub async fn list(&self) -> Result<Vec<ProfileItem>> {
        let rows = sqlx::query(
            r#"
            SELECT p.*
            FROM profile_items p
            LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
            ORDER BY COALESCE(e.sort, 0), p.index_id
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(row_to_profile).collect()
    }

    pub async fn list_by_subid(&self, subid: Option<&str>) -> Result<Vec<ProfileItem>> {
        let rows = if let Some(subid) = subid.filter(|value| !value.is_empty()) {
            sqlx::query(
                r#"
                SELECT p.*
                FROM profile_items p
                LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
                WHERE p.subid = ?
                ORDER BY COALESCE(e.sort, 0), p.index_id
                "#,
            )
            .bind(subid)
            .fetch_all(self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT p.*
                FROM profile_items p
                LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
                ORDER BY COALESCE(e.sort, 0), p.index_id
                "#,
            )
            .fetch_all(self.pool)
            .await?
        };

        rows.into_iter().map(row_to_profile).collect()
    }

    pub async fn list_with_profile_ex(
        &self,
        subid: Option<&str>,
    ) -> Result<Vec<(ProfileItem, ProfileExItem)>> {
        let rows = if let Some(subid) = subid.filter(|value| !value.is_empty()) {
            sqlx::query(
                r#"
                SELECT
                    p.*,
                    COALESCE(e.delay, 0) AS ex_delay,
                    COALESCE(e.speed, 0) AS ex_speed,
                    COALESCE(e.sort, 0) AS ex_sort,
                    e.message AS ex_message,
                    e.ip_info AS ex_ip_info
                FROM profile_items p
                LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
                WHERE p.subid = ?
                ORDER BY COALESCE(e.sort, 0), p.index_id
                "#,
            )
            .bind(subid)
            .fetch_all(self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    p.*,
                    COALESCE(e.delay, 0) AS ex_delay,
                    COALESCE(e.speed, 0) AS ex_speed,
                    COALESCE(e.sort, 0) AS ex_sort,
                    e.message AS ex_message,
                    e.ip_info AS ex_ip_info
                FROM profile_items p
                LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
                ORDER BY COALESCE(e.sort, 0), p.index_id
                "#,
            )
            .fetch_all(self.pool)
            .await?
        };

        rows.into_iter()
            .map(|row| {
                let profile = row_to_profile_ref(&row)?;
                let profile_ex = row_to_profile_ex_joined(&row)?;

                Ok((profile, profile_ex))
            })
            .collect()
    }

    pub async fn exists(&self, index_id: &str) -> Result<bool> {
        let exists: i64 =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM profile_items WHERE index_id = ?)")
                .bind(index_id)
                .fetch_one(self.pool)
                .await?;

        Ok(exists != 0)
    }

    pub async fn delete(&self, index_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM profile_items WHERE index_id = ?")
            .bind(index_id)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_many(&self, index_ids: &[String]) -> Result<u64> {
        let mut deleted = 0;
        for index_id in index_ids {
            if self.delete(index_id).await? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }

    pub async fn delete_by_subid(&self, subid: &str, is_sub_only: bool) -> Result<u64> {
        let result = if is_sub_only {
            sqlx::query("DELETE FROM profile_items WHERE subid = ? AND is_sub = 1")
                .bind(subid)
                .execute(self.pool)
                .await?
        } else {
            sqlx::query("DELETE FROM profile_items WHERE subid = ?")
                .bind(subid)
                .execute(self.pool)
                .await?
        };

        Ok(result.rows_affected())
    }

    pub async fn update_subid_many(&self, index_ids: &[String], subid: &str) -> Result<u64> {
        let mut updated = 0;
        for index_id in index_ids {
            let result = sqlx::query("UPDATE profile_items SET subid = ? WHERE index_id = ?")
                .bind(subid)
                .bind(index_id)
                .execute(self.pool)
                .await?;
            updated += result.rows_affected();
        }

        Ok(updated)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileExRepository<'pool> {
    pool: &'pool SqlitePool,
}

impl<'pool> ProfileExRepository<'pool> {
    #[must_use]
    pub fn new(pool: &'pool SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, item: &ProfileExItem) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO profile_ex_items (
                index_id, delay, speed, sort, message, ip_info
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                delay = excluded.delay,
                speed = excluded.speed,
                sort = excluded.sort,
                message = excluded.message,
                ip_info = excluded.ip_info
            "#,
        )
        .bind(&item.index_id)
        .bind(item.delay)
        .bind(item.speed)
        .bind(item.sort)
        .bind(&item.message)
        .bind(&item.ip_info)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, index_id: &str) -> Result<Option<ProfileExItem>> {
        let row = sqlx::query("SELECT * FROM profile_ex_items WHERE index_id = ?")
            .bind(index_id)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_profile_ex).transpose()
    }

    pub async fn ensure(&self, index_id: &str) -> Result<ProfileExItem> {
        if let Some(item) = self.get(index_id).await? {
            return Ok(item);
        }

        let item = ProfileExItem {
            index_id: index_id.to_string(),
            ..ProfileExItem::default()
        };
        self.upsert(&item).await?;

        Ok(item)
    }

    pub async fn list(&self) -> Result<Vec<ProfileExItem>> {
        let rows = sqlx::query("SELECT * FROM profile_ex_items ORDER BY sort, index_id")
            .fetch_all(self.pool)
            .await?;

        rows.into_iter().map(row_to_profile_ex).collect()
    }

    pub async fn max_sort(&self) -> Result<i32> {
        let max_sort: Option<i32> = sqlx::query_scalar("SELECT MAX(sort) FROM profile_ex_items")
            .fetch_one(self.pool)
            .await?;

        Ok(max_sort.unwrap_or(0))
    }

    pub async fn set_sort(&self, index_id: &str, sort: i32) -> Result<()> {
        let mut item = self.ensure(index_id).await?;
        item.sort = sort;
        self.upsert(&item).await
    }

    pub async fn delete_orphans(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM profile_ex_items
            WHERE index_id NOT IN (SELECT index_id FROM profile_items)
            "#,
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ServerStatRepository<'pool> {
    pool: &'pool SqlitePool,
}

impl<'pool> ServerStatRepository<'pool> {
    #[must_use]
    pub fn new(pool: &'pool SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, item: &ServerStatItem) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO server_stat_items (
                index_id, total_up, total_down, today_up, today_down, date_now
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(index_id) DO UPDATE SET
                total_up = excluded.total_up,
                total_down = excluded.total_down,
                today_up = excluded.today_up,
                today_down = excluded.today_down,
                date_now = excluded.date_now
            "#,
        )
        .bind(&item.index_id)
        .bind(item.total_up)
        .bind(item.total_down)
        .bind(item.today_up)
        .bind(item.today_down)
        .bind(item.date_now)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, index_id: &str) -> Result<Option<ServerStatItem>> {
        let row = sqlx::query("SELECT * FROM server_stat_items WHERE index_id = ?")
            .bind(index_id)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_server_stat).transpose()
    }

    pub async fn ensure(&self, index_id: &str, date_now: i64) -> Result<ServerStatItem> {
        if let Some(mut item) = self.get(index_id).await? {
            if item.date_now != date_now {
                item.today_up = 0;
                item.today_down = 0;
                item.date_now = date_now;
                self.upsert(&item).await?;
            }

            return Ok(item);
        }

        let item = ServerStatItem {
            index_id: index_id.to_string(),
            date_now,
            ..ServerStatItem::default()
        };
        self.upsert(&item).await?;

        Ok(item)
    }

    pub async fn list(&self) -> Result<Vec<ServerStatItem>> {
        let rows = sqlx::query("SELECT * FROM server_stat_items ORDER BY index_id")
            .fetch_all(self.pool)
            .await?;

        rows.into_iter().map(row_to_server_stat).collect()
    }

    pub async fn clear_all(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM server_stat_items")
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn delete_orphans(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM server_stat_items
            WHERE index_id NOT IN (SELECT index_id FROM profile_items)
            "#,
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn reset_rollover(&self, date_now: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE server_stat_items
            SET today_up = 0, today_down = 0, date_now = ?
            WHERE date_now <> ?
            "#,
        )
        .bind(date_now)
        .bind(date_now)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn add_traffic(
        &self,
        index_id: &str,
        date_now: i64,
        proxy_up: i64,
        proxy_down: i64,
    ) -> Result<ServerStatItem> {
        let mut item = self.ensure(index_id, date_now).await?;
        item.today_up = item.today_up.saturating_add(proxy_up.max(0));
        item.today_down = item.today_down.saturating_add(proxy_down.max(0));
        item.total_up = item.total_up.saturating_add(proxy_up.max(0));
        item.total_down = item.total_down.saturating_add(proxy_down.max(0));
        item.date_now = date_now;
        self.upsert(&item).await?;

        Ok(item)
    }

    pub async fn clone_stat(
        &self,
        index_id: &str,
        to_index_id: &str,
    ) -> Result<Option<ServerStatItem>> {
        if index_id == to_index_id {
            return Ok(self.get(index_id).await?);
        }

        let Some(mut item) = self.get(index_id).await? else {
            return Ok(None);
        };

        item.index_id = to_index_id.to_string();
        self.upsert(&item).await?;

        Ok(Some(item))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SubscriptionRepository<'pool> {
    pool: &'pool SqlitePool,
}

impl<'pool> SubscriptionRepository<'pool> {
    #[must_use]
    pub fn new(pool: &'pool SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, item: &SubItem) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO subscriptions (
                id, remarks, url, more_url, enabled, user_agent, sort, filter,
                auto_update_interval, update_time, convert_target, prev_profile,
                next_profile, pre_socks_port, memo
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                remarks = excluded.remarks,
                url = excluded.url,
                more_url = excluded.more_url,
                enabled = excluded.enabled,
                user_agent = excluded.user_agent,
                sort = excluded.sort,
                filter = excluded.filter,
                auto_update_interval = excluded.auto_update_interval,
                update_time = excluded.update_time,
                convert_target = excluded.convert_target,
                prev_profile = excluded.prev_profile,
                next_profile = excluded.next_profile,
                pre_socks_port = excluded.pre_socks_port,
                memo = excluded.memo
            "#,
        )
        .bind(&item.id)
        .bind(&item.remarks)
        .bind(&item.url)
        .bind(&item.more_url)
        .bind(item.enabled)
        .bind(&item.user_agent)
        .bind(item.sort)
        .bind(&item.filter)
        .bind(item.auto_update_interval)
        .bind(item.update_time)
        .bind(&item.convert_target)
        .bind(&item.prev_profile)
        .bind(&item.next_profile)
        .bind(item.pre_socks_port)
        .bind(&item.memo)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<SubItem>> {
        let row = sqlx::query("SELECT * FROM subscriptions WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_subscription).transpose()
    }

    pub async fn get_by_url(&self, url: &str) -> Result<Option<SubItem>> {
        let row = sqlx::query("SELECT * FROM subscriptions WHERE url = ?")
            .bind(url)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_subscription).transpose()
    }

    pub async fn list(&self) -> Result<Vec<SubItem>> {
        let rows = sqlx::query("SELECT * FROM subscriptions ORDER BY sort, id")
            .fetch_all(self.pool)
            .await?;

        rows.into_iter().map(row_to_subscription).collect()
    }

    pub async fn max_sort(&self) -> Result<i32> {
        let max_sort: Option<i32> = sqlx::query_scalar("SELECT MAX(sort) FROM subscriptions")
            .fetch_one(self.pool)
            .await?;

        Ok(max_sort.unwrap_or(0))
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM subscriptions WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RoutingRepository<'pool> {
    pool: &'pool SqlitePool,
}

impl<'pool> RoutingRepository<'pool> {
    #[must_use]
    pub fn new(pool: &'pool SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, item: &RoutingItem) -> Result<()> {
        let rule_set = blob::rules_to_text(&item.rule_set)?;
        let rule_num = i32::try_from(item.rule_set.len()).unwrap_or(i32::MAX);

        sqlx::query(
            r#"
            INSERT INTO routing_items (
                id, remarks, url, rule_set, rule_num, enabled, locked,
                custom_icon, custom_ruleset_path4_singbox, domain_strategy,
                domain_strategy4_singbox, sort, is_active
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                remarks = excluded.remarks,
                url = excluded.url,
                rule_set = excluded.rule_set,
                rule_num = excluded.rule_num,
                enabled = excluded.enabled,
                locked = excluded.locked,
                custom_icon = excluded.custom_icon,
                custom_ruleset_path4_singbox = excluded.custom_ruleset_path4_singbox,
                domain_strategy = excluded.domain_strategy,
                domain_strategy4_singbox = excluded.domain_strategy4_singbox,
                sort = excluded.sort,
                is_active = excluded.is_active
            "#,
        )
        .bind(&item.id)
        .bind(&item.remarks)
        .bind(&item.url)
        .bind(rule_set)
        .bind(rule_num)
        .bind(item.enabled)
        .bind(item.locked)
        .bind(&item.custom_icon)
        .bind(&item.custom_ruleset_path4_singbox)
        .bind(&item.domain_strategy)
        .bind(&item.domain_strategy4_singbox)
        .bind(item.sort)
        .bind(item.is_active)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<RoutingItem>> {
        let row = sqlx::query("SELECT * FROM routing_items WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_routing).transpose()
    }

    pub async fn list(&self) -> Result<Vec<RoutingItem>> {
        let rows = sqlx::query("SELECT * FROM routing_items ORDER BY sort, id")
            .fetch_all(self.pool)
            .await?;

        rows.into_iter().map(row_to_routing).collect()
    }

    pub async fn active(&self) -> Result<Option<RoutingItem>> {
        let row = sqlx::query(
            "SELECT * FROM routing_items WHERE is_active = 1 ORDER BY sort, id LIMIT 1",
        )
        .fetch_optional(self.pool)
        .await?;

        row.map(row_to_routing).transpose()
    }

    pub async fn first(&self) -> Result<Option<RoutingItem>> {
        let row = sqlx::query("SELECT * FROM routing_items ORDER BY sort, id LIMIT 1")
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_routing).transpose()
    }

    pub async fn exists(&self, id: &str) -> Result<bool> {
        let exists: i64 =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM routing_items WHERE id = ?)")
                .bind(id)
                .fetch_one(self.pool)
                .await?;

        Ok(exists != 0)
    }

    pub async fn max_sort(&self) -> Result<i32> {
        let max_sort: Option<i32> = sqlx::query_scalar("SELECT MAX(sort) FROM routing_items")
            .fetch_one(self.pool)
            .await?;

        Ok(max_sort.unwrap_or(0))
    }

    pub async fn set_active(&self, id: &str) -> Result<bool> {
        if !self.exists(id).await? {
            return Ok(false);
        }

        sqlx::query("UPDATE routing_items SET is_active = 0")
            .execute(self.pool)
            .await?;
        let result = sqlx::query("UPDATE routing_items SET is_active = 1 WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM routing_items WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_many(&self, ids: &[String]) -> Result<u64> {
        let mut deleted = 0;
        for id in ids {
            if self.delete(id).await? {
                deleted += 1;
            }
        }

        Ok(deleted)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DnsRepository<'pool> {
    pool: &'pool SqlitePool,
}

impl<'pool> DnsRepository<'pool> {
    #[must_use]
    pub fn new(pool: &'pool SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, item: &DnsItem) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO dns_items (
                id, remarks, enabled, core_type, use_system_hosts,
                normal_dns, tun_dns, domain_strategy4_freedom, domain_dns_address
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                remarks = excluded.remarks,
                enabled = excluded.enabled,
                core_type = excluded.core_type,
                use_system_hosts = excluded.use_system_hosts,
                normal_dns = excluded.normal_dns,
                tun_dns = excluded.tun_dns,
                domain_strategy4_freedom = excluded.domain_strategy4_freedom,
                domain_dns_address = excluded.domain_dns_address
            "#,
        )
        .bind(&item.id)
        .bind(&item.remarks)
        .bind(item.enabled)
        .bind(item.core_type.as_i32())
        .bind(item.use_system_hosts)
        .bind(&item.normal_dns)
        .bind(&item.tun_dns)
        .bind(&item.domain_strategy4_freedom)
        .bind(&item.domain_dns_address)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<DnsItem>> {
        let row = sqlx::query("SELECT * FROM dns_items WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;

        row.map(row_to_dns).transpose()
    }

    pub async fn get_by_core_type(&self, core_type: CoreType) -> Result<Option<DnsItem>> {
        let row = sqlx::query(
            r#"
            SELECT *
            FROM dns_items
            WHERE core_type = ?
            ORDER BY enabled DESC, id
            LIMIT 1
            "#,
        )
        .bind(core_type.as_i32())
        .fetch_optional(self.pool)
        .await?;

        row.map(row_to_dns).transpose()
    }

    pub async fn list(&self) -> Result<Vec<DnsItem>> {
        let rows = sqlx::query("SELECT * FROM dns_items ORDER BY core_type, id")
            .fetch_all(self.pool)
            .await?;

        rows.into_iter().map(row_to_dns).collect()
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM dns_items WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfigStore {
    path: PathBuf,
}

impl AppConfigStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppConfig> {
        if !self.path.exists() {
            return Ok(AppConfig::default());
        }

        let content = fs::read_to_string(&self.path).map_err(|source| DbError::Io {
            path: self.path.clone(),
            source,
        })?;

        serde_json::from_str(&content).map_err(|source| DbError::Json {
            path: self.path.clone(),
            source,
        })
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| DbError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let content = serde_json::to_string_pretty(config).map_err(|source| DbError::Json {
            path: self.path.clone(),
            source,
        })?;
        let temp_path = self.path.with_extension("json.tmp");

        fs::write(&temp_path, content).map_err(|source| DbError::Io {
            path: temp_path.clone(),
            source,
        })?;
        match fs::rename(&temp_path, &self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(&self.path).map_err(|source| DbError::Io {
                    path: self.path.clone(),
                    source,
                })?;
                fs::rename(&temp_path, &self.path).map_err(|source| DbError::Io {
                    path: self.path.clone(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(DbError::Io {
                    path: self.path.clone(),
                    source,
                });
            }
        }

        Ok(())
    }
}

fn row_to_profile(row: SqliteRow) -> Result<ProfileItem> {
    row_to_profile_ref(&row)
}

fn row_to_profile_ref(row: &SqliteRow) -> Result<ProfileItem> {
    let config_type_value = row.try_get::<i32, _>("config_type")?;
    let core_type_value = row.try_get::<Option<i32>, _>("core_type")?;
    let protocol_extra = row.try_get::<String, _>("protocol_extra")?;
    let transport_extra = row.try_get::<String, _>("transport_extra")?;

    Ok(ProfileItem {
        index_id: row.try_get("index_id")?,
        config_type: ConfigType::from_i32(config_type_value).ok_or(DbError::InvalidEnum {
            enum_name: "ConfigType",
            value: config_type_value,
        })?,
        core_type: core_type_value
            .map(|value| {
                CoreType::from_i32(value).ok_or(DbError::InvalidEnum {
                    enum_name: "CoreType",
                    value,
                })
            })
            .transpose()?,
        config_version: row.try_get("config_version")?,
        subid: row.try_get("subid")?,
        is_sub: row.try_get("is_sub")?,
        pre_socks_port: row.try_get("pre_socks_port")?,
        display_log: row.try_get("display_log")?,
        remarks: row.try_get("remarks")?,
        address: row.try_get("address")?,
        port: row.try_get("port")?,
        password: row.try_get("password")?,
        username: row.try_get("username")?,
        network: row.try_get("network")?,
        stream_security: row.try_get("stream_security")?,
        allow_insecure: row.try_get("allow_insecure")?,
        sni: row.try_get("sni")?,
        alpn: row.try_get("alpn")?,
        fingerprint: row.try_get("fingerprint")?,
        public_key: row.try_get("public_key")?,
        short_id: row.try_get("short_id")?,
        spider_x: row.try_get("spider_x")?,
        mldsa65_verify: row.try_get("mldsa65_verify")?,
        mux_enabled: row.try_get("mux_enabled")?,
        cert: row.try_get("cert")?,
        cert_sha: row.try_get("cert_sha")?,
        ech_config_list: row.try_get("ech_config_list")?,
        finalmask: row.try_get("finalmask")?,
        protocol_extra: blob::protocol_extra_from_text(&protocol_extra)?,
        transport_extra: blob::transport_extra_from_text(&transport_extra)?,
    })
}

fn row_to_profile_ex(row: SqliteRow) -> Result<ProfileExItem> {
    Ok(ProfileExItem {
        index_id: row.try_get("index_id")?,
        delay: row.try_get("delay")?,
        speed: row.try_get("speed")?,
        sort: row.try_get("sort")?,
        message: row.try_get("message")?,
        ip_info: row.try_get("ip_info")?,
    })
}

fn row_to_profile_ex_joined(row: &SqliteRow) -> Result<ProfileExItem> {
    Ok(ProfileExItem {
        index_id: row.try_get("index_id")?,
        delay: row.try_get("ex_delay")?,
        speed: row.try_get("ex_speed")?,
        sort: row.try_get("ex_sort")?,
        message: row.try_get("ex_message")?,
        ip_info: row.try_get("ex_ip_info")?,
    })
}

fn row_to_server_stat(row: SqliteRow) -> Result<ServerStatItem> {
    Ok(ServerStatItem {
        index_id: row.try_get("index_id")?,
        total_up: row.try_get("total_up")?,
        total_down: row.try_get("total_down")?,
        today_up: row.try_get("today_up")?,
        today_down: row.try_get("today_down")?,
        date_now: row.try_get("date_now")?,
    })
}

fn row_to_subscription(row: SqliteRow) -> Result<SubItem> {
    Ok(SubItem {
        id: row.try_get("id")?,
        remarks: row.try_get("remarks")?,
        url: row.try_get("url")?,
        more_url: row.try_get("more_url")?,
        enabled: row.try_get("enabled")?,
        user_agent: row.try_get("user_agent")?,
        sort: row.try_get("sort")?,
        filter: row.try_get("filter")?,
        auto_update_interval: row.try_get("auto_update_interval")?,
        update_time: row.try_get("update_time")?,
        convert_target: row.try_get("convert_target")?,
        prev_profile: row.try_get("prev_profile")?,
        next_profile: row.try_get("next_profile")?,
        pre_socks_port: row.try_get("pre_socks_port")?,
        memo: row.try_get("memo")?,
    })
}

fn row_to_routing(row: SqliteRow) -> Result<RoutingItem> {
    let rule_set = row.try_get::<String, _>("rule_set")?;
    let rules = blob::rules_from_text(&rule_set)?;

    Ok(RoutingItem {
        id: row.try_get("id")?,
        remarks: row.try_get("remarks")?,
        url: row.try_get("url")?,
        rule_num: i32::try_from(rules.len()).unwrap_or(i32::MAX),
        rule_set: rules,
        enabled: row.try_get("enabled")?,
        locked: row.try_get("locked")?,
        custom_icon: row.try_get("custom_icon")?,
        custom_ruleset_path4_singbox: row.try_get("custom_ruleset_path4_singbox")?,
        domain_strategy: row.try_get("domain_strategy")?,
        domain_strategy4_singbox: row.try_get("domain_strategy4_singbox")?,
        sort: row.try_get("sort")?,
        is_active: row.try_get("is_active")?,
    })
}

fn row_to_dns(row: SqliteRow) -> Result<DnsItem> {
    let core_type_value = row.try_get::<i32, _>("core_type")?;

    Ok(DnsItem {
        id: row.try_get("id")?,
        remarks: row.try_get("remarks")?,
        enabled: row.try_get("enabled")?,
        core_type: CoreType::from_i32(core_type_value).ok_or(DbError::InvalidEnum {
            enum_name: "CoreType",
            value: core_type_value,
        })?,
        use_system_hosts: row.try_get("use_system_hosts")?,
        normal_dns: row.try_get("normal_dns")?,
        tun_dns: row.try_get("tun_dns")?,
        domain_strategy4_freedom: row.try_get("domain_strategy4_freedom")?,
        domain_dns_address: row.try_get("domain_dns_address")?,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use sqlx::Row;
    use voya_core::{ProtocolExtraItem, RuleType, RulesItem, SysProxyType, TransportExtraItem};

    use super::*;

    #[test]
    fn database_name_is_voyavpn_specific() {
        assert_eq!(DATABASE_NAME, "voyavpn.sqlite");
    }

    #[tokio::test]
    async fn migrated_profile_schema_omits_obsolete_columns() {
        let database = Database::connect_in_memory().await.unwrap();
        let rows = sqlx::query("PRAGMA table_info(profile_items)")
            .fetch_all(database.pool())
            .await
            .unwrap();
        let columns = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();

        for obsolete in [
            "header_type",
            "request_host",
            "path",
            "extra",
            "ports",
            "alter_id",
            "flow",
            "id",
            "security",
        ] {
            assert!(
                !columns.iter().any(|column| column == obsolete),
                "{obsolete} should be absent"
            );
        }

        assert!(columns.iter().any(|column| column == "protocol_extra"));
        assert!(columns.iter().any(|column| column == "transport_extra"));
    }

    #[tokio::test]
    async fn statistics_repository_rolls_over_cleans_orphans_and_clones() {
        let database = Database::connect_in_memory().await.unwrap();
        let mut source = sample_profile();
        source.index_id = "source".to_string();
        let mut clone = sample_profile();
        clone.index_id = "clone".to_string();
        database.profiles().upsert(&source).await.unwrap();
        database.profiles().upsert(&clone).await.unwrap();

        database
            .server_stats()
            .upsert(&ServerStatItem {
                index_id: "source".to_string(),
                total_up: 1000,
                total_down: 2000,
                today_up: 300,
                today_down: 400,
                date_now: 1,
            })
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(database.pool())
            .await
            .unwrap();
        database
            .server_stats()
            .upsert(&ServerStatItem {
                index_id: "orphan".to_string(),
                total_up: 1,
                total_down: 1,
                today_up: 1,
                today_down: 1,
                date_now: 1,
            })
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(database.pool())
            .await
            .unwrap();

        let orphaned = database.server_stats().delete_orphans().await.unwrap();
        assert_eq!(orphaned, 1);
        database.server_stats().reset_rollover(2).await.unwrap();
        let rolled = database
            .server_stats()
            .get("source")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rolled.today_up, 0);
        assert_eq!(rolled.today_down, 0);
        assert_eq!(rolled.total_up, 1000);
        assert_eq!(rolled.total_down, 2000);
        assert_eq!(rolled.date_now, 2);

        let cloned = database
            .server_stats()
            .clone_stat("source", "clone")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cloned.index_id, "clone");
        assert_eq!(cloned.total_up, 1000);
        assert_eq!(cloned.total_down, 2000);

        let updated = database
            .server_stats()
            .add_traffic("clone", 3, 50, 70)
            .await
            .unwrap();
        assert_eq!(updated.today_up, 50);
        assert_eq!(updated.today_down, 70);
        assert_eq!(updated.total_up, 1050);
        assert_eq!(updated.total_down, 2070);
        assert_eq!(updated.date_now, 3);
    }

    #[tokio::test]
    async fn profile_repository_persists_typed_extra_blobs() {
        let database = Database::connect_in_memory().await.unwrap();
        let profile = sample_profile();

        database.profiles().upsert(&profile).await.unwrap();
        let loaded = database.profiles().get("profile-1").await.unwrap().unwrap();

        assert_eq!(loaded, profile);

        let raw_protocol_extra: String =
            sqlx::query_scalar("SELECT protocol_extra FROM profile_items WHERE index_id = ?")
                .bind("profile-1")
                .fetch_one(database.pool())
                .await
                .unwrap();

        assert_eq!(
            raw_protocol_extra,
            r#"{"SsMethod":"2022-blake3-aes-256-gcm","Ports":"443,8443"}"#
        );
    }

    #[tokio::test]
    async fn file_database_persists_profile_across_pool_restart() {
        let path = temp_path("restart.sqlite");
        let profile = sample_profile();

        let first = Database::connect(&path).await.unwrap();
        first.profiles().upsert(&profile).await.unwrap();
        first.close().await;

        let second = Database::connect(&path).await.unwrap();
        let loaded = second.profiles().get("profile-1").await.unwrap().unwrap();

        assert_eq!(loaded, profile);
        second.close().await;
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn profile_repository_orders_by_profile_ex_sort_and_updates_groups() {
        let database = Database::connect_in_memory().await.unwrap();
        let mut first = sample_profile();
        first.index_id = "first".to_string();
        first.subid = "old".to_string();
        let mut second = sample_profile();
        second.index_id = "second".to_string();
        second.subid = "old".to_string();

        database.profiles().upsert(&first).await.unwrap();
        database.profiles().upsert(&second).await.unwrap();
        database
            .profile_exs()
            .upsert(&ProfileExItem {
                index_id: "first".to_string(),
                sort: 20,
                ..ProfileExItem::default()
            })
            .await
            .unwrap();
        database
            .profile_exs()
            .upsert(&ProfileExItem {
                index_id: "second".to_string(),
                sort: 10,
                ..ProfileExItem::default()
            })
            .await
            .unwrap();

        let ordered = database
            .profiles()
            .list_with_profile_ex(None)
            .await
            .unwrap();
        assert_eq!(ordered[0].0.index_id, "second");
        assert_eq!(ordered[0].1.sort, 10);

        let updated = database
            .profiles()
            .update_subid_many(&["first".to_string(), "second".to_string()], "new")
            .await
            .unwrap();
        assert_eq!(updated, 2);
        assert_eq!(
            database
                .profiles()
                .list_by_subid(Some("new"))
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn profile_ex_repository_cascades_with_profile_deletes() {
        let database = Database::connect_in_memory().await.unwrap();
        let profile = sample_profile();

        database.profiles().upsert(&profile).await.unwrap();
        database
            .profile_exs()
            .upsert(&ProfileExItem {
                index_id: profile.index_id.clone(),
                delay: 42,
                sort: 10,
                ..ProfileExItem::default()
            })
            .await
            .unwrap();
        assert!(database
            .profile_exs()
            .get(&profile.index_id)
            .await
            .unwrap()
            .is_some());

        assert!(database.profiles().delete(&profile.index_id).await.unwrap());
        assert!(database
            .profile_exs()
            .get(&profile.index_id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn subscription_repository_persists_orders_and_deletes_sub_profiles() {
        let database = Database::connect_in_memory().await.unwrap();
        let first = SubItem {
            id: "sub-a".to_string(),
            remarks: "A".to_string(),
            url: "https://example.test/a".to_string(),
            sort: 20,
            filter: Some("US|JP".to_string()),
            auto_update_interval: 30,
            update_time: 123,
            convert_target: Some("clash".to_string()),
            ..SubItem::default()
        };
        let second = SubItem {
            id: "sub-b".to_string(),
            remarks: "B".to_string(),
            url: "https://example.test/b".to_string(),
            sort: 10,
            ..SubItem::default()
        };
        database.subscriptions().upsert(&first).await.unwrap();
        database.subscriptions().upsert(&second).await.unwrap();

        let listed = database.subscriptions().list().await.unwrap();
        assert_eq!(listed[0].id, "sub-b");
        assert_eq!(listed[1], first);
        assert_eq!(database.subscriptions().max_sort().await.unwrap(), 20);
        assert_eq!(
            database
                .subscriptions()
                .get_by_url("https://example.test/a")
                .await
                .unwrap()
                .unwrap()
                .id,
            "sub-a"
        );

        let mut profile = sample_profile();
        profile.index_id = "sub-profile".to_string();
        profile.subid = "sub-a".to_string();
        profile.is_sub = true;
        database.profiles().upsert(&profile).await.unwrap();
        let deleted = database
            .profiles()
            .delete_by_subid("sub-a", true)
            .await
            .unwrap();
        assert_eq!(deleted, 1);
        assert!(database
            .profiles()
            .get("sub-profile")
            .await
            .unwrap()
            .is_none());

        assert!(database.subscriptions().delete("sub-a").await.unwrap());
        assert!(database
            .subscriptions()
            .get("sub-a")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn routing_repository_serializes_rules_and_enforces_active_selection() {
        let database = Database::connect_in_memory().await.unwrap();
        let first = RoutingItem {
            id: "routing-a".to_string(),
            remarks: "A".to_string(),
            sort: 20,
            is_active: true,
            domain_strategy: "AsIs".to_string(),
            rule_set: vec![RulesItem {
                id: "rule-a".to_string(),
                outbound_tag: Some("direct".to_string()),
                domain: Some(vec!["full:direct.example.com".to_string()]),
                rule_type: Some(RuleType::Routing),
                ..RulesItem::default()
            }],
            ..RoutingItem::default()
        };
        let second = RoutingItem {
            id: "routing-b".to_string(),
            remarks: "B".to_string(),
            sort: 10,
            ..RoutingItem::default()
        };

        database.routings().upsert(&first).await.unwrap();
        database.routings().upsert(&second).await.unwrap();

        let listed = database.routings().list().await.unwrap();
        assert_eq!(listed[0].id, "routing-b");
        assert_eq!(listed[1].rule_num, 1);
        assert_eq!(
            listed[1].rule_set[0].domain.clone(),
            Some(vec!["full:direct.example.com".to_string()])
        );
        assert_eq!(
            database.routings().active().await.unwrap().unwrap().id,
            "routing-a"
        );

        assert!(database.routings().set_active("routing-b").await.unwrap());
        assert_eq!(
            database.routings().active().await.unwrap().unwrap().id,
            "routing-b"
        );
        assert!(database.routings().delete("routing-a").await.unwrap());
        assert!(database
            .routings()
            .get("routing-a")
            .await
            .unwrap()
            .is_none());
    }

    #[test]
    fn app_config_store_defaults_and_persists_across_restart() {
        let path = temp_path("guiNConfig.json");
        let store = AppConfigStore::new(&path);
        let mut config = store.load().unwrap();

        assert_eq!(config.inbound[0].local_port, 10808);
        config.index_id = "active-profile".to_string();
        config.ui_item.current_language = "fa-Ir".to_string();
        config.system_proxy_item.sys_proxy_type = SysProxyType::Unchanged;
        store.save(&config).unwrap();

        let restarted_store = AppConfigStore::new(&path);
        let loaded = restarted_store.load().unwrap();

        assert_eq!(loaded.index_id, "active-profile");
        assert_eq!(loaded.ui_item.current_language, "fa-Ir");
        assert_eq!(
            loaded.system_proxy_item.sys_proxy_type,
            SysProxyType::Unchanged
        );
        let _ = fs::remove_file(path);
    }

    fn sample_profile() -> ProfileItem {
        ProfileItem {
            index_id: "profile-1".to_string(),
            config_type: ConfigType::Shadowsocks,
            core_type: Some(CoreType::sing_box),
            remarks: "Demo".to_string(),
            address: "example.com".to_string(),
            port: 443,
            password: "secret".to_string(),
            network: "ws".to_string(),
            stream_security: "tls".to_string(),
            sni: "example.com".to_string(),
            protocol_extra: ProtocolExtraItem {
                ss_method: Some("2022-blake3-aes-256-gcm".to_string()),
                ports: Some("443,8443".to_string()),
                ..ProtocolExtraItem::default()
            },
            transport_extra: TransportExtraItem {
                host: Some("example.com".to_string()),
                path: Some("/ws".to_string()),
                ..TransportExtraItem::default()
            },
            ..ProfileItem::default()
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join("voyavpn-tests").join(format!(
            "{}-{}-{name}",
            std::process::id(),
            nanos
        ))
    }
}
