//! Recorded context: provider errors, revisions, missing days and dated comparisons.
#![allow(non_snake_case)]

mod common;

use chrono::Duration;
use common::*;
use osw::collect::context as feeds;
use osw::collect::{self, Resp, Source};
use osw::fmt::Time;
use osw::schema::Raw;
use osw::{context, cycle, regions, store};
use serde_json::{Value, json};

fn T() -> Time {
    at(2026, 10, 24, 0, 0, 0)
}

fn source(code: &str) -> Source {
    feeds::sources(regions::all())
        .into_iter()
        .find(|s| s.code == code)
        .unwrap_or_else(|| panic!("no source {code}"))
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("{v} is not a number"))
}

/// Public responses captured 2026-10-24; provenance in docs/REVIEW-LOG.md.
#[test]
fn recorded_provider_responses() {
    for (file, code) in [
        ("fred_brent.csv", "DCOILBRENTEU"),
        ("wb_energy_imports.json", "EG.IMP.CONS.ZS"),
    ] {
        let X = feeds::parse(&fix(file), &source(code)).unwrap();
        assert_eq!(X.len(), 1);
        let P = X[0].payload["points"].as_array().unwrap();
        assert!(!P.is_empty());
        assert!(P.iter().any(|p| !p["value"].is_null()));
    }
    // The small live probe deliberately requested five rows; its transfer-limit flag must
    // prevent that sample from masquerading as a complete history.
    let e = feeds::parse(&fix("portwatch_chittagong.json"), &source("port241")).err();
    assert!(e.is_some_and(|e| e.contains("incomplete page")));
}

#[test]
fn fred_keeps_missing_and_negative_prices() {
    let src = source("DCOILWTICO");
    let X = feeds::parse(
        b"observation_date,DCOILWTICO\n2026-10-19,-2.5\n2026-10-20,.\n",
        &src,
    )
    .unwrap();
    assert_eq!(X.len(), 1);
    assert_eq!(
        X[0].payload["points"],
        json!([{"date": "2026-10-19", "value": -2.5}, {"date": "2026-10-20", "value": null}])
    );
    assert_eq!(X[0].published_at, Some(at(2026, 10, 20, 0, 0, 0)));
    assert_eq!(X[0].payload["unit"], "USD/bbl");
}

#[test]
fn provider_errors_and_truncation_are_collection_failures() {
    let src = source("port241");
    for body in [
        &br#"{"error":{"code":400}}"#[..],
        &br#"{"features":[],"exceededTransferLimit":true}"#[..],
    ] {
        let (_, rows, runs) =
            collect::run(std::slice::from_ref(&src), feeds::parse, "test", |_| {
                Ok(Resp {
                    status: 200,
                    ctype: None,
                    body: body.to_vec(),
                })
            });
        assert!(rows.is_empty());
        assert_eq!(runs[0].status, "parse_error");
    }
}

#[test]
fn portwatch_accepts_current_and_legacy_dates() {
    let src = source("port241");
    let page = |date: Value| {
        json!({"features": [{"attributes": {"date": date, "portid": "port241",
               "portname": "Chittagong", "portcalls": 8, "portcalls_tanker": 0}}]})
        .to_string()
    };
    let want = json!([{"date": "2026-10-18", "value": 8.0, "tanker": 0.0}]);
    for date in [json!("2026-10-18"), json!(1792281600000_i64)] {
        let X = feeds::parse(page(date).as_bytes(), &src).unwrap();
        assert_eq!(X[0].payload["points"], want);
    }
}

#[test]
fn centered_seven_day_mean_requires_calendar_coverage() {
    let P: Vec<Value> = (1..=7)
        .rev()
        .map(|k| json!({"date": (T().date_naive() - Duration::days(k)).to_string(), "value": 5}))
        .collect();
    let m = context::mean7(&P, "value");
    assert_eq!(num(&m["mean"]), 5.0);
    assert_eq!(m["n"], 7);
    assert_eq!(
        (&m["center_date"], &m["window_start"], &m["window_end"]),
        (
            &json!("2026-10-20"),
            &json!("2026-10-17"),
            &json!("2026-10-23")
        )
    );
    assert!(context::mean7(&P[1..], "value").is_null());
    let zero: Vec<Value> = P
        .iter()
        .map(|p| json!({"date": p["date"], "value": 0}))
        .collect();
    assert_eq!(num(&context::mean7(&zero, "value")["mean"]), 0.0);
}

#[test]
fn wdi_uses_country_specific_years_and_preserves_exporters() {
    let src = Source {
        kind: "wdi".into(),
        code: "EG.IMP.CONS.ZS".into(),
        name: "Net energy imports".into(),
        countries: vec!["BGD".into(), "MMR".into()],
        ..Default::default()
    };
    let mut d = json!([{"pages": 1}, [
        {"countryiso3code": "BGD", "country": {"value": "Bangladesh"}, "date": "2024", "value": 20},
        {"countryiso3code": "MMR", "country": {"value": "Myanmar"}, "date": "2024", "value": null},
        {"countryiso3code": "MMR", "country": {"value": "Myanmar"}, "date": "2022", "value": -5}]]);
    let X = feeds::parse(d.to_string().as_bytes(), &src).unwrap();
    let out = context::series(&X[0].payload, T(), "ok", T());
    assert_eq!(out["countries"]["BGD"]["date"], "2024");
    assert_eq!(num(&out["countries"]["MMR"]["value"]), -5.0);
    assert_eq!(out["countries"]["MMR"]["date"], "2022");
    d[0]["pages"] = json!(2);
    assert!(feeds::parse(d.to_string().as_bytes(), &src).is_err());
}

