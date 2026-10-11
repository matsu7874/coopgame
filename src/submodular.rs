//! 劣モジュラ関数の最小化 (Fujishige–Wolfe の最小ノルム点法)。
//!
//! 劣モジュラ関数 `f` (`f(∅) = 0`) の基多面体 `B(f)` の、原点に最も近い点 `x*` を Wolfe の方法で求める。
//! `B(f)` 上の線形最適化は貪欲法 (`x` の小さい順に要素を加え、増分を座標にする) で解ける。
//!
//! 停止は双対ギャップで判定する。`B(f)` の点 `x` について、任意の `A` で `f(A) >= x(A) >= x⁻(V)`
//! (`x⁻(V)` は `x` の負の成分の和) なので、`x⁻(V)` は最小値の下界になる。
//! 貪欲法の途中で現れる集合 (`x` の小さい順の接頭辞) の `f` の最小値を上界とし、
//! 上界と下界の差が許容誤差以下になったら、その接頭辞を最小解として返す。
//!
//! `f` が劣モジュラでない場合、下界は成り立たないので、返す値は最小値の保証を持たない。
//! 呼び出し側は、性質を仮定して使うときにその結果を仮定付きとして扱う。

use crate::error::{Error, Result};
use crate::linalg::solve_square;

/// 最小化の結果。
#[derive(Clone, Debug, PartialEq)]
pub struct Minimum {
    /// 見つけた集合 (`true` の要素が属する)。
    pub set: Vec<bool>,
    /// `f(set)`。
    pub value: f64,
    /// 最小値の下界 `x⁻(V)` (`f` が劣モジュラなら有効)。
    pub lower_bound: f64,
    /// 貪欲法を呼んだ回数 (1 回で `f` を `m` 回評価する)。
    pub greedy_calls: usize,
}

/// `m` 要素の集合上の関数 `f` を最小化する。`f` は `f(∅) = 0` に正規化して扱う。
///
/// 双対ギャップが `tolerance` 以下になったら止める。`max_iterations` 回の反復で止まらなければエラー。
pub fn minimize(
    m: usize,
    f: &mut dyn FnMut(&[bool]) -> f64,
    tolerance: f64,
    max_iterations: usize,
) -> Result<Minimum> {
    let empty = f(&vec![false; m]);
    let mut eval = |set: &[bool]| f(set) - empty;
    if m == 0 {
        return Ok(Minimum {
            set: Vec::new(),
            value: 0.0,
            lower_bound: 0.0,
            greedy_calls: 0,
        });
    }
    let mut greedy_calls = 0;
    let (mut x, _, _) = greedy(m, &vec![0.0; m], &mut eval);
    greedy_calls += 1;
    let mut points: Vec<Vec<f64>> = vec![x.clone()];
    let mut weights: Vec<f64> = vec![1.0];
    for _ in 0..max_iterations {
        let (q, best_set, best_value) = greedy(m, &x, &mut eval);
        greedy_calls += 1;
        let lower_bound: f64 = x.iter().map(|v| v.min(0.0)).sum();
        if best_value - lower_bound <= tolerance {
            return Ok(Minimum {
                set: best_set,
                value: best_value,
                lower_bound,
                greedy_calls,
            });
        }
        let progress = dot(&x, &x) - dot(&x, &q);
        if progress <= 1e-14 * dot(&x, &x).max(1.0) {
            return Err(Error::Numerical(format!(
                "最小ノルム点法が進まない (双対ギャップ {:e})",
                best_value - lower_bound
            )));
        }
        points.push(q);
        weights.push(0.0);
        // 小反復: アフィン包の最小ノルム点が凸包の内部に来るまで点を減らす。
        loop {
            let Some(alpha) = affine_minimizer(&points) else {
                return Err(Error::Numerical(
                    "最小ノルム点法のアフィン包が退化した".into(),
                ));
            };
            let eps = 1e-12;
            if alpha.iter().all(|a| *a > eps) {
                weights = alpha;
                x = combine(&points, &weights);
                break;
            }
            // 現在の重み (凸結合) から alpha へ向かい、最初に 0 になる点で止める。
            let theta = weights
                .iter()
                .zip(&alpha)
                .filter(|(_, a)| **a <= eps)
                .map(|(w, a)| w / (w - a))
                .fold(1.0_f64, f64::min);
            for (w, a) in weights.iter_mut().zip(&alpha) {
                *w = theta * a + (1.0 - theta) * *w;
            }
            let mut k = 0;
            while k < points.len() {
                if weights[k] <= eps {
                    points.remove(k);
                    weights.remove(k);
                } else {
                    k += 1;
                }
            }
            let total: f64 = weights.iter().sum();
            for w in &mut weights {
                *w /= total;
            }
        }
    }
    Err(Error::LimitExceeded(format!(
        "劣モジュラ関数の最小化が {max_iterations} 回の反復で終わらない"
    )))
}

