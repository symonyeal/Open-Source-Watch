//! Market signals, as upstream src/app/api/{markets,oil,crypto,polymarket}: Yahoo
//! Finance chart meta per symbol, CoinGecko spot prices, Polymarket markets that
//! match any region's keywords. A snapshot is stored when its value changes.

use serde_json::{Map, Value, json};

use super::{Item, Source, encode_uri_component, epoch, global, pystr};
use crate::regions::{self, Region};

pub const CADENCE: f64 = 600.0;
const POLY: &str = "https://gamma-api.polymarket.com/markets?limit=500&closed=false&active=true\
                    &order=volume24hr&ascending=false";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

fn yahoo(s: &str) -> String {
    format!("https://query1.finance.yahoo.com/v8/finance/chart/{s}?interval=1d&range=5d")
}

fn list(k: &str) -> Vec<String> {
    global()[k]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

pub fn sources(_R: &[Region]) -> Vec<Source> {
    let q = |s: &str, url: String| Source {
        id: format!("markets:Yahoo Finance#{s}"),
        region: "global".into(),
        kind: "yahoo".into(),
        url,
        cadence: CADENCE,
        headers: vec![("User-Agent", BROWSER.into())],
        ..Default::default()
    };
    // The markets route encodes the symbol; the oil route inserts it raw.
    let mut S: Vec<Source> = list("markets")
        .iter()
        .map(|s| q(s, yahoo(&encode_uri_component(s))))
        .collect();
    S.extend(list("oil").iter().map(|s| q(s, yahoo(s))));
    S.push(Source {
        id: "markets:CoinGecko".into(),
        region: "global".into(),
        kind: "coingecko".into(),
        cadence: CADENCE,
        headers: vec![("Accept", "application/json".into())],
        url: format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd\
                                  &include_24hr_change=true",
            list("coins").join(",")
        ),
        ..Default::default()
    });
    S.push(Source {
        id: "markets:Polymarket".into(),
        region: "global".into(),
        kind: "polymarket".into(),
        url: POLY.into(),
        cadence: CADENCE,
        ..Default::default()
    });
    S
}

pub fn parse(body: &[u8], src: &Source) -> Result<Vec<Item>, String> {
    let d: Value = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    match src.kind.as_str() {
        "yahoo" => {
            let m = &d["chart"]["result"][0]["meta"];
            let rt = m["regularMarketTime"]
                .as_f64()
                .ok_or("no regularMarketTime")?;
            let p: Map<String, Value> = [
                "symbol",
                "regularMarketPrice",
                "chartPreviousClose",
                "currency",
            ]
            .iter()
            .map(|k| (k.to_string(), m[*k].clone()))
            .collect();
            Ok(vec![Item {
                native_id: format!("{}@{}", pystr(&m["symbol"]), pystr(&m["regularMarketTime"])),
                published_at: Some(epoch(1000.0 * rt)),
                url: None,
                payload: Value::Object(p),
            }])
        }
        "coingecko" => Ok(d
            .as_object()
            .ok_or("not an object")?
            .iter()
            .map(|(c, v)| {
                let mut p = json!({"coin": c});
                for (k, x) in v.as_object().into_iter().flatten() {
                    p[k] = x.clone();
                }
                Item {
                    native_id: format!("{c}@{}", pystr(&v["usd"])),
                    published_at: None,
                    url: None,
                    payload: p,
                }
            })
            .collect()),
        _ => {
            let K: Vec<_> = regions::all()
                .iter()
                .map(|r| {
                    (
                        r.regex("polymarketKeywords"),
                        r.regex("polymarketExclude"),
                        r.id.clone(),
                    )
                })
                .collect();
            let mut items = vec![];
            for m in d.as_array().ok_or("not a list")? {
                let q = m["question"].as_str().unwrap_or("");
                let rs: Vec<&String> = K
                    .iter()
                    .filter(|(k, x, _)| k.is_match(q) && !x.is_match(q))
                    .map(|x| &x.2)
                    .collect();
                if rs.is_empty() {
                    continue;
                }
                let mut p = json!({"id": m["id"], "question": q, "regions": rs});
                for k in ["outcomes", "outcomePrices", "volume24hr", "endDate"] {
                    p[k] = m[k].clone();
                }
                items.push(Item {
                    native_id: format!("{}@{}", pystr(&m["id"]), pystr(&m["outcomePrices"])),
                    published_at: None,
                    url: Some(format!(
                        "https://polymarket.com/event/{}",
                        pystr(&m["slug"])
                    )),
                    payload: p,
                });
            }
            Ok(items)
        }
    }
}
