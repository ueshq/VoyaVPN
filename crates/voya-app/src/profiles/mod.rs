mod manager;

use voya_core::ProfileListItem;

pub(crate) use manager::normalize_profile;
pub use manager::{ProfileManager, ProfileManagerError, Result};

const DEFAULT_PROFILE_SORT_STEP: i32 = 10;

/// A profile listing plus the count of stored profiles this build could not
/// read.
///
/// `voya-db` skips a row whose stored payload it cannot decode instead of
/// failing the whole listing, which keeps the rest of the servers usable. The
/// count rides along with the rows so the profiles screen can state it: a list
/// that is quietly short otherwise looks like data loss.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProfileListing {
    pub items: Vec<ProfileListItem>,
    pub undecodable_profiles: usize,
}
