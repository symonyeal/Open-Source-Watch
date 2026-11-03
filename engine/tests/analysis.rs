//! Transit counting, ACH ranking and products (analyze, produce).
#![allow(non_snake_case)]

use std::collections::HashMap;

mod common;

use chrono::Duration;
use common::*;
use osw::analyze;
use osw::cycle::SourceRow;
use osw::fmt::Time;
use osw::produce;
use osw::regions::{self, Region};
use osw::schema::Track;
use osw::store;
use serde_json::{Value, json};

fn rs() -> &'static Region {
    regions::get("red-sea").unwrap()
}

fn gate() -> [[f64; 2]; 2] {
    rs().gate["bab-el-mandeb"].line
}

#[test]
fn segment_crossing() {
    let [a, b] = gate();
    assert!(analyze::crosses(NORTH, SOUTH, a, b)); // through the strait
    assert!(!analyze::crosses([13.5, 42.5], [13.6, 42.6], a, b)); // far north
    assert!(!analyze::crosses(a, [12.0, 44.0], a, b)); // touching is not crossing
}

/// The transit indicator of the board of region red-sea at t, AIS collection healthy.
fn transit(t: Time) -> Value {
    let mut h = analyze::Health::new();
    h.insert("ais:aisstream".into(), "ok");
    analyze::board(rs(), &h, t)
        .into_iter()
        .flat_map(|p| p["indicators"].as_array().unwrap().clone())
        .find(|i| i["metric"] == "transit")
        .unwrap()
}

/// A consecutive pair further apart than the policy limit is not a crossing; a pair at
/// the same instant is not one either.
#[test]
fn transit_gap_over_policy_limit_is_not_counted() {
    let _g = fresh("analysis_gap");
    let (t0, t) = (at(2026, 10, 1, 0, 0, 0), at(2026, 10, 5, 12, 0, 0));
    let count = |P: Vec<Track>| {
        store::replace(&P);
        let (C, covered) = analyze::daily_transits(rs(), "bab-el-mandeb", t, 14);
        (C.values().sum::<i64>(), covered.len())
    };
    let [a, b] = gate();
    assert!(analyze::crosses(NORTH, SOUTH, a, b));
    let day_apart = vec![point(1, t0, NORTH), point(1, t0 + Duration::days(1), SOUTH)];
    assert_eq!(count(day_apart), (0, 0));
    let minutes_apart = vec![
        point(1, t0, NORTH),
        point(1, t0 + Duration::minutes(10), SOUTH),
    ];
    assert_eq!(count(minutes_apart), (1, 1));
    let same_instant = vec![point(1, t0, NORTH), point(1, t0, SOUTH)];
    assert_eq!(count(same_instant), (0, 0));
}

#[test]
fn transit_indicator_detects_a_collapse() {
    let _g = fresh("analysis_collapse");
    let t = at(2026, 10, 21, 12, 0, 0);
    let mut rows = vec![];
    for k in 1..=15i64 {
        // 14 prior days of 10-12 vessels, then 2 vessels
        let n = if k == 1 { 2 } else { 10 + k % 3 };
        let day = (t - Duration::days(k)).date_naive();
        for v in 0..n {
            rows.extend(vessel((1000 * k + v) as u32, day, 1));
        }
    }
    store::append(&rows);
    let x = transit(t);
    assert_eq!(x["count"], 2);
    assert!(x["z"].as_f64().unwrap() < -2.0, "{x}");
    assert_eq!(x["status"], "triggered");
}

#[test]
fn transit_indicator_without_history_has_no_data() {
    let _g = fresh("analysis_nohistory");
    assert_eq!(transit(at(2026, 10, 21, 0, 0, 0))["status"], "no data");
}

#[test]
fn isolated_ais_points_do_not_count_as_sampled_coverage() {
    let _g = fresh("analysis_isolated");
    let t = at(2026, 10, 21, 12, 0, 0);
    let rows: Vec<Track> = (1..9)
        .map(|k| point(k, t - Duration::days(k as i64), [12.60, 43.35]))
        .collect();
    store::append(&rows);
    let x = transit(t);
    assert_eq!(
        (x["count"].as_i64(), x["status"].as_str()),
        (Some(0), Some("no data"))
    );
}

