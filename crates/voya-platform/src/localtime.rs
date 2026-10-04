//! The local calendar day, for counters that reset "today" where the user is
//! rather than at UTC midnight.

use std::mem::MaybeUninit;

/// The local calendar date at `unix_seconds`, as a count of days since
/// 1970-01-01. `None` when the platform cannot convert the time, which leaves
/// the caller to fall back on the UTC day.
#[must_use]
pub fn local_day_number(unix_seconds: i64) -> Option<i64> {
    let tm = local_time(unix_seconds)?;
    Some(days_from_civil(
        i64::from(tm.tm_year) + 1900,
        i64::from(tm.tm_mon) + 1,
        i64::from(tm.tm_mday),
    ))
}

#[cfg(unix)]
fn local_time(unix_seconds: i64) -> Option<libc::tm> {
    let seconds = libc::time_t::try_from(unix_seconds).ok()?;
    let mut tm = MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid for the call, and `localtime_r` writes
    // the whole struct before it returns non-null.
    unsafe {
        if libc::localtime_r(&raw const seconds, tm.as_mut_ptr()).is_null() {
            return None;
        }
        Some(tm.assume_init())
    }
}

#[cfg(windows)]
fn local_time(unix_seconds: i64) -> Option<libc::tm> {
    let seconds = libc::time_t::try_from(unix_seconds).ok()?;
    let mut tm = MaybeUninit::<libc::tm>::uninit();
    // SAFETY: both pointers are valid for the call, and `localtime_s` writes
    // the whole struct when it returns zero.
    unsafe {
        if libc::localtime_s(tm.as_mut_ptr(), &raw const seconds) != 0 {
            return None;
        }
        Some(tm.assume_init())
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_count_days_from_the_unix_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(days_from_civil(2000, 2, 29), 11_016);
        assert_eq!(days_from_civil(2026, 10, 4), 20_730);
    }

    /// Whatever zone the machine is in, the local day is the UTC day or one of
    /// its neighbours, and it never moves backwards as time advances.
    #[test]
    fn the_local_day_stays_within_a_day_of_the_utc_day() {
        let now = 1_791_072_000_i64;
        let local = local_day_number(now).expect("local time");
        assert!((local - now / 86_400).abs() <= 1);
        let later = local_day_number(now + 86_400).expect("local time");
        assert_eq!(later, local + 1);
    }
}
