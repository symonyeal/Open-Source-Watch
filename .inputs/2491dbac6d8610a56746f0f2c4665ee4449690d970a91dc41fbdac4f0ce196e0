//! GNSS integrity cells per region and UTC day (docs/DECISIONS.md, GNSS).
//!
//! Cell rule (gpsjam.org FAQ, docs/SOURCES.md): with g and b the aircraft that flew
//! through an H3 resolution-4 cell in one UTC day and reported good and bad
//! navigation accuracy,
//!     p = 100 max(0, b − 1) / (g + b);   low if p < 2, medium if 2 ≤ p ≤ 10, high if p > 10.
//! gpsjam subtracts one bad aircraft to limit false positives where few flew (its
//! formula has no max; the max changes no band).
//! Per-aircraft rule for the engine's own adsb.lol snapshots (collect::integrity), as
//! gods-eye-view applies the US ADS-B Out minima of 14 CFR 91.227(c); it is not a
//! validated interference classifier and gpsjam does not publish its own:
//!     bad ⇔ GPS lost, or NACp < 8, or (NIC < 7 and not (NIC = 0 and NACp ≥ 10)).
//! A medium or high cell is a navigation-integrity anomaly. Jamming or spoofing is
//! one cause, avionics faults another, so neither is stated as the finding.

use std::collections::{BTreeMap, HashMap};

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use serde_json::{Value, json};

use crate::collect::stored;
use crate::fmt::{Time, round};
use crate::geo;
use crate::process::nearest;
use crate::regions::{Indicator, Region};

const RES: u8 = 4;
pub const BANDS: [&str; 3] = ["low", "medium", "high"];
/// Population of a city that names a cell (regions/geonames.tsv).
const TOWN: i64 = 100_000;

pub fn pct(g: i64, b: i64) -> f64 {
    if g + b == 0 {
        0.0
    } else {
        100.0 * (b - 1).max(0) as f64 / (g + b) as f64
    }
}

pub fn band(p: f64) -> &'static str {
    if p > 10.0 {
        "high"
    } else if p >= 2.0 {
        "medium"
    } else {
        "low"
    }
}

/// The per-aircraft rule; None stands for a field the aircraft did not report.
pub fn bad(nic: Option<f64>, nacp: Option<f64>, lost: bool) -> bool {
    lost || nacp.is_some_and(|a| a < 8.0)
        || nic.is_some_and(|n| n < 7.0 && !(n == 0.0 && nacp.unwrap_or(0.0) >= 10.0))
}

/// A cell row [hex, good, bad].
type Cell = (String, i64, i64);

fn cells(v: &Value) -> Vec<Cell> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            (
                c[0].as_str().unwrap_or("").to_string(),
                c[1].as_i64().unwrap_or(0),
                c[2].as_i64().unwrap_or(0),
            )
        })
        .collect()
}

pub fn midnight(d: NaiveDate) -> Time {
    Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
}

/// {day: payload} of gpsjam.org cells stored for region r since t0.
fn gpsjam(r: &Region, t0: Time) -> BTreeMap<String, Value> {
    stored(&["gpsjam"], &[&r.id], Some(t0))
        .into_iter()
        .map(|x| {
            (
                x.payload["date"].as_str().unwrap_or("").to_string(),
                x.payload,
            )
        })
        .collect()
}

/// {day: cells} from the engine's own adsb.lol snapshots since t0: each aircraft
/// counted once per cell-day, bad if any of its reports there was (geo::aircraft_counts).
pub fn adsb(r: &Region, t0: Time) -> BTreeMap<String, Vec<Cell>> {
    let mut X = vec![];
    for x in stored(&["integrity"], &[&r.id], Some(t0)) {
        let d = x.published_at.unwrap_or(x.fetched_at).date_naive();
        for a in x.payload["ac"].as_array().into_iter().flatten() {
            let id = a[0].as_str().unwrap_or("");
            // A '~' address is not an ICAO address (TIS-B, non-ICAO); bit 24 keeps them apart.
            let Ok(icao) = u32::from_str_radix(id.trim_start_matches('~'), 16) else {
                continue;
            };
            let icao = icao | ((id.starts_with('~') as u32) << 24);
            let Some(h) = geo::cell(
                a[1].as_f64().unwrap_or(f64::NAN),
                a[2].as_f64().unwrap_or(f64::NAN),
                RES,
            ) else {
                continue;
            };
            X.push((
                d,
                h,
                icao,
                bad(
                    a[3].as_f64(),
                    a[4].as_f64(),
                    a[5].as_bool().unwrap_or(false),
                ),
            ));
        }
    }
    let mut out: BTreeMap<String, Vec<Cell>> = BTreeMap::new();
    for (d, h, g, b) in geo::aircraft_counts(X) {
        out.entry(d.to_string())
            .or_default()
            .push((format!("{h:x}"), g, b));
    }
    out
}

