//! Places: the gazetteer and geocoding (spec § Processing, geolocation).
//!
//! The gazetteer is `regions/geonames.tsv`, written by `osw gazetteer` from GeoNames
//! (geonames.org export dump, CC BY 4.0). It holds the cities of `cities15000` in every
//! region's box and, for each country a region lists under `[gazetteer]`, every
//! populated place with its first- and second-order administrative units (ADM1:
//! oblast, krai; ADM2: raion). Every place carries the names of its two units, so a
//! report located to a village rolls up to raion and oblast.
//!
//! Legend: g, a gazetteer row and its index; a1, a2 the names of a place's first- and
//! second-order units; K the candidate rows of one name.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::sync::{LazyLock, Mutex, OnceLock};

use crate::geo::km;
use crate::regions::{self, Region};
use crate::schema::GEO_PRECISION;

use super::{COUNTRIES, words};

/// One gazetteer row. `kind` is "city" (in cities15000), "village" (any other
/// populated place), "adm1" or "adm2"; a unit row's `name` is one name the unit is
/// called by before a unit word ("Kharkiv" in "Kharkiv Oblast", "Dnipro" in "Dnipro
/// region"), and its position is GeoNames' point for the unit.
pub struct Town {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub country: String,
    pub pop: i64,
    pub kind: String,
    pub a1: String,
    pub a2: String,
}

const HEADER: &str = "name\tlat\tlon\tcountry\tpopulation\tkind\tadmin1\tadmin2";

/// regions/geonames.tsv, built by `osw gazetteer`.
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
                    country: f.get(3)?.to_string(),
                    pop: f.get(4)?.parse().ok()?,
                    kind: f.get(5)?.to_string(),
                    a1: f.get(6)?.to_string(),
                    a2: f.get(7)?.to_string(),
                })
            })
            .collect()
    })
}

/// '<city> (<d> km)', the gazetteer city of at least pop people nearest to
/// (lat, lon); None without one.
pub fn nearest(lat: f64, lon: f64, pop: i64) -> Option<String> {
    let mut best: Option<(f64, &Town)> = None;
    for t in geonames()
        .iter()
        .filter(|t| t.pop >= pop && t.kind == "city")
    {
        let d = km(lat, lon, t.lat, t.lon);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, t));
        }
    }
    best.map(|(d, t)| format!("{} ({d:.0} km)", t.name))
}

/// Words after a name that make it a first-order unit ("Kharkiv Oblast", "Dnipro
/// region", "Dnipropetrovsk OVA", the oblast military administration) or a
/// second-order one ("Chuhuiv district", "Synelnykove RVA").
const ADM1_WORDS: [&str; 9] = [
    "oblast",
    "region",
    "province",
    "krai",
    "kray",
    "republic",
    "governorate",
    "ova",
    "oda",
];
const ADM2_WORDS: [&str; 4] = ["raion", "rayon", "district", "rva"];

/// Words after a town's name that make it a front sector ("Pokrovsk axis",
/// "Zaporozhye direction"): located to the town's oblast, not the town.
const AXIS_WORDS: [&str; 4] = ["front", "direction", "axis", "sector"];

/// Words after a town's name that make it something other than a place: a force
/// grouping ("Vostok group") or the government speaking ("Kyiv says").
const NOT_PLACE_WORDS: [&str; 22] = [
    "group",
    "grouping",
    "says",
    "said",
    "claims",
    "claimed",
    "reports",
    "reported",
    "accuses",
    "accused",
    "warns",
    "warned",
    "denies",
    "denied",
    "confirms",
    "confirmed",
    "announces",
    "announced",
    "insists",
    "rejects",
    "agrees",
    "agreed",
];

/// Words one of which must precede a village's name for it to be located ("in
/// Lozova", "past Snovsk and Horodnia"): a village shares its name with people and
/// things far more often than a city does ("Yana Peshkova", "Mayor Filatov").
const VILLAGE_CUES: [&str; 18] = [
    "in", "near", "at", "on", "of", "from", "to", "towards", "toward", "past", "outside", "around",
    "over", "into", "by", "between", "and", "via",
];

/// A located place: position, precision, the names of its units, and its name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geo {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub precision: &'static str,
    pub admin1: Option<String>,
    pub admin2: Option<String>,
    /// The chosen place's name first, then every other place named, in text order.
    pub names: Vec<String>,
}

