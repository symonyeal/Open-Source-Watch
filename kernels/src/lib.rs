//! Compiled kernels of the engine. `engine.fuse` uses each one when this module is
//! installed (`py -3.14 -m pip install ./kernels`) and its NumPy version otherwise;
//! `tests/test_fuse.py` checks that the two give the same labels.

use numpy::{IntoPyArray, PyArray1, PyReadonlyArray1};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

const R_EARTH: f64 = 6371.0088; // km, IUGG mean radius, as engine.fuse

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

#[pymodule]
fn osw_kernels(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(components, m)?)
}
