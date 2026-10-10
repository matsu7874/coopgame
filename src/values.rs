//! Shapley 値・Banzhaf 値・solidarity 値。
//!
//! - 厳密計算: 明示ゲームで全提携を走査する (`O(n 2^n)`)。
//! - サンプリング: 任意人数の [`SetFunction`] で、限界貢献の標本平均と標準誤差を返す。
//!   Shapley 値はランダムな順列の限界貢献 (Castro, Gómez, Tejada 2009)、
//!   Banzhaf 値はランダムな提携 (各人を確率 1/2 で含める) に対する各人の限界貢献を平均する。
//!
//! Banzhaf 値は正規化しない値 `(1 / 2^(n-1)) sum_{S not containing i} (v(S ∪ {i}) - v(S))` を返す。
//! 投票ゲームの正規化 Banzhaf 指数は [`normalize`] で和を 1 にする。
//!
//! solidarity 値 (Nowak & Radzik 1994) は、Shapley 値の限界貢献 `v(S) - v(S \ {i})` を、
//! 提携 `S` のメンバーの限界貢献の平均 `A(S) = (1/|S|) sum_{k in S} (v(S) - v(S \ {k}))` に置き換えた値である。

use crate::game::{Coalition, ExplicitGame, PlayerSet, SetFunction};
use crate::rng::SplitMix64;
use crate::solution::Guarantee;

/// 厳密な Shapley 値: `phi_i = sum_{S not containing i} |S|! (n - |S| - 1)! / n! (v(S ∪ {i}) - v(S))`。
pub fn shapley(game: &ExplicitGame) -> Vec<f64> {
    let n = game.players();
    // weights[s] = s! (n - s - 1)! / n! = 1 / (n C(n - 1, s))
    let mut weights = vec![0.0; n];
    let mut binomial = 1.0;
    for (s, weight) in weights.iter_mut().enumerate() {
        *weight = 1.0 / (n as f64 * binomial);
        binomial = binomial * (n - 1 - s) as f64 / (s + 1) as f64;
    }
    marginal_sums(game, |size| weights[size])
}

/// 厳密な Banzhaf 値 (正規化しない): `beta_i = (1 / 2^(n-1)) sum_{S not containing i} (v(S ∪ {i}) - v(S))`。
pub fn banzhaf(game: &ExplicitGame) -> Vec<f64> {
    let weight = 1.0 / (1u64 << (game.players() - 1)) as f64;
    marginal_sums(game, |_| weight)
}

/// 厳密な solidarity 値 (Nowak & Radzik 1994):
/// `psi_i = sum_{S containing i} (|S| - 1)! (n - |S|)! / n! A(S)`、
/// `A(S) = (1/|S|) sum_{k in S} (v(S) - v(S \ {k}))`。
pub fn solidarity(game: &ExplicitGame) -> Vec<f64> {
    let n = game.players();
    // weights[s] = (s - 1)! (n - s)! / n! = 1 / (n C(n - 1, s - 1)) (s = 1..=n)
    let mut weights = vec![0.0; n + 1];
    let mut binomial = 1.0;
    for (s, weight) in weights.iter_mut().enumerate().skip(1) {
        *weight = 1.0 / (n as f64 * binomial);
        binomial = binomial * (n - s) as f64 / s as f64;
    }
    let mut values = vec![0.0; n];
    for mask in 1..1u64 << n {
        let coalition = Coalition(mask);
        let value = game.value(coalition);
        let size = coalition.len();
        let average = coalition
            .players()
            .map(|k| value - game.value(Coalition(mask & !(1 << k))))
            .sum::<f64>()
            / size as f64;
        for i in coalition.players() {
            values[i] += weights[size] * average;
        }
    }
    values
}

/// 和が 1 になるように正規化する (正規化 Banzhaf 指数など)。和が 0 ならそのまま返す。
pub fn normalize(values: &[f64]) -> Vec<f64> {
    let total: f64 = values.iter().sum();
    if total == 0.0 {
        return values.to_vec();
    }
    values.iter().map(|v| v / total).collect()
}

