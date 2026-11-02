//! Drone and missile tracks from neptun.in.ua, as upstream src/app/api/drones, for
//! regions with droneProvider "neptun". An item is one track position.

use serde_json::{Value, json};

use super::{EXCERPT, Item, Source, date, excerpt, pystr as s, truthy};
use crate::regions::Region;

pub const CADENCE: f64 = 30.0;
const FIELDS: [&str; 6] = [
    "threat_type",
    "place",
    "region",
    "count",
    "course_bearing",
    "confidence_0_100",
];

pub fn sources(R: &[Region]) -> Vec<Source> {
    R.iter()
        .filter(|r| r.c("droneProvider").as_str() == Some("neptun"))
        .map(|r| Source {
            id: "drones:neptun".into(),
            region: r.id.clone(),
            url: "https://neptun.in.ua/api/data".into(),
            cadence: CADENCE,
            headers: vec![("Accept", "application/json".into())],
            ..Default::default()
        })
        .collect()
}

pub fn parse(body: &[u8], _src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let M = [&d["markers"], &d["tracks"]]
        .into_iter()
        .find(|m| truthy(m))
        .cloned()
        .unwrap_or(json!([]));
    let mut items = vec![];
    for m in M.as_array().ok_or("markers not a list")? {
        let (Some(lat), Some(lng)) = (m["lat"].as_f64(), m["lng"].as_f64()) else {
            continue;
        };
        let tid = [&m["id"], &m["track_id"]]
            .into_iter()
            .find(|v| truthy(v))
            .cloned()
            .unwrap_or_else(|| json!(format!("{},{}", s(&m["lat"]), s(&m["lng"]))));
        let mut p = json!({"track": tid, "lat": m["lat"], "lon": m["lng"],
                           "text": excerpt(m["text"].as_str().unwrap_or(""), EXCERPT)});
        for k in FIELDS {
            p[k] = m[k].clone();
        }
        items.push(Item {
            native_id: format!("{}@{}@{lat:.5},{lng:.5}", s(&tid), s(&m["date"])),
            published_at: date(m["date"].as_str()),
            url: None,
            payload: p,
        });
    }
    Ok(items)
}
