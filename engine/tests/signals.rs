//! Signals: GNSS cells (H3, gpsjam, own ADS-B), Taiwan MND reports, IODA outages, UCDP
//! reference events and the recall grid. Fixtures are public responses captured
//! 2026-10-29 and trimmed (docs/REVIEW-LOG.md). The fetches of
//! gpsjam, MND and UCDP read the network and are not exercised; their parsers are.
#![allow(non_snake_case)]

mod common;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{Duration, Utc};
use common::*;
use osw::collect::{self, Source, adapter, adsb, gpsjam, integrity, ioda, mnd, ucdp};
use osw::fmt::Time;
use osw::schema::Raw;
use osw::{analyze, cycle, fuse, geo, gnss, regions, signals, store, validate};
use serde_json::{Value, json};

fn jam() -> String {
    text("gpsjam_2026-10-27_sample.csv")
}

fn body(x: &Value) -> impl FnOnce(&Source) -> Result<collect::Resp, collect::Fail> {
    let r = collect::response(x);
    move |_| Ok(r)
}

fn region(id: &str) -> &'static regions::Region {
    regions::get(id).unwrap()
}

fn src_of(S: Vec<Source>, region: &str) -> Source {
    S.into_iter().find(|s| s.region == region).unwrap()
}

/// The indicators of the board of r at t, by id.
fn board(r: &regions::Region, h: &[(&str, &'static str)], t: Time) -> HashMap<String, Value> {
    let h = h.iter().map(|x| (x.0.to_string(), x.1)).collect();
    analyze::board(r, &h, t)
        .into_iter()
        .flat_map(|p| p["indicators"].as_array().unwrap().clone())
        .map(|i| (i["id"].as_str().unwrap().to_string(), i))
        .collect()
}

// GNSS ---------------------------------------------------------------------------

/// Each gpsjam cell's centre maps back to the same H3 resolution-4 index.
#[test]
fn h3_cells_reproduce_gpsjam_indices() {
    let H: Vec<String> = jam()
        .lines()
        .skip(1)
        .map(|l| l.split(',').next().unwrap().to_string())
        .collect();
    assert!(H.len() > 3);
    for h in &H {
        let (lat, lon) = geo::centre(h).unwrap();
        assert_eq!(
            geo::cell(lat, lon, 4),
            Some(u64::from_str_radix(h, 16).unwrap()),
            "{h}"
        );
    }
    assert!(H[..3].iter().all(|h| geo::boundary(h).len() == 6));
    assert_eq!(geo::cell(f64::NAN, 0.0, 4), None);
    assert_eq!(geo::cell(91.0, 0.0, 4), None);
    assert_eq!(geo::cell(0.0, 181.0, 4), None);
    assert_eq!(geo::centre("3039"), None); // 12345 is not a cell
}

#[test]
fn aircraft_counts_once_per_cell_day_bad_if_any_report_bad() {
    // (day, cell, aircraft, bad)
    let X = vec![
        (1, 7, 1, false),
        (1, 7, 1, true),
        (1, 7, 2, false),
        (1, 9, 1, false),
        (2, 7, 1, false),
    ];
    assert_eq!(
        geo::aircraft_counts(X),
        [(1, 7, 1, 1), (1, 9, 1, 0), (2, 7, 1, 0)]
    );
}

/// gpsjam FAQ: 100 (bad − 1) / (good + bad); < 2 % low, 2-10 % medium, > 10 % high.
#[test]
fn gpsjam_formula_and_bands() {
    assert_eq!(
        (gnss::pct(0, 1), gnss::pct(10, 0), gnss::pct(0, 0)),
        (0.0, 0.0, 0.0)
    );
    assert_eq!(gnss::pct(98, 2), 1.0);
    assert_eq!(gnss::band(1.99), "low");
    assert_eq!((gnss::band(2.0), gnss::band(10.0)), ("medium", "medium"));
    assert_eq!(gnss::band(10.01), "high");
    assert_eq!(gnss::band(gnss::pct(8, 2)), "medium");
    assert_eq!(gnss::band(gnss::pct(3, 3)), "high");
}

#[test]
fn per_aircraft_rule() {
    assert!(!gnss::bad(Some(8.0), Some(9.0), false));
    assert!(gnss::bad(Some(8.0), Some(7.0), false));
    assert!(gnss::bad(Some(6.0), Some(9.0), false));
    assert!(gnss::bad(None, None, true));
    assert!(!gnss::bad(Some(0.0), Some(10.0), false)); // NIC 0 with NACp >= 10 is unknown NIC
    assert!(gnss::bad(Some(0.0), None, false));
    assert!(!gnss::bad(None, Some(9.0), false));
}

#[test]
fn gpsjam_region_cells_and_stored_days() {
    let _g = fresh("signals_gpsjam");
    let src = src_of(gpsjam::sources(regions::all()), "black-sea");
    let day = |k: i64| (Utc::now().date_naive() - Duration::days(k)).to_string();
    let mut d = json!({"manifest": {}, "days": {}});
    d["manifest"][day(2)] = json!({"date": day(2), "suspect": "false", "source": "merged"});
    d["days"][day(2)] = json!(jam());
    let items = gpsjam::parse(d.to_string().as_bytes(), &src).unwrap();
    assert_eq!(items.len(), 1);
    let p = &items[0].payload;
    assert_eq!(items[0].native_id, day(2));
    assert_eq!((&p["suspect"], &p["res"]), (&json!(false), &json!(4)));
    let C = p["cells"].as_array().unwrap();
    assert!(!C.is_empty());
    for c in C {
        let (a, o) = geo::centre(c[0].as_str().unwrap()).unwrap();
        assert!(region("black-sea").in_bbox(a, o));
    }
    // The region's GNSS indicator counts the latest stored day's high cells.
    let (_, raw, _) = collect::run(std::slice::from_ref(&src), gpsjam::parse, "test", body(&d));
    store::append(&raw);
    let high = C
        .iter()
        .filter(|c| gnss::band(gnss::pct(c[1].as_i64().unwrap(), c[2].as_i64().unwrap())) == "high")
        .count() as i64;
    assert!(high > 0);
    let B = board(region("black-sea"), &[(&src.id, "ok")], collect::now());
    let g = B.values().find(|i| i["metric"] == "gnss").unwrap();
    assert_eq!(g["status"], "observed");
    assert_eq!(g["count"], high);
    assert_eq!(g["date"], day(2));
    assert_eq!(g["sources"], json!([src.id]));
}

#[test]
fn integrity_filters_follow_gods_eye_view() {
    let src = src_of(integrity::sources(regions::all()), "black-sea");
    let adsb_url = cycle::sources(&["black-sea".to_string()], &["adsb".to_string()], false)
        .remove(0)
        .url;
    assert_eq!(src.url, adsb_url);
    let ac = |hex: &str, extra: Value| {
        let mut a = json!({"type": "adsb_icao", "version": 2, "alt_baro": 30000, "seen_pos": 0,
                           "lat": 44.0, "lon": 34.0, "nic": 8, "nac_p": 9, "hex": hex});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        a
    };
    let A = [
        ac("aaa001", json!({})),             // kept, seen_pos 0
        ac("aaa002", json!({"version": 0})), // no quality fields
        ac("aaa003", json!({"alt_baro": "ground"})),
        ac("aaa004", json!({"seen_pos": 61})),
        ac("aaa005", json!({"mlat": ["nic", "nac_p"]})), // derived quality only
        ac(
            "aaa006",
            json!({"gpsOkBefore": 1.7e12, "gpsOkLat": 45.0, "gpsOkLon": 35.0}),
        ),
        ac("~aaa007", json!({"lat": 10.0})), // outside the box
    ];
    let b = json!({"now": 1793291463000_i64, "ac": A}).to_string();
    let items = integrity::parse(b.as_bytes(), &src).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].payload["ac"],
        json!([
            ["aaa001", 44.0, 34.0, 8, 9, false],
            ["aaa006", 45.0, 35.0, 8, 9, true]
        ])
    );
    assert_eq!(items[0].published_at, Some(at(2026, 10, 29, 16, 31, 3)));
}

