//! Certificates for fusion (spec § Fusion) and credibility (spec § Credibility).
//! Properties are checked on seeded random inputs.
#![allow(non_snake_case)]

mod common;

use std::collections::{BTreeSet, HashMap};

use chrono::Duration;
use common::*;
use osw::fuse::{self, DELTA, TAU};
use osw::geo::{Point, components};
use osw::regions;
use osw::schema::{Event, Obs};
use osw::{analyze, store};

/// Great-circle distance in km, IUGG mean radius (independent of geo::km).
fn haversine(a: &Point, b: &Point) -> f64 {
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let h = ((p2 - p1) / 2.0).sin().powi(2)
        + p1.cos() * p2.cos() * ((b.lon - a.lon).to_radians() / 2.0).sin().powi(2);
    2.0 * 6371.0088 * h.sqrt().asin()
}

/// Connected components of the link graph by exhaustive pair check and a stack search;
/// the label of a component is its smallest index.
fn brute(P: &[Point]) -> Vec<usize> {
    let n = P.len();
    let mut adj = vec![vec![]; n];
    for i in 0..n {
        for j in i + 1..n {
            let (p, q) = (&P[i], &P[j]);
            if (p.t - q.t).abs() > TAU {
                continue;
            }
            let ok = if p.fine && q.fine {
                haversine(p, q) <= DELTA
            } else {
                p.lat == q.lat && p.lon == q.lon && !(p.country || q.country)
            };
            if ok {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
    }
    let (mut lab, mut seen) = (vec![usize::MAX; n], vec![false; n]);
    for s in 0..n {
        if seen[s] {
            continue;
        }
        let (mut stack, mut C) = (vec![s], vec![]);
        while let Some(u) = stack.pop() {
            if !seen[u] {
                seen[u] = true;
                C.push(u);
                stack.extend(&adj[u]);
            }
        }
        let m = *C.iter().min().unwrap();
        for u in C {
            lab[u] = m;
        }
    }
    lab
}

/// Random observations: times in 6 h, four places, four precisions (as the hypothesis
/// strategy), sorted by time.
fn sample(g: &mut Rng, n: usize) -> Vec<Point> {
    const PLACE: [(f64, f64); 4] = [(48.0, 35.0), (48.1, 35.1), (48.3, 35.0), (49.0, 32.0)];
    const PREC: [&str; 4] = ["point", "city", "admin", "country"];
    let mut P: Vec<Point> = (0..n)
        .map(|_| {
            let (lat, lon) = PLACE[g.below(4)];
            let prec = PREC[g.below(4)];
            Point {
                t: g.u() * 6.0 * 3600.0,
                lat,
                lon,
                fine: prec == "point" || prec == "city",
                country: prec == "country",
            }
        })
        .collect();
    P.sort_by(|a, b| a.t.total_cmp(&b.t));
    P
}

#[test]
fn components_equal_brute_force() {
    let mut g = Rng(1);
    for _ in 0..200 {
        let n = 1 + g.below(40);
        let P = sample(&mut g, n);
        assert_eq!(components(&P, TAU, DELTA), brute(&P));
    }
}

/// Dense input, locations on a 0.1° lattice (about 11 km) so both link branches occur.
#[test]
fn components_on_dense_input() {
    let mut g = Rng(0);
    for n in [0, 1, 400] {
        let mut P: Vec<Point> = (0..n)
            .map(|_| {
                let t = g.u() * 3.0 * 86400.0;
                let (lat, lon) = (
                    48.0 + g.below(10) as f64 / 10.0,
                    35.0 + g.below(10) as f64 / 10.0,
                );
                let (fine, country) = (g.u() < 0.6, g.u() < 0.1);
                Point {
                    t,
                    lat,
                    lon,
                    fine: fine && !country,
                    country,
                }
            })
            .collect();
        P.sort_by(|a, b| a.t.total_cmp(&b.t));
        assert_eq!(components(&P, TAU, DELTA), brute(&P), "n = {n}");
    }
}

fn explosion(k: usize, minutes: i64, lat: f64, lon: f64, src: &str, prec: &str) -> Obs {
    ob(
        k,
        at(2026, 10, 21, 0, 0, 0) + Duration::minutes(minutes),
        Some(lat),
        Some(lon),
        src,
        "explosions_remote_violence",
        prec,
    )
}

fn ii() -> &'static regions::Region {
    regions::get("iran-israel").unwrap()
}

fn events(X: &[&Obs]) -> Vec<Event> {
    fuse::events(ii(), X, &HashMap::new(), &[])
}

fn sets(E: &[Event]) -> BTreeSet<BTreeSet<String>> {
    E.iter()
        .map(|e| e.observation_ids.iter().cloned().collect())
        .collect()
}

#[test]
fn events_do_not_depend_on_input_order() {
    let X = [
        explosion(0, 0, 32.0, 35.0, "rss:BBC", "city"),
        explosion(1, 30, 32.1, 35.1, "rss:CNN", "city"),
        explosion(2, 200, 32.0, 35.0, "rss:WSJ", "city"),
        explosion(3, 10, 30.0, 50.0, "rss:NYT", "city"),
        explosion(4, 5, 32.5, 53.0, "rss:BBC", "country"),
        explosion(5, 6, 32.5, 53.0, "rss:CNN", "country"),
    ];
    let want: BTreeSet<BTreeSet<String>> = [
        vec!["o0", "o1"],
        vec!["o2"],
        vec!["o3"],
        vec!["o4"],
        vec!["o5"],
    ]
    .into_iter()
    .map(|v| v.into_iter().map(String::from).collect())
    .collect();
    let mut g = Rng(7);
    for _ in 0..100 {
        let mut p: Vec<usize> = (0..6).collect();
        for i in (1..6).rev() {
            p.swap(i, g.below(i + 1));
        }
        let Y: Vec<&Obs> = p.iter().map(|&i| &X[i]).collect();
        assert_eq!(sets(&events(&Y)), want, "{p:?}");
    }
}

#[test]
fn reports_and_unlocated_observations_form_no_event() {
    let t = at(2026, 10, 21, 0, 0, 0);
    let a = ob(0, t, Some(32.0), Some(35.0), "rss:BBC", "report", "city");
    let b = ob(
        1,
        t,
        None,
        None,
        "rss:BBC",
        "explosions_remote_violence",
        "none",
    );
    assert!(events(&[&a, &b]).is_empty());
}

const GRADES: &str = "ABCDEF";
const OWNERS: &str = "uvwxyz";

/// A list of at most 8 (grade, owner) pairs, and one more owner.
fn support(g: &mut Rng) -> (Vec<(&'static str, String)>, String) {
    let pick = |s: &'static str, i: usize| &s[i..i + 1];
    let S = (0..g.below(9))
        .map(|_| {
            (
                pick(GRADES, g.below(6)),
                pick(OWNERS, g.below(6)).to_string(),
            )
        })
        .collect();
    (S, pick(OWNERS, g.below(6)).to_string())
}

/// Adding an A/B source never makes an event less credible (a larger number).
#[test]
fn credibility_is_monotone_in_corroboration() {
    let mut g = Rng(3);
    for _ in 0..500 {
        let (S, o) = support(&mut g);
        let mut T = S.clone();
        T.push(("A", o));
        assert!(
            fuse::credibility(&T, false) <= fuse::credibility(&S, false),
            "{S:?}"
        );
    }
}

#[test]
fn contradiction_forces_improbable() {
    let mut g = Rng(4);
    for _ in 0..200 {
        assert_eq!(fuse::credibility(&support(&mut g).0, true), 5);
    }
}

#[test]
fn credibility_table() {
    let s = |v: &[(&'static str, &str)]| -> i8 {
        fuse::credibility(
            &v.iter().map(|x| (x.0, x.1.to_string())).collect::<Vec<_>>(),
            false,
        )
    };
    let got = [
        s(&[("A", "x"), ("B", "y"), ("B", "z")]), // n = 3
        s(&[("A", "x"), ("B", "y")]),             // n = 2
        s(&[("B", "x"), ("B", "x")]),             // one owner: n = 1
        s(&[("C", "x"), ("E", "y")]),
        s(&[("F", "x")]),
        s(&[]),
    ];
    assert_eq!(got, [1, 2, 3, 4, 6, 6]);
}

#[test]
fn google_news_independence_is_by_outlet() {
    let r = ii();
    let a = explosion(0, 0, 32.0, 35.0, "gnews:strike#1", "city");
    let b = explosion(1, 0, 32.0, 35.0, "gnews:strike#2", "city");
    let (reuters, ap) = ("Reuters".to_string(), "AP".to_string());
    assert_ne!(
        fuse::owner(r, &a, Some(&reuters)),
        fuse::owner(r, &b, Some(&ap))
    );
    let c = explosion(2, 0, 0.0, 0.0, "rss:BBC", "city");
    assert_eq!(fuse::owner(r, &c, None), "BBC");
}

/// Ten alerts in the window trigger II-1a; a PIR whose evidence no healthy source can
/// report is a gap, a healthy one with nothing happening is open; failed collection
/// cannot answer a PIR.
#[test]
fn indicator_board() {
    let _g = fresh("fuse_board");
    let r = ii();
    let now = osw::collect::now();
    let E: Vec<Event> = (0..10)
        .map(|k| Event {
            id: format!("e{k}"),
            region: r.id.clone(),
            t_start: now - Duration::hours(1),
            t_end: now - Duration::hours(1),
            lat: 32.0,
            lon: 35.0,
            kind: "air_raid_alert".into(),
            observation_ids: vec![format!("o{k}")],
            n_independent: 1,
            credibility: 3,
            admin1: None,
            admin2: None,
        })
        .collect();
    store::replace(&E);
    let O: Vec<Obs> = (0..2)
        .map(|k| {
            ob(
                k,
                now,
                Some(32.0),
                Some(35.0),
                "alerts:tzevaadom",
                "air_raid_alert",
                "city",
            )
        })
        .collect();
    store::append(&O);
    let board = |h: &[(&str, &'static str)]| -> HashMap<String, serde_json::Value> {
        let h = h.iter().map(|x| (x.0.to_string(), x.1)).collect();
        analyze::board(r, &h, now)
            .into_iter()
            .map(|p| (p["pir"].as_str().unwrap().to_string(), p))
            .collect()
    };
    let B = board(&[("alerts:tzevaadom", "ok")]);
    let i = &B["II-1"]["indicators"][0];
    assert_eq!(
        (i["count"].as_i64(), i["status"].as_str()),
        (Some(10), Some("triggered"))
    );
    assert_eq!(B["II-1"]["status"], "answered");
    assert_eq!(B["II-2"]["status"], "gap"); // no healthy text source can report them
    assert_eq!(board(&[("rss:BBC", "ok")])["II-2"]["status"], "open"); // healthy, nothing happened
    assert_eq!(
        board(&[("alerts:tzevaadom", "failed")])["II-1"]["status"],
        "gap"
    );
    assert_eq!(analyze::status(0, 1, 3), "inactive");
    assert_eq!(analyze::status(1, 1, 3), "watch");
}
