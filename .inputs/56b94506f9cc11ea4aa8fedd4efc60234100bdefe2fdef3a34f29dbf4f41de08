//! Recorded public context: EIA spot via FRED, IMF PortWatch and World Bank WDI.
//!
//! One immutable raw report is a bounded series snapshot. Its content hash changes on
//! revisions; the context view selects the latest fetched snapshot, not a union of
//! incompatible releases. These measurements never enter the event classifier.

use std::collections::HashSet;

use chrono::{Datelike, Duration, Utc};
use serde_json::{Value, json};

use super::{Item, Report, Source, decode, now, sha, urlencode};
use crate::fmt::{self, iso};
use crate::regions::Region;
use crate::schema::Raw;
use crate::store;

const BASE: &str = "https://services9.arcgis.com/weJ1QsnbMYJlCHdG/arcgis/rest/services/";
const FRED: [(&str, &str); 2] = [("DCOILBRENTEU", "Brent spot"), ("DCOILWTICO", "WTI spot")];
const WDI: [(&str, &str); 2] = [
    ("EG.IMP.CONS.ZS", "Net energy imports (% of energy use)"),
    (
        "TM.VAL.FUEL.ZS.UN",
        "Fuel imports (% of merchandise imports)",
    ),
];
const CHOKES: [(&str, &str); 4] = [
    ("chokepoint1", "Suez Canal"),
    ("chokepoint4", "Bab el-Mandeb Strait"),
    ("chokepoint5", "Malacca Strait"),
    ("chokepoint6", "Strait of Hormuz"),
];

pub fn sources(R: &[Region]) -> Vec<Source> {
    let today = Utc::now().date_naive();
    let start = (today - Duration::days(90)).to_string();
    let mut S: Vec<Source> = FRED
        .iter()
        .map(|(code, name)| Source {
            id: format!("context:FRED#{code}"),
            region: "global".into(),
            kind: "fred".into(),
            code: code.to_string(),
            name: name.to_string(),
            cadence: 21600.0,
            url: format!(
                "https://fred.stlouisfed.org/graph/fredgraph.csv?{}",
                urlencode(&[
                    ("id", code.to_string()),
                    ("cosd", start.clone()),
                    ("coed", today.to_string())
                ])
            ),
            ..Default::default()
        })
        .collect();
    for r in R {
        let C = &r.context.countries;
        if !C.is_empty() {
            for (code, name) in WDI {
                S.push(Source {
                    id: format!("context:World Bank#{code}"), region: r.id.clone(), kind: "wdi".into(),
                    code: code.into(), name: name.into(), countries: C.clone(), cadence: 86400.0,
                    url: format!("https://api.worldbank.org/v2/country/{}/indicator/{code}?format=json&date=2010:{}\
                                  &per_page=1000", C.join(";"), today.year()),
                    ..Default::default()
                });
            }
        }
        for p in &r.context.ports {
            S.push(port(&p.id, &p.name, &r.id, "port", &start));
        }
    }
    S.extend(
        CHOKES
            .iter()
            .map(|(code, name)| port(code, name, "global", "chokepoint", &start)),
    );
    S
}

fn port(code: &str, name: &str, region: &str, kind: &str, start: &str) -> Source {
    let (table, fields) = if kind == "port" {
        (
            "Daily_Ports_Data",
            "date,portid,portname,portcalls,portcalls_tanker",
        )
    } else {
        (
            "Daily_Chokepoints_Data",
            "date,portid,portname,n_total,n_tanker",
        )
    };
    // Only registry identifiers, never user input, enter ArcGIS SQL.
    let q = urlencode(&[
        (
            "where",
            format!("portid='{code}' AND date >= DATE '{start}'"),
        ),
        ("outFields", fields.into()),
        ("orderByFields", "date ASC".into()),
        ("resultRecordCount", "1000".into()),
        ("returnGeometry", "false".into()),
        ("f", "json".into()),
    ]);
    Source {
        id: format!("context:PortWatch#{code}"),
        region: region.into(),
        kind: kind.into(),
        code: code.into(),
        name: name.into(),
        cadence: 21600.0,
        url: format!("{BASE}{table}/FeatureServer/0/query?{q}"),
        ..Default::default()
    }
}

/// Missing stays missing; malformed or non-finite measures fail the poll.
fn number(x: &Value) -> Result<Value, String> {
    let v = match x {
        Value::Null => return Ok(Value::Null),
        Value::String(s) if s.is_empty() || s == "." => return Ok(Value::Null),
        Value::Bool(_) => return Err("Boolean measure".into()),
        Value::String(s) => s.trim().parse::<f64>().map_err(|e| e.to_string())?,
        Value::Number(n) => n.as_f64().unwrap(),
        _ => return Err("not a measure".into()),
    };
    if !v.is_finite() {
        return Err("Non-finite measure".into());
    }
    Ok(json!(v))
}

