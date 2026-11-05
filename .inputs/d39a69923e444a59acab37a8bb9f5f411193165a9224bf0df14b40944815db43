//! Collection, stage 2 of the cycle (spec § Intelligence cycle).
//!
//! A source is {id, region, url, cadence, headers, ...}; `region` is a region id or
//! "global". An adapter module exposes
//!
//!     sources(R)       -> its sources, R the region registry
//!     parse(body, src) -> items {native_id, published_at, url, payload}
//!     fetch(src)       -> a body assembled from several requests (optional;
//!                         default: GET src.url)
//!
//! `run` fetches one URL and parses it for every source sharing that URL. A failed
//! fetch or parse is the run's status, never an error (spec § Errors).
//!
//! A payload's format belongs to its adapter: the adapter writes it (parse), and its
//! processor (crate::process) and its readers read it. Other modules decode no
//! payload: they read raw reports through `stored`, or through the adapter's reader
//! where its rule picks among the stored reports (mnd::reports, ioda::events,
//! ucdp::events, context::history, rss::outlets).

pub mod adsb;
pub mod ais;
pub mod alerts;
pub mod context;
pub mod firms;
pub mod gpsjam;
pub mod integrity;
pub mod ioda;
pub mod markets;
pub mod mnd;
pub mod neptun;
pub mod rss;
pub mod telegram;
pub mod ucdp;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use chrono::{SubsecRound, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::fmt::{self, Time};
use crate::schema::{Raw, Run};
use crate::store;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Open-Source-Watch/0.1";
/// Characters of article or post text kept (spec § Data model).
pub const EXCERPT: usize = 280;

#[derive(Clone, Debug, Default)]
pub struct Source {
    pub id: String,
    pub region: String,
    pub url: String,
    /// Seconds between polls.
    pub cadence: f64,
    pub headers: Vec<(&'static str, String)>,
    /// A stream (AIS), not polled.
    pub stream: bool,
    /// Feed name (rss, "Google News" for gnews) or series name (context).
    pub name: String,
    /// Variant: series kind (context), quote kind (markets), provider (alerts).
    pub kind: String,
    /// Series code (context) or country code (ioda).
    pub code: String,
    /// Countries of a World Bank series.
    pub countries: Vec<String>,
}

pub struct Item {
    pub native_id: String,
    pub published_at: Option<Time>,
    pub url: Option<String>,
    pub payload: Value,
}

/// A fetched response: status, content type, body.
#[derive(Clone, Debug)]
pub struct Resp {
    pub status: u16,
    pub ctype: Option<String>,
    pub body: Vec<u8>,
}

/// Why a poll failed: its collection_run status.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fail {
    Blocked,
    Http,
    Parse,
}

impl Fail {
    pub fn status(self) -> &'static str {
        match self {
            Fail::Blocked => "blocked",
            Fail::Http => "http_error",
            Fail::Parse => "parse_error",
        }
    }
}

/// Now, to the microsecond, as every stored time.
pub fn now() -> Time {
    Utc::now().trunc_subsecs(6)
}

/// Time of epoch milliseconds.
pub fn epoch(ms: f64) -> Time {
    fmt::epoch(ms / 1000.0)
}

/// Adapter of a source id "<adapter>:<name>[#k]".
pub fn adapter(sid: &str) -> &str {
    sid.split_once(':').map_or(sid, |x| x.0)
}

pub fn sha(s: &str, n: usize) -> String {
    let h = Sha256::digest(s.as_bytes());
    h.iter().map(|b| format!("{b:02x}")).collect::<String>()[..n].to_string()
}

/// Content hash of one item; regions sharing a source keep separate reports.
pub fn raw_id(region: &str, sid: &str, native: &str) -> String {
    sha(&format!("{region}\0{sid}\0{native}"), 32)
}