fn raw(id: String, sid: String, fetched: Time, payload: &Value) -> Raw {
    Raw {
        id,
        source_id: sid,
        region: "global".into(),
        fetched_at: fetched,
        published_at: None,
        url: None,
        payload: payload.to_string(),
    }
}

#[test]
fn context_api_selects_latest_release_and_aligns_spread_dates() {
    let _g = fresh("context_api");
    for (code, first, second) in [
        ("DCOILBRENTEU", 100, json!(105)),
        ("DCOILWTICO", 90, Value::Null),
    ] {
        for (rev, delta) in [0, 1].into_iter().enumerate() {
            let payload = json!({"kind": "fred", "code": code, "name": code,
                "source": "EIA via FRED", "source_url": "https://fred.stlouisfed.org/",
                "unit": "USD/bbl", "points": [
                    {"date": "2026-10-19", "value": first + delta},
                    {"date": "2026-10-20", "value": second}]});
            let x = raw(
                format!("{code}-{rev}"),
                format!("context:FRED#{code}"),
                T() + Duration::seconds(rev as i64),
                &payload,
            );
            store::append(&[x]);
        }
    }
    let region = regions::get("south-asia").unwrap();
    let original = context::view(region, T());
    assert!(
        original["series"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["releases_known"] == 1)
    );
    assert_eq!(num(&original["spread"]["value"]), 10.0);
    let d = context::view(region, T() + Duration::seconds(2));
    for s in d["series"].as_array().unwrap() {
        assert_eq!(s["revision"]["compared_points"], 2);
        assert_eq!(s["revision"]["changed_measures"], 1);
        assert_eq!(s["releases_known"], 2);
    }
    assert_eq!(d["spread"]["date"], "2026-10-19");
    assert_eq!(num(&d["spread"]["value"]), 10.0);
    assert_eq!(d["spread"]["unit"], "USD/bbl");
    let wti = d["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == "DCOILWTICO")
        .unwrap();
    assert_eq!(wti["latest"]["date"], "2026-10-19");
    assert_eq!(num(&wti["latest"]["value"]), 91.0);
    let res = api_json("/api/context?region=south-asia");
    assert_eq!(res["countries"].as_array().unwrap().len(), 8);
}

#[test]
fn provider_acceptance_rejects_ambiguous_keys_and_invalid_counts() {
    let src = source("DCOILWTICO");
    assert!(
        feeds::parse(
            b"observation_date,DCOILWTICO\n2026-10-19,1\n2026-10-19,2\n",
            &src
        )
        .is_err()
    );
    for (n, k) in [(8.0, 9.0), (-1.0, 0.0), (8.5, 1.0)] {
        let page = json!({"features": [{"attributes": {"date": "2026-10-18", "portid": "port241",
            "portcalls": n, "portcalls_tanker": k}}]});
        assert!(feeds::parse(page.to_string().as_bytes(), &source("port241")).is_err());
    }
}

#[test]
fn shipping_comparison_adapts_only_to_complete_prior_weeks() {
    let mut points: Vec<Value> = (1..=35)
        .map(|k| {
            json!({
        "date": (T().date_naive() - Duration::days(k)).to_string(),
        "value": if k <= 7 { 15 } else { 10 }})
        })
        .collect();
    let c = context::comparison(&points, "value");
    assert_eq!(c["n_baseline"], 4);
    assert_eq!(c["baseline_median"], 10.0);
    assert_eq!(c["change_pct"], 50.0);
    assert_eq!(c["current"]["window_end"], "2026-10-23");
    points[34]["value"] = Value::Null;
    let c = context::comparison(&points, "value");
    assert_eq!(c["n_baseline"], 3);
    assert!(c["baseline_median"].is_null() && c["change_pct"].is_null());
    assert_eq!(c["status"], "insufficient_history");
    for p in &mut points {
        p["value"] = json!(0);
    }
    let c = context::comparison(&points, "value");
    assert_eq!(c["status"], "complete");
    assert!(c["change_pct"].is_null());
}

#[test]
fn context_collection_reaches_recorded_view() {
    let _g = fresh("context_collection");
    let src = cycle::sources(&[], &["context".to_string()], false)
        .into_iter()
        .find(|s| s.code == "DCOILWTICO")
        .unwrap();
    let run = ingest(
        &src,
        "observation_date,DCOILWTICO\n2026-10-19,90\n2026-10-20,91\n",
    );
    assert_eq!(run.status, "ok");
    // Health as of a time counts only runs started by then: view after the run.
    let d = context::view(regions::get("south-asia").unwrap(), collect::now());
    let S = d["series"].as_array().unwrap();
    assert_eq!(S.len(), 1);
    assert_eq!(num(&S[0]["latest"]["value"]), 91.0);
    assert_eq!(S[0]["health"], "ok");
}

#[test]
fn future_and_missing_values_do_not_become_current_prices() {
    let payload = json!({"kind": "fred", "code": "x", "points": [
        {"date": "2026-10-25", "value": 9}, {"date": "2026-10-23", "value": null}]});
    let s = context::series(&payload, T(), "ok", T());
    assert!(s["latest"].is_null() && s["age_days"].is_null());
}