/// `sum_{S not containing i} weight(|S|) (v(S ∪ {i}) - v(S))`。
fn marginal_sums(game: &ExplicitGame, weight: impl Fn(usize) -> f64) -> Vec<f64> {
    let n = game.players();
    let mut values = vec![0.0; n];
    // 全体提携には加えるプレイヤーがいないので除く。
    for mask in 0..(1u64 << n) - 1 {
        let coalition = Coalition(mask);
        let base = game.value(coalition);
        let w = weight(coalition.len());
        for (i, value) in values.iter_mut().enumerate() {
            if mask >> i & 1 == 0 {
                *value += w * (game.value(Coalition(mask | 1 << i)) - base);
            }
        }
    }
    values
}

/// サンプリングによる推定値。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Estimate {
    pub values: Vec<f64>,
    /// 各推定値の標準誤差 (標本標準偏差 / sqrt(標本数))。
    pub standard_errors: Vec<f64>,
    /// 特性関数を評価した回数。
    pub evaluations: usize,
    /// 常に [`Guarantee::Approximate`]。
    pub guarantee: Guarantee,
}

/// 標本平均と標準誤差を逐次的に求める (Welford 法)。
struct Accumulator {
    count: usize,
    mean: Vec<f64>,
    squares: Vec<f64>,
}

impl Accumulator {
    fn new(n: usize) -> Accumulator {
        Accumulator {
            count: 0,
            mean: vec![0.0; n],
            squares: vec![0.0; n],
        }
    }

    fn add(&mut self, sample: &[f64]) {
        self.count += 1;
        let count = self.count as f64;
        for ((mean, squares), &x) in self.mean.iter_mut().zip(&mut self.squares).zip(sample) {
            let delta = x - *mean;
            *mean += delta / count;
            *squares += delta * (x - *mean);
        }
    }

    fn finish(self, evaluations: usize) -> Estimate {
        let count = self.count as f64;
        let standard_errors = self
            .squares
            .iter()
            .map(|s| {
                if self.count > 1 {
                    (s / (count - 1.0) / count).sqrt()
                } else {
                    f64::INFINITY
                }
            })
            .collect();
        Estimate {
            values: self.mean,
            standard_errors,
            evaluations,
            guarantee: Guarantee::Approximate,
        }
    }
}

/// ランダムな順列 `permutations` 個の限界貢献の平均で Shapley 値を推定する (評価回数は順列あたり n 回)。
pub fn shapley_sampling<G: SetFunction + ?Sized>(
    game: &G,
    permutations: usize,
    seed: u64,
) -> Estimate {
    let n = game.players();
    let mut rng = SplitMix64::new(seed);
    let mut accumulator = Accumulator::new(n);
    let mut order: Vec<usize> = (0..n).collect();
    let mut sample = vec![0.0; n];
    let empty_value = game.value(&PlayerSet::empty(n));
    for _ in 0..permutations {
        // Fisher-Yates
        for i in (1..n).rev() {
            let j = rng.range(0, i as u64) as usize;
            order.swap(i, j);
        }
        let mut coalition = PlayerSet::empty(n);
        let mut previous = empty_value;
        for &player in &order {
            coalition.insert(player);
            let current = game.value(&coalition);
            sample[player] = current - previous;
            previous = current;
        }
        accumulator.add(&sample);
    }
    accumulator.finish(1 + permutations * n)
}