/// One region's index: the conflict's curated strikeLocations (L, lower case), and
/// gazetteer rows inside the box by name: populated places (C, matched as
/// capitalised words; names under 5 letters need population >= 500,000, since "Hit"
/// and "Kut" are words) and units (U1, U2, matched before a unit word). A village or
/// unit row is indexed only for a region that lists its country.
struct Index {
    L: HashMap<String, (f64, f64, &'static str)>,
    C: HashMap<String, Vec<usize>>,
    U1: HashMap<String, Vec<usize>>,
    U2: HashMap<String, Vec<usize>>,
    /// Populated rows by 0.5° cell, for the units of a point (`units_at`).
    cells: HashMap<(i32, i32), Vec<usize>>,
    /// One first-order unit row by the unit's name (a place's `a1`).
    oblast: HashMap<String, usize>,
}

fn cell(lat: f64, lon: f64) -> (i32, i32) {
    ((lat * 2.0).floor() as i32, (lon * 2.0).floor() as i32)
}

fn index(r: &Region) -> &'static Index {
    static G: LazyLock<Mutex<HashMap<String, &'static Index>>> = LazyLock::new(Default::default);
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
        let fine: HashSet<&str> = r.gazetteer.countries.iter().map(String::as_str).collect();
        let mut I = Index {
            L,
            C: HashMap::new(),
            U1: HashMap::new(),
            U2: HashMap::new(),
            cells: HashMap::new(),
            oblast: HashMap::new(),
        };
        for (a, t) in geonames().iter().enumerate() {
            // A listed country's cities count wherever they lie; everything else
            // only inside the box, and villages and units only for listed countries.
            let listed = fine.contains(t.country.as_str());
            let keep = match t.kind.as_str() {
                "city" | "adm1" => listed || (t.kind == "city" && r.in_bbox(t.lat, t.lon)),
                _ => listed && r.in_bbox(t.lat, t.lon),
            };
            if !keep {
                continue;
            }
            let M = match t.kind.as_str() {
                "adm1" => &mut I.U1,
                "adm2" => &mut I.U2,
                _ if t.name.chars().count() >= 5 || t.pop >= 500_000 => &mut I.C,
                _ => continue,
            };
            M.entry(t.name.clone()).or_default().push(a);
            if t.kind == "adm1" {
                I.oblast.entry(t.a1.clone()).or_insert(a);
            }
            if t.kind == "city" || t.kind == "village" {
                I.cells.entry(cell(t.lat, t.lon)).or_default().push(a);
            }
        }
        Box::leak(Box::new(I))
    })
}

/// The row of K a name denotes: K restricted to units A1 named in the same text when
/// any of K lies in one, then its only row; else its most populous, if a city (cities
/// have known populations) or ten times as populous as the next, both known. GeoNames
/// writes 0 for an unknown population, so the 25 Ukrainian villages called Pokrovske,
/// or a Kalinin of 3,423 people among twenty of unknown size, are not told apart by a
/// name alone.
fn resolve(K: &[usize], A1: &[&str]) -> Option<usize> {
    let G = geonames();
    let mut K: Vec<usize> = K.to_vec();
    if K.iter().any(|&a| A1.contains(&G[a].a1.as_str())) {
        K.retain(|&a| A1.contains(&G[a].a1.as_str()));
    }
    let city = |a: usize| G[a].kind == "city";
    K.sort_by_key(|&a| (!city(a), std::cmp::Reverse(G[a].pop), a));
    match K[..] {
        [a] => Some(a),
        [a, ..] if city(a) => Some(a),
        [a, b, ..] if G[b].pop > 0 && G[a].pop >= 10 * G[b].pop => Some(a),
        _ => None,
    }
}

/// Units (a1, a2) of the populated place nearest to (lat, lon) within 30 km, among
/// the region's indexed places; (None, None) without one. Cost: the places of the
/// 3 × 3 cells of 0.5° around the point.
pub fn units_at(r: &Region, lat: f64, lon: f64) -> (Option<String>, Option<String>) {
    let (I, G) = (index(r), geonames());
    let (i, j) = cell(lat, lon);
    let mut best: Option<(f64, usize)> = None;
    for di in -1..=1 {
        for dj in -1..=1 {
            for &a in I.cells.get(&(i + di, j + dj)).into_iter().flatten() {
                let d = km(lat, lon, G[a].lat, G[a].lon);
                if d <= 30.0 && best.is_none_or(|b| (d, a) < b) {
                    best = Some((d, a));
                }
            }
        }
    }
    best.map_or((None, None), |(_, a)| units(&G[a]))
}

fn units(t: &Town) -> (Option<String>, Option<String>) {
    let s = |x: &str| (!x.is_empty()).then(|| x.to_string());
    (s(&t.a1), s(&t.a2))
}