/// HTTP agent with the operating system's TLS and root certificates. ureq's default
/// roots (WebPKI) passed to native-tls as the only trusted roots fail on Windows
/// ("unable to find any user-specified roots in the final cert chain", 2026-11-03).
fn agent(timeout: f64) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs_f64(timeout)))
        .http_status_as_error(false)
        .tls_config(
            TlsConfig::builder()
                .provider(TlsProvider::NativeTls)
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

/// GET url, following redirects; any status is a response.
pub fn get(url: &str, headers: &[(&str, String)], timeout: f64) -> Result<Resp, Fail> {
    if std::env::var_os("OSW_OFFLINE").is_some() {
        return Err(Fail::Http);
    }
    let mut q = agent(timeout).get(url).header("User-Agent", UA);
    for (k, v) in headers {
        q = q.header(*k, v);
    }
    let mut res = q.call().map_err(|_| Fail::Http)?;
    let ctype = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let body = res
        .body_mut()
        .with_config()
        .limit(1 << 30)
        .read_to_vec()
        .map_err(|_| Fail::Http)?;
    Ok(Resp {
        status: res.status().as_u16(),
        ctype,
        body,
    })
}

/// GET src.url; a status >= 400 fails the poll (401, 403, 429: blocked).
pub fn fetch(src: &Source, timeout: f64) -> Result<Resp, Fail> {
    let r = get(&src.url, &src.headers, timeout)?;
    match r.status {
        401 | 403 | 429 => Err(Fail::Blocked),
        s if s >= 400 => Err(Fail::Http),
        _ => Ok(r),
    }
}

/// GET url's body as text.
pub fn text(url: &str, timeout: f64) -> Result<String, Fail> {
    let s = Source {
        url: url.into(),
        ..Default::default()
    };
    Ok(String::from_utf8_lossy(&fetch(&s, timeout)?.body).into_owned())
}

/// Response of a fetch that assembles several requests into one body: v as JSON.
pub fn response(v: &Value) -> Resp {
    Resp {
        status: 200,
        ctype: Some("application/json".into()),
        body: v.to_string().into_bytes(),
    }
}

pub type Parse = fn(&[u8], &Source) -> Result<Vec<Item>, String>;

/// One poll of the URL shared by sources S, fetched by `get`, each source parsed by
/// `parse`. Returns (response or None, raw_report rows, collection_run rows).
pub fn run(
    S: &[Source],
    parse: impl Fn(&[u8], &Source) -> Result<Vec<Item>, String>,
    runner: &str,
    get: impl FnOnce(&Source) -> Result<Resp, Fail>,
) -> (Option<Resp>, Vec<Raw>, Vec<Run>) {
    let (t0, started) = (Instant::now(), now());
    let res = get(&S[0]);
    let (mut raw, mut runs) = (vec![], vec![]);
    for src in S {
        let (items, status) = match &res {
            Err(e) => (vec![], e.status()),
            Ok(r) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse(&r.body, src)))
                {
                    Ok(Ok(v)) => {
                        let s = if v.is_empty() { "empty" } else { "ok" };
                        (v, s)
                    }
                    _ => (vec![], Fail::Parse.status()),
                }
            }
        };
        let n = items.len();
        raw.extend(items.into_iter().map(|it| Raw {
            id: raw_id(&src.region, &src.id, &it.native_id),
            source_id: src.id.clone(),
            region: src.region.clone(),
            fetched_at: started,
            published_at: it.published_at,
            url: it.url,
            payload: fmt::dumps(&it.payload, None, false, false),
        }));
        runs.push(Run {
            source_id: src.id.clone(),
            region: src.region.clone(),
            started_at: started,
            status: status.into(),
            n_rows: n as i32,
            latency_ms: t0.elapsed().as_millis() as i32,
            runner: runner.into(),
        });
    }
    (res.ok(), raw, runs)
}

/// A raw report with its payload decoded.
#[derive(Clone, Debug)]
pub struct Report {
    pub id: String,
    pub source_id: String,
    pub region: String,
    pub fetched_at: Time,
    pub published_at: Option<Time>,
    pub url: Option<String>,
    pub payload: Value,
}

impl Report {
    /// The report's date: published_at, or fetched_at without one (an observation's t).
    pub fn t(&self) -> Time {
        self.published_at.unwrap_or(self.fetched_at)
    }
}

pub fn decode(x: &Raw) -> Report {
    Report {
        id: x.id.clone(),
        source_id: x.source_id.clone(),
        region: x.region.clone(),
        fetched_at: x.fetched_at,
        published_at: x.published_at,
        url: x.url.clone(),
        payload: serde_json::from_str(&x.payload).unwrap_or(Value::Null),
    }
}

/// Stored raw reports of adapters A and region ids R (every adapter or region if
/// empty), dated at or after `since` (published_at, or fetched_at without one, as
/// an observation's t); decoded, in fetch order.
pub fn stored(A: &[&str], R: &[&str], since: Option<Time>) -> Vec<Report> {
    let V = store::view::<Raw>();
    let mut X: Vec<&Raw> = V
        .iter()
        .filter(|x| A.is_empty() || A.contains(&adapter(&x.source_id)))
        .filter(|x| R.is_empty() || R.contains(&x.region.as_str()))
        .filter(|x| since.is_none_or(|t| x.published_at.unwrap_or(x.fetched_at) >= t))
        .collect();
    X.sort_by(|a, b| (a.fetched_at, &a.id).cmp(&(b.fetched_at, &b.id)));
    X.into_iter().map(decode).collect()
}

/// RFC 2822 (RSS) or ISO 8601 (Atom) timestamp in UTC; None if unparseable.
pub fn date(s: Option<&str>) -> Option<Time> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    // A day name is not checked against the date, as email.utils does not.
    let body = match s.split_once(',') {
        Some((d, rest)) if d.chars().all(char::is_alphabetic) => rest.trim(),
        _ => s,
    };

    chrono::DateTime::parse_from_rfc2822(body)
        .ok()
        .map(|t| t.with_timezone(&Utc))
        .or_else(|| fmt::fromiso(&s.replace('Z', "+00:00")))
}

/// ECMAScript encodeURIComponent (ECMA-262 §19.2.6.4).
pub fn encode_uri_component(s: &str) -> String {
    quote(s, "-_.!~*'()", false)
}

/// Python's quote_plus: application/x-www-form-urlencoded.
pub fn quote_plus(s: &str) -> String {
    quote(s, "_.-~", true)
}

fn quote(s: &str, safe: &str, plus: bool) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b' ' if plus => o.push('+'),
            b if b.is_ascii_alphanumeric() || safe.as_bytes().contains(&b) => o.push(b as char),
            b => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// Python's urlencode of (key, value) pairs.
pub fn urlencode(P: &[(&str, String)]) -> String {
    P.iter()
        .map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// A value of the regions/global.json lists (market symbols, coins, Hebrew vocabulary).
pub fn global() -> &'static Value {
    static G: OnceLock<Value> = OnceLock::new();
    G.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(crate::regions::dir().join("global.json")).unwrap(),
        )
        .unwrap()
    })
}

/// The first n characters of s.
pub fn excerpt(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Python's str() of a JSON value inside an f-string (None for null).
pub fn pystr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => {
            if *b {
                "True".into()
            } else {
                "False".into()
            }
        }
        Value::Number(n) if n.is_f64() => fmt::repr(n.as_f64().unwrap()),
        v => v.to_string(),
    }
}

/// Python truth of a JSON value.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
