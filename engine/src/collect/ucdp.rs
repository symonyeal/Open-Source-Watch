//! UCDP Candidate Events Dataset, monthly release (docs/SOURCES.md).
//!
//! UCDP codes each incident of organized lethal violence (state-based, non-state,
//! one-sided; at least one direct death) with dates, coordinates and their precision
//! (Candidate codebook 1.5). The monthly file `GEDEvent_v<yy>_0_<m>.csv`, linked from
//! the downloads page, needs no key (the API needs a token). Each poll reads the page
//! and its newest monthly file; a region that already stores that version takes
//! nothing from it. One raw report is one event inside a region's box, native id
//! "<id>@<version>": the id persists across versions, so a revised event is a new
//! report and `events` keeps the latest version. These events are the reference that
//! crate::validate scores the engine's events against, so they never become
//! observations.

use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value, json};

use super::{EXCERPT, Fail, Item, Resp, Source, excerpt, response, stored, text};
use crate::fmt;
use crate::regions::{self, Region};

const PAGE: &str = "https://ucdp.uu.se/downloads/";
pub const CADENCE: f64 = 7.0 * 86400.0;
const FIELDS: [&str; 18] = [
    "id",
    "type_of_violence",
    "conflict_name",
    "side_a",
    "side_b",
    "country",
    "date_start",
    "date_end",
    "date_prec",
    "where_prec",
    "latitude",
    "longitude",
    "best",
    "low",
    "high",
    "code_status",
    "event_clarity",
    "where_description",
];
const INTS: [&str; 7] = [
    "type_of_violence",
    "date_prec",
    "where_prec",
    "best",
    "low",
    "high",
    "event_clarity",
];
static FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)href="(https://ucdp\.uu\.se/downloads/candidateged/GEDEvent_v(\d+)_0_(\d+)\.csv)""#,
    )
    .unwrap()
});

pub fn sources(R: &[Region]) -> Vec<Source> {
    R.iter()
        .map(|r| Source {
            id: "ucdp:UCDP Candidate".into(),
            region: r.id.clone(),
            url: PAGE.into(),
            cadence: CADENCE,
            ..Default::default()
        })
        .collect()
}

/// {"version", "url", "csv", "have"} of the newest monthly file, `have` the regions
/// that store that version. The file is read on every poll: a region whose box holds
/// no event of the version stores nothing that would show it was read.
pub fn fetch(src: &Source) -> Result<Resp, Fail> {
    let page = text(&src.url, 10.0)?;
    let L: Vec<(String, u64, u64)> = FILE
        .captures_iter(&page)
        .map(|c| {
            (
                c[1].to_string(),
                c[2].parse().unwrap_or(0),
                c[3].parse().unwrap_or(0),
            )
        })
        .collect();
    let (url, yy, m) = L
        .into_iter()
        .max_by_key(|x| (x.1, x.2))
        .ok_or(Fail::Parse)?;
    let v = format!("{yy}.0.{m}");
    let have: BTreeSet<String> = stored(&["ucdp"], &[], None)
        .into_iter()
        .filter(|x| x.payload["version"] == v.as_str())
        .map(|x| x.region)
        .collect();
    let csv = text(&url, 120.0)?
        .trim_start_matches('\u{feff}')
        .to_string();
    Ok(response(
        &json!({"version": v, "url": url, "have": have, "csv": csv}),
    ))
}

fn rows(text: &str) -> Result<Vec<HashMap<String, String>>, String> {
    let mut rd = csv::Reader::from_reader(text.as_bytes());
    let H = rd.headers().map_err(|e| e.to_string())?.clone();
    rd.records()
        .map(|r| {
            r.map(|r| {
                H.iter()
                    .zip(r.iter())
                    .map(|(h, v)| (h.to_string(), v.to_string()))
                    .collect()
            })
            .map_err(|e| e.to_string())
        })
        .collect()
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let r = regions::get(&src.region).ok_or("unknown region")?;
    if d["have"]
        .as_array()
        .is_some_and(|h| h.iter().any(|x| x == r.id.as_str()))
    {
        return Ok(vec![]);
    }
    let version = d["version"].as_str().ok_or("no version")?;
    let mut out = vec![];
    for x in rows(d["csv"].as_str().ok_or("no csv")?)? {
        let f = |k: &str| x.get(k).cloned().ok_or(format!("no {k}"));
        let num = |k: &str| f(k)?.trim().parse::<f64>().map_err(|e| e.to_string());
        let (lat, lon) = (num("latitude")?, num("longitude")?);
        if !r.in_bbox(lat, lon) {
            continue;
        }
        let mut p = Map::new();
        for k in FIELDS {
            p.insert(k.into(), json!(f(k)?));
        }
        for k in INTS {
            p.insert(
                k.into(),
                json!(f(k)?.trim().parse::<i64>().map_err(|e| e.to_string())?),
            );
        }
        p.insert("latitude".into(), json!(lat));
        p.insert("longitude".into(), json!(lon));
        p.insert("version".into(), json!(version));
        p.insert("date_start".into(), json!(excerpt(&f("date_start")?, 10)));
        p.insert("date_end".into(), json!(excerpt(&f("date_end")?, 10)));
        p.insert(
            "where_description".into(),
            json!(excerpt(&f("where_description")?, EXCERPT)),
        );
        out.push(Item {
            native_id: format!("{}@{version}", f("id")?),
            url: d["url"].as_str().map(String::from),
            published_at: Some(fmt::fromiso(&f("date_start")?).ok_or("bad date_start")?),
            payload: Value::Object(p),
        });
    }
    Ok(out)
}

fn version(p: &Value) -> Vec<i64> {
    p["version"]
        .as_str()
        .unwrap_or("")
        .split('.')
        .map(|x| x.parse().unwrap_or(0))
        .collect()
}

/// Payloads of region r's stored UCDP events, the latest version of each.
pub fn events(r: &Region) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for x in stored(&["ucdp"], &[&r.id], None) {
        let p = x.payload;
        match out.iter_mut().find(|q| q["id"] == p["id"]) {
            Some(q) => {
                if version(&p) > version(q) {
                    *q = p;
                }
            }
            None => out.push(p),
        }
    }
    out
}
