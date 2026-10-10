//! 交渉集合 (Aumann–Maschler の `M_1^(i)`) とプレ交渉集合への所属判定。
//!
//! 配分 `x` に対し、プレイヤー `i` の `j` への異議は、`i in S`, `j not in S` の提携 `S` と、
//! `y(S) = v(S)`、`y_k > x_k (k in S)` を満たす `y` の組である。
//! `j` の反論は、`j in T`, `i not in T` の提携 `T` と、`z(T) = v(T)`、
//! `z_k >= y_k (k in T ∩ S)`、`z_k >= x_k (k in T \ S)` を満たす `z` の組である。
//! 反論のない異議があれば `x` は交渉集合に属さない。
//!
//! - 交渉集合 ([`Domain::Imputation`]): `x` は配分。
//! - プレ交渉集合 ([`Domain::Preimputation`]): `x` は準配分。
//!
//! 反論 `(z, T)` が存在することは `v(T) >= y(T ∩ S) + x(T \ S)` と同値なので、
//! 組 `(i, j)` と提携 `S` ごとに次の LP を解く:
//!
//! ```text
//! max delta  s.t.  y(S) = v(S)
//!                  y_k >= x_k + delta                      (k in S)
//!                  y(T ∩ S) >= v(T) - x(T \ S) + delta     (j in T, i not in T)
//! ```
//!
//! 最適値が正なら、反論のない異議 (正当な異議) がある。LP の数は `n (n - 1) 2^(n-2)` 個で、
//! 超過 `e(S, x) <= 0` の提携は異議に使えないので解かない。

use microlp::Variable;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::game::coalition::Coalition;
use crate::game::default_tolerance;
use crate::lp::{self, Cmp, Counter};
use crate::verify::feasibility_violation;

/// 反論のない異議。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Objection {
    /// 異議を唱えるプレイヤー。
    pub objector: usize,
    /// 異議を受けるプレイヤー。
    pub target: usize,
    pub coalition: Coalition,
    /// `coalition` のメンバーへの支払い (メンバー以外は `x` のまま)。
    pub payoff: Vec<f64>,
    /// LP の最適値 (異議の余裕)。
    pub margin: f64,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct BargainingReport {
    /// 反論のない異議。`None` なら交渉集合 (プレ交渉集合) に属する。
    pub objection: Option<Objection>,
    pub lp_solves: usize,
}

impl BargainingReport {
    pub fn is_member(&self) -> bool {
        self.objection.is_none()
    }
}

