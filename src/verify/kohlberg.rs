//! Kohlberg 基準による仁・プレ仁の検証。
//!
//! 超過が `alpha` 以上の真部分提携の族を `D(alpha, x)` とする。
//!
//! - プレ仁: `x` がプレ仁であることと、空でない全ての `D(alpha, x)` が平衡
//!   (全提携に正の重みを付けて特性ベクトルの和を `1_N` の正数倍にできる)であることは同値。
//! - 仁: `D0(x) = { {i} : x_i = v({i}) }` を重み 0 以上で加えてよい。
//!
//! 超過の値の段ごとに平衡性を LP で判定する。族の階数が `n` に達して平衡なら、
//! それ以降の段も平衡になる(`1_N` が錐の内点にあるため)ので判定を打ち切る。

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::game::coalition::Coalition;
use crate::game::default_tolerance;
use crate::linalg::Span;
use crate::lp::{self, Cmp, Counter};
use crate::surplus::excesses;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KohlbergReport {
    pub satisfied: bool,
    /// 不成立の場合の理由。
    pub reason: Option<String>,
    /// 平衡性を判定した超過の段の数。
    pub levels_checked: usize,
    pub lp_solves: usize,
}

/// 既定の許容誤差で検証する。
pub fn kohlberg(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<KohlbergReport> {
    kohlberg_with_tolerance(game, x, domain, default_tolerance(game) * 10.0)
}

fn kohlberg_with_tolerance(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<KohlbergReport> {
    let n = game.players();
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    let fail = |reason: String| KohlbergReport {
        satisfied: false,
        reason: Some(reason),
        levels_checked: 0,
        lp_solves: 0,
    };
    if let Some(reason) = feasibility_violation(game, x, domain, tolerance) {
        return Ok(fail(reason));
    }

    let excess = excesses(game, x);
    let full = (1usize << n) - 1;
    let mut order: Vec<usize> = (1..full).collect();
    order.sort_by(|a, b| excess[*b].total_cmp(&excess[*a]));

    let at_lower_bound: Vec<Coalition> = match domain {
        Domain::Imputation => (0..n)
            .filter(|&i| x[i] <= game.value(Coalition::singleton(i)) + tolerance)
            .map(Coalition::singleton)
            .collect(),
        Domain::Preimputation => Vec::new(),
    };

    let mut counter = Counter::default();
    let mut collection: Vec<Coalition> = Vec::new();
    let mut span = Span::new(n);
    let mut levels_checked = 0;
    let mut start = 0;
    while start < order.len() {
        let level = excess[order[start]];
        let mut end = start;
        while end < order.len() && excess[order[end]] >= level - tolerance {
            let coalition = Coalition(order[end] as u64);
            collection.push(coalition);
            span.insert_coalition(coalition);
            end += 1;
        }
        levels_checked += 1;
        if !is_balanced(&collection, &at_lower_bound, n, &mut counter)? {
            return Ok(KohlbergReport {
                satisfied: false,
                reason: Some(format!("超過 {level} 以上の提携族が平衡でない")),
                levels_checked,
                lp_solves: counter.solves,
            });
        }
        if span.is_full() {
            break;
        }
        start = end;
    }
    Ok(KohlbergReport {
        satisfied: true,
        reason: None,
        levels_checked,
        lp_solves: counter.solves,
    })
}

/// 効率性と領域条件の違反を調べる。
pub(crate) fn feasibility_violation(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Option<String> {
    let total: f64 = x.iter().sum();
    let grand = game.value(game.grand());
    if (total - grand).abs() > tolerance {
        return Some(format!(
            "効率性を満たさない: x(N) = {total}, v(N) = {grand}"
        ));
    }
    if domain == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let lower = game.value(Coalition::singleton(i));
            if *xi < lower - tolerance {
                return Some(format!(
                    "個人合理性を満たさない: x_{} = {xi} < v({{{}}}) = {lower}",
                    i + 1,
                    i + 1
                ));
            }
        }
    }
    None
}

/// `collection` に重み 1 以上、`optional` に重み 0 以上を付けて
/// `sum lambda_S 1_S = mu 1_N` とできるかを判定する。
fn is_balanced(
    collection: &[Coalition],
    optional: &[Coalition],
    n: usize,
    counter: &mut Counter,
) -> Result<bool> {
    let mut problem = lp::minimize();
    let mu = problem.add_var(1.0, (0.0, f64::INFINITY));
    let mut weights = Vec::with_capacity(collection.len() + optional.len());
    for coalition in collection {
        weights.push((*coalition, problem.add_var(0.0, (1.0, f64::INFINITY))));
    }
    for coalition in optional {
        if !collection.contains(coalition) {
            weights.push((*coalition, problem.add_var(0.0, (0.0, f64::INFINITY))));
        }
    }
    for player in 0..n {
        let mut terms: Vec<_> = weights
            .iter()
            .filter(|(coalition, _)| coalition.contains(player))
            .map(|(_, var)| (*var, 1.0))
            .collect();
        terms.push((mu, -1.0));
        lp::add(&mut problem, terms, Cmp::Eq, 0.0);
    }
    Ok(counter.solve(&problem)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nucleolus_and_rejects_other_points() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        assert!(
            kohlberg(&game, &[2.0, 2.0, 0.0], Domain::Imputation)
                .unwrap()
                .satisfied
        );
        let report = kohlberg(&game, &[1.0, 3.0, 0.0], Domain::Imputation).unwrap();
        assert!(!report.satisfied);
        let report = kohlberg(&game, &[2.0, 2.0, 1.0], Domain::Imputation).unwrap();
        assert!(report.reason.unwrap().contains("効率性"));
    }

    #[test]
    fn majority_game() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let third = 1.0 / 3.0;
        assert!(
            kohlberg(&game, &[third; 3], Domain::Preimputation)
                .unwrap()
                .satisfied
        );
        assert!(
            !kohlberg(&game, &[0.5, 0.25, 0.25], Domain::Preimputation)
                .unwrap()
                .satisfied
        );
    }
}