/// ArcGIS supports both legacy epoch milliseconds and ISO date-only values.
fn day(x: &Value) -> Result<String, String> {
    let t = match x {
        Value::Number(n) => fmt::epoch(n.as_f64().unwrap() / 1000.0),
        Value::String(s) => fmt::fromiso(s).ok_or("bad date")?,
        _ => return Err("no date".into()),
    };
    Ok(t.date_naive().to_string())
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let kind = src.kind.as_str();
    if !["fred", "wdi", "port", "chokepoint"].contains(&kind) {
        return Err(format!("Unknown context kind: {kind}"));
    }
    let mut points: Vec<Value> = vec![];
    let (unit, credit, url) = match kind {
        "fred" => {
            let text = String::from_utf8_lossy(body);
            let mut rd = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes());
            let H = rd.headers().map_err(|e| e.to_string())?.clone();
            let col = |k: &str| H.iter().position(|h| h == k);
            let (d, v) = (
                col("observation_date")
                    .or(col("DATE"))
                    .ok_or("no date column")?,
                col(&src.code).ok_or("no series column")?,
            );
            for row in rd.records() {
                let row = row.map_err(|e| e.to_string())?;
                points.push(json!({"date": day(&json!(row.get(d).unwrap_or("")))?,
                                   "value": number(&json!(row.get(v).unwrap_or("")))?}));
            }
            (
                "USD/bbl",
                "U.S. EIA via FRED",
                format!("https://fred.stlouisfed.org/series/{}", src.code),
            )
        }
        "wdi" => {
            let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
            let pages = d[0]["pages"]
                .as_i64()
                .or_else(|| d[0]["pages"].as_str().and_then(|s| s.parse().ok()));
            if d.as_array().map(Vec::len) != Some(2) || pages.is_none_or(|p| p > 1) {
                return Err("World Bank error or incomplete page".into());
            }
            for row in d[1].as_array().into_iter().flatten() {
                let c = row["countryiso3code"].as_str().unwrap_or("");
                if src.countries.iter().any(|x| x == c) {
                    let year = row["date"].as_str().ok_or("no WDI year")?;
                    if year.len() != 4 || year.parse::<i32>().is_err() {
                        return Err("bad WDI year".into());
                    }
                    points.push(json!({"country": c, "name": row["country"]["value"],
                                       "date": super::pystr(&row["date"]), "value": number(&row["value"])?}));
                }
            }
            (
                "%",
                "World Bank WDI (IEA / UN Comtrade)",
                format!("https://data.worldbank.org/indicator/{}", src.code),
            )
        }
        _ => {
            let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
            if d.get("error").is_some() || super::truthy(&d["exceededTransferLimit"]) {
                return Err("PortWatch error or incomplete page".into());
            }
            let (total, tanker) = if kind == "port" {
                ("portcalls", "portcalls_tanker")
            } else {
                ("n_total", "n_tanker")
            };
            for f in d["features"].as_array().ok_or("no features")? {
                let row = &f["attributes"];
                if row["portid"].as_str() != Some(&src.code) {
                    return Err("Unexpected PortWatch location".into());
                }
                points.push(
                    json!({"date": day(&row["date"])?, "value": number(&row[total])?,
                                   "tanker": number(&row[tanker])?}),
                );
                let p = points.last().unwrap();
                for field in ["value", "tanker"] {
                    if p[field]
                        .as_f64()
                        .is_some_and(|x| x < 0.0 || x.fract() != 0.0)
                    {
                        return Err("PortWatch counts must be nonnegative integers".into());
                    }
                }
                if p["value"]
                    .as_f64()
                    .zip(p["tanker"].as_f64())
                    .is_some_and(|(n, k)| k > n)
                {
                    return Err("PortWatch tanker count exceeds total".into());
                }
            }
            (
                if kind == "port" {
                    "calls/day"
                } else {
                    "transits/day"
                },
                "UN Global Platform; IMF PortWatch",
                "https://portwatch.imf.org/".to_string(),
            )
        }
    };
    if points.is_empty() {
        return Ok(vec![]);
    }
    let k = |p: &Value| {
        (
            p["country"].as_str().unwrap_or("").to_string(),
            p["date"].as_str().unwrap_or("").to_string(),
        )
    };
    points.sort_by_key(k);
    let mut keys = HashSet::new();
    if points.iter().any(|p| !keys.insert(k(p))) {
        return Err("Duplicate context observation key".into());
    }
    let t = points
        .iter()
        .map(|p| p["date"].as_str().unwrap().to_string())
        .max()
        .unwrap();
    let payload = json!({"code": src.code, "name": src.name, "kind": kind, "unit": unit, "source": credit,
                         "source_url": url, "points": points});
    let digest = sha(&fmt::dumps(&payload, None, true, true), 24);
    let t = fmt::fromiso(&if t.len() == 4 {
        format!("{t}-01-01")
    } else {
        t
    })
    .ok_or("bad date")?;
    // Keep release order even when a provider reverts to an earlier value set.
    Ok(vec![Item {
        native_id: format!("{digest}@{}", iso(&now())),
        published_at: Some(t),
        url: Some(url),
        payload,
    }])
}

/// Each series' releases known by t, ordered by fetch time and stable report ID.
/// Date filtering is by acquisition, so historical views cannot learn later corrections.
pub fn history(r: &Region, t: fmt::Time) -> Vec<Vec<Report>> {
    let mut groups: std::collections::BTreeMap<(String, String), Vec<Report>> =
        std::collections::BTreeMap::new();
    for x in store::view::<Raw>().iter().filter(|x| {
        (x.region == r.id || x.region == "global")
            && x.source_id.starts_with("context:")
            && x.fetched_at <= t
    }) {
        groups
            .entry((x.region.clone(), x.source_id.clone()))
            .or_default()
            .push(decode(x));
    }
    for group in groups.values_mut() {
        group.sort_by(|a, b| (a.fetched_at, &a.id).cmp(&(b.fetched_at, &b.id)));
    }
    groups.into_values().collect()
}