/// The most specific place named in s: the finest precision (a second-order unit
/// before a first-order one), then the first named; a town after an oblast the text
/// names, and outside every oblast named before it, ranks below those oblasts. A
/// name followed by a unit word is that unit, never the town of the same name
/// ("Moscow region" is Moscow Oblast, not the federal city). In a region that lists
/// gazetteer countries, a town directly followed by a word of `AXIS_WORDS` is its
/// oblast, and one directly followed by a word of `NOT_PLACE_WORDS` locates nothing.
/// A village not after a word of `VILLAGE_CUES`, a phrase of the region's
/// `not_places`, and an ambiguous name (`resolve`) locate nothing. O(len(s)) phrase
/// lookups.
pub fn geocode(r: &Region, s: &str) -> Geo {
    let (I, G) = (index(r), geonames());
    let w = words(s);
    let lower: Vec<String> = w.iter().map(|x| x.to_lowercase()).collect();
    // Byte offsets in s of a word (a slice of s).
    let start = |x: &str| x.as_ptr() as usize - s.as_ptr() as usize;
    let end = |x: &str| start(x) + x.len();
    let fine = !r.gazetteer.countries.is_empty();
    struct Hit {
        name: String,
        at: usize,
        lat: f64,
        lon: f64,
        precision: &'static str,
        a1: Option<String>,
        a2: Option<String>,
    }
    let unit_hit = |a: usize, name: String, at: usize| {
        let (a1, a2) = units(&G[a]);
        Hit {
            name,
            at,
            lat: G[a].lat,
            lon: G[a].lon,
            precision: "admin",
            a1,
            a2,
        }
    };
    let mut hits: Vec<Hit> = vec![];
    let mut put = |h: Hit| match hits.iter_mut().find(|x| x.name == h.name) {
        Some(x) => {
            *x = Hit {
                at: x.at.min(h.at),
                ..h
            }
        }
        None => hits.push(h),
    };
    let mut used = vec![false; w.len()];
    for p in &r.gazetteer.not_places {
        let q: Vec<&str> = p.split(' ').collect();
        for i in 0..(w.len() + 1).saturating_sub(q.len()) {
            if w[i..i + q.len()] == q[..] {
                used[i..i + q.len()].fill(true);
            }
        }
    }
    // Units first, oblasts before raions: a unit claims its words, and the oblasts
    // named (A1, with where they are named) resolve raions and towns.
    let mut A1: Vec<(String, usize)> = vec![];
    for (U, unit_words) in [(&I.U1, &ADM1_WORDS[..]), (&I.U2, &ADM2_WORDS[..])] {
        let A: Vec<&str> = A1.iter().map(|x| x.0.as_str()).collect();
        let mut found = vec![];
        for n in [3, 2, 1] {
            for i in 0..(w.len() + 1).saturating_sub(n) {
                let next = lower.get(i + n).map_or("", String::as_str);
                if used[i..i + n].iter().any(|u| *u) || !unit_words.contains(&next) {
                    continue;
                }
                let g = w[i..i + n].join(" ");
                let Some(K) = U.get(&g) else { continue };
                // A federal city shares its name with its oblast; a region is the oblast.
                let oblast = format!("{g} Oblast");
                let K: Vec<usize> = match K.iter().find(|&&a| G[a].a1 == oblast) {
                    Some(&a) => vec![a],
                    None => K.clone(),
                };
                if let Some(a) = resolve(&K, &A) {
                    used[i..=i + n].fill(true);
                    found.push(unit_hit(a, format!("{g} {}", w[i + n]), i));
                }
            }
        }
        A1.extend(found.iter().filter_map(|h| Some((h.a1.clone()?, h.at))));
        found.into_iter().for_each(&mut put);
    }
    let A: Vec<&str> = A1.iter().map(|x| x.0.as_str()).collect();
    for n in [3, 2, 1] {
        for i in 0..(w.len() + 1).saturating_sub(n) {
            if used[i..i + n].iter().any(|u| *u) {
                continue;
            }
            // The word that directly follows, with nothing but space between
            // ("Hormuz, says" is followed by no word), in a fine-gazetteer region.
            let next = match w.get(i + n) {
                Some(x) if fine && s[end(w[i + n - 1])..start(x)].trim().is_empty() => {
                    lower[i + n].as_str()
                }
                _ => "",
            };
            let g = w[i..i + n].join(" ");
            let gl = g.to_lowercase();
            let town = I.L.get(&gl).is_none_or(|p| p.2 == "city");
            if town && NOT_PLACE_WORDS.contains(&next) {
                continue;
            }
            let hit = if let Some(&(lat, lon, precision)) = I.L.get(&gl) {
                let (a1, a2) = if precision == "city" {
                    units_at(r, lat, lon)
                } else {
                    (None, None)
                };
                Hit {
                    name: gl,
                    at: i,
                    lat,
                    lon,
                    precision,
                    a1,
                    a2,
                }
            } else if g.chars().next().is_some_and(char::is_uppercase)
                && let Some(a) = I.C.get(&g).and_then(|K| resolve(K, &A))
                && (G[a].kind == "city"
                    || i.checked_sub(1)
                        .is_some_and(|j| VILLAGE_CUES.contains(&lower[j].as_str())))
            {
                let (a1, a2) = units(&G[a]);
                Hit {
                    name: g,
                    at: i,
                    lat: G[a].lat,
                    lon: G[a].lon,
                    precision: "city",
                    a1,
                    a2,
                }
            } else {
                continue;
            };
            if !AXIS_WORDS.contains(&next) {
                put(hit);
            } else if let Some(&o) = hit.a1.as_ref().and_then(|a| I.oblast.get(a)) {
                put(unit_hit(o, format!("{} {}", hit.name, w[i + n]), i));
            }
        }
    }
    hits.sort_by_key(|h| h.at);
    let outside = |h: &Hit| {
        h.precision == "city"
            && A1.iter().any(|x| x.1 < h.at)
            && !A1
                .iter()
                .any(|x| x.1 < h.at && h.a1.as_deref() == Some(x.0.as_str()))
    };
    let order = |h: &Hit| {
        (
            outside(h),
            GEO_PRECISION
                .iter()
                .position(|x| *x == h.precision)
                .unwrap(),
            h.precision == "admin" && h.a2.is_none(),
            h.at,
        )
    };
    let Some(b) = (0..hits.len()).min_by_key(|&k| order(&hits[k])) else {
        return Geo {
            precision: "none",
            ..Default::default()
        };
    };
    let mut names = vec![hits[b].name.clone()];
    names.extend(
        (0..hits.len())
            .filter(|&k| k != b)
            .map(|k| hits[k].name.clone()),
    );
    let h = hits.swap_remove(b);
    Geo {
        lat: Some(h.lat),
        lon: Some(h.lon),
        precision: h.precision,
        admin1: h.a1,
        admin2: h.a2,
        names,
    }
}