#[test]
fn mixed_adapters_on_one_url_parse_by_their_own_adapter() {
    let both = ["adsb".to_string(), "integrity".to_string()];
    let S = cycle::sources(&["black-sea".to_string()], &both, true);
    let url = S
        .iter()
        .find(|s| s.id.starts_with("integrity:"))
        .unwrap()
        .url
        .clone();
    let G: Vec<Source> = S.into_iter().filter(|s| s.url == url).collect();
    let A: BTreeSet<&str> = G.iter().map(|s| adapter(&s.id)).collect();
    assert_eq!(A, BTreeSet::from(["adsb", "integrity"]));
    let ac = json!({"now": 1793291463000_i64, "ac": [{"hex": "43c001", "type": "adsb_icao", "version": 2,
                    "alt_baro": 30000, "seen_pos": 1, "lat": 44.0, "lon": 34.0, "nic": 8, "nac_p": 9,
                    "dbFlags": 1}]});
    let parse = |b: &[u8], s: &Source| match adapter(&s.id) {
        "adsb" => adsb::parse(b, s),
        _ => integrity::parse(b, s),
    };
    let (_, raw, runs) = collect::run(&G, parse, "test", body(&ac));
    let mut kinds: Vec<&str> = raw.iter().map(|x| adapter(&x.source_id)).collect();
    kinds.sort();
    assert_eq!(kinds, ["adsb", "integrity"]);
    assert!(runs.iter().all(|r| r.status == "ok"));
}

