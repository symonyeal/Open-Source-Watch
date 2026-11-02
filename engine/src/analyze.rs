//! Analysis, stage 5: indicators, transits, PIR status, competing hypotheses.
//!
//! Event indicator i: the events of kind(i) with credibility ≤ c(i) whose t_end lies
//! in the last window(i) hours are counted;
//!     count ≥ trigger(i) → triggered;  count ≥ watch(i) → watch;  else inactive.
//! Transit indicator i (metric "transit"): observed crossings of gate(i) per UTC day by
//! AIS tracks; the last complete day's count n is compared with the window(i) days
//! before it, z = (n − mean) / sd;
//!     z ≤ trigger_z → triggered;  z ≤ watch_z → watch;  else inactive;
//! A day has sampled AIS coverage when a valid consecutive segment has an endpoint on
//! it; this does not establish full-day coverage. Fewer than 7 prior sampled days, or
//! no sampled coverage on the last day, gives no data.
//! Descriptive indicators (metric "gnss": crate::gnss; "official": crate::signals)
//! report a measured or claimed value without a threshold; no data if no healthy
//! source produces it, its latest value is dated before RECENT, or the value is not
//! stated (count null).
//! A PIR is answered if one indicator is triggered; a collection gap if, for some
//! indicator, no healthy source can produce its evidence (PRODUCERS); otherwise open.
//! A kind that did not occur is not a gap.
//! ACH (Heuer 1999, ch. 8): evidence = indicators, present iff watch or triggered,
//! absent iff inactive; a hypothesis's score is the number of items whose state
//! contradicts its expectation; hypotheses are ranked by fewest inconsistencies.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{Duration, NaiveDate};
use serde_json::{Value, json};

use crate::collect::adapter;
use crate::fmt::{Time, round};
use crate::gnss::{self, midnight};
use crate::np;
use crate::regions::{Indicator, Pir, Region};
use crate::schema::{Event, Obs, Status, Track};
use crate::signals;
use crate::store;

const MIN_DAYS: usize = 7;
/// Analysis policy; not validated against live coverage.
const MAX_AIS_GAP: i64 = 15 * 60;
const TEXT: [&str; 3] = ["rss", "gnews", "telegram"];
/// A descriptive value dated earlier is no data.
const RECENT: i64 = 3;

/// Adapters whose sources can produce each kind; the rest come from text sources.
fn producers(kind: &str) -> Vec<&'static str> {
    match kind {
        "thermal_anomaly" => vec!["firms"],
        "airspace_activity" => [&["drones"][..], &TEXT].concat(),
        "air_raid_alert" => [&["alerts"][..], &TEXT].concat(),
        "internet_outage" => vec!["ioda"],
        _ => TEXT.to_vec(),
    }
}

/// Descriptive metrics: measure(i, r, since) -> {count, date, ...}, and their adapters.
fn measure(
    metric: &str,
) -> Option<(
    fn(&Indicator, &Region, NaiveDate) -> Value,
    &'static [&'static str],
)> {
    match metric {
        "gnss" => Some((gnss::measure, &["gpsjam"])),
        "official" => Some((signals::measure, &["mnd"])),
        _ => None,
    }
}

/// Health of each source id serving a region: ok | stale | failed | unknown.
pub type Health = HashMap<String, &'static str>;

/// Ids of the region's sources of adapters A.
fn serving(A: &[&str], h: &Health) -> Vec<String> {
    let mut S: Vec<String> = h
        .keys()
        .filter(|s| A.contains(&adapter(s)))
        .cloned()
        .collect();
    S.sort();
    S
}

/// No healthy source of the region belongs to adapters A (spec § Errors).
fn gap(A: &[&str], h: &Health) -> bool {
    !serving(A, h).iter().any(|s| h[s] == "ok")
}

pub fn status(n: i64, watch: i64, trigger: i64) -> &'static str {
    if n >= trigger {
        "triggered"
    } else if n >= watch {
        "watch"
    } else {
        "inactive"
    }
}

