//! Dated energy/trade context, separate from incident-based PIR judgements.
//!
//! Shipping follows the World Bank chokepoints notebook's centered seven-day mean,
//! restricted to complete calendar windows. The center date lags the last sample by
//! three days. Missing observations never become zeros.

use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate};
use serde_json::{Map, Value, json};

use crate::collect::Report;
use crate::collect::context::history;
use crate::cycle;
use crate::fmt::{Time, iso, round};
use crate::regions::Region;

fn date(s: &Value) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.as_str()?, "%Y-%m-%d").ok()
}

pub fn mean7(points: &[Value], field: &str) -> Value {
    let Some(end) = points.iter().filter_map(|p| date(&p["date"])).max() else {
        return Value::Null;
    };
    let values: HashMap<&str, &Value> = points
        .iter()
        .map(|p| (p["date"].as_str().unwrap_or(""), &p[field]))
        .collect();
    let x: Option<Vec<f64>> = (0..7)
        .map(|k| {
            values
                .get((end - Duration::days(k)).to_string().as_str())?
                .as_f64()
        })
        .collect();
    let Some(x) = x else { return Value::Null };
    json!({"mean": round(x.iter().fold(0.0, |s, v| s + v) / 7.0, 2), "n": 7,
           "center_date": (end - Duration::days(3)).to_string(), "window_start": (end - Duration::days(6)).to_string(),
           "window_end": end.to_string()})
}

/// Current complete week versus the four preceding complete, nonoverlapping weeks.
/// A moving descriptive baseline, not a seasonal forecast or a disruption threshold.
pub fn comparison(points: &[Value], field: &str) -> Value {
    let current = mean7(points, field);
    let Some(end) = date(&current["window_end"]) else {
        return Value::Null;
    };
    let weeks: Vec<Value> = (1..=4)
        .map(|k| {
            let stop = end - Duration::days(7 * k);
            let p: Vec<Value> = points
                .iter()
                .filter(|p| date(&p["date"]).is_some_and(|d| d <= stop))
                .cloned()
                .collect();
            let m = mean7(&p, field);
            if date(&m["window_end"]) == Some(stop) {
                m
            } else {
                Value::Null
            }
        })
        .collect();
    let mut means: Vec<f64> = weeks.iter().filter_map(|w| w["mean"].as_f64()).collect();
    means.sort_by(f64::total_cmp);
    let median = (means.len() == 4).then(|| (means[1] + means[2]) / 2.0);
    let change = median
        .filter(|m| *m > 0.0)
        .map(|m| round(100.0 * (current["mean"].as_f64().unwrap() / m - 1.0), 2));
    json!({"current": current, "baseline_weeks": weeks, "n_baseline": means.len(),
        "baseline_median": median, "change_pct": change,
        "status": if median.is_some() { "complete" } else { "insufficient_history" },
        "method": "complete-week-vs-prior-four-week-median"})
}

/// Compare overlapping observation keys; additions/expirations are not revisions.
/// Withdrawn values (number -> null) and restored values (null -> number) count.
fn revision(previous: &Report, current: &Report) -> Value {
    let key = |p: &Value| {
        (
            p["country"].as_str().unwrap_or("").to_string(),
            p["date"].as_str().unwrap_or("").to_string(),
        )
    };
    let old: HashMap<_, _> = previous.payload["points"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| (key(p), p))
        .collect();
    let fields: &[&str] = if ["port", "chokepoint"]
        .iter()
        .any(|k| current.payload["kind"] == *k)
    {
        &["value", "tanker"]
    } else {
        &["value"]
    };
    let mut compared = 0;
    let mut changed = vec![];
    for p in current.payload["points"].as_array().into_iter().flatten() {
        if let Some(q) = old.get(&key(p)) {
            compared += 1;
            for field in fields {
                if p[*field] != q[*field] {
                    changed.push(json!({"country": p["country"], "date": p["date"],
                        "field": field, "previous": q[*field], "current": p[*field]}));
                }
            }
        }
    }
    json!({"previous_raw_id": previous.id, "previous_fetched_at": iso(&previous.fetched_at),
        "current_raw_id": current.id, "compared_points": compared,
        "changed_measures": changed.len(), "changes": changed})
}

