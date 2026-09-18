use sqlx::{sqlite::SqliteRow, Row};
use voya_core::{ConfigType, ProfileExItem, ProfileItem};

use super::decode_rows;
use crate::{
    blob,
    executor::{delete_each, repository_constructors, row_exists, run_query, RepositoryExecutor},
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
        COALESCE(e.sort, 0) AS ex_sort,
        e.message AS ex_message,
        e.ip_info AS ex_ip_info,
        e.country_code AS ex_country_code
    FROM profile_items p
    LEFT JOIN profile_ex_items e ON p.index_id = e.index_id
    WHERE (? IS NULL OR p.subscription_id = ?)
    ORDER BY COALESCE(e.sort, 0), p.index_id
"#;

/// A profile listing together with the rows this build had to skip.
///
/// The count travels with the rows it is missing from rather than only reaching
/// a log file: the profiles screen states it, so a short list is explained
/// instead of looking like data loss; a log line the user has no reason to open
/// is not an explanation. See `decode_rows` for why the rows are skipped at all.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileListing {
    pub items: Vec<(ProfileItem, ProfileExItem)>,
    pub undecodable_rows: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileRepository<'executor> {
    executor: RepositoryExecutor<'executor>,
}

repository_constructors!(ProfileRepository);

impl<'executor> ProfileRepository<'executor> {
    pub async fn upsert(&self, item: &ProfileItem) -> Result<()> {
        let (protocol, transport, tls) = blob::profile_blobs(item)?;

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
            .bind(item.config_type().as_str())
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
        let mut profile_ex = profile_ex.clone();
        if self
            .get(&item.index_id)
            .await?
            .is_some_and(|previous| !voya_core::profile_items_match(&previous, item, false))
        {
            profile_ex.country_code = None;
            profile_ex.delay = 0;
            profile_ex.message = None;
            profile_ex.ip_info = None;
        }
        self.upsert_with_checked_profile_ex(item, &profile_ex).await
    }

    /// [`Self::upsert_with_profile_ex`] for a caller that has already read the
    /// stored row and cleared `profile_ex`'s measurements if the connection
    /// changed, so the row is not read a second time.
    pub async fn upsert_with_checked_profile_ex(
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
            .items
            .into_iter()
            .map(|(profile, _)| profile)
            .collect())
    }

    pub async fn list_with_profile_ex(
        &self,
        subscription_id: Option<&str>,
    ) -> Result<ProfileListing> {
        let subscription_id = subscription_id.filter(|value| !value.is_empty());
        let rows = run_query!(
            self.executor,
            sqlx::query(PROFILE_LIST_QUERY)
                .bind(subscription_id)
                .bind(subscription_id),
            fetch_all
        )?;

        let (items, undecodable_rows) = decode_rows(
            &rows,
            "index_id",
            "skipping a stored node this build cannot decode",
            "some nodes were hidden because their stored payload could not be decoded",
            |row| Ok((row_to_profile(row)?, row_to_profile_ex_joined(row)?)),
        )?;
        Ok(ProfileListing {
            items,
            undecodable_rows,
        })
    }

    pub async fn exists(&self, index_id: &str) -> Result<bool> {
        row_exists(
            self.executor,
            "SELECT EXISTS(SELECT 1 FROM profile_items WHERE index_id = ?)",
            index_id,
        )
        .await
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

/// Stored nodes naming a transport removed from the domain. sing-box has no
/// outbound for them, so none of these rows could ever connect.
const DELETE_RETIRED_TRANSPORTS: &str = "DELETE FROM profile_items \
     WHERE transport IS NOT NULL AND json_valid(transport) \
     AND json_extract(transport, '$.kind') IN ('xhttp', 'kcp')";

/// Cached probe results of every node whose TLS blob still carries a retired key.
const STASH_RETIRED_TLS_PROBES: &str = "CREATE TEMP TABLE voya_retired_tls_probes AS \
     SELECT e.index_id, e.delay, e.message, e.ip_info, e.country_code \
     FROM profile_ex_items e JOIN profile_items p ON p.index_id = e.index_id \
     WHERE p.tls IS NOT NULL AND json_valid(p.tls) AND (\
     json_type(p.tls, '$.realitySpiderX') IS NOT NULL \
     OR json_type(p.tls, '$.mldsa65Verify') IS NOT NULL \
     OR json_type(p.tls, '$.certificateSha256') IS NOT NULL \
     OR json_type(p.tls, '$.finalMask') IS NOT NULL)";

const STRIP_RETIRED_TLS_KEYS: &str = "UPDATE profile_items \
     SET tls = json_remove(tls, '$.realitySpiderX', '$.mldsa65Verify', '$.certificateSha256', '$.finalMask') \
     WHERE tls IS NOT NULL AND json_valid(tls) AND (\
     json_type(tls, '$.realitySpiderX') IS NOT NULL \
     OR json_type(tls, '$.mldsa65Verify') IS NOT NULL \
     OR json_type(tls, '$.certificateSha256') IS NOT NULL \
     OR json_type(tls, '$.finalMask') IS NOT NULL)";

/// The baseline trigger clears cached probe results whenever `tls` changes.
/// Dropping keys nothing ever read does not change the connection, so the
/// stashed results are put back.
const RESTORE_RETIRED_TLS_PROBES: &str = "UPDATE profile_ex_items \
     SET delay = r.delay, message = r.message, ip_info = r.ip_info, country_code = r.country_code \
     FROM voya_retired_tls_probes r WHERE profile_ex_items.index_id = r.index_id";

const DROP_RETIRED_TLS_PROBES: &str = "DROP TABLE voya_retired_tls_probes";

/// Runs on every database open, after the baseline has been validated, and is
/// a no-op once nothing retired remains.
///
/// Rows naming a retired transport are deleted: they were always rejected at
/// connect time, and leaving them undecodable would show a permanent "hidden
/// nodes" notice the user could not clear. Retired TLS keys are stripped in
/// place without losing the node's cached delay or country.
pub(crate) async fn normalize_retired_profile_blobs(pool: &sqlx::SqlitePool) -> Result<()> {
    let mut transaction = pool.begin().await?;
    let deleted = sqlx::query(DELETE_RETIRED_TRANSPORTS)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    if deleted > 0 {
        tracing::warn!(deleted, "removed stored nodes that use a retired transport");
    }
    sqlx::query(STASH_RETIRED_TLS_PROBES)
        .execute(&mut *transaction)
        .await?;
    let stripped = sqlx::query(STRIP_RETIRED_TLS_KEYS)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    if stripped > 0 {
        sqlx::query(RESTORE_RETIRED_TLS_PROBES)
            .execute(&mut *transaction)
            .await?;
    }
    sqlx::query(DROP_RETIRED_TLS_PROBES)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
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
    let Ok(config_type) = config_type_value.parse::<ConfigType>() else {
        return Err(DbError::InvalidEnum {
            enum_name: "ConfigType",
            value: config_type_value,
        });
    };
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

fn row_to_profile_ex_joined(row: &SqliteRow) -> Result<ProfileExItem> {
    Ok(ProfileExItem {
        index_id: row.try_get("index_id")?,
        delay: row.try_get("ex_delay")?,
        sort: row.try_get("ex_sort")?,
        message: row.try_get("ex_message")?,
        ip_info: row.try_get("ex_ip_info")?,
        country_code: row.try_get("ex_country_code")?,
    })
}
