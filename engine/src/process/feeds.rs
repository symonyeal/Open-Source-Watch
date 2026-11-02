//! Processors of the non-news feeds: raw reports → observations and track points
//! (spec § Processing). One function per adapter, of raw reports with decoded
//! payloads; `entities` merges the aircraft and vessels their reports describe.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

use chrono::{Duration, NaiveDate};
use serde_json::{Map, Value, json};

use super::{credibility_single, lang, obs_id, observations, translate};
use crate::collect::{Report, firms::key as firms_key, global, pystr, stored, truthy};
use crate::fmt;
use crate::regions::{self, Region};
use crate::schema::{Entity, Obs, Track};

pub fn telegram(raws: &[Report]) -> Vec<Obs> {
    let Y: Vec<(&Region, &Report, String)> = raws
        .iter()
        .filter_map(|x| {
            Some((
                regions::get(&x.region)?,
                x,
                x.payload["text"].as_str().unwrap_or("").to_string(),
            ))
        })
        .collect();
    observations(&Y)
}

/// Degrees; days; active days that make a cell persistent.
const CELL: f64 = 0.1;
const PRIOR: i64 = 7;
const PERSIST: usize = 3;

pub fn cell(lat: f64, lon: f64) -> (i64, i64) {
    (
        (lat / CELL).round_ties_even() as i64,
        (lon / CELL).round_ties_even() as i64,
    )
}

fn num(p: &Value, k: &str) -> f64 {
    p[k].as_str()
        .and_then(|s| s.trim().parse().ok())
        .or_else(|| p[k].as_f64())
        .unwrap_or(f64::NAN)
}

fn day(p: &Value) -> NaiveDate {
    NaiveDate::parse_from_str(p["acq_date"].as_str().unwrap_or(""), "%Y-%m-%d").unwrap_or_default()
}

/// A detection in cell c on day d is at a persistent heat source (gas flare,
/// industry) if c had detections on >= PERSIST of the PRIOR days before d; D maps
/// cell -> detection dates. Static-source masking as in FIRMS / VIIRS Nightfire
/// flare catalogues (docs/SOURCES.md).
pub fn persistent(
    D: &HashMap<(i64, i64), HashSet<NaiveDate>>,
    c: (i64, i64),
    d: NaiveDate,
) -> bool {
    D.get(&c).is_some_and(|s| {
        (1..=PRIOR)
            .filter(|k| s.contains(&(d - Duration::days(*k))))
            .count()
            >= PERSIST
    })
}

/// One observation per detection per region containing it: thermal_anomaly, or
/// report at a persistent heat source. Low-confidence ('l') detections are not
/// observations. History comes from the immutable raw layer, so the result is a
/// function of raw reports.
pub fn firms(raws: &[Report]) -> Vec<Obs> {
    let Some(t0) = raws.iter().filter_map(|x| x.published_at).min() else {
        return vec![];
    };
    let mut D: HashMap<(i64, i64), HashSet<NaiveDate>> = HashMap::new();
    for p in stored(&["firms"], &[], Some(t0 - Duration::days(PRIOR + 1)))
        .iter()
        .chain(raws)
        .map(|x| &x.payload)
    {
        D.entry(cell(num(p, "latitude"), num(p, "longitude")))
            .or_default()
            .insert(day(p));
    }
    let mut out = vec![];
    for x in raws {
        let p = &x.payload;
        let (lat, lon) = (num(p, "latitude"), num(p, "longitude"));
        if p["confidence"] == "l" {
            continue;
        }
        let hot = persistent(&D, cell(lat, lon), day(p));
        let t = x.published_at.unwrap();
        for r in regions::all().iter().filter(|r| r.in_bbox(lat, lon)) {
            out.push(Obs {
                id: obs_id(&firms_key(p), &r.id),
                admin1: None,
                admin2: None,
                raw_id: x.id.clone(),
                source_id: x.source_id.clone(),
                region: r.id.clone(),
                t,
                lat: Some(lat),
                lon: Some(lon),
                geo_precision: "point".into(),
                kind: if hot { "report" } else { "thermal_anomaly" }.into(),
                text_en: format!(
                    "VIIRS {}, FRP {} MW, confidence {}, {}",
                    if hot {
                        "persistent heat source"
                    } else {
                        "thermal anomaly"
                    },
                    pystr(&p["frp"]),
                    pystr(&p["confidence"]),
                    pystr(&p["daynight"])
                ),
                lang: "en".into(),
                credibility: credibility_single(r, &x.source_id),
                names: vec![],
                t_event: Some(t),
            });
        }
    }
    out
}

/// Track points of vessels, entity "mmsi:<mmsi>", in each region containing them.
pub fn ais(raws: &[Report]) -> Vec<Track> {
    let mut out = vec![];
    for x in raws {
        let p = &x.payload;
        let (lat, lon) = (
            p["lat"].as_f64().unwrap_or(f64::NAN),
            p["lon"].as_f64().unwrap_or(f64::NAN),
        );
        for r in regions::all().iter().filter(|r| r.in_bbox(lat, lon)) {
            out.push(Track {
                entity_id: format!("mmsi:{}", pystr(&p["mmsi"])),
                region: r.id.clone(),
                t: x.published_at.unwrap_or(x.fetched_at),
                lat,
                lon,
                heading: p["cog"].as_f64(),
                speed: p["sog"].as_f64(),
                source_id: x.source_id.clone(),
            });
        }
    }
    out
}

