//! Compiled kernels of the engine, installed with `py -3.14 -m pip install ./kernels`:
//! the fusion linkage of `engine.fuse` (`tests/test_fuse.py` checks it against a
//! brute-force check of every pair) and the H3 cells of `engine.gnss` (h3o, a Rust
//! port of Uber's H3; `tests/test_gnss.py` checks it against gpsjam.org's files).

use h3o::{CellIndex, LatLng, Resolution};
use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

const R_EARTH: f64 = 6371.0088; // km, IUGG mean radius

/// Great-circle distance in km, the formula of `engine.fuse.haversine`.
fn haversine(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
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

/// Component label of each observation under the link rule of `engine.fuse`:
/// i, j are linked when |t_i − t_j| ≤ tau and, if both are fine, d(i, j) ≤ delta,
/// else lat/lon equal and neither is country-level. The label is the smallest
/// index of the component. t must be sorted ascending (so no NaN).
///
/// Cost O(n w) distances, w the number of observations within tau of one; the
/// window end advances monotonically, so finding it costs O(n) in all.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn components<'py>(
    py: Python<'py>,
    t: PyReadonlyArray1<'py, f64>,
    lat: PyReadonlyArray1<'py, f64>,
    lon: PyReadonlyArray1<'py, f64>,
    fine: PyReadonlyArray1<'py, bool>,
    country: PyReadonlyArray1<'py, bool>,
    tau: f64,
    delta: f64,
) -> PyResult<Bound<'py, PyArray1<i64>>> {
    let (t, lat, lon) = (t.as_slice()?, lat.as_slice()?, lon.as_slice()?);
    let (fine, country) = (fine.as_slice()?, country.as_slice()?);
    let n = t.len();
    if [lat.len(), lon.len(), fine.len(), country.len()]
        .iter()
        .any(|&m| m != n)
    {
        return Err(PyValueError::new_err("arrays differ in length"));
    }
    if !t.is_sorted() {
        return Err(PyValueError::new_err("t must be sorted ascending"));
    }
    let lab = py.detach(|| {
        let mut parent: Vec<usize> = (0..n).collect();
        let mut hi = 0;
        for i in 0..n {
            while hi < n && t[hi] <= t[i] + tau {
                hi += 1;
            }
            for j in i + 1..hi {
                let linked = if fine[i] && fine[j] {
                    haversine(lat[i], lon[i], lat[j], lon[j]) <= delta
                } else {
                    lat[i] == lat[j] && lon[i] == lon[j] && !country[i] && !country[j]
                };
                if linked {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    if a != b {
                        parent[a.max(b)] = a.min(b);
                    }
                }
            }
        }
        (0..n)
            .map(|i| find(&mut parent, i) as i64)
            .collect::<Vec<_>>()
    });
    Ok(lab.into_pyarray(py))
}

/// A NumPy array returned to Python.
type Array<'py, T> = Bound<'py, PyArray1<T>>;

/// Columns (day, cell, good, bad) of `aircraft_counts`.
type Counts<'py> = (
    Array<'py, i64>,
    Array<'py, u64>,
    Array<'py, i64>,
    Array<'py, i64>,
);

fn resolution(res: u8) -> PyResult<Resolution> {
    Resolution::try_from(res).map_err(|e| PyValueError::new_err(e.to_string()))
}

fn cell(h: u64) -> PyResult<CellIndex> {
    CellIndex::try_from(h).map_err(|e| PyValueError::new_err(format!("{h:x}: {e}")))
}

/// H3 cell of each point at resolution `res`; 0 where lat/lon is not a coordinate
/// (non-finite, |lat| > 90 or |lon| > 180; h3o itself only rejects non-finite values).
#[pyfunction]
fn cells<'py>(
    py: Python<'py>,
    lat: PyReadonlyArray1<'py, f64>,
    lon: PyReadonlyArray1<'py, f64>,
    res: u8,
) -> PyResult<Bound<'py, PyArray1<u64>>> {
    let (lat, lon, r) = (lat.as_slice()?, lon.as_slice()?, resolution(res)?);
    if lat.len() != lon.len() {
        return Err(PyValueError::new_err("arrays differ in length"));
    }
    let h: Vec<u64> = lat
        .iter()
        .zip(lon)
        .map(|(&a, &b)| match LatLng::new(a, b) {
            Ok(p) if a.abs() <= 90.0 && b.abs() <= 180.0 => u64::from(p.to_cell(r)),
            _ => 0,
        })
        .collect();
    Ok(h.into_pyarray(py))
}

