//! Helpers shared by the integration tests: fixtures, a fresh store, observations,
//! a seeded generator, and the API over a local socket. No test reaches the network.
#![allow(dead_code, non_snake_case)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, Once, OnceLock};

use chrono::{TimeZone, Utc};
use osw::collect::Source;
use osw::fmt::Time;
use osw::schema::{Obs, Run, Track};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

pub fn fix(name: &str) -> Vec<u8> {
    std::fs::read(format!("{ROOT}/../tests/fixtures/{name}")).unwrap()
}

pub fn text(name: &str) -> String {
    String::from_utf8(fix(name)).unwrap()
}

/// The repository root.
pub fn repo() -> PathBuf {
    PathBuf::from(ROOT).parent().unwrap().to_path_buf()
}

/// No request leaves the process (collect::get answers with a failure).
pub fn offline() {
    static O: Once = Once::new();
    // SAFETY: set once, before any test thread reads it.
    O.call_once(|| unsafe { std::env::set_var("OSW_OFFLINE", "1") });
}

static LOCK: Mutex<()> = Mutex::new(());

/// Serialises the tests that write the store, points it at an empty directory
/// target/test-data/<name>, and forgets the in-process state. Hold the guard.
pub fn fresh(name: &str) -> MutexGuard<'static, ()> {
    let g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    offline();
    let d = PathBuf::from(ROOT)
        .join("target")
        .join("test-data")
        .join(name);
    for _ in 0..5 {
        if std::fs::remove_dir_all(&d).is_ok() || !d.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(!d.exists(), "cannot empty {}", d.display());
    std::fs::create_dir_all(&d).unwrap();
    osw::store::set_root(Some(d));
    osw::cycle::reset();
    g
}

/// Set or clear an environment variable (under the store lock).
pub fn setenv(k: &str, v: Option<&str>) {
    // SAFETY: tests that change the environment hold the lock of `fresh`.
    unsafe {
        match v {
            Some(v) => std::env::set_var(k, v),
            None => std::env::remove_var(k),
        }
    }
}

pub fn at(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> Time {
    Utc.with_ymd_and_hms(y, m, d, h, mi, s).unwrap()
}

/// An observation of region iran-israel (a located one unless lat is None).
pub fn ob(
    k: usize,
    t: Time,
    lat: Option<f64>,
    lon: Option<f64>,
    src: &str,
    kind: &str,
    prec: &str,
) -> Obs {
    Obs {
        id: format!("o{k}"),
        raw_id: format!("r{k}"),
        source_id: src.into(),
        region: "iran-israel".into(),
        t,
        lat,
        lon,
        geo_precision: prec.into(),
        kind: kind.into(),
        text_en: String::new(),
        lang: "en".into(),
        credibility: 3,
        names: vec![],
        t_event: None,
        admin1: None,
        admin2: None,
    }
}

pub fn run(sid: &str, region: &str, t: Time, status: &str) -> Run {
    Run {
        source_id: sid.into(),
        region: region.into(),
        started_at: t,
        status: status.into(),
        n_rows: 1,
        latency_ms: 1,
        runner: "test".into(),
    }
}

/// Store one poll of src whose response is `body` (the AIS ingest path serves any
/// adapter: it parses, stores new reports and what they derive, and logs the run).
pub fn ingest(src: &Source, body: &str) -> Run {
    setenv("AISSTREAM_KEY", Some("test"));
    osw::cycle::ingest(src, &[body.to_string()], "test")
        .pop()
        .unwrap()
}

/// Sources of the registry with this id (one per region that has it).
pub fn sources_of(sid: &str) -> Vec<Source> {
    let S: Vec<Source> = osw::cycle::all_sources()
        .iter()
        .filter(|s| s.id == sid)
        .cloned()
        .collect();
    assert!(!S.is_empty(), "no source {sid}");
    S
}

/// Seeded generator (a 64-bit LCG), so the loops are deterministic.
pub struct Rng(pub u64);

impl Rng {
    /// Uniform in [0, 1).
    pub fn u(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in 0..n.
    pub fn below(&mut self, n: usize) -> usize {
        ((self.u() * n as f64) as usize).min(n - 1)
    }
}

/// GET path from the engine API served on a local port: (status, body).
pub fn api(path: &str) -> (u16, String) {
    static PORT: OnceLock<u16> = OnceLock::new();
    offline();
    let p = *PORT.get_or_init(|| {
        let p = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::thread::spawn(move || osw::api::serve(p, false));
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", p)).is_ok() {
                return p;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("API did not start");
    });
    let mut s = TcpStream::connect(("127.0.0.1", p)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(120)))
        .unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut b = vec![];
    s.read_to_end(&mut b).unwrap();
    let r = String::from_utf8_lossy(&b).into_owned();
    let (head, body) = r.split_once("\r\n\r\n").unwrap_or((&r, ""));
    let code = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (code, body.to_string())
}

pub fn api_json(path: &str) -> serde_json::Value {
    let (c, b) = api(path);
    assert_eq!(c, 200, "{path}: {b}");
    serde_json::from_str(&b).unwrap()
}

/// Run `node -e code` in the repository; None without node.
pub fn node(args: &[&str]) -> Option<std::process::Output> {
    std::process::Command::new("node")
        .args(args)
        .current_dir(repo())
        .output()
        .ok()
}

/// A red-sea AIS track point of vessel mmsi.
pub fn point(mmsi: u32, t: Time, p: [f64; 2]) -> Track {
    Track {
        entity_id: format!("mmsi:{mmsi}"),
        region: "red-sea".into(),
        t,
        lat: p[0],
        lon: p[1],
        heading: None,
        speed: None,
        source_id: "ais:aisstream".into(),
    }
}

/// South and north of the Bab el-Mandeb gate line.
pub const SOUTH: [f64; 2] = [12.50, 43.45];
pub const NORTH: [f64; 2] = [12.70, 43.30];

/// Track points of a vessel that crosses the gate `crossings` times on `day`.
pub fn vessel(mmsi: u32, day: chrono::NaiveDate, crossings: i64) -> Vec<Track> {
    let t = osw::gnss::midnight(day) + chrono::Duration::hours(1);
    (0..=crossings)
        .map(|k| {
            let p = if k % 2 == 0 { SOUTH } else { NORTH };
            point(mmsi, t + chrono::Duration::minutes(k), p)
        })
        .collect()
}
