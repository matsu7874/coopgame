//! 凸ゲームの仁を、全提携を列挙せずに求める。
//!
//! 根拠 (いずれも文献の定理で、このモジュールはそれを組み合わせる):
//!
//! - 凸ゲームのカーネルは仁の 1 点である (Maschler, Peleg & Shapley 1971)。
//! - 凸ゲームは 0-単調なので、カーネルとプレカーネルは一致する (Maschler, Peleg & Shapley 1979)。
//!   また凸ゲームではコアが空でなく仁はコアに属するので、仁とプレ仁も一致する。
//! - transfer scheme はプレカーネルの点に収束する (Stearns 1968)。
//!
//! したがって、プレカーネル条件 `s_ij(x) = s_ji(x)` (全ての組) を満たす `x` を見つければ、それが仁である。
//! 最大余剰 `s_ij(x) = max { v(S) - x(S) : i in S, j not in S }` は、凸ゲームでは
//! `T -> -(v(T ∪ {i}) - x(T ∪ {i}))` (`T ⊆ N \ {i, j}`) が劣モジュラなので、
//! 劣モジュラ関数の最小化 (`submodular` モジュール) で全提携を列挙せずに求める。
//!
//! 手順: 等分から始め、組 `(i, j)` を順に回って `s_ij` と `s_ji` を釣り合わせる移転を行う。
//! 1 周の間に、どの組の不釣り合い `|s_ij - s_ji|` も許容誤差以下で、移転が起きなかったら停止する。
//! この最後の 1 周は停止時の `x` で全ての組を調べたことになるので、停止時の `x` はプレカーネル条件を満たす。
//!
//! 保証の種類はゲームの性質の根拠 ([`crate::structure::ProofKind`]) で決まる。
//! 性質を宣言しただけのゲーム ([`crate::structure::Assume`]) では、最大余剰の最小化も
//! カーネル = 仁 の定理も成り立たないことがあるので、結果は [`crate::Unverified`] で返る。

use std::cell::Cell;

use crate::error::{Error, Result};
use crate::guarantee::Property;
use crate::oracle::{PlayerSet, SetFunction, value_scale};
use crate::solution::{Concept, Solution};
use crate::structure::{ConvexGame, ProofKind};
use crate::submodular;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ConvexOptions {
    /// 不釣り合い `|s_ij - s_ji|` の許容誤差。`None` なら `1e-9 * scale`。
    pub tolerance: Option<f64>,
    /// 組を回る周回の上限。
    pub max_sweeps: usize,
    /// 1 回の劣モジュラ最小化の反復の上限。
    pub max_sfm_iterations: usize,
}

impl Default for ConvexOptions {
    fn default() -> ConvexOptions {
        ConvexOptions {
            tolerance: None,
            max_sweeps: 10_000,
            max_sfm_iterations: 100_000,
        }
    }
}

/// 計算の統計。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConvexStats {
    pub sweeps: usize,
    pub transfers: usize,
    /// 劣モジュラ最小化の回数。
    pub minimizations: usize,
    /// 特性関数を評価した回数。
    pub evaluations: usize,
}

/// 凸ゲームの仁。結果の型は性質の根拠で決まる (`Proven` なら [`Solution`]、`Assumed` なら [`crate::Unverified`])。
pub fn nucleolus<G: ConvexGame + ?Sized>(
    game: &G,
) -> Result<<G::Proof as ProofKind>::Output<Solution>> {
    Ok(nucleolus_with(game, ConvexOptions::default())?.0)
}

pub fn nucleolus_with<G: ConvexGame + ?Sized>(
    game: &G,
    options: ConvexOptions,
) -> Result<(<G::Proof as ProofKind>::Output<Solution>, ConvexStats)> {
    let (allocation, stats) = prekernel_point(game, options)?;
    let solution = Solution {
        concept: Concept::Nucleolus,
        allocation,
        guarantee: G::Proof::guarantee(Property::Convex),
        method: "convex-transfer",
    };
    Ok((G::Proof::wrap(solution, Property::Convex), stats))
}

