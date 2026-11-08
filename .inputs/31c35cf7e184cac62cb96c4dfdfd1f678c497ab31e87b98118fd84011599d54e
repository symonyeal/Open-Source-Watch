//! Taiwan Ministry of National Defense daily reports of PLA activity around Taiwan
//! (docs/SOURCES.md).
//!
//! The English list (`/en/news/plaactlist/<page>`, ten reports a page) links one
//! report a day. A report states its window, "6 a.m. Oct. 28 (Wed.) to 6 a.m. Oct. 29
//! (Thu.) (UTC+8)", and counts: sorties of PLA aircraft, PLAN ships, official ships,
//! balloons, and how many of the sorties entered the ADIZ or crossed the median line.
//! One raw report is one MND report, its native id the report number. These are a
//! party's own claims: graded in region.toml, shown as a dated series, never fused
//! into events. A report without a window (not a daily report) is skipped; a page
//! on which no report parses fails the poll.

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use chrono::{Datelike, Duration, TimeZone, Utc};
use regex::Regex;
use serde_json::{Map, Value, json};

use super::{EXCERPT, Fail, Item, Resp, Source, excerpt, response, stored, text};
use crate::fmt::{Time, fromiso, iso};
use crate::regions::Region;

const SITE: &str = "https://www.mnd.gov.tw";
const REGION: &str = "taiwan-strait";
/// Backfill depth, about 90 days.
const PAGES: usize = 9;
pub const CADENCE: f64 = 43200.0;
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
/// Counts of a payload; null where unstated.
pub const FIELDS: [&str; 6] = [
    "aircraft",
    "ships",
    "official",
    "balloons",
    "adiz",
    "median_line",
];

fn re(s: &str) -> Regex {
    Regex::new(s).unwrap()
}

static LINK: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r#"(?i)href="/en/News/PLAAct/(\d+)"[^>]*>\s*<div class="date[^"]*">\s*(\d{4}\.\d{2}\.\d{2})"#,
    )
});
static WINDOW: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?s)6\s*a\.m\.\s*([A-Za-z]{3})[a-z]*\.?\s*(\d{1,2})\b.*?to\s*6\s*a\.m\.\s*([A-Za-z]{3})[a-z]*\.?\s*(\d{1,2})\b.*?\(UTC\+8\)",
    )
});
static PUB: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)pageinfo.*?(\d{4})\.(\d{2})\.(\d{2})"));
static MOVED: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)(\d+)\s+out\s+of\s+\d+\s+sorties?\s+([^.]*)"));
static MEDIAN: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)median\s+line"));
static TAG: LazyLock<Regex> = LazyLock::new(|| re(r"<[^>]+>"));
static SPACE: LazyLock<Regex> = LazyLock::new(|| re(r"\s+"));
/// field: (pattern of n, pattern of an explicit none)
static COUNTS: LazyLock<Vec<(&str, Regex, Regex)>> = LazyLock::new(|| {
    [
        (
            "aircraft",
            r"(\d+)\s+sorties?\s+of\s+PLA\s+aircraft",
            r"\bno\s+PLA\s+aircraft",
        ),
        (
            "ships",
            r"(\d+)\s+PLAN\s+(?:ships?|vessels?)",
            r"\bno\s+PLAN\s+(?:ships?|vessels?)",
        ),
        (
            "official",
            r"(\d+)\s+official\s+ships?",
            r"\bno\s+official\s+ships?",
        ),
        (
            "balloons",
            r"(\d+)\s+(?:PRC\s+)?balloons?",
            r"\bno\s+(?:PRC\s+)?balloons?",
        ),
    ]
    .into_iter()
    .map(|(k, p, z)| (k, re(&format!("(?i){p}")), re(&format!("(?i){z}"))))
    .collect()
});

fn list(k: usize) -> String {
    if k == 1 {
        format!("{SITE}/en/news/plaactlist")
    } else {
        format!("{SITE}/en/news/plaactlist/{k}")
    }
}

pub fn sources(R: &[Region]) -> Vec<Source> {
    if !R.iter().any(|r| r.id == REGION) {
        return vec![];
    }
    vec![Source {
        id: "mnd:Taiwan MND".into(),
        region: REGION.into(),
        url: list(1),
        cadence: CADENCE,
        ..Default::default()
    }]
}

/// (report number, publication day) of one list page.
pub fn links(page: &str) -> Vec<(String, String)> {
    LINK.captures_iter(page)
        .map(|c| (c[1].to_string(), c[2].replace('.', "-")))
        .collect()
}

/// {report number: page html} of the reports not yet stored, from list pages
/// newest first; stops at the first page with a stored report, or after PAGES.
pub fn fetch(_src: &Source) -> Result<Resp, Fail> {
    let have: HashSet<String> = stored(&["mnd"], &[], None)
        .iter()
        .filter_map(|x| x.payload["id"].as_str().map(String::from))
        .collect();
    let mut out = Map::new();
    for k in 1..=PAGES {
        let L = links(&text(&list(k), 10.0)?);
        let new: Vec<&String> = L
            .iter()
            .map(|x| &x.0)
            .filter(|n| !have.contains(*n))
            .collect();
        for n in &new {
            out.insert(
                (*n).clone(),
                json!(text(&format!("{SITE}/en/News/PLAAct/{n}"), 10.0)?),
            );
        }
        if new.len() < L.len() || L.is_empty() {
            break;
        }
    }
    Ok(response(&Value::Object(out)))
}

