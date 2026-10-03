//! 結合テストと examples で共有する、独立に実装した基準と補助関数。
#![allow(dead_code)] // テストごとに使う関数が異なる

/// 2 つのベクトルの差の最大絶対値 (無限大ノルム)。
pub fn max_abs_difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q).abs())
        .fold(0.0, f64::max)
}

/// 2 つの配分の各成分の差が `tolerance` 以下であることを確かめる。
pub fn assert_close(actual: &[f64], expected: &[f64], tolerance: f64, label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a - e).abs() <= tolerance,
            "{label}: {actual:?} != {expected:?}"
        );
    }
}

/// Aumann & Maschler (1985) のタルムード則。破産ゲームの仁は常にこの規則と一致する (同論文の定理)。
///
/// 請求の半分 h_i = d_i / 2、請求の和 D として、
/// - E <= D / 2 なら x_i = min(h_i, λ) (Σ x_i = E となる λ)
/// - E > D / 2 なら x_i = d_i - min(h_i, μ) (Σ min(h_i, μ) = D - E となる μ)
pub fn talmud_rule(estate: f64, claims: &[f64]) -> Vec<f64> {
    let total: f64 = claims.iter().sum();
    let halves: Vec<f64> = claims.iter().map(|d| d / 2.0).collect();
    // Σ min(h_i, t) = target となる t を二分法で求める。
    let level = |target: f64| {
        let (mut low, mut high) = (0.0, halves.iter().cloned().fold(0.0, f64::max));
        for _ in 0..200 {
            let mid = (low + high) / 2.0;
            let sum: f64 = halves.iter().map(|h| h.min(mid)).sum();
            if sum < target { low = mid } else { high = mid }
        }
        (low + high) / 2.0
    };
    if estate <= total / 2.0 {
        let lambda = level(estate);
        halves.iter().map(|h| h.min(lambda)).collect()
    } else {
        let mu = level(total - estate);
        claims
            .iter()
            .zip(&halves)
            .map(|(d, h)| d - h.min(mu))
            .collect()
    }
}

/// Littlechild & Owen (1973) の公式。費用が「提携内の最大の費用」で決まるゲーム
/// (`c(S) = max_{i in S} c_i`、空港ゲーム) の Shapley 値は、費用を小さい順に並べて
/// 増分 `c_(j) - c_(j-1)` を、それを必要とする `n - j + 1` 人で等分した和になる。
pub fn littlechild_owen(costs: &[f64]) -> Vec<f64> {
    let n = costs.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| costs[a].total_cmp(&costs[b]));
    let mut shares = vec![0.0; n];
    let mut accumulated = 0.0;
    let mut previous = 0.0;
    for (rank, &player) in order.iter().enumerate() {
        accumulated += (costs[player] - previous) / (n - rank) as f64;
        previous = costs[player];
        shares[player] = accumulated;
    }
    shares
}