/// ランダムな提携 `samples` 個に対する各人の限界貢献の平均で Banzhaf 値を推定する
/// (評価回数は標本あたり n + 1 回)。各人を確率 1/2 で含めた提携 `S` について、
/// `S` から `i` を除いた提携は他の人の部分集合として一様なので、`v(S ∪ {i}) - v(S \ {i})` は不偏推定量になる。
pub fn banzhaf_sampling<G: SetFunction + ?Sized>(game: &G, samples: usize, seed: u64) -> Estimate {
    let n = game.players();
    let mut rng = SplitMix64::new(seed);
    let mut accumulator = Accumulator::new(n);
    let mut sample = vec![0.0; n];
    for _ in 0..samples {
        let mut coalition = PlayerSet::empty(n);
        for i in 0..n {
            if rng.next_u64() & 1 == 1 {
                coalition.insert(i);
            }
        }
        let base = game.value(&coalition);
        for (i, marginal) in sample.iter_mut().enumerate() {
            let mut other = coalition.clone();
            other.toggle(i);
            let flipped = game.value(&other);
            *marginal = if coalition.contains(i) {
                base - flipped
            } else {
                flipped - base
            };
        }
        accumulator.add(&sample);
    }
    accumulator.finish(samples * (n + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    fn assert_close(actual: &[f64], expected: &[f64], label: &str) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-9, "{label}: {actual:?} != {expected:?}");
        }
    }

    #[test]
    fn shapley_is_efficient_and_symmetric() {
        let game = generators::bnf(1, 6, 3).unwrap();
        let phi = shapley(&game);
        assert!((phi.iter().sum::<f64>() - game.value(game.grand())).abs() < 1e-9);
        let symmetric = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 60.0, 60.0, 60.0, 72.0]).unwrap();
        assert_close(&shapley(&symmetric), &[24.0; 3], "対称ゲーム");
    }

    #[test]
    fn solidarity_is_efficient_and_differs_from_shapley_on_null_player() {
        // v(S) = 1 iff 0 ∈ S (プレイヤー 1, 2 はナルプレイヤー)。Shapley 値は (1, 0, 0) だが、
        // solidarity 値はナルプレイヤーにも正の値を与える。
        // 手計算: A({0}) = 1、A({0,j}) = 1/2、A({0,1,2}) = 1/3、0 を含まない提携は A = 0。
        // psi_0 = (1/3)(1) + 2 (1/6)(1/2) + (1/3)(1/3) = 11/18、psi_1 = psi_2 = (1/6)(1/2) + (1/3)(1/3) = 7/36。
        let game = ExplicitGame::from_lex(&[1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0]).unwrap();
        assert_close(&shapley(&game), &[1.0, 0.0, 0.0], "Shapley 値");
        assert_close(
            &solidarity(&game),
            &[11.0 / 18.0, 7.0 / 36.0, 7.0 / 36.0],
            "solidarity 値",
        );
        // Nowak & Radzik の例 (Diffo Lambo 2015 の Example 2 が再掲): 全員一致ゲーム u_{1,2} で (7/18, 7/18, 4/18)。
        let unanimity = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]).unwrap();
        assert_close(
            &solidarity(&unanimity),
            &[7.0 / 18.0, 7.0 / 18.0, 4.0 / 18.0],
            "u_{1,2}",
        );
        let random = generators::bnf(1, 6, 7).unwrap();
        let psi = solidarity(&random);
        assert!((psi.iter().sum::<f64>() - random.value(random.grand())).abs() < 1e-9);
    }

    #[test]
    fn banzhaf_of_majority_game() {
        // 3 人多数決: 各人は 4 通りの提携のうち 2 通りで決定的なので 1/2。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        assert_close(&banzhaf(&game), &[0.5; 3], "多数決");
        assert_close(&normalize(&banzhaf(&game)), &[1.0 / 3.0; 3], "正規化");
    }

    #[test]
    fn sampling_estimates_agree_with_exact_values() {
        let game = generators::bnf(2, 8, 5).unwrap();
        let exact_shapley = shapley(&game);
        let exact_banzhaf = banzhaf(&game);
        let shapley_estimate = shapley_sampling(&game, 20_000, 1);
        let banzhaf_estimate = banzhaf_sampling(&game, 20_000, 2);
        for i in 0..8 {
            // 標準誤差の 5 倍以内 (正規近似で外れる確率は 1e-6 未満)。
            assert!(
                (shapley_estimate.values[i] - exact_shapley[i]).abs()
                    < 5.0 * shapley_estimate.standard_errors[i]
            );
            assert!(
                (banzhaf_estimate.values[i] - exact_banzhaf[i]).abs()
                    < 5.0 * banzhaf_estimate.standard_errors[i]
            );
        }
        // 順列ごとの限界貢献の和は v(N) なので、Shapley 値の推定値の和は v(N) に一致する。
        assert!(
            (shapley_estimate.values.iter().sum::<f64>() - game.value(game.grand())).abs() < 1e-6
        );
    }
}
