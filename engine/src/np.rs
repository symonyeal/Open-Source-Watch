//! Reductions in NumPy's floating-point order, so that means and deviations equal
//! those the engine has always stored (numpy/_core/src/umath/loops_utils.h.src).

/// Σ x by NumPy's pairwise summation: sequential below 8 terms, eight running
/// sums up to 128, halves (cut at a multiple of 8) above.
pub fn sum(x: &[f64]) -> f64 {
    let n = x.len();
    if n < 8 {
        return x.iter().fold(0.0, |s, v| s + v);
    }
    if n <= 128 {
        let mut r: [f64; 8] = x[..8].try_into().unwrap();
        let m = n - n % 8;
        for i in (8..m).step_by(8) {
            for j in 0..8 {
                r[j] += x[i + j];
            }
        }
        let mut s = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for v in &x[m..] {
            s += v;
        }
        return s;
    }
    let h = n / 2;
    let h = h - h % 8;
    sum(&x[..h]) + sum(&x[h..])
}

pub fn mean(x: &[f64]) -> f64 {
    sum(x) / x.len() as f64
}

/// Sample standard deviation (ddof = 1), as `np.std(x, ddof=1)`.
pub fn sd(x: &[f64]) -> f64 {
    let m = mean(x);
    let d: Vec<f64> = x.iter().map(|v| (v - m) * (v - m)).collect();
    (sum(&d) / (x.len() as f64 - 1.0)).sqrt()
}
