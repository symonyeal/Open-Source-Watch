//! Event-time extraction (process::when), the contradiction rule (fuse::contradicted)
//! and the thermal persistence rule (process::feeds).
#![allow(non_snake_case)]

mod common;

use std::collections::{HashMap, HashSet};

use chrono::{Duration, NaiveDate};
use common::*;
use osw::fmt::Time;
use osw::fuse;
use osw::process::feeds;
use osw::process::when::event_time;
use osw::regions;
use osw::schema::{Obs, Run};

fn pub_() -> Time {
    at(2026, 10, 21, 7, 21, 53) // 10:21 in Kyiv (UTC+3)
}

const KYIV: &str = "An air raid alert sounded in Kyiv at around 5:20 a.m. local time on Oct. 21";
const TZ: &str = "Europe/Kyiv";

#[test]
fn event_time_cases() {
    let cases = [
        (KYIV, Some(at(2026, 10, 21, 2, 20, 0))),
        (
            "Explosions heard at 23:40 local time",
            Some(at(2026, 10, 20, 20, 40, 0)),
        ),
        ("Sirens at 11 pm", Some(at(2026, 10, 20, 20, 0, 0))), // day before
        ("overnight attack on the port", None),
        ("Talks at 3 p.m. on Nov. 2", None), // announced
        ("A 5 member team", None),
        ("13 pm", None),
    ];
    for (s, want) in cases {
        assert_eq!(event_time(s, pub_(), TZ), want, "{s}");
    }
}

#[test]
fn event_time_across_new_year() {
    let jan2 = at(2027, 1, 2, 9, 0, 0);
    assert_eq!(
        event_time("Strike at 4 a.m. on Dec. 31", jan2, TZ),
        Some(at(2026, 12, 31, 2, 0, 0))
    );
}

/// Regression: an impossible date raised and stopped the poll's processing; "0 pm"
/// was read as noon.
#[test]
fn impossible_time_is_none_not_an_error() {
    for s in [
        "Strike at 5 p.m. on Feb. 30",
        "at 5 pm on Oct 45",
        "Sirens at 11 pm on Feb 29",
        "Explosion at 0 pm",
        "at 13 pm",
    ] {
        assert_eq!(event_time(s, pub_(), TZ), None, "{s}");
    }
}

fn ru() -> &'static regions::Region {
    regions::get("russia-ukraine").unwrap()
}

fn claim(tau: Option<Time>) -> Obs {
    let mut x = ob(
        0,
        pub_(),
        None,
        None,
        "telegram:KyivIndependent_official",
        "air_raid_alert",
        "none",
    );
    x.t_event = tau;
    x
}

fn runs(t0: Time, n: i64, status: &str) -> Vec<Run> {
    (0..n)
        .map(|k| {
            run(
                "alerts:alertsua",
                "russia-ukraine",
                t0 + Duration::minutes(k),
                status,
            )
        })
        .collect()
}

fn contradicted(M: &[Obs], alerts: &[Time], R: &[Run]) -> bool {
    let (M, R): (Vec<&Obs>, Vec<&Run>) = (M.iter().collect(), R.iter().collect());
    fuse::contradicted(ru(), &M, alerts, &R)
}

/// The live false positive of 2026-10-21: no warning-source runs at 02:20 UTC.
#[test]
fn kyiv_report_of_an_alert_before_collection_is_not_contradicted() {
    let tau = event_time(KYIV, pub_(), TZ);
    assert!(tau.is_some());
    assert!(!contradicted(
        &[claim(tau)],
        &[],
        &runs(pub_() - Duration::minutes(20), 40, "ok")
    ));
}

#[test]
fn covered_silence_contradicts() {
    let tau = at(2026, 10, 21, 3, 0, 0);
    assert!(contradicted(
        &[claim(Some(tau))],
        &[],
        &runs(tau - Duration::minutes(5), 10, "ok")
    ));
}

#[test]
fn no_contradiction() {
    let tau = at(2026, 10, 21, 3, 0, 0);
    let mut warning = claim(None);
    warning.source_id = "alerts:alertsua".into();
    let cases: [(Vec<Obs>, Vec<Time>, &str); 4] = [
        (vec![claim(None)], vec![], "ok"), // no stated time
        (
            vec![claim(Some(tau))],
            vec![at(2026, 10, 21, 3, 20, 0)],
            "ok",
        ), // alert recorded
        (vec![claim(Some(tau))], vec![], "http_error"), // source failing
        (vec![claim(Some(tau)), warning], vec![], "ok"), // a warning source supports
    ];
    for (k, (M, alerts, status)) in cases.into_iter().enumerate() {
        assert!(
            !contradicted(&M, &alerts, &runs(tau - Duration::minutes(5), 10, status)),
            "case {k}"
        );
    }
}

/// Active on >= 3 of the 7 prior days: persistent; a new cell is an anomaly.
#[test]
fn persistent_heat_source() {
    let d0 = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let c = feeds::cell(29.5, 47.7);
    let mut D: HashMap<(i64, i64), HashSet<NaiveDate>> = HashMap::new();
    D.insert(
        c,
        [1, 3, 6]
            .map(|k| d0 - Duration::days(k))
            .into_iter()
            .collect(),
    );
    assert!(feeds::persistent(&D, c, d0));
    assert!(!feeds::persistent(&D, c, d0 + Duration::days(5))); // 1 of the prior 7 days
    assert!(!feeds::persistent(&D, feeds::cell(31.0, 35.0), d0));
}
