//! Region registry (spec § Seam): regions/<id>/region.toml, engine-owned.
//!
//! A region's conflict config (the dashboard's ConflictConfig) is either
//!   - regions/<id>/conflict.json, exported from a TypeScript config (the two
//!     upstream theaters), or
//!   - the [conflict] table of region.toml (every later region, spec P3), completed
//!     with CLIENT / SERVER defaults and written to TypeScript by `build_ts`
//!     (src/lib/conflicts/generated.ts), so a new region is one TOML file.
//!
//! A source id is "<adapter>:<name>" or "<adapter>:<name>#<k>" when several feeds
//! share a name; grades and owners are keyed by the part before '#'.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, OnceLock};

use regex::Regex;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::fmt;
use crate::store::repo;

pub fn dir() -> PathBuf {
    repo().join("regions")
}

pub fn ts_path() -> PathBuf {
    repo()
        .join("src")
        .join("lib")
        .join("conflicts")
        .join("generated.ts")
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Indicator {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub metric: Option<String>,
    pub window_h: Option<i64>,
    pub watch: Option<i64>,
    pub trigger: Option<i64>,
    pub max_credibility: Option<i64>,
    pub gate: Option<String>,
    pub window_days: Option<i64>,
    pub watch_z: Option<f64>,
    pub trigger_z: Option<f64>,
    pub band: Option<String>,
    pub field: Option<String>,
    /// Only events of this target class (process::what).
    pub target: Option<String>,
    /// "admin1": counted per oblast (analyze).
    pub by: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Hypothesis {
    pub id: String,
    pub text: String,
    pub expect: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Pir {
    pub id: String,
    pub question: String,
    pub indicator: Vec<Indicator>,
    pub hypothesis: Vec<Hypothesis>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Gate {
    /// Endpoints (lat, lon) of the crossing line.
    pub line: [[f64; 2]; 2],
}

#[derive(Clone, Debug, Default, Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Link {
    pub title: String,
    pub url: String,
    pub note: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Port {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Context {
    pub countries: Vec<String>,
    pub ports: Vec<Port>,
}

/// A country whose connectivity IODA measures, placed at its seat of government.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Country {
    pub code: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Outages {
    pub countries: Vec<Country>,
}

/// Countries whose every populated place and administrative unit is in the
/// gazetteer (process::place); elsewhere it holds GeoNames cities15000 only.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Gazetteer {
    pub countries: Vec<String>,
    /// Phrases that share a place's name but name something else (a person),
    /// matched case-sensitively and never located.
    pub not_places: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Region {
    pub id: String,
    pub name: String,
    /// lat_min, lat_max, lon_min, lon_max
    pub bbox: [f64; 4],
    pub timezone: String,
    pub reliability: HashMap<String, String>,
    pub owner: HashMap<String, String>,
    pub pir: Vec<Pir>,
    pub gate: HashMap<String, Gate>,
    pub link: Vec<Link>,
    pub context: Context,
    pub outages: Outages,
    pub gazetteer: Gazetteer,
    /// The server config plus the client tables the engine reads, as in conflict.json.
    #[serde(skip)]
    pub conflict: Value,
    /// ConflictConfig of a TOML region, written to TypeScript.
    #[serde(skip)]
    pub config: Option<Value>,
}

impl Region {
    /// A key of the conflict config.
    pub fn c(&self, k: &str) -> &Value {
        &self.conflict[k]
    }

    /// A key of the conflict's client config.
    pub fn client(&self, k: &str) -> &Value {
        &self.conflict["client"][k]
    }

    pub fn in_bbox(&self, lat: f64, lon: f64) -> bool {
        let [a, b, c, d] = self.bbox;
        a <= lat && lat <= b && c <= lon && lon <= d
    }

    /// Admiralty grade A-F; F (cannot be judged) for an ungraded source.
    pub fn reliability(&self, sid: &str) -> &str {
        self.reliability.get(key(sid)).map_or("F", |s| s)
    }

    pub fn owner(&self, sid: &str) -> String {
        let k = key(sid);
        self.owner
            .get(k)
            .cloned()
            .unwrap_or_else(|| k.split_once(':').map_or(k, |x| x.1).to_string())
    }

    /// A JavaScript RegExp {regex, flags} of the conflict config, compiled once
    /// (a keyword list compiles in tens of milliseconds; callers ask per item).
    pub fn regex(&self, k: &str) -> Regex {
        static RE: LazyLock<Mutex<HashMap<(String, String), Regex>>> =
            LazyLock::new(Default::default);
        let mut M = RE.lock().unwrap();
        M.entry((self.id.clone(), k.to_string()))
            .or_insert_with(|| {
                let d = self.c(k);
                let i = d["flags"].as_str().unwrap_or("").contains('i');
                Regex::new(&format!(
                    "{}{}",
                    if i { "(?i)" } else { "" },
                    d["regex"].as_str().unwrap_or("")
                ))
                .unwrap()
            })
            .clone()
    }
}

pub fn key(sid: &str) -> &str {
    sid.split('#').next().unwrap()
}

fn merge(a: Value, b: &Value) -> Value {
    let mut m = a.as_object().cloned().unwrap_or_default();
    for (k, v) in b.as_object().into_iter().flatten() {
        m.insert(k.clone(), v.clone());
    }
    Value::Object(m)
}

/// Full ConflictConfig of a TOML region: defaults, then geometry derived from the
/// bounding box (fires and flights boxes; flights radius = half-diagonal).
fn complete(r: &Region, c: &Value) -> Value {
    let client_defaults = json!({
        "mapZoom": 5, "cities": [], "cityColors": {"default": "#999999"}, "launchSites": [],
        "strikeLocations": {}, "strikeTargets": [], "alertCities": {}, "missileOrigins": [],
        "flightColors": [], "navyColors": {}, "timeZones": [], "sourceColors": {}, "navyOrder": [],
        "countryColors": {}, "regionBoxes": [],
        "alertSystemName": "No air-raid feed", "alertStatusTitle": "NO AIR-RAID FEED",
        "hasDroneTracker": false,
    });
    let server_defaults = json!({
        "strikeQueries": [], "countryAttribution": [], "defaultCountry": "Unknown",
        "conflictQueries": [], "conflictLocations": [], "countryQueries": [],
        "telegramChannels": [], "alertProvider": "none",
    });
    let [a, b, lo, hi] = r.bbox;
    let bx = json!({"latMin": a, "latMax": b, "lonMin": lo, "lonMax": hi});
    let center = &c["client"]["mapCenter"];
    let (lat, lon) = (center[0].clone(), center[1].clone());
    let half = 111.2 * (b - a).hypot((hi - lo) * ((a + b) / 2.0).to_radians().cos()) / 2.0;
    let client = merge(
        merge(
            client_defaults,
            &json!({
        "alertFallbackCenter": [lat, lon], "defaultMissileOrigin": [lat, lon],
        "defaultRegion": r.name}),
        ),
        &c["client"],
    );
    let server = merge(
        merge(
            server_defaults,
            &json!({
        "firesBBox": bx, "flightsBBox": bx,
        "flightsCenter": {"lat": lat, "lon": lon, "dist": (half.round_ties_even() as i64).min(2500)}}),
        ),
        &c["server"],
    );
    json!({"key": r.id, "label": c["label"],
           "theater": c.get("theater").cloned().unwrap_or("OSINT COMMAND CENTER // UNCLASSIFIED".into()),
           "client": client, "server": server})
}

fn load() -> Vec<Region> {
    let mut P: Vec<PathBuf> = fs::read_dir(dir())
        .unwrap()
        .flatten()
        .map(|e| e.path().join("region.toml"))
        .filter(|p| p.exists())
        .collect();
    P.sort();
    P.iter()
        .map(|p| {
            let v: Value = toml::from_str(&fs::read_to_string(p).unwrap()).unwrap();
            let mut r: Region = serde_json::from_value(v.clone()).unwrap();
            if let Some(c) = v.get("conflict") {
                let cfg = complete(&r, c);
                let mut m = Map::new();
                m.insert("key".into(), cfg["key"].clone());
                m.insert("label".into(), cfg["label"].clone());
                for (k, x) in cfg["server"].as_object().unwrap() {
                    m.insert(k.clone(), x.clone());
                }
                m.insert("client".into(), cfg["client"].clone());
                r.conflict = Value::Object(m);
                r.config = Some(cfg);
            } else {
                r.conflict = serde_json::from_str(
                    &fs::read_to_string(p.with_file_name("conflict.json")).unwrap(),
                )
                .unwrap();
            }
            r
        })
        .collect()
}

/// Every region, in the order of their directories.
pub fn all() -> &'static [Region] {
    static R: OnceLock<Vec<Region>> = OnceLock::new();
    R.get_or_init(load)
}

pub fn get(id: &str) -> Option<&'static Region> {
    all().iter().find(|r| r.id == id)
}

/// Text of src/lib/conflicts/generated.ts: every TOML region's ConflictConfig,
/// typed so that tsc checks it against upstream's interface.
pub fn build_ts() -> String {
    let C = Value::Array(all().iter().filter_map(|r| r.config.clone()).collect());
    format!(
        "// Generated by `osw regions` from regions/*/region.toml. Do not edit.\n\
         import type {{ ConflictConfig, ServerConfig }} from './types';\n\n\
         type Re = {{ regex: string; flags: string }};\n\
         type RegexKeys = 'newsRelevanceKeywords' | 'polymarketKeywords' | 'polymarketExclude';\n\
         type Raw = Omit<ConflictConfig, 'server'> & \
         {{ server: Omit<ServerConfig, RegexKeys> & Record<RegexKeys, Re> }};\n\n\
         const re = (r: Re) => new RegExp(r.regex, r.flags);\n\n\
         const RAW: Raw[] = {};\n\n\
         export const GENERATED: ConflictConfig[] = RAW.map(c => ({{\n  ...c,\n  server: {{\n    \
         ...c.server,\n    newsRelevanceKeywords: re(c.server.newsRelevanceKeywords),\n    \
         polymarketKeywords: re(c.server.polymarketKeywords),\n    \
         polymarketExclude: re(c.server.polymarketExclude),\n  }},\n}}));\n",
        fmt::dumps(&C, Some(2), false, false)
    )
}