/// The dashboard's aircraft requests must not take integrity snapshots every minute:
/// a source sharing the URL is polled again only when its own cadence has elapsed.
#[test]
fn proxy_polls_a_colocated_source_by_its_own_cadence() {
    let _g = fresh("signals_proxy");
    let both = ["adsb".to_string(), "integrity".to_string()];
    let S = cycle::sources(&["black-sea".to_string()], &both, true);
    let url = S
        .iter()
        .find(|s| s.id.starts_with("integrity:"))
        .unwrap()
        .url
        .clone();
    let count = |prefix: &str| {
        store::rows::<osw::schema::Run>()
            .iter()
            .filter(|x| x.source_id.starts_with(prefix))
            .count()
    };
    assert!(cycle::proxy(&url).is_none()); // offline
    assert_eq!((count("adsb:"), count("integrity:")), (1, 1));
    assert!(cycle::proxy(&url).is_none());
    assert_eq!((count("adsb:"), count("integrity:")), (2, 1));
}

#[test]
fn own_adsb_cells_and_agreement() {
    let _g = fresh("signals_own_adsb");
    let r = region("black-sea");
    let t = at(2026, 10, 29, 12, 0, 0);
    let snap = |k: i64, ac: Value| Raw {
        id: format!("i{k}"),
        source_id: "integrity:adsb.lol#black-sea".into(),
        region: "black-sea".into(),
        fetched_at: t + Duration::minutes(10 * k),
        published_at: Some(t + Duration::minutes(10 * k)),
        url: None,
        payload: json!({"ac": ac}).to_string(),
    };
    let a = json!(["aaa001", 44.0, 34.0, 8, 9, false]);
    let b = json!(["aaa002", 44.01, 34.01, 8, 9, false]);
    let c = json!(["~aaa001", 44.02, 34.02, 8, 5, false]); // a non-ICAO address, not aircraft a
    let a5 = json!(["aaa001", 44.0, 34.0, 8, 5, false]);
    store::append(&[snap(0, json!([a, b, c])), snap(1, json!([a5, b]))]);
    let A = gnss::adsb(r, t - Duration::days(1));
    assert_eq!(A.len(), 1);
    let cells = A["2026-10-29"].clone();
    assert_eq!(cells.len(), 1);
    assert_eq!((cells[0].1, cells[0].2), (1, 2)); // a bad in one report, c bad, b good
    let J = |g: i64, b: i64| -> BTreeMap<String, Value> {
        BTreeMap::from([(
            "2026-10-29".to_string(),
            json!({"cells": [[cells[0].0, g, b]]}),
        )])
    };
    assert_eq!(
        gnss::agreement(&J(40, 10), &A),
        json!({"both": 1, "gpsjam": 0, "engine": 0, "neither": 0})
    );
    assert_eq!(gnss::agreement(&J(99, 1), &A)["engine"], 1);
}

// Taiwan MND ------------------------------------------------------------------------

#[test]
fn mnd_list_and_report() {
    let L = mnd::links(&text("mnd_list.html"));
    assert_eq!(L[0], ("88094".to_string(), "2026-10-29".to_string()));
    assert!(L.len() >= 5);
    let p = mnd::report("88094", &text("mnd_report_88094.html"))
        .unwrap()
        .unwrap();
    let counts: Vec<&Value> = [
        "aircraft",
        "ships",
        "official",
        "balloons",
        "adiz",
        "median_line",
    ]
    .iter()
    .map(|k| &p[*k])
    .collect();
    assert_eq!(
        counts,
        [
            &json!(6),
            &json!(7),
            &json!(1),
            &Value::Null,
            &json!(5),
            &Value::Null
        ]
    );
    assert_eq!(
        (&p["start"], &p["end"], &p["day"]),
        (
            &json!("2026-10-27T22:00:00+00:00"),
            &json!("2026-10-28T22:00:00+00:00"),
            &json!("2026-10-29")
        )
    );
    assert!(p["text"].as_str().unwrap().chars().count() <= collect::EXCERPT);
}

