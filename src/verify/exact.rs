//! 有理数による仁・プレ仁の厳密な検証。
//!
//! 浮動小数点で求めた配分 `x` を、次の 2 段階で厳密に検証する。
//!
//! 1. 復元 ([`recover_allocation`]): `x` での超過の段 (同じ超過を持つ提携の族) を読み取り、
//!    「同じ段の提携の超過は等しい」「`x(N) = v(N)`」「下限に張り付いた `x_i = v({i})`」を
//!    有理数の連立一次方程式として解き、厳密な配分 `x*` を求める。
//!    `x*` が仁なら、Kohlberg 基準の平衡性からこの連立方程式の解は一意になる。
//! 2. 判定 ([`kohlberg_exact`]): `x*` で全提携の超過を有理数で計算し、超過の各段の提携族が
//!    平衡であることを有理数の単体法 (Bland の規則) で判定する。
//!
//! 判定は復元に使った段の読み取りに依存しない。読み取りを誤っても、誤った `x*` は判定で不合格になる。
//! 仁は一意なので、判定に合格した `x*` が仁であり、`x*` と異なる配分は仁ではない。

use std::cmp::Ordering;

use num_traits::{One, Signed, Zero};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::exact::{ExactGame, Rational, format_rational, to_f64, to_rational};
use crate::game::{Coalition, ExplicitGame};
use crate::linalg::Span;
use crate::rational::simplex;
use crate::rational::{ExactEchelon, Inserted, indicator};
use crate::surplus::excesses;

/// 厳密な検証の結果。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ExactReport {
    /// 判定した配分 (復元に失敗した場合は空)。
    pub allocation: Vec<Rational>,
    pub satisfied: bool,
    pub reason: Option<String>,
    /// 平衡性を判定した超過の段の数。
    pub levels_checked: usize,
}

impl ExactReport {
    fn fail(allocation: Vec<Rational>, reason: String, levels_checked: usize) -> ExactReport {
        ExactReport {
            allocation,
            satisfied: false,
            reason: Some(reason),
            levels_checked,
        }
    }
}

