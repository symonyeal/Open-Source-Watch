//! Recall of the engine's violent events against UCDP Candidate events
//! (docs/VALIDATION.md, UCDP reference).
//!
//! A UCDP event is eligible if its date is exact (date_prec = 1) and it lies within
//! ca. 25 km of its coded point (where_prec ≤ 2), Candidate codebook 1.5, and its
//! window lies inside the engine's collection period for the region. Its window at lag
//! L is [d, d + 1 + L) in the region's local time, d its date: L allows for reports
//! published on the days after. It is recalled if an engine event of a VIOLENT kind
//! starts in the window within DELTA km. L = 0, 1, 2 are all reported; none is chosen.
//! The converse share (engine violent events with an eligible UCDP event in reach) is
//! not precision: UCDP codes only organized violence with at least one death, so an
//! unmatched engine event is not an error.

use chrono::{Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{Value, json};

use crate::fmt::{Time, iso, round};
use crate::fuse::DELTA;
use crate::geo::km;
use crate::regions::Region;
use crate::schema::{Event, Run};
use crate::store;

const VIOLENT: [&str; 3] = [
    "battles",
    "explosions_remote_violence",
    "violence_against_civilians",
];
/// Days.
const LAGS: [i64; 3] = [0, 1, 2];

fn midnight(d: NaiveDate, z: Tz) -> Time {
    let n = d.and_hms_opt(0, 0, 0).unwrap();
    match z.from_local_datetime(&n) {
        LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.with_timezone(&Utc),
        LocalResult::None => {
            let b = z
                .from_local_datetime(&(n - Duration::hours(3)))
                .earliest()
                .unwrap();
            Utc.from_utc_datetime(
                &(n - Duration::seconds(chrono::Offset::fix(b.offset()).local_minus_utc() as i64)),
            )
        }
    }
}

fn eligible(u: &Value) -> bool {
    u["date_prec"].as_i64() == Some(1) && u["where_prec"].as_i64().is_some_and(|w| w <= 2)
}

/// (eligible, recalled, engine events in reach, engine events considered) of UCDP
/// events U and engine events E at lag, within period = (first, last) collection time.
pub fn matches(
    U: &[Value],
    E: &[(Time, f64, f64)],
    tz: &str,
    lag: i64,
    period: (Time, Time),
) -> (usize, usize, usize, usize) {
    let z: Tz = tz.parse().unwrap();
    let W: Vec<(&Value, Time, Time)> = U
        .iter()
        .filter(|u| eligible(u))
        .filter_map(|u| {
            let d =
                NaiveDate::parse_from_str(u["date_start"].as_str()?.get(..10)?, "%Y-%m-%d").ok()?;
            let s = midnight(d, z);
            Some((u, s, s + Duration::days(1 + lag)))
        })
        .filter(|(_, s, e)| period.0 <= *s && *e <= period.1)
        .collect();
    if E.is_empty() {
        return (W.len(), 0, 0, 0);
    }
    let (mut hit, mut reach) = (0, vec![false; E.len()]);
    for (u, s, e) in &W {
        let (a, o) = (
            u["latitude"].as_f64().unwrap_or(f64::NAN),
            u["longitude"].as_f64().unwrap_or(f64::NAN),
        );
        let mut any = false;
        for (k, (t, lat, lon)) in E.iter().enumerate() {
            if t >= s && t < e && km(a, o, *lat, *lon) <= DELTA {
                any = true;
                reach[k] = true;
            }
        }
        hit += any as usize;
    }
    let inside: Vec<bool> = match (W.iter().map(|w| w.1).min(), W.iter().map(|w| w.2).max()) {
        (Some(lo), Some(hi)) => E.iter().map(|(t, _, _)| *t >= lo && *t < hi).collect(),
        _ => vec![false; E.len()],
    };
    let n_reach = reach
        .iter()
        .zip(&inside)
        .filter(|(a, b)| **a && **b)
        .count();
    (W.len(), hit, n_reach, inside.iter().filter(|x| **x).count())
}

/// Recall grid of region r against its UCDP events U (ucdp::events), with the engine's
/// collection period and the UCDP dates it is scored over (no grid without both).
pub fn recall(r: &Region, U: &[Value]) -> Value {
    let R = store::rows::<Run>();
    let T: Vec<Time> = R
        .iter()
        .filter(|x| {
            (x.region == r.id || x.region == "global")
                && (x.status == "ok" || x.status == "empty")
                && !x.source_id.starts_with("ucdp:")
        })
        .map(|x| x.started_at)
        .collect();
    let (first, last) = (T.iter().min().copied(), T.iter().max().copied());
    let E: Vec<(Time, f64, f64)> = store::view::<Event>()
        .iter()
        .filter(|e| e.region == r.id && VIOLENT.contains(&e.kind.as_str()))
        .map(|e| (e.t_start, e.lat, e.lon))
        .collect();
    let dates = |k: &str| {
        U.iter()
            .filter_map(|u| u[k].as_str())
            .map(String::from)
            .collect::<Vec<_>>()
    };
    let mut grid = vec![];
    if let (Some(a), Some(b)) = (first, last) {
        for L in LAGS {
            let (n, k, x, y) = matches(U, &E, &r.timezone, L, (a, b));
            grid.push(json!({"lag_days": L, "eligible": n, "recalled": k,
                             "recall": if n > 0 { json!(round(k as f64 / n as f64, 3)) } else { Value::Null },
                             "engine_events": y, "engine_in_reach": x}));
        }
    }
    json!({"engine_period": [first.map(|t| iso(&t)), last.map(|t| iso(&t))],
           "ucdp_period": [dates("date_start").into_iter().min(), dates("date_end").into_iter().max()],
           "grid": grid})
}
