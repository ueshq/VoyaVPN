//! Preserve subscription provenance when adopting imported nodes.
use voya_core::ProfileItem;

pub(super) fn profile_is_adoptable(
    existing: &ProfileItem,
    target_subscription_id: Option<&str>,
) -> bool {
    existing.subscription_id.as_deref() == target_subscription_id
}
