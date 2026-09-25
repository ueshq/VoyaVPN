mod auto_update;
mod manager;
mod update_flow;
mod use_cases;

/// Wall-clock seconds since the Unix epoch, saturating at zero, shared by the
/// update flow and the auto-update scheduler.
fn unix_now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(0))
}

pub use auto_update::{
    due_subscription_ids, AttemptState, AutoUpdateOutcome, SubscriptionAutoUpdateScheduler,
    SubscriptionAutoUpdateSink,
};
pub use manager::{Result, SubscriptionManager, SubscriptionManagerError};
pub use update_flow::PreparedSubscriptionUpdate;
pub use use_cases::{
    delete_subscriptions_use_case, import_profiles_use_case, save_subscription_use_case,
    update_subscriptions_use_case, SubscriptionUpdate,
};
