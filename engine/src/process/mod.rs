//! Processing, stage 3 of the cycle (spec § Processing): raw report → observation.
//!
//! Rule-based, no language models. Each adapter has a processor (news, feeds)
//! built from the functions below.

pub mod feeds;
pub mod news;
pub mod when;

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::{LazyLock, Mutex, OnceLock};

use serde_json::Value;

use crate::classifier;
use crate::collect::{Report, sha};
use crate::fmt::Time;
use crate::geo::km;
use crate::regions::{self, Region};
use crate::schema::{GEO_PRECISION, Obs};
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
fn google(s: &str) -> Option<String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=en&dt=t&q={}",
        crate::collect::quote_plus(s)
    );
    let r = crate::collect::get(&url, &[("User-Agent", "Mozilla/5.0".into())], 5.0).ok()?;
    if r.status >= 400 {
        return None;
    }
    let v: Value = serde_json::from_slice(&r.body).ok()?;
    let t: String = v[0]
        .as_array()?
        .iter()
        .filter_map(|p| p[0].as_str())
        .collect();
    Some(if t.is_empty() { s.to_string() } else { t })
}

/// English text of s; s itself if the endpoint fails. Cached on disk.
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
            return t.clone();
        }
    }
    let Some(t) = google(s) else {
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

const COUNTRIES: [&str; 32] = [
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

/// A gazetteer city: name, position, population.
pub struct Town {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub pop: i64,
}

/// regions/geonames.tsv, built by `osw gazetteer` (GeoNames cities15000).
pub fn geonames() -> &'static [Town] {
    static G: OnceLock<Vec<Town>> = OnceLock::new();
    G.get_or_init(|| {
        fs::read_to_string(regions::dir().join("geonames.tsv"))
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                Some(Town {
                    name: f.first()?.to_string(),
                    lat: f.get(1)?.parse().ok()?,
                    lon: f.get(2)?.parse().ok()?,
                    pop: f.get(4)?.parse().ok()?,
                })
            })
            .collect()
    })
}

/// '<city> (<d> km)', the gazetteer city of at least pop people nearest to
/// (lat, lon); None without one.
pub fn nearest(lat: f64, lon: f64, pop: i64) -> Option<String> {
    let mut best: Option<(f64, &Town)> = None;
    for t in geonames().iter().filter(|t| t.pop >= pop) {
        let d = km(lat, lon, t.lat, t.lon);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, t));
        }
    }
    best.map(|(d, t)| format!("{} ({d:.0} km)", t.name))
}

