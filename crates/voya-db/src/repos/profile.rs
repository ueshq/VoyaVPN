use sqlx::{sqlite::SqliteRow, Row};
use voya_core::{ConfigType, ProfileExItem, ProfileItem};

use crate::{
    blob,
    executor::{delete_each, repository_constructors, run_query, RepositoryExecutor},
    DbError, ProfileExRepository, Result,
};

/// The one listing query behind every profile list path.
///
/// `list`, `list_by_subscription_id` and `list_with_profile_ex` used to repeat
/// this join and sort five times, so a sort fix or a new column had to be
/// applied in five places. The subscription filter is expressed as a nullable
/// bind instead of a second query string: passing `NULL` selects everything.
const PROFILE_LIST_QUERY: &str = r#"
    SELECT
        p.*,
        COALESCE(e.delay, 0) AS ex_delay,
        COALESCE(e.speed, 0.0) AS ex_speed,
        COALESCE(e.sort, 0) AS ex_sort,
        e.message AS ex_message,
        e.ip_info AS ex_ip_info
    FROM profile_items p
    LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
    WHERE (? IS NULL OR p.subscription_id = ?)
    ORDER BY COALESCE(e.sort, 0), p.index_id
"#;

#[derive(Debug, Clone, Copy)]
pub struct ProfileRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(ProfileRepository);

impl<'executor> ProfileRepository<'executor> {
    pub async fn upsert(&self, item: &ProfileItem) -> Result<()> {
        let protocol = blob::profile_protocol_to_text(&item.protocol)?;
        let transport = item
            .transport
            .as_ref()
            .map(blob::profile_transport_to_text)
            .transpose()?;
        let tls = item
            .tls
            .as_ref()
            .map(blob::tls_settings_to_text)
            .transpose()?;

        run_query!(
            self.executor,
            sqlx::query(
                r#"
            INSERT INTO profile_items (
                index_id, config_type, subscription_id,
                display_log, remarks, protocol, transport, tls
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?
            )
            ON CONFLICT(index_id) DO UPDATE SET
                config_type = excluded.config_type,
                subscription_id = excluded.subscription_id,
                display_log = excluded.display_log,
                remarks = excluded.remarks,
                protocol = excluded.protocol,
                transport = excluded.transport,
                tls = excluded.tls
            "#,
            )
            .bind(&item.index_id)
            .bind(config_type_to_str(item.config_type()))
            .bind(item.subscription_id.as_deref())
            .bind(item.display_log)
            .bind(&item.remarks)
            .bind(protocol)
            .bind(transport)
            .bind(tls),
            execute
        )?;

        Ok(())
    }

    pub async fn upsert_with_profile_ex(
        &self,
        item: &ProfileItem,
        profile_ex: &ProfileExItem,
    ) -> Result<()> {
        self.upsert(item).await?;
        ProfileExRepository::from_executor(self.executor)
            .upsert(profile_ex)
            .await
    }

    /// Single-row lookup, deliberately strict.
    ///
    /// A caller that asked for one profile has to hear that its stored payload
    /// cannot be decoded; only the list paths trade that for showing the rest.
    pub async fn get(&self, index_id: &str) -> Result<Option<ProfileItem>> {
        let row = run_query!(
            self.executor,
            sqlx::query("SELECT * FROM profile_items WHERE index_id = ?").bind(index_id),
            fetch_optional
        )?;

        row.map(|row| row_to_profile(&row)).transpose()
    }

    pub async fn list(&self) -> Result<Vec<ProfileItem>> {
        self.list_by_subscription_id(None).await
    }

    pub async fn list_by_subscription_id(
        &self,
        subscription_id: Option<&str>,
    ) -> Result<Vec<ProfileItem>> {
        Ok(self
            .list_with_profile_ex(subscription_id)
            .await?
            .into_iter()
            .map(|(profile, _)| profile)
            .collect())
    }

    pub async fn list_with_profile_ex(
        &self,
        subscription_id: Option<&str>,
    ) -> Result<Vec<(ProfileItem, ProfileExItem)>> {
        let subscription_id = subscription_id.filter(|value| !value.is_empty());
        let rows = run_query!(
            self.executor,
            sqlx::query(PROFILE_LIST_QUERY)
                .bind(subscription_id)
                .bind(subscription_id),
            fetch_all
        )?;

        decode_profile_rows(&rows)
    }

    pub async fn exists(&self, index_id: &str) -> Result<bool> {
        let exists: i64 = run_query!(
            self.executor,
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM profile_items WHERE index_id = ?)")
                .bind(index_id),
            fetch_one
        )?;

