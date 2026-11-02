//! Transit counting, ACH ranking and products (analyze, produce).
#![allow(non_snake_case)]

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
    let text = produce::report(r, &B, &S, "intsum", t);
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