/// Sign of the turn u → v → w (cross product).
fn orient(u: [f64; 2], v: [f64; 2], w: [f64; 2]) -> f64 {
    let c = (v[0] - u[0]) * (w[1] - u[1]) - (v[1] - u[1]) * (w[0] - u[0]);
    if c > 0.0 {
        1.0
    } else if c < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Segment pq properly crosses segment ab (planar lat/lon, short segments).
pub fn crosses(p: [f64; 2], q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> bool {
    orient(p, q, a) * orient(p, q, b) < 0.0 && orient(a, b, p) * orient(a, b, q) < 0.0
}

/// ({day: observed crossings}, {days with sampled AIS coverage}) over the days + 1
/// complete UTC days before t's day. Sampled coverage is not full-day uptime.
pub fn daily_transits(
    r: &Region,
    gate: &str,
    t: Time,
    days: i64,
) -> (HashMap<NaiveDate, i64>, HashSet<NaiveDate>) {
    let (t0, t1) = (
        midnight(t.date_naive() - Duration::days(days + 1)),
        midnight(t.date_naive()),
    );
    let mut V: BTreeMap<&str, Vec<&Track>> = BTreeMap::new();
    let T = store::rows::<Track>();
    for x in T
        .iter()
        .filter(|x| x.region == r.id && x.entity_id.starts_with("mmsi:") && x.t >= t0 && x.t < t1)
    {
        V.entry(&x.entity_id).or_default().push(x);
    }
    let line = r.gate.get(gate).map(|g| g.line).unwrap_or_default();
    let (mut C, mut covered) = (HashMap::new(), HashSet::new());
    for P in V.values_mut() {
        P.sort_by_key(|x| x.t);
        for w in P.windows(2) {
            let (x, y) = (w[0], w[1]);
            let dt = (y.t - x.t).num_microseconds().unwrap_or(i64::MAX);
            if !(dt > 0 && dt <= MAX_AIS_GAP * 1_000_000) {
                continue;
            }
            covered.insert(x.t.date_naive());
            covered.insert(y.t.date_naive());
            if crosses([x.lat, x.lon], [y.lat, y.lon], line[0], line[1]) {
                *C.entry(y.t.date_naive()).or_default() += 1;
            }
        }
    }
    (C, covered)
}

fn transit(r: &Region, i: &Indicator, t: Time) -> Value {
    let days = i.window_days.unwrap_or(0);
    let (C, covered) = daily_transits(r, i.gate.as_deref().unwrap_or(""), t, days);
    let last = t.date_naive() - Duration::days(1);
    let x: Vec<f64> = (1..=days)
        .map(|k| last - Duration::days(k))
        .filter(|d| covered.contains(d))
        .map(|d| *C.get(&d).unwrap_or(&0) as f64)
        .collect();
    let n = *C.get(&last).unwrap_or(&0);
    let mut out = json!({"id": i.id, "name": i.name, "metric": "transit", "count": n, "window_days": days,
                         "watch_z": i.watch_z, "trigger_z": i.trigger_z, "sources": ["ais:aisstream"]});
    if x.len() < MIN_DAYS || !covered.contains(&last) {
        out["z"] = Value::Null;
        out["status"] = json!("no data");
        return out;
    }
    let (m, sd) = (np::mean(&x), np::sd(&x));
    let d = n as f64 - m;
    let z = if sd == 0.0 && d == 0.0 {
        0.0
    } else if sd != 0.0 {
        d / sd
    } else {
        d.signum() * f64::INFINITY
    };
    let s = if z <= i.trigger_z.unwrap_or(f64::NEG_INFINITY) {
        "triggered"
    } else if z <= i.watch_z.unwrap_or(f64::NEG_INFINITY) {
        "watch"
    } else {
        "inactive"
    };
    out["z"] = if z.is_finite() {
        json!(round(z, 2))
    } else {
        Value::Null
    };
    out["status"] = json!(s);
    out["baseline_mean"] = json!(m);
    out["baseline_sd"] = json!(sd);
    out
}

/// Hypotheses of PIR p ranked by inconsistency with indicator states I.
pub fn ach(p: &Pir, I: &[Value]) -> Vec<Value> {
    let state: HashMap<&str, Option<&str>> = I
        .iter()
        .map(|i| {
            (
                i["id"].as_str().unwrap_or(""),
                match i["status"].as_str().unwrap_or("") {
                    "watch" | "triggered" => Some("present"),
                    "inactive" => Some("absent"),
                    _ => None,
                },
            )
        })
        .collect();
    let mut H: Vec<(usize, String, Value)> = p
        .hypothesis
        .iter()
        .map(|h| {
            let E = |k: &str| state.get(k).copied().flatten();
            let bad: Vec<&String> = h
                .expect
                .iter()
                .filter(|(k, v)| E(k).is_some_and(|e| v.as_str() != Some(e)))
                .map(|x| x.0)
                .collect();
            let un: Vec<&String> = h.expect.keys().filter(|k| E(k).is_none()).collect();
            (
                bad.len(),
                h.id.clone(),
                json!({"id": h.id, "text": h.text, "inconsistent": bad, "unassessed": un}),
            )
        })
        .collect();
    H.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    H.into_iter().map(|h| h.2).collect()
}

fn merge(a: Value, b: &Value) -> Value {
    let mut m = a.as_object().cloned().unwrap_or_default();
    for (k, v) in b.as_object().into_iter().flatten() {
        m.insert(k.clone(), v.clone());
    }
    Value::Object(m)
}

/// [{pir, question, status, indicators, hypotheses}] of region r at time t.
pub fn board(r: &Region, h: &Health, t: Time) -> Vec<Value> {
    let hours = r
        .pir
        .iter()
        .flat_map(|p| &p.indicator)
        .map(|i| i.window_h.unwrap_or(0))
        .max()
        .unwrap_or(0);
    let EV = store::view::<Event>();
    let E: Vec<&Event> = EV
        .iter()
        .filter(|e| e.region == r.id && e.t_end > t - Duration::hours(hours) && e.t_end <= t)
        .collect();
    let OV = store::view::<Obs>();
    let P: BTreeSet<(&str, &str)> = OV
        .iter()
        .filter(|x| x.region == r.id && x.t > t - Duration::days(7))
        .map(|x| (x.kind.as_str(), x.source_id.as_str()))
        .collect();
    let mut out: Vec<(Value, Vec<Value>)> = vec![];
    for p in &r.pir {
        let (mut I, mut miss) = (vec![], false);
        for i in &p.indicator {
            let metric = i.metric.as_deref().unwrap_or("");
            let x = if let Some((f, A)) = measure(metric) {
                let x = f(i, r, t.date_naive() - Duration::days(RECENT));
                let missing = gap(A, h) || x["date"].is_null() || x["count"].is_null();
                miss |= missing;
                merge(
                    merge(json!({"id": i.id, "name": i.name, "metric": metric}), &x),
                    &json!({"status": if missing { "no data" } else { "observed" }, "sources": serving(A, h)}),
                )
            } else if metric == "transit" {
                let mut x = transit(r, i, t);
                let missing =
                    h.get("ais:aisstream").copied() != Some("ok") || x["status"] == "no data";
                miss |= missing;
                if missing {
                    x["status"] = json!("no data");
                }
                x
            } else {
                let kind = i.kind.as_deref().unwrap_or("");
                let w = t - Duration::hours(i.window_h.unwrap_or(0));
                let matched: Vec<&&Event> = E
                    .iter()
                    .filter(|e| {
                        e.kind == kind
                            && e.credibility as i64 <= i.max_credibility.unwrap_or(6)
                            && e.t_end > w
                    })
                    .collect();
                let n = matched.len() as i64;
                let S: Vec<&str> = P
                    .iter()
                    .filter(|x| x.0 == kind)
                    .map(|x| x.1)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                let missing = gap(&producers(kind), h);
                miss |= missing;
                let descriptive = metric == "event_count";
                let state = if descriptive {
                    "observed"
                } else {
                    status(n, i.watch.unwrap_or(0), i.trigger.unwrap_or(0))
                };
                let mut x = json!({"id": i.id, "name": i.name, "kind": kind, "count": n, "window_h": i.window_h,
                                   "status": if missing { "no data" } else { state },
                                   "best_credibility": matched.iter().map(|e| e.credibility).min(), "sources": S});
                if descriptive {
                    x["metric"] = json!("event_count");
                } else {
                    x["watch"] = json!(i.watch);
                    x["trigger"] = json!(i.trigger);
                }
                x
            };
            I.push(x);
        }
        let s = if I.iter().any(|x| x["status"] == "triggered") {
            "answered"
        } else if miss {
            "gap"
        } else {
            "open"
        };
        out.push((json!({"pir": p.id, "question": p.question, "status": s}), I));
    }
    let all: Vec<Value> = out.iter().flat_map(|x| x.1.clone()).collect(); // hypotheses may cite any PIR
    out.into_iter()
        .zip(&r.pir)
        .map(|((mut x, I), p)| {
            x["indicators"] = json!(I);
            x["hypotheses"] = json!(ach(p, &all));
            x
        })
        .collect()
}

/// indicator_status rows of board B at time t (the indicator board's history).
pub fn snapshot(r: &Region, B: &[Value], t: Time) -> Vec<Status> {
    B.iter()
        .flat_map(|p| {
            p["indicators"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(move |i| Status {
                    region: r.id.clone(),
                    t,
                    pir: p["pir"].as_str().unwrap_or("").into(),
                    indicator: i["id"].as_str().unwrap_or("").into(),
                    status: i["status"].as_str().unwrap_or("").into(),
                    count: i["count"].as_i64().map(|c| c as i32),
                })
        })
        .collect()
}