fn summary(day: &str, C: &[Cell], source: &str, suspect: bool) -> Value {
    let mut n: HashMap<&str, i64> = HashMap::new();
    for (_, g, b) in C {
        *n.entry(band(pct(*g, *b))).or_default() += 1;
    }
    json!({"date": day, "source": source, "suspect": suspect, "cells": C.len(),
           "low": n.get("low").unwrap_or(&0), "medium": n.get("medium").unwrap_or(&0), "high": n.get("high").unwrap_or(&0)})
}

/// Medium and high cells with their centres, boundaries and nearest city.
fn anomalies(C: &[Cell]) -> Vec<Value> {
    C.iter()
        .filter(|(_, g, b)| band(pct(*g, *b)) != "low")
        .filter_map(|(h, g, b)| {
            let (a, o) = geo::centre(h)?;
            let p = pct(*g, *b);
            Some(json!({"h3": h, "lat": round(a, 4), "lon": round(o, 4), "near": nearest(a, o, TOWN), "good": g,
                        "bad": b, "pct": round(p, 1), "band": band(p),
                        "boundary": geo::boundary(h).iter().map(|(y, x)| json!([round(*y, 4), round(*x, 4)])).collect::<Vec<_>>()}))
        })
        .collect()
}

/// Cell-days present in both gpsjam (J) and the engine's snapshots (A), counted by
/// whether each calls the cell an anomaly (medium or high). The engine samples every
/// CADENCE seconds where gpsjam sees the whole day, so this measures agreement, not
/// either one's accuracy.
pub fn agreement(J: &BTreeMap<String, Value>, A: &BTreeMap<String, Vec<Cell>>) -> Value {
    let (mut both, mut gj, mut en, mut neither) = (0, 0, 0, 0);
    for (d, cs) in A {
        let Some(p) = J.get(d) else { continue };
        let j: HashMap<String, bool> = cells(&p["cells"])
            .into_iter()
            .map(|(h, g, b)| (h, band(pct(g, b)) != "low"))
            .collect();
        for (h, g, b) in cs {
            if let Some(&jh) = j.get(h) {
                let e = band(pct(*g, *b)) != "low";
                match (jh, e) {
                    (true, true) => both += 1,
                    (true, false) => gj += 1,
                    (false, true) => en += 1,
                    _ => neither += 1,
                }
            }
        }
    }
    json!({"both": both, "gpsjam": gj, "engine": en, "neither": neither})
}

/// Daily band counts from both sources over `days`, the latest gpsjam day's anomaly
/// cells (the map), and the agreement of the two sources.
pub fn view(r: &Region, t: Time, days: i64) -> Value {
    let t0 = midnight(t.date_naive() - Duration::days(days));
    let (J, A) = (gpsjam(r, t0), adsb(r, t0));
    let last = J.keys().next_back().cloned();
    let mut D: Vec<Value> = J
        .iter()
        .map(|(d, p)| {
            summary(
                d,
                &cells(&p["cells"]),
                "gpsjam.org",
                p["suspect"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    D.extend(A.iter().map(|(d, c)| summary(d, c, "adsb.lol", false)));
    json!({"days": D,
           "map": {"date": last, "source": "gpsjam.org",
                   "cells": last.as_ref().map_or(vec![], |d| anomalies(&cells(&J[d]["cells"])))},
           "agreement": agreement(&J, &A)})
}

/// Value of indicator i of metric "gnss" (crate::analyze): the cells at band(i) or
/// above on the latest gpsjam day on or after the date `since`; date None if none is
/// stored.
pub fn measure(i: &Indicator, r: &Region, since: NaiveDate) -> Value {
    let J = gpsjam(r, midnight(since));
    let k = BANDS
        .iter()
        .position(|b| Some(*b) == i.band.as_deref())
        .unwrap_or(0);
    let last = J.keys().next_back();
    let n = last.map_or(0, |d| {
        cells(&J[d]["cells"])
            .iter()
            .filter(|(_, g, b)| BANDS.iter().position(|x| *x == band(pct(*g, *b))).unwrap() >= k)
            .count()
    });
    json!({"count": n, "date": last, "band": BANDS[k]})
}
