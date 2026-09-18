mod policy_group;
mod profile;
mod profile_ex;
mod routing;
mod self_host;
mod server_stat;
mod settings;
mod state;
mod subscription;
mod subscription_metadata;

pub use policy_group::PolicyGroupRepository;
pub(crate) use profile::normalize_retired_profile_blobs;
pub use profile::{ProfileListing, ProfileRepository};
pub use profile_ex::ProfileExRepository;
pub use routing::RoutingRepository;
pub use self_host::SelfHostRepository;
pub use server_stat::ServerStatRepository;
pub(crate) use settings::normalize_retired_settings;
pub use settings::SettingsRepository;
pub use state::{AppStateRecord, AppStateRepository};
pub use subscription::SubscriptionRepository;
pub use subscription_metadata::SubscriptionMetadataRepository;

use sqlx::{sqlite::SqliteRow, Row};

use crate::Result;

/// Decodes a listing's rows, skipping the ones this build cannot read, and
/// returns the decoded rows with the number skipped.
///
/// Stored blobs are strict domain types with no version tag, so a row written
/// by a newer build, or one whose `config_type` column disagrees with its blob,
/// fails on its own. Propagating that would fail every screen and flow that
/// lists the table together, leaving the user unable to see, connect to or even
/// delete what is still readable. Such rows are therefore logged and skipped,
/// while genuine database faults (a missing column, a closed pool) still
/// propagate.
fn decode_rows<T>(
    rows: &[SqliteRow],
    id_column: &str,
    skipped_message: &str,
    hidden_message: &str,
    decode: impl Fn(&SqliteRow) -> Result<T>,
) -> Result<(Vec<T>, usize)> {
    let mut decoded = Vec::with_capacity(rows.len());
    let mut skipped_rows = 0usize;

    for row in rows {
        match decode(row) {
            Ok(item) => decoded.push(item),
            Err(error) if error.is_row_payload() => {
                skipped_rows += 1;
                tracing::warn!(
                    row_id = %row.try_get::<String, _>(id_column).unwrap_or_default(),
                    error = %error,
                    "{skipped_message}"
                );
            }
            Err(error) => return Err(error),
        }
    }

    if skipped_rows > 0 {
        tracing::warn!(skipped_rows, "{hidden_message}");
    }

    Ok((decoded, skipped_rows))
}