pub fn series(payload: &Value, fetched: Time, health: &str, t: Time) -> Value {
    let mut out: Map<String, Value> = payload.as_object().cloned().unwrap_or_default();
    out.remove("points");
    out.insert("fetched_at".into(), json!(iso(&fetched)));
    out.insert("health".into(), json!(health));
    let P = payload["points"].as_array().cloned().unwrap_or_default();
    if payload["kind"] == "wdi" {
        let mut latest: Map<String, Value> = Map::new();
        for p in P.iter().filter(|p| !p["value"].is_null()) {
            let (c, d) = (
                p["country"].as_str().unwrap_or(""),
                p["date"].as_str().unwrap_or(""),
            );
            if d.parse::<i32>().is_ok_and(|y| y <= t.year())
                && latest
                    .get(c)
                    .is_none_or(|q| d > q["date"].as_str().unwrap_or(""))
            {
                latest.insert(c.into(), p.clone());
            }
        }
        out.insert("countries".into(), Value::Object(latest));
        return Value::Object(out);
    }
    let today = t.date_naive().to_string();
    let mut points: Vec<Value> = P
        .into_iter()
        .filter(|p| p["date"].as_str().unwrap_or("") < today.as_str())
        .collect();
    points.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    let valid: Vec<&Value> = points.iter().filter(|p| !p["value"].is_null()).collect();
    let latest = valid.last().copied();
    let age = latest
        .and_then(|p| date(&p["date"]))
        .map(|d| (t.date_naive() - d).num_days());
    out.insert("latest".into(), latest.cloned().unwrap_or(Value::Null));
    out.insert("age_days".into(), json!(age));
    if payload["kind"] == "fred" {
        let prev = (valid.len() > 1)
            .then(|| valid[valid.len() - 2]["value"].as_f64())
            .flatten()
            .filter(|v| *v != 0.0);
        let change = prev.map(|v| {
            round(
                100.0 * (latest.unwrap()["value"].as_f64().unwrap() / v - 1.0),
                2,
            )
        });
        out.insert("points".into(), json!(points));
        out.insert("change_pct".into(), json!(change));
    } else {
        let (m, k) = (mean7(&points, "value"), mean7(&points, "tanker"));
        out.insert("comparison".into(), comparison(&points, "value"));
        out.insert("tanker_comparison".into(), comparison(&points, "tanker"));
        out.insert("points".into(), json!(points));
        out.insert("mean7".into(), m);
        out.insert("tanker_mean7".into(), k);
    }
    Value::Object(out)
}

pub fn view(r: &Region, t: Time) -> Value {
    let sources: Vec<cycle::SourceRow> = cycle::source_table(r, t)
        .into_iter()
        .filter(|s| s.id.starts_with("context:"))
        .collect();
    let health: HashMap<(&str, &str), &str> = sources
        .iter()
        .map(|s| ((s.region.as_str(), s.id.as_str()), s.health))
        .collect();
    let mut data: Vec<Value> = history(r, t)
        .iter()
        .map(|releases| {
            let x = releases.last().unwrap();
            let mut s = series(
                &x.payload,
                x.fetched_at,
                health
                    .get(&(x.region.as_str(), x.source_id.as_str()))
                    .unwrap_or(&"unknown"),
                t,
            );
            s["raw_id"] = json!(x.id);
            s["releases_known"] = json!(releases.len());
            s["revision"] = releases
                .iter()
                .rev()
                .nth(1)
                .map_or(Value::Null, |p| revision(p, x));
            s
        })
        .collect();
    let price = |code: &str| {
        data.iter()
            .find(|x| x["kind"] == "fred" && x["code"] == code)
            .cloned()
    };
    let values = |s: &Value| -> HashMap<String, f64> {
        s["points"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| Some((p["date"].as_str()?.to_string(), p["value"].as_f64()?)))
            .collect()
    };
    let spread = match (price("DCOILBRENTEU"), price("DCOILWTICO")) {
        (Some(b), Some(w)) => {
            let (B, W) = (values(&b), values(&w));
            B.keys().filter(|d| W.contains_key(*d)).max().map_or(
                Value::Null,
                |d| json!({"date": d, "value": round(B[d] - W[d], 2), "unit": "USD/bbl"}),
            )
        }
        _ => Value::Null,
    };
    data.sort_by(|a, b| {
        (a["kind"].as_str(), a["name"].as_str()).cmp(&(b["kind"].as_str(), b["name"].as_str()))
    });
    json!({"region": r.id, "countries": r.context.countries, "series": data, "spread": spread, "sources": sources,
           "updated": iso(&t)})
}