/// 浮動小数点の配分を復元し、有理数で Kohlberg 基準を判定する。
///
/// ゲームの値は浮動小数点数の 2 進数の値どおりの有理数として扱う。
pub fn certify(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<ExactReport> {
    certify_exact(&ExactGame::from_explicit(game)?, x, domain)
}

/// 有理数で構築したゲーム `exact` について、浮動小数点の配分 `x` を検証する。
///
/// 段の読み取り (復元) には `exact` の値を浮動小数点数にしたゲームを使い、判定は `exact` の値で行う。
pub fn certify_exact(exact: &ExactGame, x: &[f64], domain: Domain) -> Result<ExactReport> {
    let game = exact.to_explicit()?;
    let tolerance = 1e-6 * game.max_abs_value().max(1.0);
    match recover_allocation(&game, exact, x, domain, tolerance)? {
        Some(allocation) => Ok(kohlberg_exact(exact, &allocation, domain)),
        None => Ok(ExactReport::fail(
            Vec::new(),
            "超過の段から厳密な配分を一意に復元できない".into(),
            0,
        )),
    }
}

/// 浮動小数点の配分 `x` の超過の段から、厳密な配分を復元する。一意に定まらなければ `None`。
///
/// 未知数は `x_1..x_n` と各段の超過 `alpha_k`。超過の大きい段から順に、段の提携を全て方程式
/// `x(S) + alpha_k = v(S)` として加え、提携の特性ベクトルが `R^n` を張った段で止める。
pub fn recover_allocation(
    game: &ExplicitGame,
    exact: &ExactGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<Option<Vec<Rational>>> {
    let n = game.players();
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    let full = (1usize << n) - 1;
    let excess = excesses(game, x);
    let mut order: Vec<usize> = (1..full).collect();
    order.sort_by(|a, b| excess[*b].total_cmp(&excess[*a]));

    // 段に分ける (段の先頭の超過から tolerance 以内を同じ段とする)。
    let mut groups: Vec<Vec<usize>> = Vec::new();
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
    let mut start = 0;
    while start < order.len() && !span.is_full() {
        let head = excess[order[start]];
        let mut end = start;
        while end < order.len() && excess[order[end]] >= head - tolerance {
            span.insert_coalition(Coalition(order[end] as u64));
            end += 1;
        }
        groups.push(order[start..end].to_vec());
        start = end;
    }
    if !span.is_full() {
        return Ok(None);
    }

    // 未知数 (x_1..x_n, alpha_1..alpha_K) と右辺からなる行を加える。
    let unknowns = n + groups.len();
    let mut echelon = ExactEchelon::default();
    let mut push = |mut row: Vec<Rational>, rhs: Rational| -> bool {
        row.push(rhs);
        !matches!(echelon.insert(row, unknowns), Inserted::Inconsistent)
    };
    let grand = Coalition::grand(n);
    let mut efficiency = indicator(grand, n);
    efficiency.resize(unknowns, Rational::zero());
    if !push(efficiency, exact.value(grand).clone()) {
        return Ok(None);
    }
    for &i in &lower_bound {
        let mut row = vec![Rational::zero(); unknowns];
        row[i] = Rational::one();
        if !push(row, exact.value(Coalition::singleton(i)).clone()) {
            return Ok(None);
        }
    }
    for (k, group) in groups.iter().enumerate() {
        for &mask in group {
            let coalition = Coalition(mask as u64);
            let mut row = indicator(coalition, n);
            row.resize(unknowns, Rational::zero());
            row[n + k] = Rational::one();
            if !push(row, exact.value(coalition).clone()) {
                return Ok(None);
            }
        }
    }
    if echelon.rank() != unknowns {
        return Ok(None);
    }
    // 全ての列がピボットなので、既約な行階段形の右辺が解になる。
    let mut solution = vec![Rational::zero(); unknowns];
    for (pivot, row) in &echelon.rows {
        solution[*pivot] = row[unknowns].clone();
    }
    solution.truncate(n);
    Ok(Some(solution))
}

/// 有理数の配分 `x` が仁 (プレ仁) であるかを Kohlberg 基準で厳密に判定する。
pub fn kohlberg_exact(game: &ExactGame, x: &[Rational], domain: Domain) -> ExactReport {
    let n = game.players();
    let grand = Coalition::grand(n);
    let total = x.iter().fold(Rational::zero(), |acc, v| acc + v);
    if &total != game.value(grand) {
        return ExactReport::fail(
            x.to_vec(),
            format!(
                "効率性を満たさない: x(N) = {}, v(N) = {}",
                format_rational(&total),
                format_rational(game.value(grand))
            ),
            0,
        );
    }
    let mut at_lower_bound: Vec<Coalition> = Vec::new();
    if domain == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let lower = game.value(Coalition::singleton(i));
            match xi.cmp(lower) {
                Ordering::Less => {
                    return ExactReport::fail(
                        x.to_vec(),
                        format!("個人合理性を満たさない: x_{} < v({{{}}})", i + 1, i + 1),
                        0,
                    );
                }
                Ordering::Equal => at_lower_bound.push(Coalition::singleton(i)),
                Ordering::Greater => {}
            }
        }
    }

    // 全ての真部分提携の超過を有理数で求め、大きい順に並べる。
    let full = (1usize << n) - 1;
    let mut sums = vec![Rational::zero(); full + 1];
    for mask in 1..=full {
        let lowest = mask.trailing_zeros() as usize;
        sums[mask] = &sums[mask & (mask - 1)] + &x[lowest];
    }
    let excess: Vec<Rational> = (0..=full)
        .map(|mask| game.value(Coalition(mask as u64)) - &sums[mask])
        .collect();
    let mut order: Vec<usize> = (1..full).collect();
    order.sort_by(|a, b| excess[*b].cmp(&excess[*a]));

    let mut collection: Vec<Coalition> = Vec::new();
    let mut span = ExactEchelon::default();
    let mut levels_checked = 0;
    let mut start = 0;
    while start < order.len() {
        let level = excess[order[start]].clone();
        let mut end = start;
        while end < order.len() && excess[order[end]] == level {
            let coalition = Coalition(order[end] as u64);
            collection.push(coalition);
            span.insert(indicator(coalition, n), n);
            end += 1;
        }
        levels_checked += 1;
        if !is_balanced(&collection, &at_lower_bound, n) {
            return ExactReport::fail(
                x.to_vec(),
                format!("超過 {} 以上の提携族が平衡でない", format_rational(&level)),
                levels_checked,
            );
        }
        // 族が R^n を張って平衡なら、それ以降の段も平衡 (kohlberg モジュールの説明を参照)。
        if span.rank() == n {
            break;
        }
        start = end;
    }
    ExactReport {
        allocation: x.to_vec(),
        satisfied: true,
        reason: None,
        levels_checked,
    }
}

