//! Kind classifier for text observations (spec P3): TF-IDF (1-2-grams, sublinear tf)
//! and multinomial logistic regression (C = 4, balanced class weights), trained at
//! first use on docs/labels/kinds-sample.jsonl, a stratified sample of 200 live
//! observations labelled against the ACLED definitions (docs/DECISIONS.md). The
//! model is scikit-learn's TfidfVectorizer + LogisticRegression, solved here to
//! optimality:
//!
//!     tf(t, d) = 1 + ln n(t, d),   idf(t) = ln((1 + N) / (1 + df(t))) + 1,
//!     x_d = tf · idf / ‖tf · idf‖₂;
//!     min_{W, b}  Σ_i s_i (lse(W x_i + b) − (W x_i + b)_{y_i}) + ‖W‖² / (2C),
//!     s_i = N / (K n_{y_i})   (balanced weights), K classes,
//!
//! by L-BFGS (Nocedal and Wright 2006, alg. 7.4/7.5, memory 10, backtracking
//! line search, Armijo or approximate Wolfe); the kind is argmax_k (W x + b)_k.
//! On the 200 labels it reaches ‖∇‖∞ ≤ 1e-10 in about 100 iterations.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::process::wordchar;
use crate::store::repo;

const C: f64 = 4.0;

/// Sparse row: (feature, value).
type Row = Vec<(usize, f64)>;

struct Model {
    vocab: HashMap<String, usize>,
    idf: Vec<f64>,
    classes: Vec<String>,
    /// K × F weights, row-major, then K intercepts.
    w: Vec<f64>,
}

/// Unigrams and bigrams of `\b\w\w+\b` tokens of the lower-cased text.
fn grams(s: &str) -> Vec<String> {
    let s = s.to_lowercase();
    let mut T: Vec<String> = vec![];
    let mut cur = String::new();
    for c in s.chars().chain(std::iter::once(' ')) {
        if wordchar(c) {
            cur.push(c);
        } else {
            if cur.chars().count() >= 2 {
                T.push(cur.clone());
            }
            cur.clear();
        }
    }
    let bi: Vec<String> = T.windows(2).map(|w| format!("{} {}", w[0], w[1])).collect();
    T.extend(bi);
    T
}

fn row(m_vocab: &HashMap<String, usize>, idf: &[f64], s: &str) -> Row {
    let mut n: HashMap<usize, f64> = HashMap::new();
    for g in grams(s) {
        if let Some(&j) = m_vocab.get(&g) {
            *n.entry(j).or_default() += 1.0;
        }
    }
    let mut x: Row = n
        .into_iter()
        .map(|(j, c)| (j, (1.0 + c.ln()) * idf[j]))
        .collect();
    x.sort_by_key(|e| e.0);
    let norm = x.iter().map(|e| e.1 * e.1).sum::<f64>().sqrt();
    if norm > 0.0 {
        for e in &mut x {
            e.1 /= norm;
        }
    }
    x
}

