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
//! LP の数と各 LP の行の数 (`2^(n-2)` 個の反論の提携) がともに指数的に増えるので、
//! プレイヤー数は [`MAX_BARGAINING_SET_PLAYERS`] 以下に限る。

use microlp::Variable;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::feasibility_violation;
use crate::game::scale::{check_tolerance, normalized_tolerance};
use crate::game::{Coalition, ExplicitGame, verification_tolerance};
use crate::lp::{self, Cmp, Counter};

/// [`check`] (と [`is_in_bargaining_set`]) が扱うプレイヤー数の上限。
///
/// 超過が正の提携が多い配分では、LP の数が `n (n - 1) 2^(n-2)` (`n = 11` で 56,320) に近づく。
/// 上限は、1 回の判定が 1 分以内に収まる人数である (計測は `docs/performance.md`)。
pub const MAX_BARGAINING_SET_PLAYERS: usize = 11;

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
///
/// 異議の余裕は浮動小数点の LP で求めるので、`tolerance = 0` でも厳密な判定ではない。
/// LP は、他の LP と同じく値を 2 のべき乗で割って最大絶対値を `[1, 2)` にそろえて解くので、判定は値の大きさによらない
/// (2 のべき乗倍したゲームと配分・許容誤差では、結論も異議の支払いと余裕もちょうど同じ倍率になる)。
///
/// プレイヤー数が [`MAX_BARGAINING_SET_PLAYERS`] を超えれば [`Error::TooManyPlayers`]、
/// `tolerance` が有限で 0 以上でなければ [`Error::InvalidArgument`] を返す。
/// 異議の LP は数学的には常に実行可能で有界なので、ソルバーが実行不可能・非有界と答えたときは
/// 異議なしとはみなさず、[`Error::Numerical`] を返す。
pub fn check(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<BargainingReport> {
    let n = game.players();
    if n > MAX_BARGAINING_SET_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_BARGAINING_SET_PLAYERS,
        });
    }
    check_tolerance(tolerance)?;
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    if let Some(reason) = feasibility_violation(game, x, domain, tolerance) {
        return Err(Error::InvalidArgument(format!("配分が不正: {reason}")));
    }
    // LP ソルバーは絶対誤差で判定するので、他の LP と同じく値を 2 のべき乗で割って最大絶対値を [1, 2) にそろえて解く
    // (2 のべき乗による乗除は丸めを生まないので、判定は値の大きさによらず、異議の余裕と支払いはちょうど元の大きさに戻る)。
    let (normalized, divisor) = game.normalized();
    let game = &normalized;
    let scaled: Vec<f64> = x.iter().map(|v| v / divisor).collect();
    let x = scaled.as_slice();
    let tolerance = normalized_tolerance(tolerance, divisor)?;
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
                let excess = game.value(coalition) - coalition.sum(x);
                if excess <= tolerance {
                    continue;
                }
                let (margin, payoff) =
                    best_objection(game, x, objector, target, coalition, &mut counter)?;
                if margin > tolerance {
                    return Ok(BargainingReport {
                        objection: Some(Objection {
                            objector,
                            target,
                            coalition,
                            payoff: payoff.iter().map(|v| v * divisor).collect(),
                            margin: margin * divisor,
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

/// 検証の既定の許容誤差 ([`crate::game::verification_tolerance`]、`max|v(S)|` の `1e-11` 倍) で判定する。
/// Kohlberg 基準 ([`crate::verify::kohlberg`]) やカーネル条件の既定と同じ幅である。
/// プレイヤー数の上限は [`check`] と同じ。
pub fn is_in_bargaining_set(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<bool> {
    Ok(check(game, x, domain, verification_tolerance(game))?.is_member())
}

/// 異議の余裕 `delta` を最大化する。最適値と、そのときの `y` (`S` 以外は `x`) を返す。
fn best_objection(
    game: &ExplicitGame,
    x: &[f64],
    objector: usize,
    target: usize,
    coalition: Coalition,
    counter: &mut Counter,
) -> Result<(f64, Vec<f64>)> {
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
        for k in counter_coalition.members() {
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
    // delta を十分小さくすれば全ての行を満たすので、LP は常に実行可能で、
    // y_k >= x_k + delta と y(S) = v(S) から delta <= e(S, x) / abs(S) で有界。
    // ソルバーが実行不可能・非有界と答えたら数値的な失敗なので、異議なしとはみなさずエラーにする。
    let solution = counter.solve_feasible(
        &problem,
        &format!("プレイヤー {objector} の {target} への提携 {coalition} での異議の LP "),
    )?;
    let payoff = (0..n)
        .map(|k| y[k].map_or(x[k], |var| solution[var]))
        .collect();
    Ok((solution[delta], payoff))
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
    fn judgment_does_not_depend_on_the_scale_of_values() {
        // LP ソルバーは絶対誤差 (1e-10) で判定するので、値を正規化せずに解くと、値の小さいゲームでは
        // 異議の余裕が誤差に埋もれていた。2 のべき乗倍したゲームでは、判定も異議の支払いもちょうど同じ倍率になる。
        let reference = check(&majority(), &[0.6, 0.4, 0.0], Domain::Imputation, 1e-12).unwrap();
        let reference = reference.objection.expect("正当な異議がある");
        for power in [-60, -40, -20, 20, 40] {
            let scale = 2f64.powi(power);
            let values: Vec<f64> = majority().values()[1..].iter().map(|v| v * scale).collect();
            let game = ExplicitGame::from_binary(&values).unwrap();
            let x: Vec<f64> = [0.6, 0.4, 0.0].iter().map(|v| v * scale).collect();
            let report = check(&game, &x, Domain::Imputation, 1e-12 * scale).unwrap();
            let objection = report.objection.unwrap_or_else(|| panic!("2^{power} 倍"));
            assert_eq!(
                (objection.objector, objection.target),
                (1, 0),
                "2^{power} 倍"
            );
            assert_eq!(objection.margin, reference.margin * scale, "2^{power} 倍");
            let third = [scale / 3.0; 3];
            assert!(
                is_in_bargaining_set(&game, &third, Domain::Imputation).unwrap(),
                "2^{power} 倍"
            );
        }
    }

    #[test]
    fn rejects_games_above_player_limit() {
        // 対称な多数決ゲームの均等配分は交渉集合に属し、上限がなければ多くの LP を解く。
        let n = MAX_BARGAINING_SET_PLAYERS + 1;
        let game = ExplicitGame::from_fn(n, |s| if 2 * s.len() > n { 1.0 } else { 0.0 }).unwrap();
        let x = vec![1.0 / n as f64; n];
        let err = check(&game, &x, Domain::Imputation, 1e-9).unwrap_err();
        assert!(
            matches!(err, Error::TooManyPlayers { max, .. } if max == MAX_BARGAINING_SET_PLAYERS),
            "{err}"
        );
        assert!(is_in_bargaining_set(&game, &x, Domain::Imputation).is_err());
    }

    #[test]
    fn rejects_invalid_tolerance() {
        let game = majority();
        for tolerance in [f64::NAN, -1e-9, f64::INFINITY] {
            let err = check(&game, &[1.0 / 3.0; 3], Domain::Imputation, tolerance).unwrap_err();
            assert!(matches!(err, Error::InvalidArgument(_)), "{err}");
        }
        assert!(check(&game, &[1.0 / 3.0; 3], Domain::Imputation, 0.0).is_ok());
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
