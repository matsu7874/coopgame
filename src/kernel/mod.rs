//! カーネル・プレカーネルの判定と、transfer scheme による 1 点の計算。
//!
//! 最大余剰 `s_ij(x)` を使うと、
//!
//! - プレカーネル: 効率的な `x` で、全ての `i != j` について `s_ij = s_ji`。
//! - カーネル: 配分 `x` で、全ての `i != j` について `(s_ij - s_ji)(x_j - v({j})) <= 0`。
//!
//! transfer scheme (Maschler / Stearns 1968) は、`j` から `i` への支払い
//! `delta_ij = (s_ij - s_ji) / 2`(カーネルでは `x_j - v({j})` で頭打ち)が最大の組を選び、
//! その支払いを実行することを繰り返す。支払い後はその組の最大余剰が釣り合う。
//!
//! 6 人までのゲームでは、カーネル・プレカーネル全体を多面体の和集合として求められる ([`kernel_set`])。

mod set;

pub(crate) use set::for_each_combination;
pub use set::{KernelPiece, KernelSet, MAX_KERNEL_SET_PLAYERS, Row, SetOptions, kernel_set};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::game::allocation::check_imputation_set;
use crate::game::allocation::feasibility_violation;
use crate::game::coalition::Coalition;
use crate::game::default_tolerance;
use crate::solution::Guarantee;
use crate::surplus::max_surplus;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct TransferOptions {
    /// `2 * max delta_ij` がこの値以下になったら停止する。
    pub tolerance: f64,
    pub max_iterations: usize,
}

impl TransferOptions {
    /// LP を使わないため、仁より 2 桁厳しい許容誤差を既定にする。
    pub fn for_game(game: &ExplicitGame) -> TransferOptions {
        TransferOptions {
            tolerance: 0.01 * default_tolerance(game),
            max_iterations: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KernelResult {
    pub allocation: Vec<f64>,
    pub iterations: usize,
    /// 停止時の [`kernel_violation`]。
    pub violation: f64,
    pub converged: bool,
    /// 収束すれば [`Guarantee::Exact`] (カーネル条件を許容誤差の範囲で満たす)、しなければ [`Guarantee::Approximate`]。
    pub guarantee: Guarantee,
}

/// 最大余剰の不釣り合い。`x` が(プレ)カーネルに属すれば 0。
///
/// 組 `(i, j)` ごとの値は `2 * delta_ij` で、その最大値を返す。
pub fn kernel_violation(game: &ExplicitGame, x: &[f64], domain: Domain) -> f64 {
    best_transfer(game, x, domain).map_or(0.0, |(_, _, delta)| 2.0 * delta)
}

/// `x` が効率性・領域条件を満たし、不釣り合いが `tolerance` 以下かを判定する。
pub fn is_in_kernel(game: &ExplicitGame, x: &[f64], domain: Domain, tolerance: f64) -> bool {
    x.len() == game.players()
        && feasibility_violation(game, x, domain, tolerance).is_none()
        && kernel_violation(game, x, domain) <= tolerance
}

/// 各プレイヤーに `v({i})` を与え、残りを等分した配分。
pub fn equal_surplus_division(game: &ExplicitGame) -> Vec<f64> {
    let singles = game.singleton_values();
    let surplus = game.value(game.grand()) - singles.iter().sum::<f64>();
    let share = surplus / game.players() as f64;
    singles.iter().map(|v| v + share).collect()
}

/// transfer scheme で(プレ)カーネルの 1 点を求める。
///
/// `start` を省略すると [`equal_surplus_division`] から始める。
/// 収束しないまま `max_iterations` に達した場合も `converged = false` で結果を返す。
pub fn kernel_point(
    game: &ExplicitGame,
    domain: Domain,
    start: Option<&[f64]>,
    options: TransferOptions,
) -> Result<KernelResult> {
    check_imputation_set(game, domain, options.tolerance)?;
    let mut x = match start {
        Some(start) => start.to_vec(),
        None => equal_surplus_division(game),
    };
    if x.len() != game.players() {
        return Err(Error::InvalidArgument(format!(
            "初期配分の長さ {} がプレイヤー数 {} と異なる",
            x.len(),
            game.players()
        )));
    }
    if let Some(reason) = feasibility_violation(game, &x, domain, options.tolerance) {
        return Err(Error::InvalidArgument(format!("初期配分が不正: {reason}")));
    }

    let mut iterations = 0;
    loop {
        let transfer = best_transfer(game, &x, domain);
        let violation = transfer.map_or(0.0, |(_, _, delta)| 2.0 * delta);
        if violation <= options.tolerance || iterations >= options.max_iterations {
            return Ok(KernelResult {
                allocation: x,
                iterations,
                violation,
                converged: violation <= options.tolerance,
                guarantee: if violation <= options.tolerance {
                    Guarantee::Exact
                } else {
                    Guarantee::Approximate
                },
            });
        }
        let (i, j, delta) = transfer.expect("violation > 0 なら支払いが存在する");
        x[i] += delta;
        x[j] -= delta;
        iterations += 1;
    }
}

/// 支払い額 `delta_ij` が最大の組 `(i, j, delta_ij)`。支払いが不要なら `None`。
fn best_transfer(game: &ExplicitGame, x: &[f64], domain: Domain) -> Option<(usize, usize, f64)> {
    let n = game.players();
    let surplus = max_surplus(game, x);
    let mut best: Option<(usize, usize, f64)> = None;
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let mut delta = (surplus[i * n + j] - surplus[j * n + i]) / 2.0;
            if domain == Domain::Imputation {
                delta = delta.min(x[j] - game.value(Coalition::singleton(j)));
            }
            if delta > 0.0 && best.is_none_or(|(_, _, current)| delta > current) {
                best = Some((i, j, delta));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_game_kernel() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let result = kernel_point(
            &game,
            Domain::Imputation,
            None,
            TransferOptions::for_game(&game),
        )
        .unwrap();
        assert!(result.converged);
        assert!(is_in_kernel(
            &game,
            &result.allocation,
            Domain::Imputation,
            1e-6
        ));
        // このゲームのカーネルは仁 (2, 2, 0) の 1 点。
        for (a, e) in result.allocation.iter().zip([2.0, 2.0, 0.0]) {
            assert!((a - e).abs() < 1e-6);
        }
    }

    #[test]
    fn rejects_unbalanced_point() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        assert!(!is_in_kernel(
            &game,
            &[1.0, 3.0, 0.0],
            Domain::Imputation,
            1e-6
        ));
        assert!(
            (kernel_violation(&game, &[1.0, 3.0, 0.0], Domain::Imputation) - 2.0).abs() < 1e-12
        );
    }
}