#[test]
fn ach_ranks_by_inconsistency() {
    let p = rs().pir.iter().find(|p| p.id == "RS-2").unwrap();
    let I = [
        json!({"id": "RS-1a", "status": "triggered"}),
        json!({"id": "RS-2a", "status": "watch"}),
    ];
    let H = analyze::ach(p, &I);
    assert_eq!(H[0]["id"], "RS-2-H1");
    assert_eq!(H[0]["inconsistent"], json!([]));
    let n: Vec<(&str, usize)> = ["RS-2-H1", "RS-2-H2", "RS-2-H3"]
        .into_iter()
        .map(|id| {
            let h = H.iter().find(|h| h["id"] == id).unwrap();
            (id, h["inconsistent"].as_array().unwrap().len())
        })
        .collect();
    assert_eq!(n, [("RS-2-H1", 0), ("RS-2-H2", 1), ("RS-2-H3", 2)]);
    let I = [
        json!({"id": "RS-1a", "status": "inactive"}),
        json!({"id": "RS-2a", "status": "no data"}),
    ];
    assert_eq!(analyze::ach(p, &I)[0]["unassessed"], json!(["RS-2a"]));
}

#[test]
fn intsum_reports_each_pir_and_cites_its_standards() {
    let _g = fresh("analysis_intsum");
    let r = regions::get("iran-israel").unwrap();
    let t = osw::collect::now();
    let B = analyze::board(r, &Default::default(), t);
    let S = [SourceRow {
        id: "rss:BBC".into(),
        region: r.id.clone(),
        url: String::new(),
        reliability: "B".into(),
        owner: "BBC".into(),
        cadence_s: 300,
        health: "failed",
        last_status: None,
        last_run: None,
    }];
    let text = produce::report(r, &B, &S, "intsum", t, None);
    for p in &r.pir {
        assert!(text.contains(&format!("**{}**", p.id)), "{}", p.id);
    }
    assert!(text.contains("Collection gap") || text.contains("collection gap"));
    assert!(text.contains("0 of 1 sources healthy"));
    assert_eq!(
        (produce::confidence(2), produce::confidence(3)),
        ("high", "moderate")
    );
    assert_eq!(produce::product(r, "sitrep", text, t).period_h, 6);
}

#[test]
fn poisson_tail() {
    assert_eq!(analyze::tail(0, 2.0), 1.0);
    assert!((analyze::tail(1, 0.5) - (1.0 - (-0.5f64).exp())).abs() < 1e-12);
    // 1 − e^(−1/2)(1 + 1/2 + 1/8)
    assert!((analyze::tail(3, 0.5) - 0.014388).abs() < 1e-6);
}

/// An explosion in Russia/Ukraine at t in oblast a, of target class x.
fn strike(k: usize, t: Time, a: &str, x: Option<&str>) -> osw::schema::Event {
    osw::schema::Event {
        id: format!("e{k}"),
        region: "russia-ukraine".into(),
        t_start: t,
        t_end: t,
        kind: "explosions_remote_violence".into(),
        credibility: 4,
        admin1: Some(a.into()),
        observation_ids: vec![format!("o{k}")],
        target: x.map(String::from),
        ..Default::default()
    }
}

/// Indicators of the Russia/Ukraine board at t, Telegram collection healthy, after
/// `days` days of reporting and these strikes.
fn ru_board(t: Time, days: i64, E: Vec<osw::schema::Event>) -> HashMap<String, Value> {
    let O: Vec<_> = (0..=days)
        .map(|k| {
            let mut x = ob(
                k as usize,
                t - Duration::days(k) - Duration::hours(1),
                None,
                None,
                "telegram:uniannet",
                "report",
                "none",
            );
            x.region = "russia-ukraine".into();
            x
        })
        .collect();
    store::replace(&O);
    store::replace(&E);
    let mut h = analyze::Health::new();
    h.insert("telegram:uniannet".into(), "ok");
    analyze::board(regions::get("russia-ukraine").unwrap(), &h, t)
        .into_iter()
        .flat_map(|p| p["indicators"].as_array().unwrap().clone())
        .map(|i| (i["id"].as_str().unwrap().to_string(), i))
        .collect()
}

