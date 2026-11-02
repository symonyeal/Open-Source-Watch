//! Air-raid alerts, as upstream src/app/api/alerts: Tzeva Adom (Pikud HaOref mirror)
//! for alertProvider "tzevaadom", alerts.com.ua oblast states for "alertsua". An
//! item is one alert; a continuing alert repeats its native id and is stored once.

use serde_json::{Value, json};

use super::{Item, Source, date, pystr as s, sha, truthy};
use crate::fmt;
use crate::regions::Region;

pub const CADENCE: f64 = 15.0;

fn url(p: &str) -> Option<&'static str> {
    match p {
        "tzevaadom" => Some("https://api.tzevaadom.co.il/notifications"),
        "alertsua" => Some("https://alerts.com.ua/api/states"),
        _ => None,
    }
}

pub fn sources(R: &[Region]) -> Vec<Source> {
    R.iter()
        .filter_map(|r| {
            let p = r.c("alertProvider").as_str().unwrap_or("");
            url(p).map(|u| Source {
                id: format!("alerts:{p}"),
                region: r.id.clone(),
                url: u.into(),
                cadence: CADENCE,
                kind: p.into(),
                headers: vec![("Accept", "application/json".into())],
                ..Default::default()
            })
        })
        .collect()
}

fn or<'a>(a: &'a Value, b: &'a Value) -> &'a Value {
    if truthy(a) { a } else { b }
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    if src.kind == "alertsua" {
        return Ok(d["states"].as_array().into_iter().flatten().filter(|x| truthy(&x["alert"]))
            .map(|x| Item {
                native_id: format!("{}@{}", s(&x["id"]), s(&x["changed"])),
                published_at: date(x["changed"].as_str()), url: None,
                payload: json!({"provider": "alertsua", "threat": "Air Raid Alert",
                                "locations": [or(&x["name_en"], or(&x["name"], &json!("Unknown")))]}),
            })
            .collect());
    }
    let mut items = vec![];
    for a in d.as_array().into_iter().flatten() {
        let threat = or(&a["threat"], or(&a["title"], &json!("Alert"))).clone();
        let cities = if a["cities"].is_array() {
            a["cities"].clone()
        } else {
            json!([or(&a["data"], &json!("Unknown"))])
        };
        let key = fmt::dumps(&json!([a["date"], threat, cities]), None, false, false);
        items.push(Item {
            native_id: sha(&key, 16),
            published_at: date(a["date"].as_str()),
            url: None,
            payload: json!({"provider": "tzevaadom", "threat": threat, "locations": cities}),
        });
    }
    Ok(items)
}
