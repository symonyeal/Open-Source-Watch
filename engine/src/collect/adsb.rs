//! adsb.lol aircraft, as upstream src/app/api/flights: the global military feed and
//! each region's radius query. Kept: military feed aircraft inside a region's
//! bounding box, and radius aircraft with dbFlags military (1) or interesting (2).
//! Upstream also shows aircraft it classifies by callsign or type alone; the engine
//! does not store those (docs/DECISIONS.md).

use chrono::Duration;
use serde_json::{Map, Value};

use super::{Item, Source, epoch, now};
use crate::regions::{self, Region};

pub const CADENCE: f64 = 30.0;
const FIELDS: [&str; 13] = [
    "hex", "flight", "t", "r", "desc", "ownOp", "lat", "lon", "alt_baro", "gs", "track", "squawk",
    "dbFlags",
];

pub fn sources(R: &[Region]) -> Vec<Source> {
    let h = || vec![("Accept", "application/json".to_string())];
    let mut S = vec![Source {
        id: "adsb:adsb.lol#mil".into(),
        region: "global".into(),
        url: "https://api.adsb.lol/v2/mil".into(),
        cadence: CADENCE,
        headers: h(),
        ..Default::default()
    }];
    for r in R {
        let c = r.c("flightsCenter");
        let n = |k: &str| match &c[k] {
            Value::Number(x) if x.is_f64() => crate::fmt::repr(x.as_f64().unwrap()),
            v => v.to_string(),
        };
        S.push(Source {
            id: format!("adsb:adsb.lol#{}", r.id),
            region: r.id.clone(),
            cadence: CADENCE,
            url: format!(
                "https://api.adsb.lol/v2/lat/{}/lon/{}/dist/{}",
                n("lat"),
                n("lon"),
                n("dist")
            ),
            headers: h(),
            ..Default::default()
        });
    }
    S
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let t0 = d.get("now").and_then(Value::as_f64).map_or_else(now, epoch);
    let mut items = vec![];
    for a in d["ac"].as_array().into_iter().flatten() {
        let (Some(lat), Some(lon)) = (a["lat"].as_f64(), a["lon"].as_f64()) else {
            continue;
        };
        if src.region == "global" {
            if !regions::all().iter().any(|r| r.in_bbox(lat, lon)) {
                continue;
            }
        } else if a["dbFlags"].as_i64().unwrap_or(0) & 3 == 0 {
            continue;
        }
        let seen = a["seen_pos"].as_f64().filter(|s| *s != 0.0).unwrap_or(0.0);
        let t = t0 - Duration::microseconds((seen * 1e6).round_ties_even() as i64);
        let hex = a["hex"].as_str().unwrap_or("").trim().replace('~', "");
        let secs = t.timestamp_micros().div_euclid(1_000_000);
        let p: Map<String, Value> = FIELDS
            .iter()
            .filter_map(|k| a.get(*k).map(|v| (k.to_string(), v.clone())))
            .collect();
        items.push(Item {
            native_id: format!("{hex}@{secs}"),
            published_at: Some(t),
            url: None,
            payload: Value::Object(p),
        });
    }
    Ok(items)
}