/// `collection` に重み 1 以上、`optional` に重み 0 以上を付けて `sum lambda_S 1_S = mu 1_N` とできるか。
///
/// `lambda_S = 1 + z_S` (`z_S >= 0`) と置き換え、各プレイヤー `i` について
/// `sum_{S ∋ i} z_S + sum_{T ∋ i} lambda_T - mu = -|{S in collection : i in S}|` の非負解を探す。
fn is_balanced(collection: &[Coalition], optional: &[Coalition], n: usize) -> bool {
    let extra: Vec<Coalition> = optional
        .iter()
        .filter(|c| !collection.contains(c))
        .copied()
        .collect();
    let columns: Vec<Coalition> = collection.iter().chain(&extra).copied().collect();
    let mut rows = Vec::with_capacity(n);
    let mut rhs = Vec::with_capacity(n);
    for i in 0..n {
        let mut row: Vec<Rational> = columns
            .iter()
            .map(|c| {
                if c.contains(i) {
                    Rational::one()
                } else {
                    Rational::zero()
                }
            })
            .collect();
        row.push(-Rational::one()); // mu
        rows.push(row);
        let count = collection.iter().filter(|c| c.contains(i)).count();
        rhs.push(-Rational::from_integer(num_bigint::BigInt::from(count)));
    }
    simplex::is_feasible(&rows, &rhs)
}

/// `a` と `b` の差の最大値 (有理数で計算し、浮動小数点数で返す)。
pub fn max_difference(a: &[Rational], b: &[f64]) -> Result<f64> {
    let mut worst = Rational::zero();
    for (x, y) in a.iter().zip(b) {
        let difference = (x - to_rational(*y)?).abs();
        if difference > worst {
            worst = difference;
        }
    }
    Ok(to_f64(&[worst])[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nucleolus;

    fn r(numerator: i64, denominator: i64) -> Rational {
        Rational::new(numerator.into(), denominator.into())
    }

    #[test]
    fn recovers_exact_ferguson_nucleolus() {
        // 文献の例 (tests/literature.rs): 仁は (8/3, 2/3, 5/3)。
        let game = ExplicitGame::from_lex(&[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0]).unwrap();
        let x = nucleolus::nucleolus(&game).unwrap().allocation;
        let report = certify(&game, &x, Domain::Imputation).unwrap();
        assert!(report.satisfied, "{report:?}");
        assert_eq!(report.allocation, vec![r(8, 3), r(2, 3), r(5, 3)]);
    }

    #[test]
    fn rejects_non_nucleolus_exactly() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let exact = ExactGame::from_explicit(&game).unwrap();
        assert!(kohlberg_exact(&exact, &[r(2, 1), r(2, 1), r(0, 1)], Domain::Imputation).satisfied);
        let report = kohlberg_exact(&exact, &[r(1, 1), r(3, 1), r(0, 1)], Domain::Imputation);
        assert!(!report.satisfied);
        // 1/10^12 ずらしただけでも厳密には不合格になる。
        let tiny = r(1, 1_000_000_000_000);
        let report = kohlberg_exact(
            &exact,
            &[r(2, 1) + &tiny, r(2, 1) - &tiny, r(0, 1)],
            Domain::Imputation,
        );
        assert!(!report.satisfied);
    }
}
