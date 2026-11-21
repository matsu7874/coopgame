//! 超過の段の読み取りと、段の提携族の平衡性の判定 (浮動小数点数)。
//!
//! 配分 `x` での超過 `v(S) - x(S)` を大きい順に並べ、許容誤差以内で等しいものを 1 つの段として読む。
//! 仁 (プレ仁) では、段を上から順に合わせた提携族がどれも平衡になる (Kohlberg 基準)。
//! 厳密な検証 ([`crate::verify`]) は読み取った段 ([`SortedExcess`]) から厳密な配分を復元する。
//! 逐次 LP ([`crate::nucleolus`]) は、解いた配分が細かい許容誤差でも Kohlberg 基準を満たすかを確かめる ([`kohlberg_at`])。

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::feasibility_violation;
use crate::game::excess_table;
use crate::game::{Coalition, ExplicitGame};
use crate::linalg::Span;
use crate::lp::{self, Cmp, Counter};

/// 配分 `x` での真部分提携の超過と、超過の大きい順の並び (段の読み取りの許容誤差によらない)。
pub(crate) struct SortedExcess {
    excess: Vec<f64>,
    /// 真部分提携 (ビット表現) を超過の大きい順に並べたもの。
    pub(crate) order: Vec<usize>,
}

/// 段の読み取りの結果: 下限に張り付いたプレイヤーと、[`SortedExcess::order`] の段の区切り。
#[derive(Debug, PartialEq)]
pub(crate) struct Reading {
    pub(crate) lower_bound: Vec<usize>,
    /// 各段の終わりの位置 (`order` の添字。その位置は含まない)。
    pub(crate) ends: Vec<usize>,
}

impl SortedExcess {
    pub(crate) fn new(game: &ExplicitGame, x: &[f64]) -> SortedExcess {
        let full = (1usize << game.players()) - 1;
        let excess = excess_table(game, x);
        let mut order: Vec<usize> = (1..full).collect();
        order.sort_by(|a, b| excess[*b].total_cmp(&excess[*a]));
        SortedExcess { excess, order }
    }

    /// 段の先頭の超過から `tolerance` 以内を同じ段として、提携の特性ベクトル・`1_N`・下限の単位ベクトルが
    /// `R^n` を張るまで段を読む。張らなければ `None`。
    pub(crate) fn read(
        &self,
        game: &ExplicitGame,
        x: &[f64],
        domain: Domain,
        tolerance: f64,
    ) -> Result<Option<Reading>> {
        let n = game.players();
        let mut span = Span::new(n);
        span.insert(&vec![1.0; n]);
        let lower_bound: Vec<usize> = match domain {
            Domain::Imputation => (0..n)
                .filter(|&i| (x[i] - game.value(Coalition::singleton(i))).abs() <= tolerance)
                .collect(),
            Domain::Preimputation => Vec::new(),
        };
        for &i in &lower_bound {
            span.insert_coalition(Coalition::singleton(i));
        }
        let mut ends = Vec::new();
        let mut start = 0;
        while start < self.order.len() && !span.is_full() {
            let head = self.excess[self.order[start]];
            let mut end = start;
            while end < self.order.len() && self.excess[self.order[end]] >= head - tolerance {
                span.insert_coalition(Coalition(self.order[end] as u64));
                end += 1;
            }
            if end == start {
                // 超過が NaN で比較できず、段が 1 つも進まない (有限な入力からは生じないが、無限ループを避ける)。
                return Err(Error::Numerical(format!(
                    "超過 {head} の段を作れない (比較できない値がある)"
                )));
            }
            ends.push(end);
            start = end;
        }
        Ok(span.is_full().then_some(Reading { lower_bound, ends }))
    }