/// `x` が交渉集合 (プレ交渉集合) に属するかを判定する。異議の余裕が `tolerance` 以下なら異議とみなさない。
pub fn check(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<BargainingReport> {
    let n = game.players();
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    if let Some(reason) = feasibility_violation(game, x, domain, tolerance) {
        return Err(Error::InvalidArgument(format!("配分が不正: {reason}")));
    }
    let mut counter = Counter::default();
    let grand = game.grand();
    for objector in 0..n {
        for target in 0..n {
            if objector == target {
                continue;
            }
            for mask in 1..grand.0 {
                let coalition = Coalition(mask);
                if !coalition.contains(objector) || coalition.contains(target) {
                    continue;
                }
                let excess = game.value(coalition) - coalition.players().map(|k| x[k]).sum::<f64>();
                if excess <= tolerance {
                    continue;
                }
                if let Some((margin, payoff)) =
                    best_objection(game, x, objector, target, coalition, &mut counter)?
                    && margin > tolerance
                {
                    return Ok(BargainingReport {
                        objection: Some(Objection {
                            objector,
                            target,
                            coalition,
                            payoff,
                            margin,
                        }),
                        lp_solves: counter.solves,
                    });
                }
            }
        }
    }
    Ok(BargainingReport {
        objection: None,
        lp_solves: counter.solves,
    })
}

/// 既定の許容誤差 (`1e-7 * max(1, max |v|)` の 10 倍) で判定する。
pub fn is_in_bargaining_set(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<bool> {
    Ok(check(game, x, domain, 10.0 * default_tolerance(game))?.is_member())
}

/// 異議の余裕 `delta` を最大化する。最適値と、そのときの `y` (`S` 以外は `x`) を返す。
fn best_objection(
    game: &ExplicitGame,
    x: &[f64],
    objector: usize,
    target: usize,
    coalition: Coalition,
    counter: &mut Counter,
) -> Result<Option<(f64, Vec<f64>)>> {
    let n = game.players();
    let mut problem = lp::maximize();
    let y: Vec<Option<Variable>> = (0..n)
        .map(|k| {
            coalition
                .contains(k)
                .then(|| problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY)))
        })
        .collect();
    let delta = problem.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
    let members: Vec<(Variable, f64)> = y.iter().flatten().map(|&var| (var, 1.0)).collect();
    lp::add(&mut problem, members, Cmp::Eq, game.value(coalition));
    for (k, var) in y.iter().enumerate() {
        if let Some(var) = var {
            lp::add(
                &mut problem,
                vec![(*var, 1.0), (delta, -1.0)],
                Cmp::Ge,
                x[k],
            );
        }
    }
    for mask in 1..=game.grand().0 {
        let counter_coalition = Coalition(mask);
        if !counter_coalition.contains(target) || counter_coalition.contains(objector) {
            continue;
        }
        let mut terms: Vec<(Variable, f64)> = Vec::new();
        let mut outside = 0.0;
        for k in counter_coalition.players() {
            match y[k] {
                Some(var) => terms.push((var, 1.0)),
                None => outside += x[k],
            }
        }
        terms.push((delta, -1.0));
        lp::add(
            &mut problem,
            terms,
            Cmp::Ge,
            game.value(counter_coalition) - outside,
        );
    }
    // y_k >= x_k + delta と y(S) = v(S) から delta <= e(S, x) / abs(S) で有界。
    Ok(counter.solve(&problem)?.map(|solution| {
        let payoff = (0..n)
            .map(|k| y[k].map_or(x[k], |var| solution[var]))
            .collect();
        (solution[delta], payoff)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn majority() -> ExplicitGame {
        ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap()
    }

    #[test]
    fn three_player_majority_game() {
        let game = majority();
        assert!(is_in_bargaining_set(&game, &[1.0 / 3.0; 3], Domain::Imputation).unwrap());
        // (1/2, 1/2, 0): 3 が {2, 3} で y_2 > 1/2, y_3 > 0 を約束すると、1 の反論先は
        // {1} (v = 0 < 1/2) と {1, 2} (1/2 + y_2 > 1) しかなく、反論できない。
        let report = check(&game, &[0.5, 0.5, 0.0], Domain::Imputation, 1e-9).unwrap();
        let objection = report.objection.expect("正当な異議がある");
        assert_eq!((objection.objector, objection.target), (2, 0));
        assert_eq!(objection.coalition, Coalition::from_players(&[1, 2]));
        // 2 が {2, 3} で (0.45, 0.55) を約束すると、1 は {1, 3} で 3 に 0.55 以上を払えない。
        let report = check(&game, &[0.6, 0.4, 0.0], Domain::Imputation, 1e-9).unwrap();
        let objection = report.objection.expect("正当な異議がある");
        assert_eq!((objection.objector, objection.target), (1, 0));
        assert_eq!(objection.coalition, Coalition::from_players(&[1, 2]));
        assert!(objection.payoff[1] > 0.4 && objection.payoff[2] > 0.4);
    }

    #[test]
    fn rejects_infeasible_allocations() {
        let game = majority();
        assert!(check(&game, &[0.5, 0.5], Domain::Imputation, 1e-9).is_err());
        assert!(check(&game, &[0.5, 0.6, 0.0], Domain::Imputation, 1e-9).is_err());
        assert!(check(&game, &[1.5, -0.5, 0.0], Domain::Imputation, 1e-9).is_err());
        assert!(check(&game, &[1.5, -0.5, 0.0], Domain::Preimputation, 1e-9).is_ok());
    }
}
