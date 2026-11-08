//! Geometry: great-circle distance, the fusion linkage, and H3 cells (h3o, a Rust
//! port of Uber's H3; tests check it against gpsjam.org's files).

use h3o::{CellIndex, LatLng, Resolution};

pub const R_EARTH: f64 = 6371.0088; // km, IUGG mean radius

/// Great-circle distance in km (haversine).
pub fn km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let a = ((p2 - p1) / 2.0).sin().powi(2)
        + p1.cos() * p2.cos() * ((lon2 - lon1).to_radians() / 2.0).sin().powi(2);
    2.0 * R_EARTH * a.sqrt().asin()
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]]; // path halving
        i = parent[i];
    }
    i
}

/// One point of the linkage: time (s), position, and whether it is located to
/// city precision or finer (`fine`) or only to a country (`country`).
pub struct Point {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    pub fine: bool,
    pub country: bool,
}

/// Component label of each point, the smallest index of its component: i, j are
/// linked when |t_i − t_j| ≤ tau and, if both are fine, d(i, j) ≤ delta, else
/// lat/lon equal and neither is country-level. P sorted by t ascending.
///
/// Cost O(n w) distances, w the points within tau of one; the window end advances
/// monotonically, so finding it costs O(n) in all.
pub fn components(P: &[Point], tau: f64, delta: f64) -> Vec<usize> {
    let n = P.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut hi = 0;
    for i in 0..n {
        while hi < n && P[hi].t <= P[i].t + tau {
            hi += 1;
        }
        for j in i + 1..hi {
            let (p, q) = (&P[i], &P[j]);
            let linked = if p.fine && q.fine {
                km(p.lat, p.lon, q.lat, q.lon) <= delta
            } else {
                p.lat == q.lat && p.lon == q.lon && !p.country && !q.country
            };
            if linked {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
    }
    (0..n).map(|i| find(&mut parent, i)).collect()
}

/// H3 cell of a point at resolution res; None where lat/lon is not a coordinate
/// (non-finite, |lat| > 90 or |lon| > 180; h3o itself only rejects non-finite values).
pub fn cell(lat: f64, lon: f64, res: u8) -> Option<u64> {
    let r = Resolution::try_from(res).unwrap();
    match LatLng::new(lat, lon) {
        Ok(p) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => Some(u64::from(p.to_cell(r))),
        _ => None,
    }
}

/// Centre (lat, lon) in degrees of the cell with hex index h; None if not a cell.
pub fn centre(h: &str) -> Option<(f64, f64)> {
    let c = index(h)?;
    let p = LatLng::from(c);
    Some((p.lat(), p.lng()))
}

/// Boundary of a cell, its vertices (lat, lon) in degrees, counter-clockwise.
pub fn boundary(h: &str) -> Vec<(f64, f64)> {
    index(h).map_or(vec![], |c| {
        c.boundary().iter().map(|p| (p.lat(), p.lng())).collect()
    })
}

fn index(h: &str) -> Option<CellIndex> {
    CellIndex::try_from(u64::from_str_radix(h, 16).ok()?).ok()
}

/// Aircraft per (day, cell), gpsjam.org's daily count (docs/SOURCES.md): an aircraft
/// counts once in each cell-day it was seen in, as bad if any of its reports there
/// was bad, else as good. Reports are (day, cell, aircraft, bad); rows (day, cell,
/// good, bad) sorted by day, then cell.
///
/// Cost O(n log n): one sort of the n reports by (day, cell, aircraft).
pub fn aircraft_counts<D: Ord + Copy>(mut X: Vec<(D, u64, u32, bool)>) -> Vec<(D, u64, i64, i64)> {
    X.sort_by_key(|x| (x.0, x.1, x.2));
    let mut out: Vec<(D, u64, i64, i64)> = vec![];
    let mut k = 0;
    while k < X.len() {
        let (d, c, a) = (X[k].0, X[k].1, X[k].2);
        let mut worse = false;
        while k < X.len() && (X[k].0, X[k].1, X[k].2) == (d, c, a) {
            worse |= X[k].3;
            k += 1;
        }
        match out.last_mut() {
            Some(r) if (r.0, r.1) == (d, c) => {
                if worse {
                    r.3 += 1
                } else {
                    r.2 += 1
                }
            }
            _ => out.push((d, c, !worse as i64, worse as i64)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(n: usize, seed: u64) -> Vec<Point> {
        let mut s = seed;
        let mut u = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut P: Vec<Point> = (0..n)
            .map(|_| {
                let g = (u() * 3.0) as u8;
                Point {
                    t: (u() * 40000.0).floor(),
                    lat: 31.0 + (u() * 6.0).floor() * 0.1,
                    lon: 34.0 + (u() * 6.0).floor() * 0.1,
                    fine: g == 0,
                    country: g == 2,
                }
            })
            .collect();
        P.sort_by(|a, b| a.t.total_cmp(&b.t));
        P
    }

    /// Linkage equals the components of the graph of every linked pair.
    #[test]
    fn components_equal_brute_force() {
        for seed in 0..20 {
            let P = pts(60, seed);
            let lab = components(&P, 7200.0, 25.0);
            let mut parent: Vec<usize> = (0..P.len()).collect();
            for i in 0..P.len() {
                for j in 0..P.len() {
                    let (p, q) = (&P[i], &P[j]);
                    let link = (p.t - q.t).abs() <= 7200.0
                        && if p.fine && q.fine {
                            km(p.lat, p.lon, q.lat, q.lon) <= 25.0
                        } else {
                            p.lat == q.lat && p.lon == q.lon && !p.country && !q.country
                        };
                    if link && i != j {
                        let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                        parent[a.max(b)] = a.min(b);
                    }
                }
            }
            let want: Vec<usize> = (0..P.len()).map(|i| find(&mut parent, i)).collect();
            assert_eq!(lab, want);
        }
    }

    #[test]
    fn aircraft_count_once_per_cell_day_bad_if_any_report_bad() {
        let X = vec![
            (1, 7, 10, false),
            (1, 7, 10, true),
            (1, 7, 11, false),
            (1, 8, 10, false),
            (2, 7, 10, false),
        ];
        assert_eq!(
            aircraft_counts(X),
            vec![(1, 7, 1, 1), (1, 8, 1, 0), (2, 7, 1, 0)]
        );
    }
}
