//! Mechanical stages of the cycle: collect → process → store; the recording proxy;
//! the scheduler; rebuild.
//!
//! A source id is "<adapter>:<name>[#k]"; `parse_of` maps the adapter to its parser,
//! `fetch_of` to its own fetch where one URL is not one request (a list page and its
//! reports, a dated window), `derive` to its processor. Sources sharing a URL are
//! fetched once per poll, and each is parsed by its own adapter. The latest response
//! per URL is kept in CACHE, from which the proxy answers the front end (spec § Seam):
//! what the dashboard shows is what was stored.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, mpsc};
use std::time::{Duration as StdDuration, Instant};

use chrono::Duration;
use serde::Serialize;
use serde_json::{Value, json};

use crate::analyze::{self, Health};
use crate::collect::{self, Fail, Report, Resp, Source, adapter, decode, now};
use crate::fmt::{Time, ser::py_opt};
use crate::fuse;
use crate::process::{feeds, news};
use crate::produce;
use crate::regions::{self, Region};
use crate::schema::{Entity, Event, Obs, Raw, Run, Track};
use crate::store;

/// Every source, in adapter order: of sources sharing a URL, the first is the one
/// the proxy always polls.
pub fn all_sources() -> &'static [Source] {
    static S: OnceLock<Vec<Source>> = OnceLock::new();
    S.get_or_init(|| {
        let R = regions::all();
        use collect::*;
        [
            rss::sources(R),
            telegram::sources(R),
            firms::sources(R),
            adsb::sources(R),
            alerts::sources(R),
            neptun::sources(R),
            markets::sources(R),
            ais::sources(R),
            context::sources(R),
            gpsjam::sources(R),
            integrity::sources(R),
            mnd::sources(R),
            ioda::sources(R),
            ucdp::sources(R),
        ]
        .concat()
    })
}

/// Sources of the given regions and adapters (every one if empty); polled drops
/// stream sources.
pub fn sources(R: &[String], A: &[String], polled: bool) -> Vec<Source> {
    all_sources()
        .iter()
        .filter(|s| R.is_empty() || R.contains(&s.region))
        .filter(|s| A.is_empty() || A.iter().any(|a| a == adapter(&s.id)))
        .filter(|s| !(polled && s.stream))
        .cloned()
        .collect()
}

fn parse_of(a: &str) -> collect::Parse {
    use collect::*;
    match a {
        "rss" | "gnews" => rss::parse,
        "telegram" => telegram::parse,
        "firms" => firms::parse,
        "adsb" => adsb::parse,
        "alerts" => alerts::parse,
        "drones" => neptun::parse,
        "markets" => markets::parse,
        "ais" => ais::parse,
        "context" => context::parse,
        "gpsjam" => gpsjam::parse,
        "integrity" => integrity::parse,
        "mnd" => mnd::parse,
        "ioda" => ioda::parse,
        "ucdp" => ucdp::parse,
        _ => |_: &[u8], _: &Source| -> Result<Vec<Item>, String> { Err("no adapter".into()) },
    }
}

fn fetch_of(a: &str) -> Option<fn(&Source) -> Result<Resp, Fail>> {
    use collect::*;
    match a {
        "gpsjam" => Some(gpsjam::fetch),
        "mnd" => Some(mnd::fetch),
        "ioda" => Some(ioda::fetch),
        "ucdp" => Some(ucdp::fetch),
        _ => None,
    }
}

fn good(x: &Run) -> bool {
    x.status == "ok" || x.status == "empty"
}

/// Sources of S of which 90 % of the cadence has elapsed since their last
/// successful run in the store (a stateless runner, such as the cron, polls only
/// these; the 10 % absorbs its schedule jitter).
pub fn due(S: Vec<Source>, t: Time) -> Vec<Source> {
    let mut last: HashMap<(&str, &str), Time> = HashMap::new();
    let R = store::rows::<Run>();
    for x in R.iter().filter(|x| good(x)) {
        let e = last
            .entry((&x.region, &x.source_id))
            .or_insert(x.started_at);
        *e = (*e).max(x.started_at);
    }
    S.into_iter()
        .filter(|s| {
            last.get(&(s.region.as_str(), s.id.as_str()))
                .is_none_or(|l| {
                    (t - *l).num_microseconds().unwrap_or(i64::MAX) as f64 / 1e6 >= 0.9 * s.cadence
                })
        })
        .collect()
}