fn hex(p: &Value) -> String {
    p["hex"].as_str().unwrap_or("").trim().replace('~', "")
}

/// Track points of aircraft, entity "icao:<hex>", in each region containing them.
pub fn adsb(raws: &[Report]) -> Vec<Track> {
    let mut out = vec![];
    for x in raws {
        let p = &x.payload;
        let (lat, lon) = (
            p["lat"].as_f64().unwrap_or(f64::NAN),
            p["lon"].as_f64().unwrap_or(f64::NAN),
        );
        for r in regions::all().iter().filter(|r| r.in_bbox(lat, lon)) {
            out.push(Track {
                entity_id: format!("icao:{}", hex(p)),
                region: r.id.clone(),
                t: x.published_at.unwrap_or(x.fetched_at),
                lat,
                lon,
                heading: p["track"].as_f64(),
                speed: p["gs"].as_f64(),
                source_id: x.source_id.clone(),
            });
        }
    }
    out
}

/// Hebrew → English: upstream's alert vocabulary and CBS locality names.
fn hebrew() -> &'static (Value, Value) {
    static H: OnceLock<(Value, Value)> = OnceLock::new();
    H.get_or_init(|| {
        let city = std::fs::read_to_string(
            crate::store::repo()
                .join("src")
                .join("data")
                .join("city-data.json"),
        )
        .unwrap();
        (
            global()["threatTranslations"].clone(),
            serde_json::from_str(&city).unwrap(),
        )
    })
}

/// One air_raid_alert per alert, placed at its first location in alertCities.
pub fn alerts(raws: &[Report]) -> Vec<Obs> {
    let (T, C) = hebrew();
    let mut out = vec![];
    for x in raws {
        let (Some(r), p) = (regions::get(&x.region), &x.payload) else {
            continue;
        };
        let threat = p["threat"].as_str().unwrap_or("");
        let threat_en = T[threat].as_str().unwrap_or(threat);
        let places: Vec<String> = p["locations"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| {
                let s = s.as_str().unwrap_or("").trim();
                C[s].as_str().unwrap_or(s).to_string()
            })
            .collect();
        let en = translate(&format!("{threat_en}: {}", places.join(", ")));
        let A = r.client("alertCities");
        let hit = places
            .iter()
            .map(|s| s.to_lowercase())
            .find(|s| A.get(s).is_some());
        let (lat, lon) = hit
            .as_ref()
            .map_or((None, None), |h| (A[h][0].as_f64(), A[h][1].as_f64()));
        let prec = match &hit {
            Some(h) if h.contains("oblast") => "admin",
            Some(_) => "city",
            None => "none",
        };
        // An oblast alert ("kharkiv oblast") is that oblast; a city's units follow
        // from its position (process::units).
        let admin1 = hit.as_ref().filter(|_| prec == "admin").and_then(|h| {
            let title: Vec<String> = h
                .split(' ')
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
                })
                .collect();
            super::geocode(r, &title.join(" ")).admin1
        });
        let t = x.t();
        out.push(Obs {
            id: obs_id(&x.id, "0"),
            admin1,
            admin2: None,
            raw_id: x.id.clone(),
            source_id: x.source_id.clone(),
            region: r.id.clone(),
            t,
            lat,
            lon,
            geo_precision: prec.into(),
            kind: "air_raid_alert".into(),
            text_en: en,
            lang: lang(threat).into(),
            credibility: credibility_single(r, &x.source_id),
            names: places,
            t_event: Some(t),
        });
    }
    out
}

/// One internet_outage per IODA event, placed at the seat of its country's
/// government (region.toml [outages]) with admin precision: IODA measures the
/// country as one unit, so outages of one country within τ of each other link under
/// the fusion rule (same unit). The id depends on the event's country, datasource
/// and start only, so a lengthening event stays one observation. A country no
/// longer in [outages] has no seat to place it at, so it gives none.
pub fn ioda(raws: &[Report]) -> Vec<Obs> {
    let mut out = vec![];
    for x in raws {
        let (Some(r), p) = (regions::get(&x.region), &x.payload) else {
            continue;
        };
        let code = p["country"].as_str().unwrap_or("");
        let Some(c) = r.outages.countries.iter().find(|c| c.code == code) else {
            continue;
        };
        let h = p["duration"].as_f64().unwrap_or(0.0) / 3600.0;
        let t = x.published_at.unwrap();
        let score = p["score"].as_f64().unwrap_or(0.0);
        out.push(Obs {
            admin1: None,
            admin2: None,
            id: obs_id(
                &format!(
                    "ioda:{code}:{}:{}",
                    pystr(&p["datasource"]),
                    pystr(&p["start"])
                ),
                &r.id,
            ),
            raw_id: x.id.clone(),
            source_id: x.source_id.clone(),
            region: r.id.clone(),
            t,
            lat: Some(c.lat),
            lon: Some(c.lon),
            geo_precision: "admin".into(),
            kind: "internet_outage".into(),
            text_en: format!(
                "IODA {} outage signal in {} from {} UTC, {h:.1} h listed (score {score:.0})",
                pystr(&p["datasource"]),
                c.name,
                t.format("%Y-%m-%d %H:%M")
            ),
            lang: "en".into(),
            credibility: credibility_single(r, &x.source_id),
            names: vec![c.name.clone()],
            t_event: Some(t),
        });
    }
    out
}

