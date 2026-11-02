//! Collection isolation and missing-evidence regressions from the dashboard audit.
#![allow(non_snake_case)]

mod common;

use chrono::Duration;
use common::*;
use osw::collect::{Source, now};
use osw::schema::Event;
use osw::{analyze, cycle, process, produce, regions, store};
use serde_json::Value;

fn health(h: &[(&str, &'static str)]) -> analyze::Health {
    h.iter().map(|x| (x.0.to_string(), x.1)).collect()
}

#[test]
fn due_is_scoped_to_region() {
    let _g = fresh("regress_due");
    let t = now();
    store::append(&[run("rss:BBC", "iran-israel", t, "ok")]);
    let src = Source {
        id: "rss:BBC".into(),
        region: "south-asia".into(),
        cadence: 300.0,
        ..Default::default()
    };
    let D = cycle::due(vec![src], t);
    assert_eq!(D.len(), 1);
    assert_eq!(D[0].region, "south-asia");
}

#[test]
fn missing_collection_is_not_absent_evidence() {
    let _g = fresh("regress_missing");
    let r = regions::get("red-sea").unwrap();
    let board = analyze::board(r, &health(&[]), now());
    let i = &board[0]["indicators"][0];
    assert_eq!(
        (i["count"].as_i64(), i["status"].as_str()),
        (Some(0), Some("no data"))
    );
    for h in board[1]["hypotheses"].as_array().unwrap() {
        assert!(
            h["unassessed"]
                .as_array()
                .unwrap()
                .contains(&"RS-1a".into()),
            "{h}"
        );
    }
}

/// A collapse in the stored tracks is not evidence while AIS collection is unhealthy.
#[test]
fn stale_transits_are_not_current_evidence() {
    let _g = fresh("regress_stale");
    let t = at(2026, 10, 21, 12, 0, 0);
    let mut rows = vec![];
    for k in 1..=15i64 {
        let n = if k == 1 { 2 } else { 10 + k % 3 };
        for v in 0..n {
            rows.extend(vessel(
                (1000 * k + v) as u32,
                (t - Duration::days(k)).date_naive(),
                1,
            ));
        }
    }
    store::append(&rows);
    let r = regions::get("red-sea").unwrap();
    let ok = analyze::board(r, &health(&[("ais:aisstream", "ok")]), t);
    assert_eq!(ok[1]["indicators"][0]["status"], "triggered");
    let stale = analyze::board(r, &health(&[("ais:aisstream", "failed")]), t);
    assert_eq!(stale[1]["status"], "gap");
    assert_eq!(stale[1]["indicators"][0]["status"], "no data");
}

#[test]
fn source_health_includes_daily_baseline() {
    let _g = fresh("regress_baseline");
    let t = now();
    store::append(&[run(
        "firms:VIIRS#7d",
        "global",
        t - Duration::hours(25),
        "ok",
    )]);
    let S = cycle::source_table(regions::get("red-sea").unwrap(), t);
    let s = S.iter().find(|s| s.id == "firms:VIIRS#7d").unwrap();
    assert_eq!(s.health, "ok");
}

#[test]
fn api_rejects_invalid_filters() {
    let _g = fresh("regress_filters");
    for path in [
        "/events?region=typo",
        "/events?limit=-1",
        "/observations?limit=oops",
        "/tracks?since=garbage",
        "/events?limit=10001",
    ] {
        assert_eq!(api(path).0, 400, "{path}");
    }
}

/// A constant baseline has no z-score; the response is still JSON.
#[test]
fn constant_transit_baseline_serializes_as_json() {
    let _g = fresh("regress_constant");
    let t = now();
    let mut rows = vec![];
    for k in 1..=15i64 {
        let n = if k == 1 { 1 } else { 10 };
        for v in 0..n {
            rows.extend(vessel(
                (1000 * k + v) as u32,
                (t - Duration::days(k)).date_naive(),
                1,
            ));
        }
    }
    store::append(&rows);
    store::append(&[run("ais:aisstream", "global", t, "ok")]);
    let B = api_json("/api/indicators?region=red-sea");
    let i = &B[1]["indicators"][0];
    assert_eq!(i["status"], "triggered");
    assert!(i["z"].is_null());
    assert_eq!(i["baseline_sd"].as_f64(), Some(0.0));
}

#[test]
fn product_confidence_uses_the_indicator_window() {
    let _g = fresh("regress_confidence");
    let t = now();
    let E: Vec<Event> = (0..3)
        .map(|k| Event {
            id: format!("ship-{k}"),
            region: "red-sea".into(),
            t_start: t - Duration::hours(48),
            t_end: t - Duration::hours(48),
            lat: 14.0,
            lon: 43.0,
            kind: "maritime_incident".into(),
            observation_ids: vec![format!("o-{k}")],
            n_independent: 2,
            credibility: 2,
        })
        .collect();
    store::append(&E);
    let r = regions::get("red-sea").unwrap();
    let board = analyze::board(r, &health(&[("rss:BBC", "ok")]), t);
    let text = produce::report(r, &board, &[], "sitrep", t);
    assert!(text.contains("(RS-1a); high confidence"), "{text}");
}

#[test]
fn south_asia_gazetteer_includes_places_beyond_curated_capitals() {
    let r = regions::get("south-asia").unwrap();
    for (title, name) in [
        ("Flooding reported in Khulna", "Khulna"),
        ("Protests in Lahore", "Lahore"),
        ("Fighting near Mandalay", "Mandalay"),
    ] {
        let (lat, lon, precision, names) = process::geocode(r, title);
        assert_eq!(precision, "city", "{title}");
        assert!(names.iter().any(|n| n == name), "{names:?}");
        assert!(lat.is_some() && lon.is_some());
    }
}

#[test]
fn south_asia_reporting_counts_cannot_trigger_an_uncalibrated_alert() {
    let _g = fresh("regress_south_asia");
    let t = now();
    let E: Vec<Event> = (0..20)
        .map(|k| Event {
            id: format!("sa-{k}"),
            region: "south-asia".into(),
            t_start: t - Duration::hours(1),
            t_end: t - Duration::hours(1),
            lat: 22.3,
            lon: 91.8,
            kind: "maritime_incident".into(),
            observation_ids: vec![format!("o-{k}")],
            n_independent: 1,
            credibility: 3,
        })
        .collect();
    store::append(&E);
    let r = regions::get("south-asia").unwrap();
    let board = analyze::board(r, &health(&[("rss:BBC Asia", "ok")]), t);
    assert_eq!(board[0]["status"], "open");
    let i = &board[0]["indicators"][0];
    assert_eq!(
        (i["count"].as_i64(), i["status"].as_str()),
        (Some(20), Some("observed"))
    );
    assert!(
        i.get("watch").is_none_or(Value::is_null) && i.get("trigger").is_none_or(Value::is_null)
    );
    assert!(produce::report(r, &board, &[], "sitrep", t).contains("no calibrated alert threshold"));
}
