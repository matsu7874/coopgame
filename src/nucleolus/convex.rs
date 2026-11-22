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
//! `T -> -(v(T ∪ {i}) - x(T ∪ {i}))` (`T ⊆ N \ {i, j}`) が劣モジュラなので
//! (Lean による証明: リポジトリの `proofs/` の `surplus_objective_submodular`)、
//! 劣モジュラ関数の最小化 (`submodular` モジュール) で全提携を列挙せずに求める。
//!
//! 手順: 等分から始め、組 `(i, j)` を順に回って `s_ij` と `s_ji` を釣り合わせる移転を行う。
//! 1 周の間に、どの組の不釣り合い `|s_ij - s_ji|` も許容誤差以下で、移転が起きなかったら停止する。
//! この最後の 1 周は停止時の `x` で全ての組を調べたことになるので、停止時の `x` はプレカーネル条件を満たす。
//!
//! 劣モジュラ最小化と停止条件は絶対誤差で判定するので、[`value_scale`] 以下で最大の 2 のべき乗で値を割った
//! ゲームで求め、結果を元の大きさに戻す (値を `c` 倍すると結果も `c` 倍になる)。
//!
//! 保証の種類はゲームの性質の根拠 ([`crate::properties::ProofKind`]) で決まる。
//! 性質を宣言しただけのゲーム ([`crate::properties::Assume`]) では、最大余剰の最小化も
//! カーネル = 仁 の定理も成り立たないことがあるので、結果は [`crate::Unverified`] で返る。

use std::cell::Cell;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::scale::{Scaled, normalized_tolerance, normalizer};
use crate::game::{PlayerSet, SetFunction, value_scale};
use crate::properties::{ConvexGame, ProofKind};
use crate::solution::{Property, Solution};
use crate::submodular;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ConvexOptions {
    /// 不釣り合い `|s_ij - s_ji|` の許容誤差 (ゲームの値と同じ単位)。`None` なら `1e-9 * value_scale(game)`。
    ///
    /// 有限で 0 以上でなければ、[`nucleolus_with`] は [`Error::InvalidArgument`] を返す。
    /// 結果のプレカーネル条件は許容誤差の範囲で成り立つ。保証の種類は許容誤差の大きさを反映しない
    /// (大きい値を指定すると、仁から離れた配分にも同じ保証が付く)。
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
    /// 特性関数 ([`SetFunction::value`]) を呼んだ回数。
    ///
    /// 値の大きさの目安 ([`value_scale`]) と全体提携の値、劣モジュラ最小化の中と外 (1 人提携) の評価を全て含む。
    /// 空提携は `v(∅) = 0` ([`SetFunction`] の約束) として呼ばない。
    /// [`crate::Unverified::verify`] などの事後検証での評価は含まない。
    pub evaluations: usize,
}

/// 凸ゲームの仁。結果の型は性質の根拠で決まる (`Proven` なら [`Solution`]、`Assumed` なら [`crate::Unverified`])。
pub fn nucleolus<G: ConvexGame + ?Sized>(
    game: &G,
) -> Result<<G::Proof as ProofKind>::Output<Solution>> {
    Ok(nucleolus_with(game, ConvexOptions::default())?.0)
}

/// 設定を指定した [`nucleolus`]。`options.tolerance` が有限で 0 以上でないとき、またはゲームの値の大きさで割ると
/// 有限でない (値に比べて大きすぎる) ときは [`Error::InvalidArgument`] を返す。
pub fn nucleolus_with<G: ConvexGame + ?Sized>(
    game: &G,
    options: ConvexOptions,
) -> Result<(<G::Proof as ProofKind>::Output<Solution>, ConvexStats)> {
    let (allocation, stats) = prekernel_point(game, options)?;
    let solution = Solution {
        domain: Domain::Imputation,
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
) -> Result<f64> {
    let n = game.players();
    let others: Vec<usize> = (0..n).filter(|&k| k != i && k != j).collect();
    let mut f = |set: &[bool]| {
        let mut coalition = PlayerSet::singleton(n, i);
        for (k, inside) in others.iter().zip(set) {
            if *inside {
                coalition.insert(*k);
            }
        }
        -(game.value(&coalition) - coalition.sum(x))
    };
    let minimum = submodular::minimize(others.len(), &mut f, tolerance, max_iterations)?;
    // minimize は f(∅) を引いて正規化しているので戻す: f(∅) = -(v({i}) - x_i)。
    let single = game.value(&PlayerSet::singleton(n, i)) - x[i];
    Ok(single - minimum.value)
}

/// 特性関数を呼んだ回数を数える ([`ConvexStats::evaluations`])。
/// 空提携は `v(∅) = 0` ([`SetFunction`] の約束) として呼ばない。
struct Counted<'a, G: ?Sized> {
    game: &'a G,
    calls: Cell<usize>,
}

impl<G: SetFunction + ?Sized> SetFunction for Counted<'_, G> {
    fn players(&self) -> usize {
        self.game.players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if coalition.is_empty() {
            return 0.0;
        }
        self.calls.set(self.calls.get() + 1);
        self.game.value(coalition)
    }
}

