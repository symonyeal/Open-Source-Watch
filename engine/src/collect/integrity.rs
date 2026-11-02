//! Navigation-integrity reports of aircraft in each region's adsb.lol radius query,
//! the response collect::adsb already fetches (docs/SOURCES.md).
//!
//! ADS-B version 1/2 aircraft report how far their position can be trusted: NIC
//! (containment radius) and NACp (position accuracy); readsb adds `gpsOkBefore`
//! while GPS is lost. One raw report is one region's snapshot every CADENCE seconds:
//! [icao, lat, lon, nic, nacp, lost] per airborne aircraft in the box. The validity
//! filters transcribe gods-eye-view `src/layers/gnss/records.js`; whether a report is
//! bad is decided later (crate::gnss), so the rule can change without recollection.
//! A local engine only: the cron does not poll aircraft.

use serde_json::{Value, json};

use super::{Item, Source, adsb, epoch};
use crate::fmt::{self, iso};
use crate::regions::{self, Region};

pub const CADENCE: f64 = 600.0;
/// After GPS loss the position source can switch to MLAT.
const TYPES: [&str; 2] = ["adsb_icao", "mlat"];
/// s; an older position is not current evidence.
const AGE: f64 = 60.0;

pub fn sources(R: &[Region]) -> Vec<Source> {
    adsb::sources(R)
        .into_iter()
        .filter(|s| s.region != "global")
        .map(|s| Source {
            id: s.id.replacen("adsb:", "integrity:", 1),
            cadence: CADENCE,
            ..s
        })
        .collect()
}

/// A finite number (not a boolean), as JSON gives it.
fn finite(x: &Value) -> Option<f64> {
    x.as_f64().filter(|v| v.is_finite())
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let r = regions::get(&src.region).ok_or("unknown region")?;
    let mut out = vec![];
    for a in d["ac"].as_array().into_iter().flatten() {
        let version = a["version"].as_i64().filter(|_| a["version"].is_number());
        if !TYPES.contains(&a["type"].as_str().unwrap_or(""))
            || !matches!(version, Some(1 | 2))
            || finite(&a["alt_baro"]).is_none()
            || !finite(&a["seen_pos"]).is_some_and(|s| (0.0..=AGE).contains(&s))
        {
            continue;
        }
        let derived = |k: &str| {
            [&a["tisb"], &a["mlat"]]
                .iter()
                .any(|l| l.as_array().is_some_and(|l| l.iter().any(|x| x == k)))
        };
        let nic = finite(&a["nic"])
            .filter(|_| !derived("nic"))
            .map(|_| a["nic"].clone())
            .unwrap_or(Value::Null);
        let nacp = finite(&a["nac_p"])
            .filter(|_| !derived("nac_p"))
            .map(|_| a["nac_p"].clone())
            .unwrap_or(Value::Null);
        let lost = finite(&a["gpsOkBefore"]).is_some_and(|x| x > 0.0);
        let (mut lat, mut lon) = (finite(&a["lat"]), finite(&a["lon"]));
        if lost && finite(&a["gpsOkLat"]).is_some() && finite(&a["gpsOkLon"]).is_some() {
            (lat, lon) = (finite(&a["gpsOkLat"]), finite(&a["gpsOkLon"])); // the last good GNSS position
        }
        let (Some(lat), Some(lon)) = (lat, lon) else {
            continue;
        };
        if (nic.is_null() && nacp.is_null() && !lost) || !r.in_bbox(lat, lon) {
            continue;
        }
        let hex = a["hex"].as_str().ok_or("no hex")?.trim();
        out.push(json!([
            hex,
            fmt::round(lat, 4),
            fmt::round(lon, 4),
            nic,
            nacp,
            lost
        ]));
    }
    if out.is_empty() {
        return Ok(vec![]);
    }
    let t = epoch(d["now"].as_f64().ok_or("no now")?);
    Ok(vec![Item {
        native_id: iso(&t),
        published_at: Some(t),
        url: None,
        payload: json!({"ac": out}),
    }])
}
