//! 特性関数の値が不確かなときの配分の分析。
//!
//! 実務の `v(S)` は見積もりなので誤差を含む。次の 3 つで、誤差が配分に与える影響を調べる。
//!
//! - [`monte_carlo`]: `v` の分布からゲームを繰り返し引いて解き、配分の平均・標準偏差・分位点を求める。
//! - [`IntervalGame`]: 各提携の値を区間 `[下限, 上限]` で与え、区間内の一様分布として [`monte_carlo`] に渡す。
//! - [`influence`]: 提携 `S` の値を少し動かしたときの配分の変化率 `dx / dv(S)` (中心差分)。
//!   仁では、超過の大きい段の提携 (配分を決めている提携) の値だけが効く。[`key_coalitions`] でその提携を選べる。
//!
//! 解き方 (仁・Shapley 値など) は関数として渡す。

use crate::analysis::explain::{self, ExplainOptions};
use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame};
use crate::rng::SplitMix64;

/// 配分の分布の要約。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Distribution {
    pub mean: Vec<f64>,
    /// 標本標準偏差。
    pub std: Vec<f64>,
    /// `(分位, プレイヤーごとの値)`。既定は 5%, 50%, 95%。
    pub quantiles: Vec<(f64, Vec<f64>)>,
    /// 解けた標本の数。
    pub samples: usize,
    /// 解けなかった標本の数 (配分集合が空など)。
    pub failures: usize,
}

const QUANTILES: [f64; 3] = [0.05, 0.5, 0.95];

/// `sample` でゲームを `samples` 回引き、`solve` で解いた配分の分布を求める。
pub fn monte_carlo(
    samples: usize,
    seed: u64,
    mut sample: impl FnMut(&mut SplitMix64) -> Result<ExplicitGame>,
    mut solve: impl FnMut(&ExplicitGame) -> Result<Vec<f64>>,
) -> Result<Distribution> {
    let mut rng = SplitMix64::new(seed);
    let mut allocations: Vec<Vec<f64>> = Vec::with_capacity(samples);
    let mut failures = 0;
    for _ in 0..samples {
        let game = sample(&mut rng)?;
        match solve(&game) {
            Ok(x) => allocations.push(x),
            Err(_) => failures += 1,
        }
    }
    summarize(&allocations, failures)
}

fn summarize(allocations: &[Vec<f64>], failures: usize) -> Result<Distribution> {
    let Some(first) = allocations.first() else {
        return Err(Error::InvalidArgument(format!(
            "解けた標本がない ({failures} 個とも失敗)"
        )));
    };
    let n = first.len();
    let count = allocations.len() as f64;
    let mean: Vec<f64> = (0..n)
        .map(|i| allocations.iter().map(|x| x[i]).sum::<f64>() / count)
        .collect();
    let std: Vec<f64> = (0..n)
        .map(|i| {
            if allocations.len() < 2 {
                return 0.0;
            }
            let squares: f64 = allocations.iter().map(|x| (x[i] - mean[i]).powi(2)).sum();
            (squares / (count - 1.0)).sqrt()
        })
        .collect();
    let quantiles = QUANTILES
        .iter()
        .map(|&q| {
            let values = (0..n)
                .map(|i| {
                    let mut column: Vec<f64> = allocations.iter().map(|x| x[i]).collect();
                    column.sort_by(f64::total_cmp);
                    // 線形補間なしの最近傍の順位。
                    let rank =
                        ((q * (column.len() - 1) as f64).round() as usize).min(column.len() - 1);
                    column[rank]
                })
                .collect();
            (q, values)
        })
        .collect();
    Ok(Distribution {
        mean,
        std,
        quantiles,
        samples: allocations.len(),
        failures,
    })
}

/// 各提携の値を区間で与えたゲーム。
#[derive(Clone, Debug, PartialEq)]
pub struct IntervalGame {
    lower: ExplicitGame,
    upper: ExplicitGame,
}

