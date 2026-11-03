//! Processing, stage 3 of the cycle (spec § Processing): raw report → observation.
//!
//! Rule-based, no language models. Each adapter has a processor (news, feeds)
//! built from the functions below.

pub mod feeds;
pub mod news;
pub mod place;
pub mod what;
pub mod when;

pub use place::{Geo, build_geonames, geocode, nearest, units_at};

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;

use crate::classifier;
use crate::collect::{EXCERPT, Report, excerpt, sha};
use crate::fmt::Time;
use crate::regions::{self, Region};
use crate::schema::Obs;
use crate::store;

/// Language guess by script (Unicode 15 blocks): Hebrew, Arabic, Cyrillic, else English.
pub fn lang(s: &str) -> &'static str {
    let any = |lo: char, hi: char| s.chars().any(|c| (lo..=hi).contains(&c));
    if any('\u{0590}', '\u{05FF}') {
        "he"
    } else if any('\u{0600}', '\u{06FF}') {
        "ar"
    } else if any('\u{0400}', '\u{04FF}') {
        "ru"
    } else {
        "en"
    }
}

static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

fn cache_file() -> std::path::PathBuf {
    store::data_dir().join("cache").join("translate.jsonl")
}

/// Forget the loaded translations (a new data root).
pub fn reset() {
    *CACHE.lock().unwrap() = None;
}

/// Google's unofficial translation endpoint, as upstream (src/lib/hebrew.ts).
/// After this many failures in a row the endpoint is not asked again for PAUSE: a
/// refused run would otherwise wait on every item (`osw retranslate` repairs them).
const FAILS: usize = 20;
const PAUSE: std::time::Duration = std::time::Duration::from_secs(600);
static DOWN: Mutex<(usize, Option<std::time::Instant>)> = Mutex::new((0, None));

/// English of s from Google's endpoint; one retry after 2 s.
fn google(s: &str) -> Option<String> {
    if DOWN.lock().unwrap().1.is_some_and(|t| t.elapsed() < PAUSE) {
        return None;
    }
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=en&dt=t&q={}",
        crate::collect::quote_plus(s)
    );
    let ask = || {
        crate::collect::get(&url, &[("User-Agent", "Mozilla/5.0".into())], 5.0)
            .ok()
            .filter(|r| r.status < 400)
    };
    let r = ask().or_else(|| {
        std::thread::sleep(std::time::Duration::from_secs(2));
        ask()
    });
    {
        let mut d = DOWN.lock().unwrap();
        *d = match r {
            Some(_) => (0, None),
            None if d.0 + 1 >= FAILS => (0, Some(std::time::Instant::now())),
            None => (d.0 + 1, d.1),
        };
    }
    let v: Value = serde_json::from_slice(&r?.body).ok()?;
    let t: String = v[0]
        .as_array()?
        .iter()
        .filter_map(|p| p[0].as_str())
        .collect();
    Some(if t.is_empty() { s.to_string() } else { t })
}

/// English text of s, at most EXCERPT characters like every stored text (a 280-character
/// original can translate longer); s itself if the endpoint fails. Cached on disk.
pub fn translate(s: &str) -> String {
    if lang(s) == "en" {
        return s.to_string();
    }
    {
        let mut c = CACHE.lock().unwrap();
        let C = c.get_or_insert_with(|| {
            let mut C = HashMap::new();
            for l in fs::read_to_string(cache_file()).unwrap_or_default().lines() {
                if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(l)
                    && let (Some(k), Some(v)) = (
                        a.first().and_then(Value::as_str),
                        a.get(1).and_then(Value::as_str),
                    )
                {
                    C.insert(k.to_string(), v.to_string());
                }
            }
            C
        });
        if let Some(t) = C.get(s) {
            return excerpt(t, EXCERPT);
        }
    }
    let Some(t) = google(s).map(|t| excerpt(&t, EXCERPT)) else {
        return s.to_string();
    };
    let mut c = CACHE.lock().unwrap();
    c.get_or_insert_with(HashMap::new)
        .insert(s.to_string(), t.clone());
    let p = cache_file();
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let line = crate::fmt::dumps(&serde_json::json!([s, t]), None, false, false);
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
        .unwrap();
    writeln!(f, "{line}").unwrap();
    t
}

pub(crate) const COUNTRIES: [&str; 32] = [
    "iran",
    "israel",
    "iraq",
    "syria",
    "lebanon",
    "yemen",
    "jordan",
    "egypt",
    "saudi arabia",
    "qatar",
    "kuwait",
    "bahrain",
    "oman",
    "uae",
    "turkey",
    "ukraine",
    "russia",
    "belarus",
    "moldova",
    "poland",
    "romania",
    "gaza",
    "west bank",
    "bangladesh",
    "india",
    "pakistan",
    "sri lanka",
    "myanmar",
    "burma",
    "nepal",
    "bhutan",
    "maldives",
];