        Ok(exists != 0)
    }

    pub async fn delete(&self, index_id: &str) -> Result<bool> {
        let result = run_query!(
            self.executor,
            sqlx::query("DELETE FROM profile_items WHERE index_id = ?").bind(index_id),
            execute
        )?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_many(&self, index_ids: &[String]) -> Result<u64> {
        delete_each(
            self.executor,
            "DELETE FROM profile_items WHERE index_id = ?",
            index_ids,
        )
        .await
    }

    pub async fn delete_by_subscription_id(&self, subscription_id: &str) -> Result<u64> {
        let result = run_query!(
            self.executor,
            sqlx::query("DELETE FROM profile_items WHERE subscription_id = ?")
                .bind(subscription_id),
            execute
        )?;

        Ok(result.rows_affected())
    }
}

/// Decodes a listing's rows, dropping the ones this build cannot read.
///
/// The stored protocol/transport/TLS blobs are strict domain types with no
/// version tag, so a row written by a newer build, or one whose `config_type`
/// column disagrees with its blob, fails on its own. Propagating that would make
/// every profile screen, the runtime's config generation, group resolution and
/// subscription import fail together — the user could no longer see, connect to
/// or even delete a server, and the only recovery would be deleting the database
/// file. Bad rows are therefore logged and skipped, while genuine database
/// faults (a missing column, a closed pool) still propagate.
fn decode_profile_rows(rows: &[SqliteRow]) -> Result<Vec<(ProfileItem, ProfileExItem)>> {
    let mut decoded = Vec::with_capacity(rows.len());
    let mut skipped_rows = 0usize;

    for row in rows {
        match row_to_profile(row) {
            Ok(profile) => decoded.push((profile, row_to_profile_ex_joined(row)?)),
            Err(error) if error.is_row_payload() => {
                skipped_rows += 1;
                tracing::warn!(
                    index_id = %row.try_get::<String, _>("index_id").unwrap_or_default(),
                    error = %error,
                    "skipping a stored profile this build cannot decode"
                );
            }
            Err(error) => return Err(error),
        }
    }

    if skipped_rows > 0 {
        tracing::warn!(
            skipped_rows,
            "some profiles were hidden because their stored payload could not be decoded"
        );
    }

    Ok(decoded)
}

fn row_to_profile(row: &SqliteRow) -> Result<ProfileItem> {
    let config_type_value = row.try_get::<String, _>("config_type")?;
    let subscription_id = row.try_get::<Option<String>, _>("subscription_id")?;
    let protocol = blob::profile_protocol_from_text(&row.try_get::<String, _>("protocol")?)?;
    let transport = row
        .try_get::<Option<String>, _>("transport")?
        .as_deref()
        .map(blob::profile_transport_from_text)
        .transpose()?;
    let tls = row
        .try_get::<Option<String>, _>("tls")?
        .as_deref()
        .map(blob::tls_settings_from_text)
        .transpose()?;
    let config_type = config_type_from_str(&config_type_value)?;
    if protocol.config_type() != config_type {
        return Err(DbError::InvalidEnum {
            enum_name: "ProfileProtocol/config_type",
            value: config_type_value,
        });
    }

    Ok(ProfileItem {
        index_id: row.try_get("index_id")?,
        subscription_id,
        display_log: row.try_get("display_log")?,
        remarks: row.try_get("remarks")?,
        protocol,
        transport,
        tls,
    })
}

const fn config_type_to_str(value: ConfigType) -> &'static str {
    match value {
        ConfigType::VMess => "vmess",
        ConfigType::Custom => "custom",
        ConfigType::Shadowsocks => "shadowsocks",
        ConfigType::SOCKS => "socks",
        ConfigType::VLESS => "vless",
        ConfigType::Trojan => "trojan",
        ConfigType::Hysteria2 => "hysteria2",
        ConfigType::TUIC => "tuic",
        ConfigType::WireGuard => "wireGuard",
        ConfigType::HTTP => "http",
        ConfigType::Anytls => "anytls",
        ConfigType::Naive => "naive",
        ConfigType::PolicyGroup => "policyGroup",
        ConfigType::ProxyChain => "proxyChain",
    }
}

fn config_type_from_str(value: &str) -> Result<ConfigType> {
    match value {
        "vmess" => Ok(ConfigType::VMess),
        "custom" => Ok(ConfigType::Custom),
        "shadowsocks" => Ok(ConfigType::Shadowsocks),
        "socks" => Ok(ConfigType::SOCKS),
        "vless" => Ok(ConfigType::VLESS),
        "trojan" => Ok(ConfigType::Trojan),
        "hysteria2" => Ok(ConfigType::Hysteria2),
        "tuic" => Ok(ConfigType::TUIC),
        "wireGuard" => Ok(ConfigType::WireGuard),
        "http" => Ok(ConfigType::HTTP),
        "anytls" => Ok(ConfigType::Anytls),
        "naive" => Ok(ConfigType::Naive),
        "policyGroup" => Ok(ConfigType::PolicyGroup),
        "proxyChain" => Ok(ConfigType::ProxyChain),
        _ => Err(DbError::InvalidEnum {
            enum_name: "ConfigType",
            value: value.to_string(),
        }),
    }
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