/// A gazetteer place: position and precision.
type Place = (f64, f64, &'static str);

/// (L, C): the conflict's curated strikeLocations, matched case-insensitively, and
/// GeoNames cities inside the region's box, matched as capitalised words. Names
/// under 5 letters need population >= 500,000 ("Hit", "Kut" are words).
pub fn gazetteer(r: &Region) -> &'static (HashMap<String, Place>, HashMap<String, Place>) {
    static G: LazyLock<
        Mutex<HashMap<String, &'static (HashMap<String, Place>, HashMap<String, Place>)>>,
    > = LazyLock::new(Default::default);
    let mut g = G.lock().unwrap();
    g.entry(r.id.clone()).or_insert_with(|| {
        let mut L = HashMap::new();
        for (k, v) in r
            .client("strikeLocations")
            .as_object()
            .into_iter()
            .flatten()
        {
            let p = if COUNTRIES.contains(&k.as_str()) {
                "country"
            } else {
                "city"
            };
            L.insert(
                k.clone(),
                (v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), p),
            );
        }
        let mut T: Vec<&Town> = geonames().iter().collect();
        T.sort_by_key(|t| t.pop); // largest wins a name
        let mut C = HashMap::new();
        for t in T {
            if r.in_bbox(t.lat, t.lon) && (t.name.chars().count() >= 5 || t.pop >= 500_000) {
                C.insert(t.name.clone(), (t.lat, t.lon, "city"));
            }
        }
        Box::leak(Box::new((L, C)))
    })
}

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
fn words(s: &str) -> Vec<&str> {
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

/// (lat, lon, precision, names) of the most specific place named in s: the finest
/// precision, then the longest name. O(len(s)) phrase lookups.
pub fn geocode(r: &Region, s: &str) -> (Option<f64>, Option<f64>, &'static str, Vec<String>) {
    let (L, C) = gazetteer(r);
    let w = words(s);
    let mut hits: Vec<(String, Place)> = vec![];
    let mut put = |k: String, p: Place| match hits.iter_mut().find(|h| h.0 == k) {
        Some(h) => h.1 = p,
        None => hits.push((k, p)),
    };
    for n in [3, 2, 1] {
        for i in 0..(w.len() + 1).saturating_sub(n) {
            let g = w[i..i + n].join(" ");
            let gl = g.to_lowercase();
            if let Some(p) = L.get(&gl) {
                put(gl, *p);
            } else if let Some(p) = C
                .get(&g)
                .filter(|_| g.chars().next().is_some_and(char::is_uppercase))
            {
                put(g, *p);
            }
        }
    }
    let order = |p: &str| GEO_PRECISION.iter().position(|x| *x == p).unwrap();
    let Some(best) = hits
        .iter()
        .min_by_key(|(k, p)| (order(p.2), std::cmp::Reverse(k.chars().count())))
    else {
        return (None, None, "none", vec![]);
    };
    (
        Some(best.1.0),
        Some(best.1.1),
        best.1.2,
        hits.iter().map(|h| h.0.clone()).collect(),
    )
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

/// Observations of [(r, x, text)], raw report x of region r whose claim is `text`:
/// translated, geocoded, classified, graded, with the event time it states
/// (spec § Processing).
pub fn observations(Y: &[(&Region, &Report, String)]) -> Vec<Obs> {
    let E: Vec<String> = Y.iter().map(|(_, _, s)| translate(s)).collect();
    let K = classifier::predict(&E);
    Y.iter()
        .zip(E)
        .zip(K)
        .map(|(((r, x, text), en), kind)| {
            let (lat, lon, prec, names) = geocode(r, &en);
            let t: Time = x.t();
            Obs {
                id: obs_id(&x.id, "0"),
                raw_id: x.id.clone(),
                source_id: x.source_id.clone(),
                region: r.id.clone(),
                t,
                lat,
                lon,
                geo_precision: prec.into(),
                kind,
                t_event: when::event_time(&en, t, &r.timezone),
                text_en: en,
                lang: lang(text).into(),
                credibility: credibility_single(r, &x.source_id),
                names,
            }
        })
        .collect()
}

/// Write regions/geonames.tsv from GeoNames cities15000.zip (geonames.org export
/// dump, CC BY 4.0): name and ASCII name of every city inside some region's box.
/// Returns the rows written; takes effect at the next start.
pub fn build_geonames(zip_path: &str) -> usize {
    let mut z = zip::ZipArchive::new(fs::File::open(zip_path).unwrap()).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut z.by_name("cities15000.txt").unwrap(), &mut text).unwrap();
    let mut out: std::collections::BTreeMap<(String, String), [String; 5]> = Default::default();
    for l in text.lines() {
        // geonameid name asciiname alternatenames lat lon ... country(8) ... population(14)
        let f: Vec<&str> = l.split('\t').collect();
        let (Ok(lat), Ok(lon)) = (f[4].parse::<f64>(), f[5].parse::<f64>()) else {
            continue;
        };
        if regions::all().iter().any(|r| r.in_bbox(lat, lon)) {
            for n in [f[1], f[2]] {
                out.insert(
                    (n.into(), f[8].into()),
                    [n, f[4], f[5], f[8], f[14]].map(String::from),
                );
            }
        }
    }
    let mut rows: Vec<&[String; 5]> = out.values().collect();
    rows.sort();
    let body: String = rows.iter().map(|x| x.join("\t") + "\n").collect();
    fs::write(
        regions::dir().join("geonames.tsv"),
        format!("name\tlat\tlon\tcountry\tpopulation\n{body}"),
    )
    .unwrap();
    rows.len()
}