/// ok | stale | failed | unknown (spec § Errors); runs newest first.
pub fn health(runs: &[&Run], cadence: f64, t: Time) -> &'static str {
    if runs.is_empty() {
        return "unknown";
    }
    if runs.len() >= 3 && !runs[..3].iter().any(|x| good(x)) {
        return "failed";
    }
    match runs.iter().find(|x| good(x)) {
        Some(g)
            if ((t - g.started_at).num_microseconds().unwrap() as f64 / 1e6) <= 3.0 * cadence =>
        {
            "ok"
        }
        _ => "stale",
    }
}

/// A source serving a region, with its grade, owner and health.
#[derive(Clone, Debug, Serialize)]
pub struct SourceRow {
    pub id: String,
    pub region: String,
    pub url: String,
    pub reliability: String,
    pub owner: String,
    pub cadence_s: u64,
    pub health: &'static str,
    pub last_status: Option<String>,
    #[serde(serialize_with = "py_opt")]
    pub last_run: Option<Time>,
}

/// Every source serving region r, with its grade, owner and health at t.
pub fn source_table(r: &Region, t: Time) -> Vec<SourceRow> {
    let S = sources(&[r.id.clone(), "global".into()], &[], false);
    let horizon = S
        .iter()
        .map(|s| 3.0 * s.cadence)
        .fold(None, |m: Option<f64>, c| Some(m.map_or(c, |m| m.max(c))))
        .unwrap_or(86400.0);
    let t0 = t - Duration::microseconds((horizon * 1e6) as i64);
    let R = store::rows::<Run>();
    let mut runs: HashMap<(&str, &str), Vec<&Run>> = HashMap::new();
    for x in R.iter().filter(|x| {
        (x.region == r.id || x.region == "global") && x.started_at > t0 && x.started_at <= t
    }) {
        runs.entry((&x.region, &x.source_id)).or_default().push(x);
    }
    for v in runs.values_mut() {
        v.sort_by_key(|x| std::cmp::Reverse(x.started_at));
    }
    S.iter()
        .map(|s| {
            let x = runs
                .get(&(s.region.as_str(), s.id.as_str()))
                .map_or(&[][..], |v| v.as_slice());
            SourceRow {
                id: s.id.clone(),
                region: s.region.clone(),
                url: s.url.clone(),
                reliability: r.reliability(&s.id).into(),
                owner: r.owner(&s.id),
                cadence_s: s.cadence as u64,
                health: health(x, s.cadence, t),
                last_status: x.first().map(|y| y.status.clone()),
                last_run: x.first().map(|y| y.started_at),
            }
        })
        .collect()
}

pub fn health_of(S: &[SourceRow]) -> Health {
    S.iter().map(|s| (s.id.clone(), s.health)).collect()
}

/// Indicator board of region r with current source health (crate::analyze).
pub fn board(r: &Region, t: Time) -> Vec<Value> {
    analyze::board(r, &health_of(&source_table(r, t)), t)
}

