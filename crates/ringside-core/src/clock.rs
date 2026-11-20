//! Timestamps of hand histories and summaries in one zone: this computer's.
//!
//! PokerStars writes the time in the zone the client shows, and the Eastern Time in
//! brackets after it ("20:10:00 CET [2026/11/16 14:10:00 ET]"); a client set to Eastern Time
//! writes only that ("14:10:00 ET"). Both can end up in one database, six hours apart, so a
//! time in Eastern Time alone is converted to local time.

use std::sync::LazyLock;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, Utc, Weekday};
use regex::Regex;

static STAMP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4}/\d{2}/\d{2}) (\d{1,2}):(\d{2}):(\d{2})( ET\b)?").unwrap());

const FORMAT: &str = "%Y/%m/%d %H:%M:%S";

/// The first timestamp in `text` as "YYYY/MM/DD HH:MM:SS" in local time.
pub(crate) fn local_time(text: &str) -> Option<String> {
    local_time_in(text, |utc| utc.with_timezone(&Local).naive_local())
}

/// [`local_time`] with the local zone given as `zone`, which maps a UTC instant to its wall clock.
pub(crate) fn local_time_in(text: &str, zone: impl Fn(DateTime<Utc>) -> NaiveDateTime) -> Option<String> {
    let c = STAMP.captures(text)?;
    let padded = format!("{} {:0>2}:{}:{}", &c[1], &c[2], &c[3], &c[4]);
    if c.get(5).is_none() {
        return Some(padded);
    }
    let Ok(eastern) = NaiveDateTime::parse_from_str(&padded, FORMAT) else { return Some(padded) };
    Some(zone(eastern_to_utc(eastern)).format(FORMAT).to_string())
}

/// US Eastern wall clock to UTC: daylight time (UTC−4) from the second Sunday in March to the
/// first Sunday in November, each at 2:00; the hour that repeats in November counts as daylight time.
fn eastern_to_utc(t: NaiveDateTime) -> DateTime<Utc> {
    let sunday = |month: u32, n: u8| {
        NaiveDate::from_weekday_of_month_opt(t.year(), month, Weekday::Sun, n).and_then(|d| d.and_hms_opt(2, 0, 0))
    };
    let daylight = matches!((sunday(3, 2), sunday(11, 1)), (Some(from), Some(to)) if t >= from && t < to);
    let offset = if daylight { 4 } else { 5 };
    (t + Duration::hours(offset)).and_utc()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn cet(utc: DateTime<Utc>) -> NaiveDateTime {
        utc.with_timezone(&FixedOffset::east_opt(3600).unwrap()).naive_local()
    }

    #[test]
    fn eastern_time_alone_becomes_local() {
        // November 2026: standard time from Sunday the 1st.
        assert_eq!(local_time_in("Level III (35/70) - 2026/11/16 12:46:50 ET", cet).unwrap(), "2026/11/16 18:46:50");
        assert_eq!(local_time_in("2026/11/01 01:30:00 ET", cet).unwrap(), "2026/11/01 06:30:00");
        assert_eq!(local_time_in("2026/11/01 2:00:00 ET", cet).unwrap(), "2026/11/01 08:00:00");
        assert_eq!(local_time_in("2026/07/01 9:05:00 ET", cet).unwrap(), "2026/07/01 14:05:00");
        assert_eq!(local_time_in("2026/03/08 01:59:59 ET", cet).unwrap(), "2026/03/08 07:59:59");
        assert_eq!(local_time_in("2026/03/08 03:00:00 ET", cet).unwrap(), "2026/03/08 08:00:00");
    }

    #[test]
    fn the_clients_own_zone_stays() {
        let both = "2026/11/17 14:24:29 CET [2026/11/17 8:24:29 ET]";
        assert_eq!(local_time_in(both, cet).unwrap(), "2026/11/17 14:24:29");
        assert_eq!(local_time_in("2026/11/16 5:00:45 CET", cet).unwrap(), "2026/11/16 05:00:45");
        assert_eq!(local_time_in("2026/11/16 5:00:45 EET", cet).unwrap(), "2026/11/16 05:00:45");
        assert_eq!(local_time_in("no time", cet), None);
    }

    #[test]
    fn both_forms_of_one_moment_agree() {
        let local = local_time("2026/11/17 8:24:29 ET").unwrap();
        let utc = NaiveDateTime::parse_from_str("2026/11/17 13:24:29", FORMAT).unwrap().and_utc();
        assert_eq!(local, utc.with_timezone(&Local).naive_local().format(FORMAT).to_string());
    }
}
