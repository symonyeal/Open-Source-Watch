//! Exploitation and fusion, stage 4 (spec § Fusion, § Credibility).
//!
//! Observations o, o' of a region are linked when
//!     kind(o) = kind(o') ≠ report,   |t − t'| ≤ τ,   and
//!     d(o, o') ≤ δ  if both are located to city precision or finer (site, point),
//!     lat/lon equal (the same administrative unit) if neither is country-level.
//! In a region with a gazetteer (oblasts and raions, `[gazetteer]` in region.toml), two
//! text reports (news, Telegram) are also linked as one incident (`incidents`) when
//! both name the same oblast (admin1), their English texts overlap,
//!     |W ∩ W'| / min(|W|, |W'|) ≥ θ   (Szymkiewicz–Simpson overlap coefficient),
//! W the lower-cased words of three letters or more less English function words, and
//! the merged incident spans at most τ_I: reports of one night's strike run for a day,
//! past τ (Validation 2026-11-03). The span bound stops a daily story chaining days.
//! Events are the connected components of the space-time links (single linkage,
//! geo::components), which do not depend on input order. The incident links are merged
//! greedily in (t, id) order, each merge allowed only while the span stays within τ_I,
//! so the partition depends on that order; `events` sorts by it, so the result is
//! deterministic for any input order. A country-level observation links
//! to nothing (a single-observation event); an unlocated one forms no event.
//!
//! Credibility of an event (Admiralty 1-6), first match wins, with n the number of
//! distinct owners among its A/B-graded sources:
//!     contradicted → 5;  n ≥ 3 → 1;  n = 2 → 2;  n = 1 → 3;
//!     any C/D/E source → 4;  otherwise → 6.

use std::collections::{BTreeSet, HashMap};

use crate::collect::{Report, adapter, rss, sha, stored};
use crate::fmt::Time;
use crate::geo::{Point, components, km};
use crate::process::feeds::entities;
use crate::regions::{self, Region};
use crate::schema::{Entity, Event, Obs, Run};
use crate::store;

/// s, km (spec § Fusion).
pub const TAU: f64 = 7200.0;
pub const DELTA: f64 = 25.0;
/// s, and overlap of words (`incidents`), chosen on half the judged flags.
pub const TAU_I: f64 = 86400.0;
pub const THETA: f64 = 0.6;
const STOP: [&str; 21] = [
    "the", "and", "for", "with", "after", "from", "that", "this", "have", "has", "was", "were",
    "are", "into", "over", "near", "its", "their", "his", "her", "not",
];

/// W of a text (module doc).
fn wordset(s: &str) -> BTreeSet<String> {
    crate::process::words(s)
        .into_iter()
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(&w.as_str()))
        .collect()
}

/// Labels lab of the observations Y (sorted by t) with the incident links added: the
/// smallest index of each merged component.
fn incidents(Y: &[&Obs], mut lab: Vec<usize>) -> Vec<usize> {
    let text = |x: &Obs| ["rss", "gnews", "telegram"].contains(&adapter(&x.source_id));
    let W: Vec<BTreeSet<String>> = Y
        .iter()
        .map(|x| {
            if text(x) {
                wordset(&x.text_en)
            } else {
                BTreeSet::new()
            }
        })
        .collect();
    // Span [t0, t1] of each component, held at its root.
    let mut T: Vec<(f64, f64)> = Y.iter().map(|x| (secs(&x.t), secs(&x.t))).collect();
    for i in 0..Y.len() {
        let r = lab[i];
        T[r] = (T[r].0.min(T[i].0), T[r].1.max(T[i].1));
    }
    let root = |lab: &mut Vec<usize>, mut i: usize| {
        while lab[i] != i {
            i = lab[i];
        }
        i
    };
    let mut hi = 0;
    for i in 0..Y.len() {
        while hi < Y.len() && secs(&Y[hi].t) <= secs(&Y[i].t) + TAU_I {
            hi += 1;
        }
        let Some(a) = &Y[i].admin1 else { continue };
        for j in i + 1..hi {
            let m = W[i].len().min(W[j].len());
            if Y[j].admin1.as_ref() != Some(a)
                || m == 0
                || (W[i].intersection(&W[j]).count() as f64) < THETA * m as f64
            {
                continue;
            }
            let (p, q) = (root(&mut lab, i), root(&mut lab, j));
            let s = (T[p].0.min(T[q].0), T[p].1.max(T[q].1));
            if p != q && s.1 - s.0 <= TAU_I {
                let (u, v) = (p.min(q), p.max(q));
                lab[v] = u;
                T[u] = s;
            }
        }
    }
    (0..Y.len()).map(|i| root(&mut lab, i)).collect()
}

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
        let lab = if r.gazetteer.countries.is_empty() {
            lab
        } else {
            incidents(&Y, lab)
        };
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
            let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
            let (a, o) = (mean(&lat), mean(&lon));
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
    store::transaction(|| {
        let E = all_events();
        store::replace(&E);
        let raws: Vec<Report> = stored(&["adsb", "drones", "ais"], &[], None);
        store::replace::<Entity>(&entities(&raws));
        store::inputs();
        E.len()
    })
}
