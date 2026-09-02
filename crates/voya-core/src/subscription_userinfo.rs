//! Parser for the de-facto `subscription-userinfo` response header emitted by
//! subscription servers (`upload=...; download=...; total=...; expire=...`),
//! plus the companion `profile-update-interval` header (a value in hours).

/// Usage figures reported by a subscription server. All fields are optional
/// because servers routinely omit keys they cannot report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubscriptionUserInfo {
    pub upload_bytes: Option<i64>,
    pub download_bytes: Option<i64>,
    pub total_bytes: Option<i64>,
    pub expire_unix_seconds: Option<i64>,
}

impl SubscriptionUserInfo {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.upload_bytes.is_none()
            && self.download_bytes.is_none()
            && self.total_bytes.is_none()
            && self.expire_unix_seconds.is_none()
    }
}

/// Parses a `subscription-userinfo` header value. Returns `None` when no
/// recognized key carries a usable value. `expire=0` is treated as "no
/// expiry", matching how panels use `0` as the unset marker.
#[must_use]
pub fn parse_subscription_userinfo(header: &str) -> Option<SubscriptionUserInfo> {
    let mut info = SubscriptionUserInfo::default();
    for segment in header.split(';') {
        let Some((key, value)) = segment.split_once('=') else {
            continue;
        };
        let value = parse_userinfo_number(value);
        match key.trim().to_ascii_lowercase().as_str() {
            "upload" => info.upload_bytes = value,
            "download" => info.download_bytes = value,
            "total" => info.total_bytes = value,
            "expire" => info.expire_unix_seconds = value.filter(|expire| *expire > 0),
            _ => {}
        }
    }

    if info.is_empty() {
        None
    } else {
        Some(info)
    }
}

/// Parses a `profile-update-interval` header value (hours) into minutes.
/// Non-positive or non-numeric values yield `None`.
#[must_use]
pub fn parse_profile_update_interval_minutes(header: &str) -> Option<i32> {
    let hours = header
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|hours| *hours > 0)?;
    hours.checked_mul(60)
}

fn parse_userinfo_number(value: &str) -> Option<i64> {
    let value = value.trim();
    let parsed = value.parse::<i64>().ok().or_else(|| {
        let float = value.parse::<f64>().ok()?;
        // Some panels emit scientific notation; accept it when it maps onto a
        // whole in-range byte count.
        (float.is_finite() && float >= 0.0 && float <= i64::MAX as f64).then_some(float as i64)
    })?;

    (parsed >= 0).then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_four_keys() {
        let info = parse_subscription_userinfo(
            "upload=455727941; download=6174315083; total=1073741824000; expire=1671815872",
        )
        .expect("full header should parse");
        assert_eq!(info.upload_bytes, Some(455_727_941));
        assert_eq!(info.download_bytes, Some(6_174_315_083));
        assert_eq!(info.total_bytes, Some(1_073_741_824_000));
        assert_eq!(info.expire_unix_seconds, Some(1_671_815_872));
    }

    #[test]
    fn parses_partial_headers_and_ignores_unknown_keys() {
        let info = parse_subscription_userinfo("download=42;custom=9;;upload")
            .expect("partial header should parse");
        assert_eq!(
            info,
            SubscriptionUserInfo {
                download_bytes: Some(42),
                ..SubscriptionUserInfo::default()
            }
        );
    }

    #[test]
    fn expire_zero_means_no_expiry() {
        let info = parse_subscription_userinfo("total=10; expire=0").expect("header should parse");
        assert_eq!(info.total_bytes, Some(10));
        assert_eq!(info.expire_unix_seconds, None);
    }

    #[test]
    fn rejects_garbage_negative_and_overflow_values() {
        assert_eq!(parse_subscription_userinfo("nonsense"), None);
        assert_eq!(parse_subscription_userinfo(""), None);
        assert_eq!(parse_subscription_userinfo("upload=abc; total=-5"), None);
        assert_eq!(
            parse_subscription_userinfo("upload=999999999999999999999999999"),
            None
        );
    }

    #[test]
    fn accepts_scientific_notation_totals() {
        let info =
            parse_subscription_userinfo("total=1.073741824e12").expect("header should parse");
        assert_eq!(info.total_bytes, Some(1_073_741_824_000));
    }

    #[test]
    fn update_interval_converts_hours_to_minutes() {
        assert_eq!(parse_profile_update_interval_minutes("6"), Some(360));
        assert_eq!(parse_profile_update_interval_minutes(" 24 "), Some(1440));
        assert_eq!(parse_profile_update_interval_minutes("0"), None);
        assert_eq!(parse_profile_update_interval_minutes("-3"), None);
        assert_eq!(parse_profile_update_interval_minutes("abc"), None);
    }
}