impl IntervalGame {
    /// 下限と上限のゲーム (同じ人数で、全ての提携で下限 <= 上限)。
    pub fn new(lower: ExplicitGame, upper: ExplicitGame) -> Result<IntervalGame> {
        if lower.players() != upper.players() {
            return Err(Error::InvalidArgument("下限と上限の人数が異なる".into()));
        }
        if lower
            .values()
            .iter()
            .zip(upper.values())
            .any(|(l, u)| l > u)
        {
            return Err(Error::InvalidArgument(
                "下限が上限を超える提携がある".into(),
            ));
        }
        Ok(IntervalGame { lower, upper })
    }

    /// 各値の前後 `relative` (例: 0.1 なら ±10%) の区間。
    pub fn around(game: &ExplicitGame, relative: f64) -> Result<IntervalGame> {
        if !(relative.is_finite() && relative >= 0.0) {
            return Err(Error::InvalidArgument("相対誤差は非負の有限値".into()));
        }
        let shift = |sign: f64| game.map_values(|_, v| v + sign * relative * v.abs());
        IntervalGame::new(shift(-1.0)?, shift(1.0)?)
    }

    pub fn lower(&self) -> &ExplicitGame {
        &self.lower
    }

    pub fn upper(&self) -> &ExplicitGame {
        &self.upper
    }

    /// 各提携の値を区間から独立に一様に引いたゲーム。
    pub(crate) fn sample(&self, rng: &mut SplitMix64) -> Result<ExplicitGame> {
        self.lower
            .map_values(|s, l| l + (self.upper.value(s) - l) * rng.next_f64())
    }
}

/// 名前で選べる解き方 ([`solver`])。
pub type Solver = Box<dyn FnMut(&ExplicitGame) -> Result<Vec<f64>>>;

/// [`solver`] が受け付ける名前。
pub const SOLVERS: [&str; 3] = ["nucleolus", "prenucleolus", "shapley"];

/// 名前 (`"nucleolus"`・`"prenucleolus"`・`"shapley"`) から解き方を選ぶ。
pub fn solver(name: &str) -> Option<Solver> {
    match name {
        "nucleolus" => Some(Box::new(|g: &ExplicitGame| {
            Ok(crate::nucleolus::nucleolus(g)?.allocation)
        })),
        "prenucleolus" => Some(Box::new(|g: &ExplicitGame| {
            Ok(crate::nucleolus::prenucleolus(g)?.allocation)
        })),
        "shapley" => Some(Box::new(|g: &ExplicitGame| Ok(crate::values::shapley(g)))),
        _ => None,
    }
}

/// 区間ゲームの値を一様に引いて解いた配分の分布。
pub fn interval_monte_carlo(
    game: &IntervalGame,
    samples: usize,
    seed: u64,
    solve: impl FnMut(&ExplicitGame) -> Result<Vec<f64>>,
) -> Result<Distribution> {
    monte_carlo(samples, seed, |rng| game.sample(rng), solve)
}

/// 提携の値に対する配分の感度。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Influence {
    pub coalition: Coalition,
    /// 各プレイヤーの `dx_i / dv(S)` (中心差分)。
    pub sensitivity: Vec<f64>,
}

impl Influence {
    /// 感度の絶対値の最大。
    pub fn magnitude(&self) -> f64 {
        self.sensitivity.iter().fold(0.0, |acc, s| acc.max(s.abs()))
    }
}

/// 提携 `coalitions` の値を `±delta` 動かしたときの配分の変化率。
pub fn influence(
    game: &ExplicitGame,
    coalitions: &[Coalition],
    delta: f64,
    mut solve: impl FnMut(&ExplicitGame) -> Result<Vec<f64>>,
) -> Result<Vec<Influence>> {
    if !(delta.is_finite() && delta > 0.0) {
        return Err(Error::InvalidArgument("delta は正の有限値".into()));
    }
    coalitions
        .iter()
        .map(|&coalition| {
            let shifted =
                |sign: f64| game.with_value(coalition, game.value(coalition) + sign * delta);
            let plus = solve(&shifted(1.0)?)?;
            let minus = solve(&shifted(-1.0)?)?;
            Ok(Influence {
                coalition,
                sensitivity: plus
                    .iter()
                    .zip(&minus)
                    .map(|(p, m)| (p - m) / (2.0 * delta))
                    .collect(),
            })
        })
        .collect()
}

