//! Event time stated in a report (spec P3, needed by contradiction).
//!
//! A report's timestamp is its publication; what it describes may be hours earlier.
//! Recognised, in English text, local to the region's time zone:
//!     a clock time   "5:20 a.m.", "11 pm", "05:20 local time", "at 0520 local"
//!     an optional date after it   "on Oct. 21", "on October 21"
//! Without a date, the event day is the publication day, or the day before if the
//! clock time would lie after publication. A dated time after publication is an
//! announcement, not a report, and yields None. Anything vaguer ("overnight", "this
//! morning"), or impossible ("0 pm", "Feb. 30"), yields None: no time is better than
//! a guessed one. Times are compared on the local clock.

use std::sync::LazyLock;

use chrono::{Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use regex::Regex;

use crate::fmt::Time;

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
static CLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?P<h>\d{1,2})(?::(?P<m>\d{2}))?\s*(?P<ap>[ap])\.?\s?m\b\.?|\b(?P<H>[01]?\d|2[0-3]):?(?P<M>[0-5]\d)\s*(?:\(?local(?: time)?\)?)").unwrap()
});
static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\W*(?:local time\W*)?on\s+(?P<mon>[A-Za-z]{3})[a-z]*\.?\s+(?P<d>\d{1,2})\b")
        .unwrap()
});

/// UTC of a local clock time n in zone z: the first of two (a repeated hour), and
/// for a skipped hour the offset in force before the change.
fn utc(z: Tz, n: NaiveDateTime) -> Time {
    match z.from_local_datetime(&n) {
        LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.with_timezone(&Utc),
        LocalResult::None => {
            let before = z
                .from_local_datetime(&(n - Duration::hours(3)))
                .earliest()
                .unwrap();
            let off = chrono::Offset::fix(before.offset()).local_minus_utc();
            Utc.from_utc_datetime(&(n - Duration::seconds(off as i64)))
        }
    }
}

/// UTC time of the first clock time stated in text, or None.
pub fn event_time(text: &str, published: Time, tz: &str) -> Option<Time> {
    let m = CLOCK.captures(text)?;
    let (h, mi) = if let Some(h) = m.name("h") {
        let (h, mi): (u32, u32) = (
            h.as_str().parse().ok()?,
            m.name("m").map_or(Some(0), |x| x.as_str().parse().ok())?,
        );
        if !(1..=12).contains(&h) || mi > 59 {
            return None;
        }
        (
            h % 12
                + if m["ap"].eq_ignore_ascii_case("p") {
                    12
                } else {
                    0
                },
            mi,
        )
    } else {
        (m["H"].parse().ok()?, m["M"].parse().ok()?)
    };
    let z: Tz = tz.parse().ok()?;
    let pub_ = published.with_timezone(&z).naive_local();
    let rest = &text[m.get(0).unwrap().end()..];
    let dated = DATE.captures(rest).and_then(|d| {
        let mon = MONTHS
            .iter()
            .position(|x| x.eq_ignore_ascii_case(&d["mon"]))? as u32
            + 1;
        Some((mon, d["d"].parse::<u32>().ok()))
    });
    let t = match dated {
        Some((mon, day)) => {
            let at = |y: i32| NaiveDate::from_ymd_opt(y, mon, day?)?.and_hms_opt(h, mi, 0);
            let mut t = at(pub_.year())?; // "Feb. 30": None
            if t - pub_ > Duration::days(180) {
                t = at(pub_.year() - 1)?; // "Dec. 31" read on Jan. 2
            }
            if t > pub_ {
                return None; // announced, not yet happened
            }
            t
        }
        None => {
            let mut t = pub_.date().and_hms_opt(h, mi, 0)?;
            if t > pub_ {
                t -= Duration::days(1);
            }
            t
        }
    };
    Some(utc(z, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fmt::fromiso;

    fn at(s: &str, p: &str, tz: &str) -> Option<String> {
        event_time(s, fromiso(p).unwrap(), tz).map(|t| crate::fmt::iso(&t))
    }

    #[test]
    fn stated_times() {
        let p = "2026-10-21T10:00:00+00:00"; // 13:00 in Jerusalem (UTC+3)
        assert_eq!(
            at("Sirens at 5:20 a.m. in Haifa", p, "Asia/Jerusalem").as_deref(),
            Some("2026-10-21T02:20:00+00:00")
        );
        assert_eq!(
            at("Strike at 11 pm", p, "Asia/Jerusalem").as_deref(),
            Some("2026-10-20T20:00:00+00:00")
        );
        assert_eq!(
            at("at 0520 local time on Oct. 20", p, "Asia/Jerusalem").as_deref(),
            Some("2026-10-20T02:20:00+00:00")
        );
        assert_eq!(at("overnight", p, "Asia/Jerusalem"), None);
    }

    #[test]
    fn across_new_year_and_impossible_times() {
        let p = "2026-01-02T10:00:00+00:00";
        assert_eq!(
            at("at 10 p.m. on Dec. 31", p, "Europe/Kyiv").as_deref(),
            Some("2025-12-31T20:00:00+00:00")
        );
        assert_eq!(at("at 0 pm", p, "Europe/Kyiv"), None);
        assert_eq!(at("at 5 a.m. on Feb. 30", p, "Europe/Kyiv"), None);
        assert_eq!(at("at 5 a.m. on Jan. 5", p, "Europe/Kyiv"), None); // announced
    }
}
