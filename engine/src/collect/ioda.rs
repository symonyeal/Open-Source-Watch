//! IODA (Internet Outage Detection and Analysis, Georgia Tech) outage events per
//! country (docs/SOURCES.md).
//!
//! IODA detects country-level connectivity drops in three measurements, each its own
//! `datasource`: BGP announcements, active probing of /24 blocks (ping-slash24) and
//! unsolicited traffic at a network telescope (merit-nt). `/v2/outages/events`
//! returns each detected outage with its start, duration and score. One raw report
//! is one event as listed; an ongoing event lengthens, so its native id carries the
//! duration and each new length is a new report (the view keeps the longest). The
//! countries of a region are its `[outages]` table in region.toml. The merged
//! `overall=true` events answered HTTP 500 when inspected (2026-10-29).

use serde_json::{Value, json};

use super::{Fail, Item, Resp, Source, epoch, fetch as get, pystr, stored};
use crate::fmt::Time;
use crate::regions::Region;

const API: &str = "https://api.ioda.inetintel.cc.gatech.edu/v2/outages/events";
const DAYS: i64 = 7;
pub const CADENCE: f64 = 21600.0;

pub fn sources(R: &[Region]) -> Vec<Source> {
    R.iter()
        .flat_map(|r| {
            r.outages.countries.iter().map(|c| Source {
                id: format!("ioda:IODA#{}", c.code),
                region: r.id.clone(),
                code: c.code.clone(),
                url: format!("{API}?entityType=country&entityCode={}", c.code),
                cadence: CADENCE,
                ..Default::default()
            })
        })
        .collect()
}

/// The events of the last DAYS; the window is set at fetch time. IODA answers in
/// about ten seconds, so the timeout is longer than the default.
pub fn fetch(src: &Source) -> Result<Resp, Fail> {
    let t = chrono::Utc::now().timestamp();
    let s = Source {
        url: format!("{}&from={}&until={t}", src.url, t - DAYS * 86400),
        ..Default::default()
    };
    get(&s, 60.0)
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if super::truthy(&d["error"]) || !d["data"].is_array() {
        return Err(format!("IODA error: {}", pystr(&d["error"])));
    }
    let loc = format!("country/{}", src.code);
    let mut out = vec![];
    for e in d["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["location"].as_str() == Some(&loc))
    {
        let (start, dur) = (
            e["start"].as_i64().ok_or("no start")?,
            e["duration"].as_i64().ok_or("no duration")?,
        );
        let ds = e["datasource"].as_str().ok_or("no datasource")?;
        out.push(Item {
            native_id: format!("{ds}:{start}:{dur}"), published_at: Some(epoch(1000.0 * start as f64)),
            url: Some(format!("https://ioda.inetintel.cc.gatech.edu/country/{}?from={start}&until={}", src.code, start + dur)),
            payload: json!({"country": src.code, "name": e["location_name"], "datasource": ds, "method": e["method"],
                            "start": start, "duration": dur, "score": e["score"]}),
        });
    }
    Ok(out)
}

/// Payloads, with url and fetched_at, of region r's IODA events starting at or after
/// `since`, one per (country, datasource, start) at its longest listing.
pub fn events(r: &Region, since: Time) -> Vec<(Value, Option<String>, Time)> {
    let mut E: Vec<(Value, Option<String>, Time)> = vec![];
    let k = |p: &Value| {
        (
            p["country"].clone(),
            p["datasource"].clone(),
            p["start"].clone(),
        )
    };
    for x in stored(&["ioda"], &[&r.id], Some(since)) {
        let d = x.payload["duration"].as_i64().unwrap_or(0);
        match E.iter_mut().find(|e| k(&e.0) == k(&x.payload)) {
            Some(e) => {
                if d > e.0["duration"].as_i64().unwrap_or(0) {
                    *e = (x.payload, x.url, x.fetched_at);
                }
            }
            None => E.push((x.payload, x.url, x.fetched_at)),
        }
    }
    E
}