/// The board of region r every `step` from t0 to t1, with source health as of each
/// time, or every source ok if `healthy` (history collected less often than the
/// sources' cadences reads as stale): per indicator, how often it was in each state, its episodes (runs of
/// consecutive triggered steps), the sum of its count over triggered steps, and every
/// triggered step with its count and areas (`start` on the first of an episode).
pub fn backtest(r: &Region, t0: Time, t1: Time, step: Duration, healthy: bool) -> Vec<Value> {
    let mut I: BTreeMap<String, (Value, BTreeMap<String, i64>, Vec<Value>, bool)> = BTreeMap::new();
    let mut t = t0;
    while t <= t1 {
        let mut h = health_of(&source_table(r, t));
        if healthy {
            h.values_mut().for_each(|x| *x = "ok");
        }
        for p in analyze::board(r, &h, t) {
            for i in p["indicators"].as_array().into_iter().flatten() {
                let s = i["status"].as_str().unwrap_or("").to_string();
                let x = I
                    .entry(i["id"].as_str().unwrap_or("").into())
                    .or_insert_with(|| {
                        (
                            json!({"pir": p["pir"], "id": i["id"], "name": i["name"]}),
                            BTreeMap::new(),
                            vec![],
                            false,
                        )
                    });
                *x.1.entry(s.clone()).or_default() += 1;
                let on = s == "triggered";
                if on {
                    *x.1.entry("triggered_count".into()).or_default() +=
                        i["count"].as_i64().unwrap_or(0);
                }
                if on {
                    x.2.push(json!({"t": crate::fmt::iso(&t), "count": i["count"],
                                    "areas": i["areas"], "start": !x.3}));
                }
                x.3 = on;
            }
        }
        t += step;
    }
    I.into_values()
        .map(|(mut x, S, E, _)| {
            x["steps"] = json!(S);
            x["episodes"] = json!(E.iter().filter(|e| e["start"] == true).count());
            x["triggered"] = json!(E);
            x
        })
        .collect()
}

/// After fusion: append each region's indicator snapshot and the given products.
pub fn analyse(kinds: &[&str], t: Time) {
    let (mut S, mut P) = (vec![], vec![]);
    for r in regions::all() {
        let T = source_table(r, t);
        let B = analyze::board(r, &health_of(&T), t);
        S.extend(analyze::snapshot(r, &B, t));
        P.extend(
            kinds
                .iter()
                .map(|k| produce::product(r, k, produce::report(r, &B, &T, k, t, None), t)),
        );
    }
    store::append(&S);
    store::append(&P);
}

fn by_url() -> &'static HashMap<String, Vec<Source>> {
    static U: OnceLock<HashMap<String, Vec<Source>>> = OnceLock::new();
    U.get_or_init(|| {
        let mut M: HashMap<String, Vec<Source>> = HashMap::new();
        for s in all_sources() {
            M.entry(s.url.clone()).or_default().push(s.clone());
        }
        M
    })
}

/// Longer than any source's lookback (the FIRMS 7-day file).
const SEEN_DAYS: i64 = 8;
/// url -> (time, latest response)
static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Resp)>>> = LazyLock::new(Default::default);
/// (region, source id) -> time of its last poll in this process
static POLLED: LazyLock<Mutex<HashMap<(String, String), Instant>>> =
    LazyLock::new(Default::default);
/// {raw report id: last time a poll listed it}
static SEEN: Mutex<Option<HashMap<String, Time>>> = Mutex::new(None);

/// Forget the in-process state (a new data root).
pub fn reset() {
    CACHE.lock().unwrap().clear();
    POLLED.lock().unwrap().clear();
    *SEEN.lock().unwrap() = None;
    crate::process::reset();
}

/// The seen set over the last SEEN_DAYS: read from every stored copy, then refreshed
/// by each poll, so an item that stays in its feed is stored once (a stateless
/// runner stores it again at most once per SEEN_DAYS).
fn seen(g: &mut Option<HashMap<String, Time>>) -> &mut HashMap<String, Time> {
    g.get_or_insert_with(|| {
        let t = now() - Duration::days(SEEN_DAYS);
        let mut K: HashMap<String, Time> = HashMap::new();
        for x in store::rows::<Raw>().iter().filter(|x| x.fetched_at > t) {
            let e = K.entry(x.id.clone()).or_insert(x.fetched_at);
            *e = (*e).max(x.fetched_at);
        }
        K
    })
}

pub fn prune() {
    let t = now() - Duration::days(SEEN_DAYS);
    seen(&mut SEEN.lock().unwrap()).retain(|_, v| *v >= t);
}

/// Rows derived from raw reports by their adapters' processors.
#[derive(Default)]
pub struct Derived {
    pub obs: Vec<Obs>,
    pub tracks: Vec<Track>,
}

