//! Signals of a region (/api/signals): GNSS integrity cells (crate::gnss), internet
//! outages (IODA), official daily counts (Taiwan MND) and reference events (UCDP
//! Candidate), each with its dates, units and collection health. Descriptive
//! throughout: nothing here has a threshold.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use serde_json::{Map, Value, json};

use crate::collect::{adapter, ioda, mnd, ucdp};
use crate::cycle::SourceRow;
use crate::fmt::{Time, epoch, iso, round};
use crate::gnss;
use crate::regions::{Indicator, Region};
use crate::validate;

const ADAPTERS: [&str; 5] = ["gpsjam", "integrity", "mnd", "ioda", "ucdp"];
/// Reports whose median the latest is shown beside.
const PRIOR: usize = 30;

/// statistics.median: the middle value, or the mean of the two middle values.
fn median(mut v: Vec<i64>) -> Value {
    if v.is_empty() {
        return Value::Null;
    }
    v.sort();
    let n = v.len();
    if n % 2 == 1 {
        json!(v[n / 2])
    } else {
        json!((v[n / 2 - 1] + v[n / 2]) as f64 / 2.0)
    }
}

/// The region's latest n MND reports, one per reporting day (mnd::reports), and each
/// count's median over the PRIOR reports before the latest; a count a report does
/// not state is left out of its median.
pub fn official(r: &Region, n: usize) -> Value {
    let all = mnd::reports(r);
    let S = &all[all.len().saturating_sub(n)..];
    let prior = &S[S.len().saturating_sub(PRIOR + 1)..S.len().saturating_sub(1)];
    let med: Map<String, Value> = mnd::FIELDS
        .iter()
        .map(|k| {
            (
                k.to_string(),
                median(prior.iter().filter_map(|p| p[*k].as_i64()).collect()),
            )
        })
        .collect();
    let n: Map<String, Value> = mnd::FIELDS
        .iter()
        .map(|k| {
            (
                k.to_string(),
                json!(prior.iter().filter(|p| p[*k].as_i64().is_some()).count()),
            )
        })
        .collect();
    json!({"reports": S, "latest": S.last(), "median": med, "n_prior": prior.len(),
           "n_by_field": n, "baseline_start": prior.first().map(|p| &p["day"]),
           "baseline_end": prior.last().map(|p| &p["day"]),
           "unit": "per 06:00-06:00 (UTC+8) report", "source": "Taiwan Ministry of National Defense"})
}

/// Value of indicator i of metric "official" (crate::analyze): count field(i) of the
/// latest MND report if its day is on or after the date `since` (count and date null
/// otherwise; count null where the report does not state it), beside the count's
/// median over the PRIOR reports before it.
pub fn measure(i: &Indicator, r: &Region, since: NaiveDate) -> Value {
    let o = official(r, PRIOR + 1);
    let f = i.field.clone().unwrap_or_default();
    let p = o["latest"]
        .as_object()
        .filter(|p| p["day"].as_str().unwrap_or("") >= since.to_string().as_str());
    json!({"field": f, "count": p.map_or(Value::Null, |p| p[&f].clone()), "date": p.map_or(Value::Null, |p| p["day"].clone()),
           "median": o["median"][&f], "n_prior": o["n_by_field"][&f]})
}

/// IODA events of the region's countries starting in the last `days`, each at its
/// longest listed duration (ioda::events), newest first.
fn outages(r: &Region, t: Time, days: i64) -> Value {
    let C: Map<String, Value> = r
        .outages
        .countries
        .iter()
        .map(|c| (c.code.clone(), json!(c.name)))
        .collect();
    let mut E = ioda::events(r, t - Duration::days(days));
    E.sort_by_key(|e| -e.0["start"].as_i64().unwrap_or(0));
    let ev: Vec<Value> = E
        .into_iter()
        .map(|(mut p, url, fetched)| {
            let start = p["start"].as_i64().unwrap_or(0);
            let name = C
                .get(p["country"].as_str().unwrap_or(""))
                .cloned()
                .unwrap_or(p["name"].clone());
            p["url"] = json!(url);
            p["fetched_at"] = json!(iso(&fetched));
            p["country_name"] = name;
            p["start_utc"] = json!(iso(&epoch(start as f64)));
            p["hours"] = json!(round(p["duration"].as_f64().unwrap_or(0.0) / 3600.0, 1));
            p
        })
        .collect();
    json!({"countries": C, "events": ev})
}

/// The region's UCDP Candidate events (latest version of each): totals by month,
/// country and type of violence, and the recall grid of crate::validate.
pub fn reference(r: &Region) -> Value {
    let U = ucdp::events(r);
    if U.is_empty() {
        return Value::Null;
    }
    let mut month: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    let mut country: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    let mut types: Vec<(&str, i64)> = vec![];
    for u in &U {
        let best = u["best"].as_i64().unwrap_or(0);
        for (m, k) in [
            (
                &mut month,
                u["date_start"]
                    .as_str()
                    .unwrap_or("")
                    .get(..7)
                    .unwrap_or("")
                    .to_string(),
            ),
            (
                &mut country,
                u["country"].as_str().unwrap_or("").to_string(),
            ),
        ] {
            let e = m.entry(k).or_default();
            e.0 += 1;
            e.1 += best;
        }
        let ty = match u["type_of_violence"].as_i64() {
            Some(1) => "state-based",
            Some(2) => "non-state",
            _ => "one-sided",
        };
        match types.iter_mut().find(|x| x.0 == ty) {
            Some(x) => x.1 += 1,
            None => types.push((ty, 1)),
        }
    }
    let group = |m: BTreeMap<String, (i64, i64)>| -> Map<String, Value> {
        m.into_iter()
            .map(|(k, (n, d))| (k, json!({"events": n, "deaths_best": d})))
            .collect()
    };
    let mut versions: Vec<&str> = U.iter().filter_map(|u| u["version"].as_str()).collect();
    versions.sort();
    versions.dedup();
    let ok = U
        .iter()
        .filter(|u| {
            u["date_prec"].as_i64() == Some(1) && u["where_prec"].as_i64().is_some_and(|w| w <= 2)
        })
        .count();
    json!({"versions": versions, "events": U.len(),
           "first": U.iter().filter_map(|u| u["date_start"].as_str()).min(),
           "last": U.iter().filter_map(|u| u["date_end"].as_str()).max(),
           "by_month": group(month), "by_country": group(country),
           "by_type": types.into_iter().map(|(k, n)| (k.to_string(), json!(n))).collect::<Map<_, _>>(),
           "eligible_share": round(ok as f64 / U.len() as f64, 3), "recall": validate::recall(r, &U)})
}

/// The signals of region r; `sources` is its source table (cycle::source_table).
pub fn view(r: &Region, sources: &[SourceRow], t: Time) -> Value {
    let claims = sources.iter().any(|s| adapter(&s.id) == "mnd");
    json!({"region": r.id, "gnss": gnss::view(r, t, 7), "official": if claims { official(r, 90) } else { Value::Null },
           "outages": outages(r, t, 7), "reference": reference(r),
           "sources": sources.iter().filter(|s| ADAPTERS.contains(&adapter(&s.id))).collect::<Vec<_>>(),
           "updated": iso(&t)})
}