#[test]
fn mnd_year_boundary_and_failures() {
    assert_eq!(
        mnd::local6("Dec", "31", at(2027, 1, 1, 0, 0, 0)),
        Ok(at(2026, 12, 30, 22, 0, 0))
    );
    let page = r#"<div class="pageinfo">2026.10.29</div><div class="maincontent">"#;
    assert_eq!(
        mnd::report("1", &format!("{page}A statement without a window.</div>")),
        Ok(None)
    );
    let none = mnd::report(
        "1",
        &format!(
            "{page}1.Date: 6 a.m. Oct. 28 to 6 a.m. Oct. 29 (UTC+8) 2.Nothing to report.</div>"
        ),
    );
    assert!(none.unwrap_err().contains("no counts"));
    let b = json!({"1": format!("{page}no window</div>")}).to_string();
    let src = mnd::sources(regions::all()).remove(0);
    assert!(
        mnd::parse(b.as_bytes(), &src)
            .err()
            .unwrap()
            .contains("parsed")
    );
    let page0 = format!(
        "{page}1.Date: 6 a.m. Oct. 28 to 6 a.m. Oct. 29 (UTC+8) 2.PLA activities: \
         no PLA aircraft, 3 PLAN ships and no official ships.</div>"
    );
    let p = mnd::report("2", &page0).unwrap().unwrap();
    assert_eq!(
        (&p["aircraft"], &p["ships"], &p["official"]),
        (&json!(0), &json!(3), &json!(0))
    );
}

fn mnd_raw(id: &str, region: &str, sid: &str, t: Time, published: Time, p: Value) -> Raw {
    Raw {
        id: id.into(),
        source_id: sid.into(),
        region: region.into(),
        fetched_at: t,
        published_at: Some(published),
        url: None,
        payload: p.to_string(),
    }
}

#[test]
fn official_series_median_and_indicator() {
    let _g = fresh("signals_official");
    let r = region("taiwan-strait");
    let src = mnd::sources(regions::all()).remove(0);
    let t = at(2026, 10, 29, 12, 0, 0);
    let raw: Vec<Raw> = (0..5)
        .map(|k| {
            let day = (t - Duration::days(k)).date_naive().to_string();
            let p = json!({"id": (100 - k).to_string(), "day": day, "aircraft": k, "ships": 5,
                           "official": null, "balloons": null, "adiz": null, "median_line": null});
            mnd_raw(
                &format!("m{k}"),
                &r.id,
                &src.id,
                t,
                t - Duration::days(k),
                p,
            )
        })
        .collect();
    store::append(&raw);
    let o = signals::official(r, 90);
    assert_eq!(o["latest"]["aircraft"], 0);
    assert_eq!(o["median"]["aircraft"].as_f64(), Some(2.5));
    assert!(o["median"]["official"].is_null());
    assert_eq!(o["n_prior"], 4);
    assert_eq!(o["n_by_field"]["aircraft"], 4);
    assert_eq!(o["n_by_field"]["official"], 0);
    assert_eq!(o["baseline_start"], "2026-10-25");
    assert_eq!(o["baseline_end"], "2026-10-28");
    let B = board(r, &[(&src.id, "ok")], t);
    let ok = &B["TW-1c"];
    assert_eq!(
        (&ok["status"], &ok["count"], &ok["date"]),
        (&json!("observed"), &json!(0), &json!("2026-10-29"))
    );
    assert_eq!(ok["sources"], json!([src.id]));
    assert_eq!(
        board(r, &[(&src.id, "failed")], t)["TW-1c"]["status"],
        "no data"
    );
    let late = t + Duration::days(4); // the latest report is older than the 3 days an indicator accepts
    assert_eq!(
        board(r, &[(&src.id, "ok")], late)["TW-1c"]["status"],
        "no data"
    );
}

