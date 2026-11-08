//! NASA FIRMS VIIRS (Suomi NPP) active fires: the global 24 h CSV every 10 minutes,
//! as upstream src/app/api/fires, and the 7-day CSV once a day for the persistence
//! baseline (process::feeds::firms). Detections inside any region's box are kept;
//! the 7-day file contributes only detections older than 24 h, so the two sources
//! do not overlap.

use chrono::Duration;
use serde_json::{Map, Value};

use super::{Item, Source, now};
use crate::fmt::{self, repr};
use crate::regions::{self, Region};

pub const CADENCE: f64 = 600.0;
const BASE: &str = "https://firms.modaps.eosdis.nasa.gov/data/active_fire/suomi-npp-viirs-c2/csv/";
const FIELDS: [&str; 9] = [
    "latitude",
    "longitude",
    "bright_ti4",
    "frp",
    "confidence",
    "acq_date",
    "acq_time",
    "daynight",
    "satellite",
];

pub fn sources(_R: &[Region]) -> Vec<Source> {
    vec![
        Source {
            id: "firms:VIIRS#7d".into(),
            region: "global".into(),
            cadence: 86400.0,
            url: format!("{BASE}SUOMI_VIIRS_C2_Global_7d.csv"),
            ..Default::default()
        },
        Source {
            id: "firms:VIIRS".into(),
            region: "global".into(),
            cadence: CADENCE,
            url: format!("{BASE}SUOMI_VIIRS_C2_Global_24h.csv"),
            ..Default::default()
        },
    ]
}

fn num(p: &Value, k: &str) -> f64 {
    p[k].as_str()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(f64::NAN)
}

fn zfill4(s: &str) -> String {
    format!("{s:0>4}")
}

/// Identity of one detection, the same in the 24 h and 7-day files.
pub fn key(p: &Value) -> String {
    let s = |k: &str| p[k].as_str().unwrap_or("").to_string();
    format!(
        "{},{},{},{},{}",
        repr(num(p, "latitude")),
        repr(num(p, "longitude")),
        s("acq_date"),
        zfill4(&s("acq_time")),
        s("satellite")
    )
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let old = src.id.ends_with("#7d").then(|| now() - Duration::hours(24));
    let mut rd = csv::Reader::from_reader(body);
    let H = rd.headers().map_err(|e| e.to_string())?.clone();
    let mut items = vec![];
    for row in rd.records() {
        let row = row.map_err(|e| e.to_string())?;
        let mut p = Map::new();
        for k in FIELDS {
            let i = H.iter().position(|h| h == k).ok_or(format!("no {k}"))?;
            p.insert(k.into(), Value::String(row.get(i).unwrap_or("").into()));
        }
        let p = Value::Object(p);
        let (lat, lon) = (num(&p, "latitude"), num(&p, "longitude"));
        if lat.is_nan() || lon.is_nan() {
            continue;
        }
        if !regions::all().iter().any(|r| r.in_bbox(lat, lon)) {
            continue;
        }
        let hhmm = zfill4(p["acq_time"].as_str().unwrap());
        let t = fmt::fromiso(&format!(
            "{}T{}:{}:00+00:00",
            p["acq_date"].as_str().unwrap(),
            &hhmm[..2],
            &hhmm[2..]
        ))
        .ok_or("bad time")?;
        if old.is_some_and(|o| t >= o) {
            continue;
        }
        items.push(Item {
            native_id: key(&p),
            published_at: Some(t),
            url: None,
            payload: p,
        });
    }
    Ok(items)
}