/// Suffixes stripped from a unit's GeoNames name to give the name used before a unit
/// word ("Kyiv Oblast" -> "Kyiv"); "Kyiv City" keeps its suffix, so "Kyiv region" is
/// the oblast.
const UNIT_SUFFIXES: [&str; 6] = [" Oblast", " Krai", " Kray", " Republic", " Raion", " Rayon"];

fn strip(s: &str) -> &str {
    UNIT_SUFFIXES
        .iter()
        .find_map(|x| s.strip_suffix(x))
        .unwrap_or(s)
}

fn read_zip(path: &std::path::Path, name: &str) -> String {
    let mut z = zip::ZipArchive::new(
        fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

/// Names of GeoNames row f: its name and ASCII name and, if `alt`, its Latin-script
/// alternate names of at least 5 letters that begin with a capital (the Russian
/// spellings "Kharkov", "Zaporozhye" of Ukrainian towns, as translations render them).
fn names<'a>(f: &[&'a str], alt: bool) -> Vec<&'a str> {
    let latin = |c: char| (c as u32) < 0x250 || ('\u{1E00}'..='\u{1EFF}').contains(&c);
    let mut N = vec![f[1], f[2]];
    if alt {
        N.extend(f[3].split(',').filter(|a| {
            a.chars().count() >= 5
                && a.chars().next().is_some_and(char::is_uppercase)
                && a.chars()
                    .all(|c| latin(c) && !c.is_ascii_digit() && c != '(')
        }));
    }
    N.sort();
    N.dedup();
    N
}