/// Centre (lat, lon) in degrees of each cell; an invalid index is an error.
#[pyfunction]
fn centres<'py>(
    py: Python<'py>,
    h: PyReadonlyArray1<'py, u64>,
) -> PyResult<(Array<'py, f64>, Array<'py, f64>)> {
    let pts = h
        .as_slice()?
        .iter()
        .map(|&x| cell(x).map(LatLng::from))
        .collect::<PyResult<Vec<_>>>()?;
    let lat: Vec<f64> = pts.iter().map(|p| p.lat()).collect();
    let lon: Vec<f64> = pts.iter().map(|p| p.lng()).collect();
    Ok((lat.into_pyarray(py), lon.into_pyarray(py)))
}

/// Boundary of each cell, its vertices [(lat, lon), ...] in degrees, counter-clockwise.
#[pyfunction]
fn boundaries(h: PyReadonlyArray1<'_, u64>) -> PyResult<Vec<Vec<(f64, f64)>>> {
    h.as_slice()?
        .iter()
        .map(|&x| {
            Ok(cell(x)?
                .boundary()
                .iter()
                .map(|p| (p.lat(), p.lng()))
                .collect())
        })
        .collect()
}

/// Aircraft per (day, cell), gpsjam.org's daily count (docs/SOURCES.md): an aircraft
/// counts once in each cell-day it was seen in, as bad if any of its reports there
/// was bad, else as good. Rows (day, cell, good, bad) sorted by day, then cell;
/// reports with h = 0 (no coordinate) are dropped.
///
/// Cost O(n log n): one sort of the n reports by (day, cell, aircraft).
#[pyfunction]
fn aircraft_counts<'py>(
    py: Python<'py>,
    day: PyReadonlyArray1<'py, i64>,
    h: PyReadonlyArray1<'py, u64>,
    icao: PyReadonlyArray1<'py, u32>,
    bad: PyReadonlyArray1<'py, bool>,
) -> PyResult<Counts<'py>> {
    let (day, h, icao, bad) = (
        day.as_slice()?,
        h.as_slice()?,
        icao.as_slice()?,
        bad.as_slice()?,
    );
    let n = day.len();
    if [h.len(), icao.len(), bad.len()].iter().any(|&m| m != n) {
        return Err(PyValueError::new_err("arrays differ in length"));
    }
    let (dd, hh, gg, bb) = py.detach(|| {
        let mut o: Vec<usize> = (0..n).filter(|&i| h[i] != 0).collect();
        o.sort_unstable_by_key(|&i| (day[i], h[i], icao[i]));
        let (mut dd, mut hh, mut gg, mut bb) = (vec![], vec![], vec![], vec![]);
        let mut k = 0;
        while k < o.len() {
            let (d, c) = (day[o[k]], h[o[k]]);
            let (mut g, mut b) = (0, 0);
            while k < o.len() && (day[o[k]], h[o[k]]) == (d, c) {
                let a = icao[o[k]];
                let mut worse = false;
                while k < o.len() && (day[o[k]], h[o[k]], icao[o[k]]) == (d, c, a) {
                    worse |= bad[o[k]];
                    k += 1;
                }
                if worse { b += 1 } else { g += 1 }
            }
            dd.push(d);
            hh.push(c);
            gg.push(g);
            bb.push(b);
        }
        (dd, hh, gg, bb)
    });
    Ok((
        dd.into_pyarray(py),
        hh.into_pyarray(py),
        gg.into_pyarray(py),
        bb.into_pyarray(py),
    ))
}

#[pymodule]
fn osw_kernels(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(components, m)?)?;
    m.add_function(wrap_pyfunction!(cells, m)?)?;
    m.add_function(wrap_pyfunction!(centres, m)?)?;
    m.add_function(wrap_pyfunction!(boundaries, m)?)?;
    m.add_function(wrap_pyfunction!(aircraft_counts, m)?)
}
