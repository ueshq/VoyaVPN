//! Preserve subscription provenance when adopting imported nodes.
use voya_core::ProfileItem;

pub(super) fn profile_is_adoptable(
    existing: &ProfileItem,
    target_subscription_id: Option<&str>,
) -> bool {
    match existing.subscription_id.as_deref() {
        None => true,
        Some(owner) => target_subscription_id == Some(owner),
    }
}