/// Write regions/geonames.tsv from the GeoNames files in directory d: cities15000.zip,
/// admin1CodesASCII.txt, admin2Codes.txt and <CC>.zip for each country a region lists
/// under `[gazetteer]`. Rows lie inside some region's box, except that a listed
/// country's cities are kept whole (a strike on Omsk is news in the Russia/Ukraine
/// region). Cities, towns of 1,000 people or more and unit seats carry their
/// alternate names (`names`); villages their own. A unit is named by its stripped
/// name and by its seat's names (PPLA, PPLA2), each before a unit word. Returns the
/// rows written; takes effect at the next start.
pub fn build_geonames(d: &str) -> usize {
    let d = std::path::Path::new(d);
    let codes = |f: &str| -> HashMap<String, String> {
        fs::read_to_string(d.join(f))
            .unwrap_or_else(|e| panic!("{f}: {e}"))
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                Some((f.first()?.to_string(), f.get(1)?.to_string()))
            })
            .collect()
    };
    let (N1, N2) = (codes("admin1CodesASCII.txt"), codes("admin2Codes.txt"));
    let R = regions::all();
    // geonameid name asciiname alternatenames lat lon class(6) code(7) country(8) cc2
    // admin1(10) admin2(11) admin3 admin4 population(14)
    let mut out: BTreeMap<(String, String), [String; 8]> = BTreeMap::new();
    let mut row = |id: &str, name: &str, f: &[&str], kind: &str| {
        let k1 = format!("{}.{}", f[8], f[10]);
        let a1 = N1.get(&k1).cloned().unwrap_or_default();
        let a2 = N2
            .get(&format!("{k1}.{}", f[11]))
            .cloned()
            .unwrap_or_default();
        out.insert(
            (id.into(), name.into()),
            [name, f[4], f[5], f[8], f[14], kind, &a1, &a2].map(String::from),
        );
    };
    let inside = |f: &[&str], fine: Option<&str>| {
        let (Ok(lat), Ok(lon)) = (f[4].parse::<f64>(), f[5].parse::<f64>()) else {
            return false;
        };
        R.iter().any(|r| {
            r.in_bbox(lat, lon) && fine.is_none_or(|c| r.gazetteer.countries.iter().any(|x| x == c))
        })
    };
    let mut C: Vec<&String> = R.iter().flat_map(|r| &r.gazetteer.countries).collect();
    C.sort();
    C.dedup();
    let cities = read_zip(&d.join("cities15000.zip"), "cities15000.txt");
    // A village called Paris or London is not what a report naming it means.
    let world: HashSet<&str> = cities
        .lines()
        .flat_map(|l| l.split('\t').skip(1).take(2))
        .collect();
    let mut city: HashSet<String> = HashSet::new();
    for l in cities.lines() {
        let f: Vec<&str> = l.split('\t').collect();
        let listed = C.iter().any(|c| *c == f[8]);
        if inside(&f, None) || listed {
            city.insert(f[0].into());
            for n in names(&f, listed) {
                row(f[0], n, &f, "city");
            }
        }
    }
    for c in C {
        let text = read_zip(&d.join(format!("{c}.zip")), &format!("{c}.txt"));
        let F: Vec<Vec<&str>> = text
            .lines()
            .map(|l| l.split('\t').collect::<Vec<_>>())
            .filter(|f| f.len() > 14 && (inside(f, Some(c)) || ["ADM1", "PPLA"].contains(&f[7])))
            .collect();
        // Seats by unit: PPLA names its ADM1 (admin1 code), PPLA2 its ADM2.
        let mut seat: HashMap<(String, String, &str), Vec<&str>> = HashMap::new();
        for f in &F {
            let k = match f[7] {
                "PPLA" => (f[10].into(), String::new(), "adm1"),
                "PPLA2" => (f[10].into(), f[11].into(), "adm2"),
                _ => continue,
            };
            seat.entry(k).or_default().extend(names(f, true));
        }
        for f in &F {
            match (f[6], f[7]) {
                ("P", _) if !city.contains(f[0]) && inside(f, Some(c)) => {
                    let town = f[14].parse::<i64>().is_ok_and(|p| p >= 1000);
                    for n in names(f, town).into_iter().filter(|n| !world.contains(n)) {
                        row(f[0], n, f, "village");
                    }
                }
                ("A", "ADM1") | ("A", "ADM2") => {
                    let (kind, k2) = if f[7] == "ADM1" {
                        ("adm1", "")
                    } else {
                        ("adm2", f[11])
                    };
                    let own = if kind == "adm1" {
                        N1.get(&format!("{c}.{}", f[10]))
                    } else {
                        N2.get(&format!("{c}.{}.{}", f[10], f[11]))
                    };
                    let Some(own) = own else { continue };
                    let mut names = vec![strip(own)];
                    names.extend(
                        seat.get(&(f[10].into(), k2.into(), kind))
                            .into_iter()
                            .flatten(),
                    );
                    for n in names {
                        row(f[0], n, f, kind);
                    }
                }
                _ => {}
            }
        }
    }
    let mut rows: Vec<&[String; 8]> = out.values().collect();
    rows.sort();
    let body: String = rows.iter().map(|x| x.join("\t") + "\n").collect();
    fs::write(
        regions::dir().join("geonames.tsv"),
        format!("{HEADER}\n{body}"),
    )
    .unwrap();
    rows.len()
}