/// Unicode general category L (letter), N (number) or Nd (decimal) of c, the
/// classes of Python's `\w` (L, N, `_`) and `\d` (Nd); ASCII without a lookup.
pub fn category(c: char, class: &str) -> bool {
    use regex::Regex;
    static R: LazyLock<[Regex; 3]> =
        LazyLock::new(|| [r"^\p{L}$", r"^\p{N}$", r"^\p{Nd}$"].map(|p| Regex::new(p).unwrap()));
    if c.is_ascii() {
        return if class == "L" {
            c.is_ascii_alphabetic()
        } else {
            c.is_ascii_digit()
        };
    }
    let k = ["L", "N", "Nd"].iter().position(|x| *x == class).unwrap();
    R[k].is_match(c.encode_utf8(&mut [0; 4]))
}

/// Python's `\w`.
pub fn wordchar(c: char) -> bool {
    c == '_' || category(c, "L") || category(c, "N")
}

/// The words of s as `[^\W\d_][\w'’-]*` finds them.
pub(crate) fn words(s: &str) -> Vec<&str> {
    let start = |c: char| category(c, "L") || (category(c, "N") && !category(c, "Nd"));
    let more = |c: char| wordchar(c) || "'’-".contains(c);
    let mut out = vec![];
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if !start(c) {
            continue;
        }
        let mut j = i + c.len_utf8();
        while let Some(&(k, d)) = it.peek() {
            if !more(d) {
                break;
            }
            j = k + d.len_utf8();
            it.next();
        }
        out.push(&s[i..j]);
    }
    out
}

/// Admiralty credibility of a one-observation event (spec § Credibility).
pub fn credibility_single(r: &Region, sid: &str) -> i8 {
    match r.reliability(sid) {
        "A" | "B" => 3,
        "C" | "D" | "E" => 4,
        _ => 6,
    }
}

/// Id of claim k (an index or, for a FIRMS detection, a region id) of raw id.
pub fn obs_id(raw: &str, k: &str) -> String {
    sha(&format!("obs\0{raw}\0{k}"), 32)
}

/// Kinds of violence a statement is filed as ("Zelensky: 2 ways to halt strikes");
/// one that states no incident (`what::detail`) is a report instead.
const VIOLENT: [&str; 3] = [
    "battles",
    "explosions_remote_violence",
    "violence_against_civilians",
];

/// Observations of [(r, x, text)], raw report x of region r whose claim is `text`:
/// translated, geocoded, classified, graded, with the event time it states and, in a
/// region with a gazetteer, its incident detail (spec § Processing).
pub fn observations(Y: &[(&Region, &Report, String)]) -> Vec<Obs> {
    let E: Vec<String> = Y.iter().map(|(_, _, s)| translate(s)).collect();
    let K = classifier::predict(&E);
    Y.iter()
        .zip(E)
        .zip(K)
        .map(|(((r, x, text), en), kind)| {
            let g = geocode(r, &en);
            let t: Time = x.t();
            let fine = !r.gazetteer.countries.is_empty();
            let d = if fine && kind != "report" {
                what::detail(&en, g.kind, g.country.as_deref())
            } else {
                what::Detail::default()
            };
            let kind = if fine && !d.event && VIOLENT.contains(&kind.as_str()) {
                "report".into()
            } else {
                kind
            };
            let s = |x: Option<&str>| x.map(String::from);
            Obs {
                id: obs_id(&x.id, "0"),
                raw_id: x.id.clone(),
                source_id: x.source_id.clone(),
                region: r.id.clone(),
                t,
                lat: g.lat,
                lon: g.lon,
                geo_precision: g.precision.into(),
                kind,
                t_event: when::event_time(&en, t, &r.timezone),
                text_en: en,
                lang: lang(text).into(),
                credibility: credibility_single(r, &x.source_id),
                names: g.names,
                admin1: g.admin1,
                admin2: g.admin2,
                site: g.site,
                sub_event: s(d.sub_event),
                weapon: d.weapon.iter().map(|x| x.to_string()).collect(),
                missile: s(d.missile),
                target: s(d.target),
                killed: d.killed,
                injured: d.injured,
                actor1: s(d.actor1),
                actor2: s(d.actor2),
            }
        })
        .collect()
}

/// Give every observation located to a point, site or city without units the units of
/// its nearest populated place (`units_at`), so sensor detections roll up as text
/// reports do. Coarser locations are a unit's or country's centre and get none.
pub fn units(X: &mut [Obs]) {
    for x in X {
        if let (None, Some(lat), Some(lon), Some(r)) =
            (&x.admin1, x.lat, x.lon, regions::get(&x.region))
            && ["point", "site", "city"].contains(&x.geo_precision.as_str())
        {
            (x.admin1, x.admin2) = units_at(r, lat, lon);
        }
    }
}