/// Plain text of the report body (the 'maincontent' block up to its keywords).
fn body(page: &str) -> Result<String, String> {
    let i = page.find("maincontent").ok_or("MND report body missing")?;
    let s = htmlize::unescape(TAG.replace_all(&page[i..], " ").as_ref()).to_string();
    Ok(SPACE
        .replace_all(s.split("Keywords").next().unwrap(), " ")
        .trim()
        .to_string())
}

/// 06:00 UTC+8 of the stated month and day, in UTC; the year is the publication
/// year, moved by one across a December/January boundary.
pub fn local6(month: &str, day: &str, published: Time) -> Result<Time, String> {
    let m = MONTHS
        .iter()
        .position(|x| *x == month.to_lowercase())
        .ok_or("unknown month")? as u32
        + 1;
    let y = published.year() + (m == 1 && published.month() == 12) as i32
        - (m == 12 && published.month() == 1) as i32;
    let d: u32 = day.parse().map_err(|_| "bad day")?;
    let t = Utc
        .with_ymd_and_hms(y, m, d, 6, 0, 0)
        .single()
        .ok_or("day out of range for month")?;
    Ok(t - Duration::hours(8))
}

/// Payload of report n, or None if the page states no reporting window.
pub fn report(n: &str, page: &str) -> Result<Option<Value>, String> {
    let p = PUB.captures(page).ok_or("MND publication date missing")?;
    let published = Utc
        .with_ymd_and_hms(
            p[1].parse().unwrap(),
            p[2].parse().unwrap(),
            p[3].parse().unwrap(),
            0,
            0,
            0,
        )
        .single()
        .ok_or("bad publication date")?;
    let s = body(page)?;
    let Some(w) = WINDOW.captures(&s) else {
        return Ok(None);
    };
    let (start, end) = (
        local6(&w[1], &w[2], published)?,
        local6(&w[3], &w[4], published)?,
    );
    if !(end > start && end - start <= Duration::days(3)) {
        return Err("MND reporting window invalid".into());
    }
    let mut c: BTreeMap<&str, Value> = BTreeMap::new();
    for (k, p, z) in COUNTS.iter() {
        c.insert(
            k,
            match p.captures(&s) {
                Some(m) => json!(m[1].parse::<i64>().map_err(|e| e.to_string())?),
                None if z.is_match(&s) => json!(0),
                None => Value::Null,
            },
        );
    }
    let moved = MOVED.captures(&s);
    let clause = moved.as_ref().map_or("", |m| m.get(2).unwrap().as_str());
    let k = || json!(moved.as_ref().unwrap()[1].parse::<i64>().unwrap());
    c.insert(
        "adiz",
        if moved.is_some() && clause.contains("ADIZ") {
            k()
        } else {
            Value::Null
        },
    );
    c.insert(
        "median_line",
        if moved.is_some() && MEDIAN.is_match(clause) {
            k()
        } else {
            Value::Null
        },
    );
    if c.values().all(Value::is_null) {
        return Err("MND report states no counts".into());
    }
    let mut out = json!({"id": n, "published": published.date_naive().to_string(), "start": iso(&start),
                         "end": iso(&end), "day": (end + Duration::hours(8)).date_naive().to_string()});
    for k in FIELDS {
        out[k] = c[k].clone();
    }
    out["text"] = json!(excerpt(&s[w.get(0).unwrap().start()..], EXCERPT));
    Ok(Some(out))
}

pub fn parse(body: &[u8], _src: &Source) -> Result<Vec<Item>, String> {
    let P: Map<String, Value> = serde_json::from_slice(body).map_err(|e| e.to_string())?;
    let mut items = vec![];
    for (n, page) in &P {
        if let Some(p) = report(n, page.as_str().ok_or("not a page")?)? {
            items.push(Item {
                native_id: n.clone(),
                published_at: fromiso(p["end"].as_str().unwrap()),
                url: Some(format!("{SITE}/en/News/PLAAct/{n}")),
                payload: p,
            });
        }
    }
    if !P.is_empty() && items.is_empty() {
        return Err("No MND report on the page parsed".into());
    }
    Ok(items)
}

/// Payloads of region r's stored MND reports, one per reporting day (the highest
/// report number wins), in day order.
pub fn reports(r: &Region) -> Vec<Value> {
    let mut P: BTreeMap<String, Value> = BTreeMap::new();
    let num = |p: &Value| {
        p["id"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };
    for x in stored(&["mnd"], &[&r.id], None) {
        let d = x.payload["day"].as_str().unwrap_or("").to_string();
        if P.get(&d).is_none_or(|q| num(&x.payload) > num(q)) {
            P.insert(d, x.payload);
        }
    }
    P.into_values().collect()
}
