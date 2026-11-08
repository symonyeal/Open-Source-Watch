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

use std::collections::{HashMap, HashSet};
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
#[serde(default, deny_unknown_fields)]
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
    /// Validate configured geometry, evidence grades and metric-specific inputs.
    /// Loading calls this before a region can serve collection or analysis.
    pub fn validate(&self) -> Result<(), String> {
        let check = |ok: bool, where_: &str, rule: &str| {
            if ok {
                Ok(())
            } else {
                Err(format!("{}: {where_}: {rule}", self.id))
            }
        };
        check(
            !self.id.is_empty() && !self.name.is_empty(),
            "region",
            "id and name are required",
        )?;
        let [a, b, c, d] = self.bbox;
        check(
            -90.0 <= a && a < b && b <= 90.0 && -180.0 <= c && c < d && d <= 180.0,
            "bbox",
            "expected ordered finite latitude/longitude bounds",
        )?;
        check(
            self.timezone.parse::<chrono_tz::Tz>().is_ok(),
            "timezone",
            "unknown timezone",
        )?;
        check(
            self.reliability
                .values()
                .all(|g| g.len() == 1 && "ABCDEF".contains(g.as_str())),
            "reliability",
            "expected Admiralty grade A-F",
        )?;
        check(
            self.owner.keys().all(|k| self.reliability.contains_key(k)),
            "owner",
            "source has no grade",
        )?;
        for k in ["newsFeeds", "telegramChannels"] {
            let prefix = if k == "newsFeeds" { "rss" } else { "telegram" };
            for s in self.c(k).as_array().into_iter().flatten() {
                let name = s["name"].as_str().unwrap_or("");
                check(
                    !name.is_empty() && self.reliability.contains_key(&format!("{prefix}:{name}")),
                    k,
                    "source requires name and grade",
                )?;
            }
        }
        for (id, g) in &self.gate {
            check(
                g.line.iter().all(|[lat, lon]| {
                    (-90.0..=90.0).contains(lat) && (-180.0..=180.0).contains(lon)
                }) && g.line[0] != g.line[1],
                id,
                "gate requires two distinct finite coordinates",
            )?;
        }
        let mut ids = HashSet::new();
        let mut pirs = HashSet::new();
        for p in &self.pir {
            check(
                !p.id.is_empty() && pirs.insert(p.id.as_str()) && !p.question.is_empty(),
                "pir",
                "unique id and question required",
            )?;
            check(
                !p.indicator.is_empty(),
                &p.id,
                "at least one indicator required",
            )?;
            for i in &p.indicator {
                check(
                    !i.id.is_empty() && ids.insert(i.id.as_str()) && !i.name.is_empty(),
                    &p.id,
                    "indicator requires unique id and name",
                )?;
                check(
                    i.max_credibility.is_none_or(|n| (1..=6).contains(&n)),
                    &i.id,
                    "max_credibility must be 1..6",
                )?;
                let metric = i.metric.as_deref();
                check(
                    matches!(
                        metric,
                        None | Some("event_count" | "unusual" | "transit" | "gnss" | "official")
                    ),
                    &i.id,
                    "unknown metric",
                )?;
                let event = matches!(metric, None | Some("event_count" | "unusual"));
                check(
                    (metric.is_none() || (i.watch.is_none() && i.trigger.is_none()))
                        && (matches!(metric, Some("unusual" | "transit"))
                            || i.window_days.is_none())
                        && (metric == Some("transit")
                            || (i.watch_z.is_none() && i.trigger_z.is_none() && i.gate.is_none()))
                        && (metric == Some("gnss") || i.band.is_none())
                        && (metric == Some("official") || i.field.is_none())
                        && (event || (i.window_h.is_none() && i.max_credibility.is_none())),
                    &i.id,
                    "metric has incompatible measurement fields",
                )?;
                if event {
                    check(
                        crate::schema::KINDS.contains(&i.kind.as_deref().unwrap_or("")),
                        &i.id,
                        "unknown event kind",
                    )?;
                    check(
                        i.window_h.is_some_and(|n| n > 0),
                        &i.id,
                        "window_h must be positive",
                    )?;
                    check(
                        i.target.as_deref().is_none_or(|x| {
                            ["energy", "military", "infrastructure", "civilian"].contains(&x)
                        }),
                        &i.id,
                        "unknown target",
                    )?;
                    check(
                        matches!(i.by.as_deref(), None | Some("admin1")),
                        &i.id,
                        "unknown grouping",
                    )?;
                } else {
                    check(
                        i.kind.is_none() && i.target.is_none() && i.by.is_none(),
                        &i.id,
                        "measurement has incompatible event fields",
                    )?;
                }
                match metric {
                    None => check(
                        matches!((i.watch, i.trigger), (Some(w), Some(t)) if 1 <= w && w <= t),
                        &i.id,
                        "require 1 <= watch <= trigger",
                    )?,
                    Some("unusual" | "transit") => {
                        check(
                            i.window_days.is_some_and(|n| n >= 7),
                            &i.id,
                            "window_days must be at least 7",
                        )?;
                        if metric == Some("transit") {
                            check(
                                i.gate.as_ref().is_some_and(|g| self.gate.contains_key(g)),
                                &i.id,
                                "unknown gate",
                            )?;
                            check(
                                matches!((i.trigger_z, i.watch_z), (Some(t), Some(w)) if t.is_finite() && w.is_finite() && t <= w && w < 0.0),
                                &i.id,
                                "require finite trigger_z <= watch_z < 0",
                            )?;
                        }
                    }
                    Some("gnss") => check(
                        matches!(i.band.as_deref(), Some("medium" | "high")),
                        &i.id,
                        "unknown GNSS band",
                    )?,
                    Some("official") => check(
                        crate::collect::mnd::FIELDS.contains(&i.field.as_deref().unwrap_or("")),
                        &i.id,
                        "unknown official count field",
                    )?,
                    _ => {}
                }
            }
        }
        for h in self.pir.iter().flat_map(|p| &p.hypothesis) {
            check(
                h.expect.iter().all(|(k, v)| {
                    ids.contains(k.as_str()) && matches!(v.as_str(), Some("present" | "absent"))
                }),
                &h.id,
                "hypothesis requires known indicators and present/absent expectations",
            )?;
        }
        Ok(())
    }

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
            r.validate()
                .unwrap_or_else(|e| panic!("{}: {e}", p.display()));
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