/// What raw reports derive, as stored. A report of a region no longer in the
/// registry derives nothing: there is no box, grade or gazetteer to process it with.
pub fn derive(raws: &[Raw]) -> Derived {
    let mut by: HashMap<&str, Vec<Report>> = HashMap::new();
    for x in raws
        .iter()
        .filter(|x| x.region == "global" || regions::get(&x.region).is_some())
    {
        by.entry(adapter(&x.source_id)).or_default().push(decode(x));
    }
    let mut D = Derived::default();
    let mut A: Vec<&&str> = by.keys().collect();
    A.sort();
    for a in A {
        let X = &by[*a];
        match *a {
            "rss" | "gnews" => D.obs.extend(news::derive(X)),
            "telegram" => D.obs.extend(feeds::telegram(X)),
            "firms" => D.obs.extend(feeds::firms(X)),
            "adsb" => D.tracks.extend(feeds::adsb(X)),
            "alerts" => D.obs.extend(feeds::alerts(X)),
            "drones" => {
                let (t, o) = feeds::neptun(X);
                D.tracks.extend(t);
                D.obs.extend(o);
            }
            "ais" => D.tracks.extend(feeds::ais(X)),
            "ioda" => D.obs.extend(feeds::ioda(X)),
            _ => {}
        }
    }
    crate::process::units(&mut D.obs);
    D
}

/// (new raw reports, what they derive, run rows) of one poll.
type Kept = (Vec<Raw>, Derived, Vec<Run>);

/// A listed report is not new, and a report is marked listed only once it has
/// derived. A feed listing one item twice yields one report.
fn keep(raw: Vec<Raw>, runs: Vec<Run>) -> Kept {
    let new = {
        let mut g = SEEN.lock().unwrap();
        let K = seen(&mut g);
        let (mut order, mut val): (Vec<String>, HashMap<String, Raw>) = (vec![], HashMap::new());
        for x in raw.iter().rev().filter(|x| !K.contains_key(&x.id)) {
            if !val.contains_key(&x.id) {
                order.push(x.id.clone());
            }
            val.insert(x.id.clone(), x.clone());
        }
        order.reverse();
        order
            .into_iter()
            .map(|k| val.remove(&k).unwrap())
            .collect::<Vec<_>>()
    };
    let D = derive(&new);
    let mut g = SEEN.lock().unwrap();
    let K = seen(&mut g);
    for x in &raw {
        K.insert(x.id.clone(), x.fetched_at);
    }
    (new, D, runs)
}

fn save(out: Vec<Kept>) -> Vec<Run> {
    let (mut R, mut O, mut T, mut U) = (vec![], vec![], vec![], vec![]);
    for (r, d, u) in out {
        R.extend(r);
        O.extend(d.obs);
        T.extend(d.tracks);
        U.extend(u);
    }
    store::append(&R);
    store::append(&O);
    store::append(&T);
    store::append(&U);
    U
}

/// One poll of the sources of one URL, stored nowhere yet.
fn one(group: &[Source], runner: &str) -> Kept {
    let a = adapter(&group[0].id).to_string();
    let parse = |b: &[u8], s: &Source| parse_of(adapter(&s.id))(b, s);
    let own = fetch_of(&a);
    let (res, raw, runs) = collect::run(group, parse, runner, |s| match own {
        Some(f) => f(s),
        None => collect::fetch(s, 10.0),
    });
    if let (Some(r), None) = (res, own) {
        // An adapter's own fetch is not the URL's response.
        CACHE
            .lock()
            .unwrap()
            .insert(group[0].url.clone(), (Instant::now(), r));
    }
    // A URL whose processing fails stores nothing, so its reports stay new.
    catch_unwind(AssertUnwindSafe(|| keep(raw, runs))).unwrap_or_else(|_| {
        eprintln!("processing {} failed", group[0].url);
        (vec![], Derived::default(), vec![])
    })
}

