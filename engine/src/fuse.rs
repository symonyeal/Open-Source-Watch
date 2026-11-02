//! Exploitation and fusion, stage 4 (spec § Fusion, § Credibility).
//!
//! Observations o, o' of a region are linked when
//!     kind(o) = kind(o') ≠ report,   |t − t'| ≤ τ,   and
//!     d(o, o') ≤ δ  if both are located to city precision or finer (site, point),
//!     lat/lon equal (the same administrative unit) if neither is country-level.
//! Events are the connected components of this graph (single linkage, geo::components),
//! so the partition does not depend on input order. A country-level observation links
//! to nothing (a single-observation event); an unlocated one forms no event.
//!
//! Credibility of an event (Admiralty 1-6), first match wins, with n the number of
//! distinct owners among its A/B-graded sources:
//!     contradicted → 5;  n ≥ 3 → 1;  n = 2 → 2;  n = 1 → 3;
//!     any C/D/E source → 4;  otherwise → 6.

use std::collections::{BTreeSet, HashMap};

use crate::collect::{Report, rss, sha, stored};
use crate::fmt::Time;
use crate::geo::{Point, components, km};
use crate::np;
use crate::process::feeds::entities;
use crate::regions::{self, Region};
use crate::schema::{Entity, Event, Obs, Run};
use crate::store;

/// s, km (spec § Fusion).
pub const TAU: f64 = 7200.0;
pub const DELTA: f64 = 25.0;

fn ab(g: &str) -> bool {
    g == "A" || g == "B"
}

/// support: (grade, owner) of the event's observations.
pub fn credibility(support: &[(&str, String)], contradicted: bool) -> i8 {
    if contradicted {
        return 5;
    }
    let n = support
        .iter()
        .filter(|s| ab(s.0))
        .map(|s| &s.1)
        .collect::<BTreeSet<_>>()
        .len();
    if n > 0 {
        return (4 - n as i8).max(1);
    }
    if support.iter().any(|s| matches!(s.0, "C" | "D" | "E")) {
        4
    } else {
        6
    }
}

/// Owner of an observation's source; a Google News item belongs to its outlet.
pub fn owner(r: &Region, x: &Obs, outlet: Option<&String>) -> String {
    match outlet {
        Some(o)
            if x.source_id.starts_with("gnews:") || x.source_id.starts_with("rss:Google News") =>
        {
            o.clone()
        }
        _ => r.owner(&x.source_id),
    }
}

fn secs(t: &Time) -> f64 {
    t.timestamp_micros() as f64 / 1e6
}

/// Contradiction of an air-raid-alert event M claimed by no warning source (spec §
/// Fusion, amendment): some member states an event time τ (process::when) such that
/// the region's A/B warning source ran at least twice, without failure, within 10
/// min of τ, and recorded no alert within 30 min of τ. A report without a stated
/// time is never contradicted.
pub fn contradicted(r: &Region, M: &[&Obs], alerts: &[Time], runs: &[&Run]) -> bool {
    if M.iter().any(|x| x.source_id.starts_with("alerts:")) {
        return false;
    }
    M.iter().filter_map(|x| x.t_event).any(|tau| {
        let R: Vec<&&Run> = runs
            .iter()
            .filter(|y| (secs(&y.started_at) - secs(&tau)).abs() <= 600.0)
            .collect();
        R.len() >= 2
            && R.iter().all(|y| y.status == "ok" || y.status == "empty")
            && ab(r.reliability(&R[0].source_id))
            && !alerts
                .iter()
                .any(|a| (secs(a) - secs(&tau)).abs() <= 1800.0)
    })
}

/// The first of members M (in time order) to have field f.
fn first(M: &[&Obs], f: impl Fn(&Obs) -> &Option<String>) -> Option<String> {
    M.iter().find_map(|x| f(x).clone())
}

