use sqlx::{sqlite::SqliteRow, Row};
use voya_core::{ConfigType, ProfileExItem, ProfileIdentity, ProfileItem};

use super::decode_rows;
use crate::{
    blob,
    executor::{delete_each, repository_constructors, row_exists, run_query, RepositoryExecutor},
    DbError, ProfileExRepository, Result,
};

/// The join and sort behind every profile list path, around `$columns` and an
/// optional `$filter`.
///
/// `list`, `list_by_subscription_id` and `list_with_profile_ex` used to repeat
/// this join and sort five times, so a sort fix or a new column had to be
/// applied in five places.
macro_rules! profile_list_query {
    ($columns:literal, $filter:literal) => {
        concat!(
            "SELECT ",
            $columns,
            " FROM profile_items p",
            " LEFT JOIN profile_ex_items e ON p.index_id = e.index_id ",
            $filter,
            " ORDER BY COALESCE(e.sort, 0), p.index_id"
        )
    };
}

/// The full listing. The subscription filter is expressed as a nullable bind
/// instead of a second query string: passing `NULL` selects everything.
const PROFILE_LIST_QUERY: &str = profile_list_query!(
    "p.*, \
     COALESCE(e.delay, 0) AS ex_delay, \
     COALESCE(e.sort, 0) AS ex_sort, \
     e.message AS ex_message, \
     e.ip_info AS ex_ip_info, \
     e.country_code AS ex_country_code",
    "WHERE (? IS NULL OR p.subscription_id = ?)"
);

/// Ids and remarks only, in list order, for the first `?` nodes.
const PROFILE_NAMES_HEAD_QUERY: &str =
    concat!(profile_list_query!("p.index_id, p.remarks", ""), " LIMIT ?");

/// What a policy group reads from each node, in list order.
const PROFILE_IDENTITIES_QUERY: &str =
    profile_list_query!("p.index_id, p.remarks, p.subscription_id", "");

/// The profiles named in a JSON array of ids, in list order.
const PROFILE_BY_IDS_QUERY: &str = profile_list_query!(
    "p.*",
    "WHERE p.index_id IN (SELECT value FROM json_each(?))"
);

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

/// The start of the node list by name, for a menu that shows only that much.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileNamesHead {
    /// `(index_id, remarks)` of the first nodes in list order, then the pinned
    /// node when it sits further down.
    pub names: Vec<(String, String)>,
    /// How many nodes there are in all.
    pub total: usize,
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

    /// The profiles among `index_ids`, in list order. Unknown ids are ignored
    /// and undecodable rows skipped, as in [`Self::list`].
    ///
    /// The ids travel as one JSON array, so the statement stays the same size
    /// however many are asked for, and a selection is read without decoding
    /// every other stored node.
    pub async fn list_by_ids(&self, index_ids: &[String]) -> Result<Vec<ProfileItem>> {
        if index_ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = serde_json::Value::from(index_ids.to_vec()).to_string();
        let rows = run_query!(
            self.executor,
            sqlx::query(PROFILE_BY_IDS_QUERY).bind(ids),
            fetch_all
        )?;

        let (items, _) = decode_rows(
            &rows,
            "index_id",
            "skipping a stored node this build cannot decode",
            "some nodes were hidden because their stored payload could not be decoded",
            row_to_profile,
        )?;
        Ok(items)
    }

    /// Every node's id, remarks and owning subscription, in list order, without
    /// decoding the stored payloads: all that policy group members are
    /// resolved and tagged from. The running group's status reads this every
    /// few seconds, and three JSON blobs per node made that cost grow with the
    /// whole node list.
    ///
    /// Nothing is decoded, so a row this build cannot read is listed too; the
    /// listings skip it, which is the only way the two can differ.
    pub async fn list_identities(&self) -> Result<Vec<ProfileIdentity>> {
        let rows = run_query!(
            self.executor,
            sqlx::query(PROFILE_IDENTITIES_QUERY),
            fetch_all
        )?;

        rows.iter()
            .map(|row| {
                Ok(ProfileIdentity {
                    index_id: row.try_get("index_id")?,
                    remarks: row.try_get("remarks")?,
                    subscription_id: row.try_get("subscription_id")?,
                })
            })
            .collect()
    }

    /// The first `limit` nodes' ids and remarks in list order, plus `pinned`
    /// when it sits further down, and the node count, without decoding the
    /// stored payloads. The tray rebuilds its node menu on every show, hide
    /// and connection change, and shows only the start of the list; reading
    /// every row for it made that cost grow with the whole subscription.
    ///
    /// Unlike the listings, a row this build cannot decode is not skipped:
    /// nothing here is decoded. Activating such a node reports the failure.
    pub async fn list_names_head(
        &self,
        limit: usize,
        pinned: Option<&str>,
    ) -> Result<ProfileNamesHead> {
        let rows = run_query!(
            self.executor,
            sqlx::query(PROFILE_NAMES_HEAD_QUERY).bind(i64::try_from(limit).unwrap_or(i64::MAX)),
            fetch_all
        )?;
        let mut names = rows
            .iter()
            .map(|row| Ok((row.try_get("index_id")?, row.try_get("remarks")?)))
            .collect::<Result<Vec<(String, String)>>>()?;
        if let Some(pinned) = pinned.filter(|id| !names.iter().any(|(name, _)| name == id)) {
            let row = run_query!(
                self.executor,
                sqlx::query("SELECT index_id, remarks FROM profile_items WHERE index_id = ?")
                    .bind(pinned),
                fetch_optional
            )?;
            if let Some(row) = row {
                names.push((row.try_get("index_id")?, row.try_get("remarks")?));
            }
        }
        let total: i64 = run_query!(
            self.executor,
            sqlx::query_scalar("SELECT COUNT(*) FROM profile_items"),
            fetch_one
        )?;

        Ok(ProfileNamesHead {
            names,
            total: usize::try_from(total).unwrap_or(0),
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