/// 劣モジュラ最小化で `s_ij(x)` を求める (`T ⊆ N \ {i, j}` 上で `-(v(T + i) - x(T + i))` を最小化)。
fn max_surplus<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    i: usize,
    j: usize,
    tolerance: f64,
    max_iterations: usize,
    evaluations: &Cell<usize>,
) -> Result<f64> {
    let n = game.players();
    let others: Vec<usize> = (0..n).filter(|&k| k != i && k != j).collect();
    let mut f = |set: &[bool]| {
        let mut coalition = PlayerSet::from_players(n, &[i]);
        for (k, inside) in others.iter().zip(set) {
            if *inside {
                coalition.insert(*k);
            }
        }
        evaluations.set(evaluations.get() + 1);
        -(game.value(&coalition) - coalition.sum(x))
    };
    let minimum = submodular::minimize(others.len(), &mut f, tolerance, max_iterations)?;
    // minimize は f(∅) を引いて正規化しているので戻す: f(∅) = -(v({i}) - x_i)。
    let single = game.value(&PlayerSet::from_players(n, &[i])) - x[i];
    Ok(single - minimum.value)
}

fn prekernel_point<G: SetFunction + ?Sized>(
    game: &G,
    options: ConvexOptions,
) -> Result<(Vec<f64>, ConvexStats)> {
    let n = game.players();
    if n == 0 {
        return Err(Error::InvalidArgument("プレイヤーがいない".into()));
    }
    let total = game.value(&PlayerSet::full(n));
    let mut x = vec![total / n as f64; n];
    let mut stats = ConvexStats::default();
    if n == 1 {
        return Ok((x, stats));
    }
    let scale = value_scale(game);
    let tolerance = options.tolerance.unwrap_or(1e-9 * scale);
    // 最大余剰はこの許容誤差より十分細かく求める。
    let sfm_tolerance = 1e-3 * tolerance;
    let evaluations = Cell::new(0);
    for _ in 0..options.max_sweeps {
        stats.sweeps += 1;
        let mut moved = false;
        for i in 0..n {
            for j in i + 1..n {
                let forward = max_surplus(
                    game,
                    &x,
                    i,
                    j,
                    sfm_tolerance,
                    options.max_sfm_iterations,
                    &evaluations,
                )?;
                let backward = max_surplus(
                    game,
                    &x,
                    j,
                    i,
                    sfm_tolerance,
                    options.max_sfm_iterations,
                    &evaluations,
                )?;
                stats.minimizations += 2;
                let gap = forward - backward;
                if gap.abs() > tolerance {
                    // s_ij > s_ji なら j から i に払う。
                    x[i] += gap / 2.0;
                    x[j] -= gap / 2.0;
                    stats.transfers += 1;
                    moved = true;
                }
            }
        }
        if !moved {
            stats.evaluations = evaluations.get();
            return Ok((x, stats));
        }
    }
    Err(Error::LimitExceeded(format!(
        "transfer scheme が {} 周で収束しない",
        options.max_sweeps
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;
    use crate::nucleolus;
    use crate::structure::ConvexChecked;

    #[test]
    fn matches_lp_nucleolus_on_random_convex_games() {
        for seed in 0..30 {
            let n = 2 + (seed % 7) as usize;
            let game = ConvexChecked::new(generators::random_convex(n, seed).unwrap())
                .expect("凸ゲームを生成する");
            let expected = nucleolus::nucleolus(game.game()).unwrap().allocation;
            let (solution, stats) = nucleolus_with(&game, ConvexOptions::default()).unwrap();
            let scale = game.game().max_abs_value().max(1.0);
            for (a, e) in solution.allocation.iter().zip(&expected) {
                assert!(
                    (a - e).abs() <= 1e-6 * scale,
                    "seed {seed} n={n}: {:?} != {expected:?} ({stats:?})",
                    solution.allocation
                );
            }
            assert_eq!(
                solution.guarantee,
                crate::Guarantee::Proven(Property::Convex)
            );
        }
    }
}