/// A report that does not state the indicator's count is not a count of zero.
#[test]
fn an_unstated_official_count_is_no_data() {
    let _g = fresh("signals_unstated");
    let r = region("taiwan-strait");
    let src = mnd::sources(regions::all()).remove(0);
    let t = at(2026, 10, 29, 12, 0, 0);
    let p = json!({"id": "1", "day": "2026-10-29", "aircraft": null, "ships": 5, "official": null,
                   "balloons": null, "adiz": null, "median_line": null});
    store::append(&[mnd_raw("m", &r.id, &src.id, t, t, p)]);
    let h = analyze::Health::from([(src.id.clone(), "ok")]);
    let B = analyze::board(r, &h, t);
    let x = B
        .iter()
        .flat_map(|q| q["indicators"].as_array().unwrap())
        .find(|i| i["id"] == "TW-1c")
        .unwrap();
    assert_eq!(
        (&x["status"], &x["date"]),
        (&json!("no data"), &json!("2026-10-29"))
    );
    assert!(x["count"].is_null());
    store::append(&analyze::snapshot(r, &B, t));
    let S = store::rows::<osw::schema::Status>();
    let c: Vec<Option<i32>> = S
        .iter()
        .filter(|s| s.indicator == "TW-1c")
        .map(|s| s.count)
        .collect();
    assert_eq!(c, [None]);
}

// IODA ------------------------------------------------------------------------------

