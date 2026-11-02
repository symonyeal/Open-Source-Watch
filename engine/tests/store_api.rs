//! The cycle's store (idempotence, derivation, rebuild, compaction), the source table
//! and the API. A poll is stored through cycle::ingest, which
//! takes a response body for any adapter; the network is never used.
#![allow(non_snake_case)]

mod common;

use std::collections::BTreeSet;

use chrono::{Duration, Utc};
use common::*;
use osw::collect::{self, Resp, Source, now, rss};
use osw::schema::{Entity, Event, Obs, Product, Raw, Run, Track};
use osw::{cycle, fuse, store};

const FILES: [(&str, &str); 8] = [
    ("rss:BBC", "bbc_rss.xml"),
    ("rss:Google News#1", "google_news.xml"),
    ("telegram:DeepStateUA", "telegram_DeepStateUA.html"),
    ("firms:VIIRS", "firms.csv"),
    ("adsb:adsb.lol#mil", "adsb_mil.json"),
    ("alerts:alertsua", "alertsua_states.json"),
    ("drones:neptun", "neptun.json"),
    ("markets:CoinGecko", "coingecko.json"),
];

/// Store one poll of every source of FILES, answered by its fixture; the sources.
fn feed() -> Vec<Source> {
    let mut S = vec![];
    for (sid, file) in FILES {
        for s in sources_of(sid) {
            let r = ingest(&s, &text(file));
            assert!(
                r.status == "ok" || r.status == "empty",
                "{sid}: {}",
                r.status
            );
            S.push(s);
        }
    }
    S
}

fn n<T: osw::schema::Table>() -> usize {
    store::rows::<T>().len()
}

#[test]
fn store_is_idempotent_and_empty_tables_query() {
    let _g = fresh("store_idempotent");
    assert_eq!(n::<Event>(), 0);
    let S = feed();
    let k = n::<Raw>();
    cycle::reset(); // a fresh process re-reads the store
    feed();
    assert!(k > 0);
    assert_eq!(n::<Raw>(), k);
    assert_eq!(n::<Run>(), 2 * S.len());
}

/// Regression: the seen set was read from the deduplicated view, which keeps an item's
/// first fetch, so an item listed for a long time was stored every poll.
#[test]
fn an_item_still_listed_is_stored_once() {
    let _g = fresh("store_listed");
    let src = sources_of("rss:BBC").remove(0);
    let get = |_: &Source| {
        Ok(Resp {
            status: 200,
            ctype: None,
            body: fix("bbc_rss.xml"),
        })
    };
    let (_, raw, _) = collect::run(std::slice::from_ref(&src), rss::parse, "test", get);
    let old: Vec<Raw> = raw
        .iter()
        .map(|x| Raw {
            fetched_at: now() - Duration::days(10),
            ..x.clone()
        })
        .collect();
    let recent: Vec<Raw> = raw
        .iter()
        .map(|x| Raw {
            fetched_at: now() - Duration::days(1),
            ..x.clone()
        })
        .collect();
    store::append(&old);
    store::append(&recent);
    let k = n::<Raw>();
    assert_eq!(k, 2 * raw.len());
    cycle::reset();
    ingest(&src, &text("bbc_rss.xml"));
    assert_eq!(n::<Raw>(), k);
}

#[test]
fn every_adapter_derives() {
    let _g = fresh("store_derives");
    feed();
    let K: BTreeSet<String> = store::view::<Obs>()
        .iter()
        .map(|x| x.kind.clone())
        .collect();
    for want in ["thermal_anomaly", "air_raid_alert", "airspace_activity"] {
        assert!(K.contains(want), "{want} in {K:?}");
    }
    assert!(n::<Track>() > 0);
    let E = fuse::fuse();
    assert!(E > 0);
    assert_eq!(E, n::<Event>());
    assert!(n::<Entity>() > 0);
}

#[test]
fn rebuild_reproduces_derived_tables() {
    let _g = fresh("store_rebuild");
    feed();
    let snap = || -> Vec<(String, String, String, i8)> {
        let mut v: Vec<_> = store::view::<Obs>()
            .iter()
            .map(|x| {
                (
                    x.id.clone(),
                    x.kind.clone(),
                    x.text_en.clone(),
                    x.credibility,
                )
            })
            .collect();
        v.sort();
        v
    };
    let (before, tp) = (snap(), n::<Track>());
    assert!(!before.is_empty());
    cycle::rebuild();
    assert_eq!(snap(), before);
    assert_eq!(n::<Track>(), tp);
}

