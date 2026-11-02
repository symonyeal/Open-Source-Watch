//! gpsjam.org daily navigation-accuracy cells (docs/SOURCES.md).
//!
//! gpsjam.org publishes, for each UTC day, every H3 resolution-4 cell that aircraft
//! flew through with the number of aircraft that reported good and bad navigation
//! accuracy (`<day>-h3_4.csv`), and a manifest of the days with a `suspect` flag for
//! incomplete ones. One raw report is one region's cells of one day, the cells whose
//! centre lies in the region's box; its native id is the day, so a day is stored
//! once. Each poll fetches the manifest and the days of the last DAYS that some
//! region does not store (all of them on the first poll).

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use serde_json::{Map, Value, json};

use super::{Fail, Item, Resp, Source, response, stored, text};
use crate::geo;
use crate::regions::{self, Region};

const BASE: &str = "https://gpsjam.org/data/";
const DAYS: i64 = 30;
pub const CADENCE: f64 = 43200.0;

pub fn sources(R: &[Region]) -> Vec<Source> {
    R.iter()
        .map(|r| Source {
            id: format!("gpsjam:gpsjam.org#{}", r.id),
            region: r.id.clone(),
            url: format!("{BASE}manifest.csv"),
            cadence: CADENCE,
            ..Default::default()
        })
        .collect()
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

/// The manifest and each day of the last DAYS some region lacks, as one JSON body
/// {"manifest": {day: row}, "days": {day: csv text}}. Every region stores a report
/// for every day it parses, so a day is complete when all regions have it.
pub fn fetch(src: &Source) -> Result<Resp, Fail> {
    let M = rows(&text(&src.url, 10.0)?).map_err(|_| Fail::Parse)?;
    let t0 = Utc::now().date_naive() - Duration::days(DAYS);
    let mut have: HashMap<String, HashSet<String>> = HashMap::new();
    for x in stored(&["gpsjam"], &[], Some(Utc.from_utc_datetime(&t0.into()))) {
        have.entry(x.payload["date"].as_str().unwrap_or("").into())
            .or_default()
            .insert(x.region);
    }
    let (mut man, mut days) = (Map::new(), Map::new());
    for m in &M {
        let d = &m["date"];
        let Ok(day) = NaiveDate::parse_from_str(d, "%Y-%m-%d") else {
            return Err(Fail::Parse);
        };
        let complete = regions::all()
            .iter()
            .all(|r| have.get(d).is_some_and(|h| h.contains(&r.id)));
        if day >= t0 && !complete {
            man.insert(d.clone(), json!(m));
            days.insert(
                d.clone(),
                json!(text(&format!("{BASE}{d}-h3_4.csv"), 10.0)?),
            );
        }
    }
    Ok(response(&json!({"manifest": man, "days": days})))
}

/// (hex, good, bad, centre) of each cell of one day's CSV.
type Day = Vec<(String, i64, i64, (f64, f64))>;

/// One day's cells, cached because every region parses the same day.
fn table(text: &str) -> Result<Arc<Day>, String> {
    static C: Mutex<Vec<(u64, Arc<Day>)>> = Mutex::new(vec![]);
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    let k = h.finish();
    if let Some((_, t)) = C.lock().unwrap().iter().find(|x| x.0 == k) {
        return Ok(t.clone());
    }
    let mut T = vec![];
    for r in rows(text)? {
        let n = |c: &str| {
            r.get(c)
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or(format!("bad {c}"))
        };
        let hex = r.get("hex").ok_or("no hex")?.clone();
        let c = geo::centre(&hex).ok_or("not an H3 cell")?;
        T.push((hex, n("count_good_aircraft")?, n("count_bad_aircraft")?, c));
    }
    let t = Arc::new(T);
    let mut c = C.lock().unwrap();
    c.push((k, t.clone()));
    if c.len() > 64 {
        c.remove(0);
    }
    Ok(t)
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let r = regions::get(&src.region).ok_or("unknown region")?;
    let mut items = vec![];
    for (day, text) in d["days"].as_object().ok_or("no days")? {
        let C: Vec<Value> = table(text.as_str().ok_or("not text")?)?
            .iter()
            .filter(|(_, _, _, (a, o))| r.in_bbox(*a, *o))
            .map(|(h, g, b, _)| json!([h, g, b]))
            .collect();
        let m = &d["manifest"][day];
        items.push(Item {
            native_id: day.clone(), url: Some(format!("{BASE}{day}-h3_4.csv")),
            published_at: Some(crate::fmt::fromiso(day).ok_or("bad day")?),
            payload: json!({"date": day, "suspect": m["suspect"] == "true", "network": m["source"], "res": 4,
                            "cells": C}),
        });
    }
    Ok(items)
}