fn prekernel_point<G: SetFunction + ?Sized>(
    game: &G,
    options: ConvexOptions,
) -> Result<(Vec<f64>, ConvexStats)> {
    let counted = Counted {
        game,
        calls: Cell::new(0),
    };
    let scale = value_scale(&counted);
    let divisor = normalizer(scale);
    let tolerance = normalized_tolerance(options.tolerance.unwrap_or(1e-9 * scale), divisor)?;
    let options = ConvexOptions {
        tolerance: Some(tolerance),
        ..options
    };
    let (mut x, mut stats) = normalized_prekernel_point(&Scaled::new(&counted, divisor), options)?;
    for value in &mut x {
        *value *= divisor;
    }
    stats.evaluations = counted.calls.get();
    Ok((x, stats))
}

/// 値の大きさを 1 前後にそろえたゲームで transfer scheme を行う。`options.tolerance` は設定済み。
fn normalized_prekernel_point<G: SetFunction + ?Sized>(
    game: &G,
    options: ConvexOptions,
) -> Result<(Vec<f64>, ConvexStats)> {
    let n = game.players();
    if n == 0 {
        return Err(Error::InvalidArgument("プレイヤーがいない".into()));
    }
    let total = game.value(&PlayerSet::grand(n));
    let mut x = vec![total / n as f64; n];
    let mut stats = ConvexStats::default();
    if n == 1 {
        return Ok((x, stats));
    }
    let tolerance = options.tolerance.expect("prekernel_point が設定する");
    // 最大余剰はこの許容誤差より十分細かく求める。
    let sfm_tolerance = 1e-3 * tolerance;
    for _ in 0..options.max_sweeps {
        stats.sweeps += 1;
        let mut moved = false;
        for i in 0..n {
            for j in i + 1..n {
                let forward =
                    max_surplus(game, &x, i, j, sfm_tolerance, options.max_sfm_iterations)?;
                let backward =
                    max_surplus(game, &x, j, i, sfm_tolerance, options.max_sfm_iterations)?;
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
    use crate::properties::ConvexChecked;

    /// `evaluations` は特性関数を実際に呼んだ回数と一致し、空提携は呼ばない (1 人のゲームでは値の大きさの目安が空提携を含む)。
    #[test]
    fn evaluations_count_actual_calls() {
        struct Counting<'a> {
            game: &'a crate::ExplicitGame,
            calls: Cell<usize>,
        }
        impl SetFunction for Counting<'_> {
            fn players(&self) -> usize {
                self.game.players()
            }
            fn value(&self, coalition: &PlayerSet) -> f64 {
                assert!(!coalition.is_empty(), "空提携を呼んだ");
                self.calls.set(self.calls.get() + 1);
                SetFunction::value(self.game, coalition)
            }
        }
        let single = crate::ExplicitGame::from_lex(&[3.0]).unwrap();
        let mut games = vec![single];
        games.extend((2..8).map(|n| generators::random_convex(n, n as u64).unwrap()));
        for game in &games {
            let counting = Counting {
                game,
                calls: Cell::new(0),
            };
            let assumed = crate::properties::Assume::convex(&counting);
            let (_, stats) = nucleolus_with(&assumed, ConvexOptions::default()).unwrap();
            assert_eq!(
                stats.evaluations,
                counting.calls.get(),
                "n = {}",
                game.players()
            );
            assert!(stats.evaluations > 0);
        }
    }

    /// 値が NaN の提携がある (例えば利用者の関数が失敗した) ゲームでは、止まらずにエラーを返す。
    #[test]
    fn non_finite_values_return_an_error() {
        struct Broken;
        impl SetFunction for Broken {
            fn players(&self) -> usize {
                6
            }
            fn value(&self, coalition: &PlayerSet) -> f64 {
                if coalition.len() == 3 {
                    f64::NAN
                } else {
                    coalition.len() as f64 * coalition.len() as f64
                }
            }
        }
        let assumed = crate::properties::Assume::convex(Broken);
        assert!(nucleolus_with(&assumed, ConvexOptions::default()).is_err());
    }

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

    #[test]
    fn rejects_invalid_tolerance() {
        let game = ConvexChecked::new(generators::random_convex(5, 3).unwrap()).unwrap();
        let single = ConvexChecked::new(crate::ExplicitGame::from_lex(&[2.0]).unwrap()).unwrap();
        for tolerance in [f64::NAN, -1.0, -1e-300, f64::INFINITY] {
            let options = ConvexOptions {
                tolerance: Some(tolerance),
                ..ConvexOptions::default()
            };
            // 1 人ゲーム (反復しない) でも確かめる。
            for result in [
                nucleolus_with(&game, options),
                nucleolus_with(&single, options),
            ] {
                assert!(
                    matches!(result, Err(Error::InvalidArgument(_))),
                    "{tolerance}: {result:?}"
                );
            }
        }
        // 0 は受け付ける (不釣り合いが厳密に 0 になるまで移転する)。
        let expected = nucleolus::nucleolus(game.game()).unwrap().allocation;
        let options = ConvexOptions {
            tolerance: Some(0.0),
            ..ConvexOptions::default()
        };
        let (solution, _) = nucleolus_with(&game, options).unwrap();
        for (a, e) in solution.allocation.iter().zip(&expected) {
            assert!(
                (a - e).abs() <= 1e-9,
                "{:?} != {expected:?}",
                solution.allocation
            );
        }
    }
}