/// Poll every URL of sources S once, in parallel; store new raw reports, what they
/// derive, and every run row. Returns the run rows. Daily baselines (the FIRMS 7-day
/// file) are polled and stored first.
pub fn collect(S: &[Source], runner: &str, workers: usize) -> Vec<Run> {
    let mut G: Vec<(String, Vec<Source>)> = vec![];
    {
        let mut P = POLLED.lock().unwrap();
        for s in S {
            match G.iter_mut().find(|g| g.0 == s.url) {
                Some(g) => g.1.push(s.clone()),
                None => G.push((s.url.clone(), vec![s.clone()])),
            }
            P.insert((s.region.clone(), s.id.clone()), Instant::now());
        }
    }
    let (base, rest): (Vec<_>, Vec<_>) = G
        .into_iter()
        .map(|g| g.1)
        .partition(|g| g[0].cadence >= 86400.0);
    let mut runs = save(base.iter().map(|g| one(g, runner)).collect());
    let k = AtomicUsize::new(0);
    let out: Mutex<Vec<(usize, Kept)>> = Mutex::new(vec![]);
    std::thread::scope(|sc| {
        for _ in 0..workers.min(rest.len()) {
            sc.spawn(|| {
                loop {
                    let i = k.fetch_add(1, Ordering::Relaxed);
                    let Some(g) = rest.get(i) else { break };
                    let x = one(g, runner);
                    out.lock().unwrap().push((i, x));
                }
            });
        }
    });
    let mut out = out.into_inner().unwrap();
    out.sort_by_key(|x| x.0);
    runs.extend(save(out.into_iter().map(|x| x.1).collect()));
    runs
}

/// Store one batch of a stream source's messages as one poll; without the source's
/// key the poll is `blocked`.
pub fn ingest(src: &Source, messages: &[String], runner: &str) -> Vec<Run> {
    if collect::ais::key().is_none() {
        return save(vec![(
            vec![],
            Derived::default(),
            vec![Run {
                source_id: src.id.clone(),
                region: src.region.clone(),
                started_at: now(),
                status: "blocked".into(),
                n_rows: 0,
                latency_ms: 0,
                runner: runner.into(),
            }],
        )]);
    }
    let body = messages.join("\n").into_bytes();
    let (_, raw, runs) = collect::run(
        std::slice::from_ref(src),
        parse_of(adapter(&src.id)),
        runner,
        |_| {
            Ok(Resp {
                status: 200,
                ctype: None,
                body,
            })
        },
    );
    save(vec![keep(raw, runs)])
}

pub fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let auth = rest.split(['/', '?', '#']).next()?;
    let auth = auth.rsplit('@').next()?;
    let h = if auth.starts_with('[') {
        auth.split(']').next()?.trim_start_matches('[')
    } else {
        auth.split(':').next()?
    };
    (!h.is_empty()).then(|| h.to_lowercase())
}

/// The latest response for url (spec § Seam). A scheduled URL is answered from
/// CACHE while fresh, else collected now, with each other source of the URL only if
/// its own cadence has elapsed (an integrity snapshot shares the aircraft URL); an
/// unscheduled URL on a scheduled host is fetched and recorded as source
/// "unscheduled:<host>"; any other is refused.
pub fn proxy(url: &str) -> Option<Resp> {
    if let Some(S) = by_url().get(url) {
        let fresh =
            |c: &(Instant, Resp)| c.0.elapsed().as_secs_f64() <= (2.0 * S[0].cadence).max(30.0);
        if !CACHE.lock().unwrap().get(url).is_some_and(fresh) {
            let P = POLLED.lock().unwrap().clone();
            let mut T = vec![S[0].clone()];
            T.extend(
                S[1..]
                    .iter()
                    .filter(|s| {
                        P.get(&(s.region.clone(), s.id.clone()))
                            .is_none_or(|p| p.elapsed().as_secs_f64() >= 0.9 * s.cadence)
                    })
                    .cloned(),
            );
            collect(&T, "local", 16);
        }
        return CACHE.lock().unwrap().get(url).map(|c| c.1.clone());
    }
    let h = host(url)?;
    let hosts: HashSet<String> = by_url().keys().filter_map(|u| host(u)).collect();
    if !hosts.contains(&h) {
        return None;
    }
    let (started, t0) = (now(), Instant::now());
    let res = collect::get(url, &[], 10.0).ok();
    let status = if res.as_ref().is_some_and(|r| r.status < 400) {
        "ok"
    } else {
        "http_error"
    };
    store::append(&[Run {
        source_id: format!("unscheduled:{h}"),
        region: "global".into(),
        started_at: started,
        status: status.into(),
        n_rows: 0,
        latency_ms: t0.elapsed().as_millis() as i32,
        runner: "proxy".into(),
    }]);
    res
}

