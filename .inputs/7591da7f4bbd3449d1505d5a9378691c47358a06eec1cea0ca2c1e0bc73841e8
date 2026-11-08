//! AIS vessel positions from aisstream.io (spec P4): a websocket stream, so not
//! polled like the other sources. Needs a free API key in $AISSTREAM_KEY; without
//! one the source is recorded as `blocked` every minute and the transit PIRs show a
//! collection gap.
//!
//! Subscription: the bounding box of every region that has a gate, PositionReport
//! messages only. Message fields follow the aisstream.io documentation
//! (MetaData.MMSI, ShipName, time_utc; Message.PositionReport.Latitude, Longitude,
//! Sog, Cog, TrueHeading).

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use chrono::{NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use serde_json::{Value, json};

use super::{Item, Source};
use crate::fmt::{Time, iso};
use crate::regions::Region;

pub const URL: &str = "wss://stream.aisstream.io/v0/stream";
pub const CADENCE: f64 = 60.0;
static TIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d)(?:\.(\d+))?").unwrap());

pub fn key() -> Option<String> {
    std::env::var("AISSTREAM_KEY")
        .ok()
        .filter(|k| !k.is_empty())
}

pub fn sources(_R: &[Region]) -> Vec<Source> {
    vec![Source {
        id: "ais:aisstream".into(),
        region: "global".into(),
        url: URL.into(),
        cadence: CADENCE,
        stream: true,
        ..Default::default()
    }]
}

pub fn subscription(k: &str, R: &[Region]) -> Value {
    json!({"APIKey": k, "FilterMessageTypes": ["PositionReport"],
           "BoundingBoxes": R.iter().filter(|r| !r.gate.is_empty())
               .map(|r| json!([[r.bbox[0], r.bbox[2]], [r.bbox[1], r.bbox[3]]])).collect::<Vec<_>>()})
}

/// aisstream time_utc, e.g. '2026-10-21 08:00:01.123456789 +0000 UTC'.
pub fn time_utc(s: &str) -> Option<Time> {
    let m = TIME.captures(s)?;
    let t = NaiveDateTime::parse_from_str(&m[1], "%Y-%m-%d %H:%M:%S").ok()?;
    let f = m.get(2).map_or("0", |x| x.as_str());
    let us: i64 = format!("{:0<6}", &f[..f.len().min(6)]).parse().ok()?;
    Some(Utc.from_utc_datetime(&t) + chrono::Duration::microseconds(us))
}

/// Collection item of one aisstream message, or None if not a position.
fn item(m: &Value) -> Result<Option<Item>, String> {
    if m["MessageType"] != "PositionReport" {
        return Ok(None);
    }
    let (md, p) = (&m["MetaData"], &m["Message"]["PositionReport"]);
    let t = time_utc(md["time_utc"].as_str().ok_or("no time_utc")?).ok_or("bad time_utc")?;
    let mmsi = &md["MMSI"];
    if mmsi.is_null() || p["Latitude"].is_null() || p["Longitude"].is_null() {
        return Err("incomplete position".into());
    }
    Ok(Some(Item {
        native_id: format!("{}@{}", super::pystr(mmsi), iso(&t)),
        published_at: Some(t),
        url: None,
        payload: json!({"mmsi": mmsi, "name": md["ShipName"].as_str().unwrap_or("").trim(),
                        "lat": p["Latitude"], "lon": p["Longitude"], "sog": p["Sog"], "cog": p["Cog"],
                        "heading": p["TrueHeading"]}),
    }))
}

/// Items of a batch of messages (one JSON message per line).
pub fn parse(body: &[u8], _src: &Source) -> Result<Vec<Item>, String> {
    let mut out = vec![];
    for line in String::from_utf8_lossy(body)
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let m: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        out.extend(item(&m)?);
    }
    Ok(out)
}

/// Run the subscription until stop is set, sending each message; reconnect after
/// 30 s on any connection failure.
pub fn stream(k: String, R: &'static [Region], put: Sender<String>, stop: &'static AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        if let Ok((mut ws, _)) = tungstenite::connect(URL)
            && ws
                .send(tungstenite::Message::text(subscription(&k, R).to_string()))
                .is_ok()
        {
            while let Ok(m) = ws.read() {
                let s = match m {
                    tungstenite::Message::Text(s) => s.to_string(),
                    tungstenite::Message::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
                    _ => continue,
                };
                if put.send(s).is_err() || stop.load(Ordering::Relaxed) {
                    return;
                }
            }
        }
        std::thread::sleep(Duration::from_secs(30));
    }
}