#[test]
fn compaction_keeps_rows() {
    let _g = fresh("store_compact");
    feed();
    cycle::reset();
    feed();
    let k = n::<Run>();
    let tomorrow = (Utc::now() + Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    assert!(store::compact(Some(&tomorrow)) > 0);
    assert_eq!(n::<Run>(), k);
}

/// Days of a past month become one file per region, with every row kept.
#[test]
fn past_months_roll_up_to_one_file() {
    let _g = fresh("store_months");
    let row = run("rss:BBC", "iran-israel", now(), "ok");
    store::append(std::slice::from_ref(&row));
    store::append(std::slice::from_ref(&row));
    let r = store::data_dir().join("collection_run").join("iran-israel");
    let dirs = || -> Vec<std::path::PathBuf> {
        std::fs::read_dir(&r)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    };
    std::fs::rename(&dirs()[0], r.join("2026-09-30")).unwrap();
    store::append(&[row]);
    let other = dirs()
        .into_iter()
        .find(|d| d.file_name().unwrap() != "2026-09-30")
        .unwrap();
    std::fs::rename(other, r.join("2026-09-01")).unwrap();
    assert_eq!(store::compact(Some("2026-10-02")), 3);
    assert_eq!(n::<Run>(), 3);
    let names: Vec<String> = dirs()
        .iter()
        .map(|d| d.file_name().unwrap().to_string_lossy().into())
        .collect();
    assert_eq!(names, ["2026-09"]);
    let files = std::fs::read_dir(r.join("2026-09")).unwrap().count();
    assert_eq!(files, 1);
}

/// A writer's unfinished file (written aside, renamed when complete) is not read.
#[test]
fn queries_do_not_see_a_partially_written_poll() {
    let _g = fresh("store_partial");
    let row = run("rss:BBC", "iran-israel", now(), "ok");
    store::append(std::slice::from_ref(&row));
    let d = std::fs::read_dir(store::data_dir().join("collection_run").join("iran-israel"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(d.join("unfinished.pending"), b"").unwrap();
    assert_eq!(n::<Run>(), 1);
    store::append(&[row]);
    assert_eq!(n::<Run>(), 2);
}

/// Offline, the proxy answers 502 for a scheduled URL (and records the failed poll) and
/// for a host that is not scheduled.
#[test]
fn proxy_refuses_unscheduled_hosts_and_records_failed_polls() {
    let _g = fresh("store_proxy");
    let url = sources_of("rss:BBC").remove(0).url;
    let q = |u: &str| format!("/fetch?url={}", collect::quote_plus(u));
    assert_eq!(api(&q(&url)).0, 502);
    let R = store::rows::<Run>();
    let r = R
        .iter()
        .find(|x| x.source_id == "rss:BBC")
        .expect("poll recorded");
    assert_eq!(
        (r.status.as_str(), r.runner.as_str()),
        ("http_error", "local")
    );
    let before = R.len();
    assert_eq!(api(&q("https://example.com/x")).0, 502);
    assert_eq!(n::<Run>(), before);
}

#[test]
fn telegram_route_has_upstream_shape() {
    let _g = fresh("store_telegram");
    feed();
    let d = api_json("/api/telegram?conflict=russia-ukraine&since=2000-01-01T00:00:00Z");
    let keys = |v: &serde_json::Value| -> BTreeSet<String> {
        v.as_object().unwrap().keys().cloned().collect()
    };
    let want = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
    assert_eq!(keys(&d), want(&["posts", "channels", "updated"]));
    let ids: Vec<i64> = d["posts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["postId"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [23913, 23912, 23911]);
    assert_eq!(
        keys(&d["posts"][0]),
        want(&[
            "channel",
            "channelLabel",
            "color",
            "postId",
            "text",
            "date",
            "url"
        ])
    );
}

#[test]
fn products_route_serves_latest_scheduled_product() {
    let _g = fresh("store_products");
    let url = "/api/products?region=iran-israel&kind=intsum";
    assert!(api_json(url).is_null());
    let p = |id: &str, day: u32, text: &str| Product {
        id: id.into(),
        region: "iran-israel".into(),
        kind: "intsum".into(),
        t: at(2026, 10, day, 0, 0, 0),
        period_h: 24,
        text: text.into(),
    };
    store::append(&[p("old", 21, "older"), p("new", 22, "newer")]);
    assert_eq!(api_json(url)["text"], "newer");
}

#[test]
fn sources_view() {
    let _g = fresh("store_sources");
    feed();
    let S = api_json("/sources?region=iran-israel");
    let by = |id: &str| {
        S.as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        (
            by("rss:BBC")["health"].as_str(),
            by("rss:BBC")["reliability"].as_str()
        ),
        (Some("ok"), Some("B"))
    );
    assert_eq!(by("firms:VIIRS")["reliability"], "A");
    assert_eq!(by("rss:CNN")["health"], "unknown");
}

#[test]
fn source_health() {
    let t = now();
    let r = |s: &str, m: i64| run("rss:BBC", "iran-israel", t - Duration::minutes(m), s);
    let h = |v: &[Run]| cycle::health(&v.iter().collect::<Vec<_>>(), 300.0, t);
    assert_eq!(h(&[]), "unknown");
    assert_eq!(h(&[r("ok", 1)]), "ok");
    assert_eq!(h(&[r("ok", 20)]), "stale");
    assert_eq!(
        h(&[
            r("http_error", 1),
            r("http_error", 2),
            r("blocked", 3),
            r("ok", 4)
        ]),
        "failed"
    );
}

#[test]
fn due_skips_sources_polled_within_their_cadence() {
    let _g = fresh("store_due");
    let ids = |v: Vec<Source>| -> BTreeSet<(String, String)> {
        v.into_iter().map(|s| (s.region, s.id)).collect()
    };
    let all = || {
        FILES
            .iter()
            .flat_map(|f| sources_of(f.0))
            .collect::<Vec<_>>()
    };
    let S = all();
    assert_eq!(ids(cycle::due(S.clone(), now())), ids(S.clone()));
    feed();
    assert!(cycle::due(S.clone(), now()).is_empty());
    let later = now() + Duration::seconds(301);
    let want: BTreeSet<(String, String)> = S
        .iter()
        .filter(|s| 0.9 * s.cadence <= 301.0)
        .map(|s| (s.region.clone(), s.id.clone()))
        .collect();
    assert!(!want.is_empty());
    assert_eq!(ids(cycle::due(S, later)), want);
}