#[test]
fn ioda_events_observations_and_linking() {
    let _g = fresh("signals_ioda");
    let S: Vec<Source> = ioda::sources(regions::all())
        .into_iter()
        .filter(|s| s.code == "IR")
        .collect();
    let R: BTreeSet<&str> = S.iter().map(|s| s.region.as_str()).collect();
    assert_eq!(R, BTreeSet::from(["iran-israel", "hormuz"]));
    assert_eq!(S.iter().map(|s| &s.url).collect::<BTreeSet<_>>().len(), 1);
    let src = src_of(S, "iran-israel");
    let items = ioda::parse(&fix("ioda_events_ir.json"), &src).unwrap();
    assert!(!items.is_empty() && items.iter().all(|i| i.payload["country"] == "IR"));
    assert!(ioda::parse(br#"{"error": "bad request", "data": null}"#, &src).is_err());
    let t0 = 1793000000_i64;
    let ev: Vec<Value> = [("bgp", 0), ("ping-slash24", 1800), ("merit-nt", 30000)]
        .iter()
        .map(|(ds, d)| {
            json!({"location": "country/IR", "start": t0 + d, "duration": 3600, "datasource": ds,
                   "method": "median", "score": 10, "location_name": "Iran"})
        })
        .collect();
    let (_, raw, _) = collect::run(
        std::slice::from_ref(&src),
        ioda::parse,
        "test",
        body(&json!({"data": ev, "error": null})),
    );
    let O = cycle::derive(&raw).obs;
    assert_eq!(O.len(), 3);
    assert!(O.iter().all(|o| o.kind == "internet_outage"));
    assert!(O.iter().all(
        |o| (o.lat, o.lon, o.geo_precision.as_str()) == (Some(35.6892), Some(51.389), "admin")
    ));
    let X: Vec<&osw::schema::Obs> = O.iter().collect();
    let E = fuse::events(region("iran-israel"), &X, &HashMap::new(), &[]);
    let mut n: Vec<usize> = E.iter().map(|e| e.observation_ids.len()).collect();
    n.sort();
    assert_eq!(n, [1, 2]); // within τ link
    // A producer of internet outages is an IODA source, and no other.
    let tw = region("taiwan-strait");
    let t = collect::now();
    assert_ne!(
        board(tw, &[("ioda:IODA#TW", "ok")], t)["TW-3b"]["status"],
        "no data"
    );
    assert_eq!(
        board(tw, &[("rss:BBC", "ok")], t)["TW-3b"]["status"],
        "no data"
    );
    // Reports of a country, or a region, since removed from the registry derive nothing.
    let mut p: Value = serde_json::from_str(&raw[0].payload).unwrap();
    p["country"] = json!("ZZ");
    let x1 = Raw {
        id: "x1".into(),
        payload: p.to_string(),
        ..raw[0].clone()
    };
    let x2 = Raw {
        id: "x2".into(),
        region: "atlantis".into(),
        source_id: "rss:Gone".into(),
        ..raw[0].clone()
    };
    let D = cycle::derive(&[x1, x2]);
    assert!(D.obs.is_empty() && D.tracks.is_empty());
}

// UCDP ------------------------------------------------------------------------------

fn ucdp_body(have: Value) -> Value {
    json!({"version": "26.0.9", "url": "https://ucdp.uu.se/downloads/",
           "csv": text("ucdp_candidate_v26_0_9_sample.csv"), "have": have})
}

#[test]
fn ucdp_parse_keeps_region_events_with_version() {
    let _g = fresh("signals_ucdp");
    let src = src_of(ucdp::sources(regions::all()), "south-asia");
    let d = ucdp_body(json!([]));
    let items = ucdp::parse(d.to_string().as_bytes(), &src).unwrap();
    assert!(!items.is_empty());
    let allowed = ["Pakistan", "Myanmar (Burma)"];
    assert!(
        items
            .iter()
            .all(|i| allowed.contains(&i.payload["country"].as_str().unwrap()))
    );
    assert!(
        items
            .iter()
            .all(|i| i.native_id.ends_with("@26.0.9") && i.payload["version"] == "26.0.9")
    );
    assert!(items.iter().all(|i| {
        i.payload["where_description"]
            .as_str()
            .unwrap()
            .chars()
            .count()
            <= collect::EXCERPT
    }));
    // A revised event is a new report; ucdp::events keeps its latest version.
    let (_, raw, _) = collect::run(std::slice::from_ref(&src), ucdp::parse, "test", body(&d));
    let mut p: Value = serde_json::from_str(&raw[0].payload).unwrap();
    p["version"] = json!("26.0.10");
    p["best"] = json!(-1);
    let later = Raw {
        id: "later".into(),
        payload: p.to_string(),
        ..raw[0].clone()
    };
    store::append(&raw);
    store::append(&[later]);
    let U = ucdp::events(region("south-asia"));
    assert_eq!(U.len(), raw.len());
    let revised = U.iter().find(|u| u["id"] == p["id"]).unwrap();
    assert_eq!(revised["best"], -1);
}

/// A poll of one region must not keep the version from the others: the regions that
/// store the version take nothing from it, the others take all of it.
#[test]
fn ucdp_version_skipped_only_by_regions_that_store_it() {
    let S = ucdp::sources(regions::all());
    let sa = src_of(S, "south-asia");
    let all = ucdp::parse(ucdp_body(json!([])).to_string().as_bytes(), &sa)
        .unwrap()
        .len();
    let have = ucdp_body(json!(["south-asia"]));
    assert!(
        ucdp::parse(have.to_string().as_bytes(), &sa)
            .unwrap()
            .is_empty()
    );
    let other = ucdp_body(json!(["hormuz"]));
    let n = ucdp::parse(other.to_string().as_bytes(), &sa)
        .unwrap()
        .len();
    assert!(n > 0);
    assert_eq!(n, all);
}

#[test]
fn recall_windows_distance_and_eligibility() {
    let u = json!({"date_start": "2026-10-22", "date_prec": 1, "where_prec": 1, "latitude": 30.0, "longitude": 70.0});
    let with = |k: &str, v: i64| {
        let mut x = u.clone();
        x[k] = json!(v);
        x
    };
    let U = [u.clone(), with("where_prec", 3), with("date_prec", 2)];
    let t = at(2026, 10, 22, 0, 0, 0);
    let (near, far) = ((30.1, 70.0), (30.5, 70.0)); // 11 km, 56 km
    let period = (t - Duration::days(3), t + Duration::days(5));
    let m = |E: &[(Time, f64, f64)], lag: i64, period: (Time, Time)| {
        let r = validate::matches(&U, E, "UTC", lag, period);
        (r.0, r.1)
    };
    let e = |p: (f64, f64), dt: Duration| (t + dt, p.0, p.1);
    assert_eq!(m(&[e(near, Duration::hours(5))], 0, period), (1, 1));
    let late = [e(near, Duration::hours(30))];
    assert_eq!(m(&late, 0, period), (1, 0));
    assert_eq!(m(&late, 1, period), (1, 1));
    assert_eq!(m(&[e(far, Duration::zero())], 2, period), (1, 0));
    assert_eq!(
        m(&[], 0, (t + Duration::days(1), t + Duration::days(5))).0,
        0
    );
    assert_eq!(
        (geo::km(30.0, 70.0, 30.1, 70.0) * 10.0).round() / 10.0,
        11.1
    );
}

// View ------------------------------------------------------------------------------

#[test]
fn signals_endpoint_and_board() {
    let _g = fresh("signals_view");
    let d = api_json("/api/signals?region=taiwan-strait");
    for k in ["gnss", "official", "outages", "reference", "sources"] {
        assert!(d.get(k).is_some(), "{k}");
    }
    assert!(d["official"]["latest"].is_null() && d["reference"].is_null());
    let I = board(region("taiwan-strait"), &[], collect::now());
    assert_eq!(I["TW-1c"]["status"], "no data");
    assert_eq!(I["TW-3a"]["status"], "no data");
    assert_eq!(I["TW-3b"]["status"], "no data"); // no healthy IODA source
}