/// A step that panics is logged and retried at its next due time.
fn step(name: &str, f: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        eprintln!("scheduled {name} failed");
    }
}

/// Collect each source whenever its cadence has elapsed, until stop is set; every
/// 600 s recompute events and snapshot the indicator board, storing a SITREP every
/// 6 h and an INTSUM every 24 h; compact the store and prune the seen set every
/// hour. Stream sources (AIS) buffer their messages, which are stored once per
/// cadence.
pub fn schedule(stop: &'static AtomicBool, runner: &'static str) {
    let (tick, fusion, housekeeping) = (15.0, 600.0, 3600.0);
    let S = sources(&[], &[], true);
    let start = Instant::now();
    let since = |t: Instant| t.elapsed().as_secs_f64();
    let mut due: HashMap<String, f64> = HashMap::new();
    let (mut fused, mut kept) = (
        Instant::now() - StdDuration::from_secs(10 * fusion as u64),
        Instant::now(),
    );
    let mut made: HashMap<&str, Instant> = produce::PERIOD
        .iter()
        .map(|p| (p.0, Instant::now()))
        .collect();
    let streams: Vec<Source> = all_sources().iter().filter(|s| s.stream).cloned().collect();
    let (tx, rx) = mpsc::channel::<String>();
    if let Some(k) = collect::ais::key() {
        for _ in &streams {
            let (k, tx) = (k.clone(), tx.clone());
            std::thread::spawn(move || collect::ais::stream(k, regions::all(), tx, stop));
        }
    }
    while !stop.load(Ordering::Relaxed) {
        let t = since(start);
        let D: Vec<Source> = S
            .iter()
            .filter(|s| due.get(&s.id).is_none_or(|d| *d <= t))
            .cloned()
            .collect();
        for s in &D {
            due.insert(s.id.clone(), t + s.cadence);
        }
        if !D.is_empty() {
            step("collect", || {
                collect(&D, runner, 16);
            });
        }
        for s in &streams {
            if due.get(&s.id).is_none_or(|d| *d <= t) {
                due.insert(s.id.clone(), t + s.cadence);
                let M: Vec<String> = rx.try_iter().collect();
                step("ingest", || {
                    ingest(s, &M, runner);
                });
            }
        }
        if since(fused) > fusion {
            fused = Instant::now();
            step("products", || {
                fuse::fuse();
                let P: Vec<&str> = produce::PERIOD
                    .iter()
                    .filter(|(k, h)| since(made[k]) > 3600.0 * *h as f64)
                    .map(|p| p.0)
                    .collect();
                analyse(&P, now());
                for k in P {
                    made.insert(k, Instant::now());
                }
            });
        }
        if since(kept) > housekeeping {
            kept = Instant::now();
            step("compact", || {
                store::compact(None);
            });
            step("prune", prune);
        }
        std::thread::sleep(StdDuration::from_secs_f64(tick));
    }
}

/// Regenerate every derived table from raw_report (spec § Errors).
pub fn rebuild() -> usize {
    store::drop::<Obs>();
    store::drop::<Event>();
    store::drop::<Entity>();
    store::drop::<Track>();
    let mut raws: Vec<Raw> = store::view::<Raw>().iter().cloned().collect();
    raws.sort_by_key(|x| x.fetched_at);
    let D = derive(&raws);
    store::append(&D.obs);
    store::append(&D.tracks);
    fuse::fuse();
    raws.len()
}