    /// 読み取った段を上から順に合わせた提携族が、どれも平衡か (浮動小数点数の LP で判定する)。
    ///
    /// 読み取りが仁の超過の段と一致していれば、Kohlberg 基準からどの族も平衡になる。平衡でない族があれば、
    /// その読み取りは仁の段ではない。LP が解けずに判定できない族は平衡として扱う (復元と厳密な判定に進む)。
    pub(crate) fn levels_are_balanced(&self, reading: &Reading, n: usize) -> bool {
        let optional: Vec<Coalition> = reading
            .lower_bound
            .iter()
            .map(|&i| Coalition::singleton(i))
            .collect();
        let mut counter = Counter::default();
        let mut collection: Vec<Coalition> = Vec::new();
        let mut start = 0;
        reading.ends.iter().all(|&end| {
            collection.extend(
                self.order[start..end]
                    .iter()
                    .map(|&mask| Coalition(mask as u64)),
            );
            start = end;
            is_balanced(&collection, &optional, n, &mut counter).unwrap_or(true)
        })
    }
}

/// 浮動小数点数の Kohlberg 判定の結果 ([`kohlberg_at`])。
pub(crate) struct KohlbergCheck {
    pub(crate) satisfied: bool,
    /// 不成立の場合の理由。
    pub(crate) reason: Option<String>,
    /// 平衡性を判定した超過の段の数。
    pub(crate) levels_checked: usize,
    pub(crate) lp_solves: usize,
}

/// 配分 `x` が Kohlberg 基準を満たすかを、許容誤差 `tolerance` で判定する ([`crate::verify::kohlberg`] の本体)。
///
/// 和・個人合理性を `tolerance` で確かめ、超過が先頭から `tolerance` 以内の提携を同じ段として読み、
/// 段を上から順に合わせた提携族がどれも平衡かを LP で判定する。仁では、`x_i <= v({i}) + tolerance` の
/// 1 人提携を重み 0 以上で加えてよい。提携族の特性ベクトルが `R^n` を張った段で打ち切る
/// (張って平衡なら、それ以降の段も平衡になる)。下限にいるプレイヤーの 1 人提携は、張る空間には数えない。
/// `x` の長さがプレイヤー数と異なる・成分が有限でない場合は不成立にする。
pub(crate) fn kohlberg_at(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<KohlbergCheck> {
    let n = game.players();
    if let Some(reason) = feasibility_violation(game, x, domain, tolerance) {
        return Ok(KohlbergCheck {
            satisfied: false,
            reason: Some(reason),
            levels_checked: 0,
            lp_solves: 0,
        });
    }

    let excess = excess_table(game, x);
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
        if end == start {
            // 超過が NaN などで比較できず、段が 1 つも進まない (入口の検証で防いでいるが、無限ループを避ける)。
            return Err(Error::Numerical(format!(
                "超過 {level} の段を作れない (比較できない値がある)"
            )));
        }
        levels_checked += 1;
        if !is_balanced(&collection, &at_lower_bound, n, &mut counter)? {
            return Ok(KohlbergCheck {
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
    Ok(KohlbergCheck {
        satisfied: true,
        reason: None,
        levels_checked,
        lp_solves: counter.solves,
    })
}

/// `collection` に重み 1 以上、`optional` に重み 0 以上を付けて
/// `sum lambda_S 1_S = mu 1_N` とできるかを判定する。
pub(crate) fn is_balanced(
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

    /// 仁の超過の段は上から順に合わせた族がどれも平衡で、仁でない点の段はそうならない。
    #[test]
    fn levels_of_nucleolus_are_balanced_and_levels_of_other_points_are_not() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let balanced = |x: &[f64]| {
            let sorted = SortedExcess::new(&game, x);
            let reading = sorted
                .read(&game, x, Domain::Imputation, 1e-9)
                .unwrap()
                .unwrap();
            sorted.levels_are_balanced(&reading, 3)
        };
        assert!(balanced(&[2.0, 2.0, 0.0]));
        // 超過 0 の段 {3}・{1, 2} は平衡だが、次の段 {1}・{1, 3} を合わせると平衡でない。
        assert!(!balanced(&[1.0, 3.0, 0.0]));
    }

    #[test]
    fn reading_stops_at_incomparable_excess() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let x = [2.0, 2.0, 0.0];
        let mut sorted = SortedExcess::new(&game, &x);
        let head = sorted.order[0];
        sorted.excess[head] = f64::NAN;
        let result = sorted.read(&game, &x, Domain::Preimputation, 1e-6);
        assert!(matches!(result, Err(Error::Numerical(_))), "{result:?}");
    }
}