/// Track points of each drone/missile track, and one airspace_activity observation
/// per track (the observation id depends on the track id only).
pub fn neptun(raws: &[Report]) -> (Vec<Track>, Vec<Obs>) {
    let (mut T, mut O) = (vec![], vec![]);
    for x in raws {
        let (Some(r), p) = (regions::get(&x.region), &x.payload) else {
            continue;
        };
        let (lat, lon) = (
            p["lat"].as_f64().unwrap_or(f64::NAN),
            p["lon"].as_f64().unwrap_or(f64::NAN),
        );
        let (track, t) = (pystr(&p["track"]), x.t());
        T.push(Track {
            entity_id: format!("neptun:{track}"),
            region: r.id.clone(),
            t,
            lat,
            lon,
            heading: p["course_bearing"].as_f64(),
            speed: None,
            source_id: x.source_id.clone(),
        });
        let text = p["text"].as_str().unwrap_or("");
        O.push(Obs {
            id: obs_id(&format!("neptun:{track}"), "0"),
            admin1: None,
            admin2: None,
            raw_id: x.id.clone(),
            source_id: x.source_id.clone(),
            region: r.id.clone(),
            t,
            lat: Some(lat),
            lon: Some(lon),
            geo_precision: "point".into(),
            kind: "airspace_activity".into(),
            text_en: translate(&format!("{}: {text}", pystr(&p["threat_type"]))),
            lang: lang(text).into(),
            credibility: credibility_single(r, &x.source_id),
            names: p["place"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| vec![s.to_string()])
                .unwrap_or_default(),
            t_event: Some(t),
        });
    }
    (T, O)
}

/// Aircraft (ADS-B, Neptun) and vessel (AIS) entities of decoded raw reports,
/// latest attributes win.
pub fn entities(raws: &[Report]) -> Vec<Entity> {
    struct E {
        id: String,
        kind: &'static str,
        names: BTreeSet<String>,
        ids: Map<String, Value>,
        attrs: Map<String, Value>,
    }
    let mut X: Vec<&Report> = raws.iter().collect();
    X.sort_by_key(|x| x.fetched_at);
    let mut E: Vec<E> = vec![];
    let mut at: HashMap<String, usize> = HashMap::new();
    let mut get =
        |E: &mut Vec<E>, id: String, kind: &'static str, ids: Map<String, Value>| -> usize {
            *at.entry(id.clone()).or_insert_with(|| {
                E.push(E {
                    id,
                    kind,
                    names: BTreeSet::new(),
                    ids,
                    attrs: Map::new(),
                });
                E.len() - 1
            })
        };
    for x in X {
        let p = &x.payload;
        if x.source_id.starts_with("adsb:") {
            let h = hex(p);
            let k = get(
                &mut E,
                format!("icao:{h}"),
                "aircraft",
                Map::from_iter([("icao".into(), json!(h))]),
            );
            let e = &mut E[k];
            for s in [
                p["flight"].as_str().unwrap_or("").trim(),
                p["r"].as_str().unwrap_or(""),
            ] {
                if !s.is_empty() {
                    e.names.insert(s.into());
                }
            }
            if truthy(&p["r"]) {
                e.ids.insert("registration".into(), p["r"].clone());
            }
            for k in ["t", "desc", "ownOp"] {
                if truthy(&p[k]) {
                    e.attrs.insert(k.into(), p[k].clone());
                }
            }
            e.attrs.insert(
                "military".into(),
                json!(p["dbFlags"].as_i64().unwrap_or(0) & 1 == 1),
            );
        } else if x.source_id.starts_with("ais:") {
            let k = get(
                &mut E,
                format!("mmsi:{}", pystr(&p["mmsi"])),
                "vessel",
                Map::from_iter([("mmsi".into(), p["mmsi"].clone())]),
            );
            if let Some(n) = p["name"].as_str().filter(|s| !s.is_empty()) {
                E[k].names.insert(n.into());
            }
        } else {
            let k = get(
                &mut E,
                format!("neptun:{}", pystr(&p["track"])),
                "aircraft",
                Map::from_iter([("neptun".into(), p["track"].clone())]),
            );
            E[k].attrs
                .insert("threat_type".into(), p["threat_type"].clone());
        }
    }
    E.into_iter()
        .map(|e| Entity {
            id: e.id,
            r#type: e.kind.into(),
            names: e.names.into_iter().collect(),
            identifiers: fmt::dumps(&Value::Object(e.ids), None, false, true),
            attributes: fmt::dumps(&Value::Object(e.attrs), None, false, false),
        })
        .collect()
}