/// Kharkiv: one strike a day for ten days, then four (p 0.022, not unusual). Samara:
/// none, then three on energy targets (unusual, and RU-2a's trigger).
#[test]
fn unusual_oblasts_against_their_trailing_days() {
    let _g = fresh("analysis_unusual");
    let t = at(2026, 10, 30, 12, 0, 0);
    let mut E = vec![];
    for k in 1..=10 {
        E.push(strike(
            k,
            t - Duration::days(k as i64) - Duration::hours(2),
            "Kharkiv",
            None,
        ));
    }
    for k in 0..4 {
        E.push(strike(
            20 + k,
            t - Duration::hours(k as i64 + 1),
            "Kharkiv",
            None,
        ));
    }
    for k in 0..3 {
        E.push(strike(
            30 + k,
            t - Duration::hours(k as i64 + 1),
            "Samara Oblast",
            Some("energy"),
        ));
    }
    let B = ru_board(t, 10, E.clone());
    let x = &B["RU-5a"];
    assert_eq!(
        (x["status"].as_str(), x["covered"].as_i64()),
        (Some("triggered"), Some(10)),
        "{x}"
    );
    assert_eq!(x["areas"].as_array().unwrap().len(), 1, "{x}");
    assert_eq!(
        (
            x["areas"][0]["area"].as_str(),
            x["areas"][0]["count"].as_i64()
        ),
        (Some("Samara Oblast"), Some(3))
    );
    let x = &B["RU-2a"];
    assert_eq!(
        (x["status"].as_str(), x["count"].as_i64()),
        (Some("triggered"), Some(3)),
        "{x}"
    );
    assert_eq!(x["areas"], json!([{"area": "Samara Oblast", "count": 3}]));
    assert_eq!(B["RU-2c"]["status"], "triggered");
    // Fewer than seven covered days: no baseline.
    assert_eq!(ru_board(t, 5, E)["RU-5a"]["status"], "no data");
}

/// The region INTSUM lists its areas; an area INTSUM keeps the area's events and the
/// indicators counted by area, at the area's count (Samara: three energy strikes).
#[test]
fn area_intsum() {
    let _g = fresh("analysis_area");
    let t = at(2026, 10, 30, 12, 0, 0);
    let mut E: Vec<_> = (0..3)
        .map(|k| {
            strike(
                k,
                t - Duration::hours(k as i64 + 1),
                "Samara Oblast",
                Some("energy"),
            )
        })
        .collect();
    E.push(strike(9, t - Duration::hours(2), "Kharkiv", None));
    ru_board(t, 10, E);
    let r = regions::get("russia-ukraine").unwrap();
    let mut h = analyze::Health::new();
    h.insert("telegram:uniannet".into(), "ok");
    let B = analyze::board(r, &h, t);
    let all = produce::report(r, &B, &[], "intsum", t, None);
    assert!(all.contains("## Areas"), "{all}");
    assert!(
        all.contains("| Samara Oblast | 3 | 3 | 0 | 3 | 0 | 0 | RU-2a, RU-2c, RU-5a |"),
        "{all}"
    );
    assert!(
        all.contains("| Kharkiv | 1 | 1 | 0 | 0 | 0 | 0 |  |"),
        "{all}"
    );
    let A = produce::of_area(&B, "Kharkiv");
    let ids: Vec<&str> = A
        .iter()
        .flat_map(|p| p["indicators"].as_array().unwrap())
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["RU-2a", "RU-2c", "RU-5a", "RU-5b"]);
    let n: Vec<i64> = A
        .iter()
        .flat_map(|p| p["indicators"].as_array().unwrap())
        .map(|i| i["count"].as_i64().unwrap())
        .collect();
    assert_eq!(n, [0, 0, 0, 0]);
    let one = produce::report(r, &B, &[], "intsum", t, Some("Samara Oblast"));
    assert!(
        one.starts_with("# INTSUM: Russia / Ukraine, Samara Oblast"),
        "{one}"
    );
    assert!(one.contains("RU-2a Strikes on energy targets in one oblast | 3 in 24 h: Samara Oblast 3 | 1 / 3 | triggered"), "{one}");
    assert!(!one.contains("## Areas") && !one.contains("RU-2b"), "{one}");
}