/// 重み `w` の小さい順に要素を加える貪欲法。基多面体の頂点 (`w` との内積が最小) と、
/// 途中の接頭辞のうち `f` が最小のものとその値を返す。
fn greedy(m: usize, w: &[f64], eval: &mut dyn FnMut(&[bool]) -> f64) -> (Vec<f64>, Vec<bool>, f64) {
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| w[a].total_cmp(&w[b]).then(a.cmp(&b)));
    let mut set = vec![false; m];
    let mut vertex = vec![0.0; m];
    let mut previous = 0.0;
    let (mut best_len, mut best_value) = (0, 0.0);
    for (position, &k) in order.iter().enumerate() {
        set[k] = true;
        let value = eval(&set);
        vertex[k] = value - previous;
        previous = value;
        if value < best_value {
            best_value = value;
            best_len = position + 1;
        }
    }
    let mut best_set = vec![false; m];
    for &k in &order[..best_len] {
        best_set[k] = true;
    }
    (vertex, best_set, best_value)
}

/// 点 `p_0..p_k` のアフィン包で原点に最も近い点の係数 (和が 1)。
fn affine_minimizer(points: &[Vec<f64>]) -> Option<Vec<f64>> {
    let k = points.len();
    if k == 1 {
        return Some(vec![1.0]);
    }
    // p_0 + sum beta_l (p_l - p_0) のノルムを最小化する正規方程式。
    let base = &points[0];
    let directions: Vec<Vec<f64>> = points[1..]
        .iter()
        .map(|p| p.iter().zip(base).map(|(a, b)| a - b).collect())
        .collect();
    let gram: Vec<Vec<f64>> = directions
        .iter()
        .map(|d| directions.iter().map(|e| dot(d, e)).collect())
        .collect();
    let rhs: Vec<f64> = directions.iter().map(|d| -dot(d, base)).collect();
    let largest = gram
        .iter()
        .enumerate()
        .map(|(i, row)| row[i])
        .fold(0.0_f64, f64::max);
    let beta = solve_square(gram, rhs, 1e-13 * largest.max(1e-300))?;
    let mut alpha = Vec::with_capacity(k);
    alpha.push(1.0 - beta.iter().sum::<f64>());
    alpha.extend(beta);
    Some(alpha)
}

fn combine(points: &[Vec<f64>], weights: &[f64]) -> Vec<f64> {
    let mut x = vec![0.0; points[0].len()];
    for (p, w) in points.iter().zip(weights) {
        for (xi, pi) in x.iter_mut().zip(p) {
            *xi += w * pi;
        }
    }
    x
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(p, q)| p * q).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    /// 総当たりの最小値。
    fn brute_force(m: usize, f: &dyn Fn(&[bool]) -> f64) -> f64 {
        (0u64..(1 << m))
            .map(|mask| {
                let set: Vec<bool> = (0..m).map(|k| mask >> k & 1 == 1).collect();
                f(&set)
            })
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn matches_brute_force_on_random_submodular_functions() {
        let mut rng = SplitMix64::new(7);
        for case in 0..200 {
            let m = 1 + (rng.next_u64() % 10) as usize;
            // 劣モジュラ: 凹関数 (平方根) と被覆関数と加法的関数の和。
            let weights: Vec<f64> = (0..m).map(|_| rng.next_f64() * 3.0).collect();
            let linear: Vec<f64> = (0..m).map(|_| rng.next_f64() * 4.0 - 2.5).collect();
            let covers: Vec<u64> = (0..m).map(|_| rng.next_u64() & 0xff).collect();
            let f = move |set: &[bool]| {
                let mass: f64 = set
                    .iter()
                    .zip(&weights)
                    .filter(|(s, _)| **s)
                    .map(|(_, w)| w)
                    .sum();
                let covered = set
                    .iter()
                    .zip(&covers)
                    .filter(|(s, _)| **s)
                    .fold(0u64, |acc, (_, c)| acc | c)
                    .count_ones() as f64;
                let line: f64 = set
                    .iter()
                    .zip(&linear)
                    .filter(|(s, _)| **s)
                    .map(|(_, l)| l)
                    .sum();
                2.0 * mass.sqrt() + 0.3 * covered + line
            };
            let expected = brute_force(m, &f);
            let mut g = |set: &[bool]| f(set);
            let found = minimize(m, &mut g, 1e-9, 10_000).unwrap();
            assert!(
                (found.value - expected).abs() <= 1e-7,
                "case {case}: {} != {expected}",
                found.value
            );
            assert!(found.lower_bound <= expected + 1e-7);
            assert!((f(&found.set) - found.value).abs() <= 1e-9);
        }
    }

    #[test]
    fn empty_ground_set() {
        let mut f = |_: &[bool]| 3.0;
        let found = minimize(0, &mut f, 1e-9, 10).unwrap();
        assert_eq!(found.value, 0.0);
    }
}