/// Event rows of region r from its observations X (with their outlets) and its
/// warning source's collection runs.
pub fn events(
    r: &Region,
    X: &[&Obs],
    outlets: &HashMap<String, String>,
    runs: &[&Run],
) -> Vec<Event> {
    let alerts: Vec<Time> = X
        .iter()
        .filter(|x| x.source_id.starts_with("alerts:"))
        .map(|x| x.t)
        .collect();
    let kinds: BTreeSet<&str> = X
        .iter()
        .map(|x| x.kind.as_str())
        .filter(|k| *k != "report")
        .collect();
    let mut out = vec![];
    for kind in kinds {
        let mut Y: Vec<&Obs> = X
            .iter()
            .copied()
            .filter(|x| x.kind == kind && x.lat.is_some())
            .collect();
        Y.sort_by(|a, b| (a.t, &a.id).cmp(&(b.t, &b.id)));
        let P: Vec<Point> = Y
            .iter()
            .map(|x| Point {
                t: secs(&x.t),
                lat: x.lat.unwrap(),
                lon: x.lon.unwrap_or(f64::NAN),
                fine: ["point", "site", "city"].contains(&x.geo_precision.as_str()),
                country: x.geo_precision == "country",
            })
            .collect();
        let lab = components(&P, TAU, DELTA);
        let mut o: Vec<usize> = (0..Y.len()).collect();
        o.sort_by_key(|&i| lab[i]); // stable: components in label order, members in index order
        for I in o.chunk_by(|a, b| lab[*a] == lab[*b]) {
            let M: Vec<&Obs> = I.iter().map(|&i| Y[i]).collect();
            let fine: Vec<usize> = I.iter().copied().filter(|&i| P[i].fine).collect();
            let F = if fine.is_empty() { vec![I[0]] } else { fine };
            let S: Vec<(&str, String)> = M
                .iter()
                .map(|x| {
                    (
                        r.reliability(&x.source_id),
                        owner(r, x, outlets.get(&x.raw_id)),
                    )
                })
                .collect();
            let lat: Vec<f64> = F.iter().map(|&i| P[i].lat).collect();
            let lon: Vec<f64> = F.iter().map(|&i| P[i].lon).collect();
            let (a, o) = (np::mean(&lat), np::mean(&lon));
            let m = *F
                .iter()
                .min_by(|&&i, &&j| {
                    km(a, o, P[i].lat, P[i].lon).total_cmp(&km(a, o, P[j].lat, P[j].lon))
                })
                .unwrap();
            out.push(Event {
                admin1: Y[m].admin1.clone(),
                admin2: Y[m].admin2.clone(),
                site: Y[m].site.clone(),
                sub_event: first(&M, |x| &x.sub_event),
                weapon: M.iter().flat_map(|x| &x.weapon).fold(vec![], |mut w, k| {
                    if !w.contains(k) {
                        w.push(k.clone());
                    }
                    w
                }),
                missile: first(&M, |x| &x.missile),
                target: first(&M, |x| &x.target),
                killed: M.iter().filter_map(|x| x.killed).max(),
                injured: M.iter().filter_map(|x| x.injured).max(),
                actor1: first(&M, |x| &x.actor1),
                actor2: first(&M, |x| &x.actor2),
                id: sha(&format!("evt\0{}", M[0].id), 32),
                region: r.id.clone(),
                t_start: M[0].t,
                t_end: M[M.len() - 1].t,
                lat: a,
                lon: o,
                kind: kind.into(),
                observation_ids: M.iter().map(|x| x.id.clone()).collect(),
                n_independent: S
                    .iter()
                    .filter(|s| ab(s.0))
                    .map(|s| &s.1)
                    .collect::<BTreeSet<_>>()
                    .len() as i16,
                credibility: credibility(
                    &S,
                    kind == "air_raid_alert" && contradicted(r, &M, &alerts, runs),
                ),
            });
        }
    }
    out
}

/// Every region's events from the stored observations and alert runs.
pub fn all_events() -> Vec<Event> {
    let O = rss::outlets();
    let X = store::view::<Obs>();
    let runs = store::rows::<Run>();
    let mut E = vec![];
    for r in regions::all() {
        let Xr: Vec<&Obs> = X
            .iter()
            .filter(|x| x.kind != "report" && x.region == r.id)
            .collect();
        let Rr: Vec<&Run> = runs
            .iter()
            .filter(|x| x.source_id.starts_with("alerts:") && x.region == r.id)
            .collect();
        E.extend(events(r, &Xr, &O, &Rr));
    }
    E
}

/// Recompute the event and entity tables from the store (a pure function of it).
pub fn fuse() -> usize {
    let E = all_events();
    store::replace(&E);
    let raws: Vec<Report> = stored(&["adsb", "drones", "ais"], &[], None);
    store::replace::<Entity>(&entities(&raws));
    E.len()
}