/// 配分 `x` で超過が大きい上位 `levels` 段の提携 (仁ではこれが配分を決めている) と全体提携。
pub fn key_coalitions(game: &ExplicitGame, x: &[f64], levels: usize) -> Result<Vec<Coalition>> {
    let report = explain::report(
        game,
        x,
        ExplainOptions {
            levels,
            max_listed: usize::MAX,
            tolerance: None,
        },
    )?;
    let mut coalitions: Vec<Coalition> = report
        .levels
        .into_iter()
        .flat_map(|level| level.coalitions)
        .collect();
    coalitions.push(game.grand());
    Ok(coalitions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{nucleolus, values};

    fn four_player() -> ExplicitGame {
        ExplicitGame::from_lex(&[
            0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
        ])
        .unwrap()
    }

    fn solve_nucleolus(game: &ExplicitGame) -> Result<Vec<f64>> {
        Ok(nucleolus::nucleolus(game)?.allocation)
    }

    #[test]
    fn zero_width_interval_has_no_spread() {
        let game = four_player();
        let interval = IntervalGame::around(&game, 0.0).unwrap();
        let distribution = interval_monte_carlo(&interval, 5, 1, solve_nucleolus).unwrap();
        let expected = solve_nucleolus(&game).unwrap();
        for ((mean, std), e) in distribution
            .mean
            .iter()
            .zip(&distribution.std)
            .zip(&expected)
        {
            assert!((mean - e).abs() < 1e-9);
            assert!(*std < 1e-9);
        }
        assert_eq!(distribution.samples, 5);
    }

    #[test]
    fn spread_grows_with_uncertainty() {
        let game = four_player();
        let narrow = interval_monte_carlo(
            &IntervalGame::around(&game, 0.01).unwrap(),
            100,
            2,
            solve_nucleolus,
        )
        .unwrap();
        let wide = interval_monte_carlo(
            &IntervalGame::around(&game, 0.1).unwrap(),
            100,
            2,
            solve_nucleolus,
        )
        .unwrap();
        for i in 0..4 {
            assert!(wide.std[i] > narrow.std[i]);
            let (low, high) = (&wide.quantiles[0].1, &wide.quantiles[2].1);
            assert!(low[i] <= wide.mean[i] && wide.mean[i] <= high[i]);
        }
    }

    /// 仁は超過の大きい段の提携の値だけで決まる。超過がずっと小さい提携の値を動かしても変わらない。
    #[test]
    fn nucleolus_depends_only_on_binding_coalitions() {
        let game = four_player();
        let x = solve_nucleolus(&game).unwrap();
        let key = key_coalitions(&game, &x, 1).unwrap();
        let binding = influence(&game, &key, 1e-4, solve_nucleolus).unwrap();
        assert!(binding.iter().any(|i| i.magnitude() > 0.1));
        // {1} の超過は -3.5 で、最大の超過 -0.5 よりずっと小さい。
        let slack = influence(&game, &[Coalition::singleton(0)], 1e-4, solve_nucleolus).unwrap();
        assert!(slack[0].magnitude() < 1e-6);
    }

    /// Shapley 値は線形で、`dφ_i / dv(S)` は i ∈ S なら (|S|-1)!(n-|S|)!/n!、i ∉ S なら -|S|!(n-|S|-1)!/n!。
    #[test]
    fn shapley_sensitivity_matches_weights() {
        let game = four_player();
        let s = Coalition::from_players(&[0, 1]);
        let result = influence(&game, &[s], 0.5, |g| Ok(values::shapley(g))).unwrap();
        let inside = 1.0 * 2.0 / 24.0; // (2-1)!(4-2)!/4!
        let outside = -2.0 * 1.0 / 24.0; // -2!(4-2-1)!/4!
        let expected = [inside, inside, outside, outside];
        for (a, e) in result[0].sensitivity.iter().zip(expected) {
            assert!((a - e).abs() < 1e-9, "{:?}", result[0].sensitivity);
        }
    }
}