/// Objective and gradient at w of the weighted, penalised multinomial loss.
fn loss(w: &[f64], X: &[Row], y: &[usize], s: &[f64], K: usize, F: usize, g: &mut [f64]) -> f64 {
    g.fill(0.0);
    let b = K * F;
    let mut f = 0.0;
    let mut z = vec![0.0; K];
    for ((x, &yi), &si) in X.iter().zip(y).zip(s) {
        for k in 0..K {
            z[k] = w[b + k] + x.iter().map(|&(j, v)| w[k * F + j] * v).sum::<f64>();
        }
        let m = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let lse = m + z.iter().map(|v| (v - m).exp()).sum::<f64>().ln();
        f += si * (lse - z[yi]);
        for k in 0..K {
            let d = si * ((z[k] - lse).exp() - (k == yi) as u8 as f64);
            g[b + k] += d;
            for &(j, v) in x {
                g[k * F + j] += d * v;
            }
        }
    }
    for k in 0..b {
        f += w[k] * w[k] / (2.0 * C);
        g[k] += w[k] / C;
    }
    f
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// argmin of loss by L-BFGS, to ‖∇‖∞ ≤ 1e-10 or a line search that cannot descend.
fn lbfgs(X: &[Row], y: &[usize], s: &[f64], K: usize, F: usize) -> Vec<f64> {
    let n = K * F + K;
    let (mut w, mut g) = (vec![0.0; n], vec![0.0; n]);
    let mut f = loss(&w, X, y, s, K, F, &mut g);
    let mut H: Vec<(Vec<f64>, Vec<f64>, f64)> = vec![]; // (s, y, 1 / yᵀs)
    let mut g2 = vec![0.0; n];
    for _ in 0..20000 {
        if g.iter().fold(0.0f64, |m, v| m.max(v.abs())) <= 1e-10 {
            break;
        }
        // Two-loop recursion: d = −H ∇f.
        let mut q = g.clone();
        let mut a = vec![0.0; H.len()];
        for (i, (sk, yk, rk)) in H.iter().enumerate().rev() {
            a[i] = rk * dot(sk, &q);
            q.iter_mut().zip(yk).for_each(|(qi, yi)| *qi -= a[i] * yi);
        }
        if let Some((sk, yk, _)) = H.last() {
            let gamma = dot(sk, yk) / dot(yk, yk);
            q.iter_mut().for_each(|v| *v *= gamma);
        }
        for (i, (sk, yk, rk)) in H.iter().enumerate() {
            let beta = rk * dot(yk, &q);
            q.iter_mut()
                .zip(sk)
                .for_each(|(qi, si)| *qi += (a[i] - beta) * si);
        }
        let d: Vec<f64> = q.iter().map(|v| -v).collect();
        let slope = dot(&g, &d);
        if slope >= 0.0 {
            H.clear();
            continue;
        }
        let mut t = if H.is_empty() {
            1.0 / g.iter().map(|v| v * v).sum::<f64>().sqrt().max(1.0)
        } else {
            1.0
        };
        let mut w2;
        let mut f2;
        loop {
            w2 = w
                .iter()
                .zip(&d)
                .map(|(wi, di)| wi + t * di)
                .collect::<Vec<_>>();
            f2 = loss(&w2, X, y, s, K, F, &mut g2);
            // Armijo, or near the optimum, where f changes below its rounding error,
            // the approximate Wolfe conditions on φ'(t) = ∇f(w + t d)ᵀ d (Hager and
            // Zhang 2005; δ = 1e-4, σ = 0.9, ε = 1e-6).
            let p = dot(&g2, &d);
            if f2 <= f + 1e-4 * t * slope
                || f2 <= f + 1e-6 * f.abs() && 0.9 * slope <= p && p <= (2e-4 - 1.0) * slope
            {
                break;
            }
            if t < 1e-20 {
                return w;
            }
            t /= 2.0;
        }
        let sk: Vec<f64> = w2.iter().zip(&w).map(|(a, b)| a - b).collect();
        let yk: Vec<f64> = g2.iter().zip(&g).map(|(a, b)| a - b).collect();
        let ys = dot(&yk, &sk);
        if ys > 1e-300 {
            H.push((sk, yk, 1.0 / ys));
            if H.len() > 10 {
                H.remove(0);
            }
        }
        (w, f) = (w2, f2);
        std::mem::swap(&mut g, &mut g2);
    }
    w
}

fn train() -> Model {
    let text = std::fs::read_to_string(
        repo()
            .join("docs")
            .join("labels")
            .join("kinds-sample.jsonl"),
    )
    .unwrap();
    let L: Vec<Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let T: Vec<&str> = L.iter().map(|x| x["text"].as_str().unwrap()).collect();
    let mut classes: Vec<String> = L
        .iter()
        .map(|x| x["label"].as_str().unwrap().to_string())
        .collect();
    classes.sort();
    classes.dedup();
    let y: Vec<usize> = L
        .iter()
        .map(|x| {
            classes
                .iter()
                .position(|c| c == x["label"].as_str().unwrap())
                .unwrap()
        })
        .collect();
    let mut terms: Vec<String> = T.iter().flat_map(|t| grams(t)).collect();
    terms.sort();
    terms.dedup();
    let vocab: HashMap<String, usize> = terms
        .iter()
        .enumerate()
        .map(|(j, t)| (t.clone(), j))
        .collect();
    let (N, F, K) = (T.len(), terms.len(), classes.len());
    let mut df = vec![0usize; F];
    for t in &T {
        let mut G: Vec<usize> = grams(t).iter().map(|g| vocab[g]).collect();
        G.sort();
        G.dedup();
        G.into_iter().for_each(|j| df[j] += 1);
    }
    let idf: Vec<f64> = df
        .iter()
        .map(|&d| ((1.0 + N as f64) / (1.0 + d as f64)).ln() + 1.0)
        .collect();
    let X: Vec<Row> = T.iter().map(|t| row(&vocab, &idf, t)).collect();
    let nk: Vec<usize> = (0..K)
        .map(|k| y.iter().filter(|&&v| v == k).count())
        .collect();
    let s: Vec<f64> = y
        .iter()
        .map(|&k| N as f64 / (K as f64 * nk[k] as f64))
        .collect();
    let w = lbfgs(&X, &y, &s, K, F);
    Model {
        vocab,
        idf,
        classes,
        w,
    }
}

fn model() -> &'static Model {
    static M: OnceLock<Model> = OnceLock::new();
    M.get_or_init(train)
}

/// Kinds of the texts T.
pub fn predict(T: &[String]) -> Vec<String> {
    if T.is_empty() {
        return vec![];
    }
    let m = model();
    let (K, F) = (m.classes.len(), m.idf.len());
    T.iter()
        .map(|t| {
            let x = row(&m.vocab, &m.idf, t);
            let z = (0..K)
                .map(|k| m.w[K * F + k] + x.iter().map(|&(j, v)| m.w[k * F + j] * v).sum::<f64>());
            let k = z
                .enumerate()
                .fold(
                    (0, f64::NEG_INFINITY),
                    |b, (k, v)| if v > b.1 { (k, v) } else { b },
                )
                .0;
            m.classes[k].clone()
        })
        .collect()
}
