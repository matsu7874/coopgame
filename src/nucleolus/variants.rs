//! 超過の測り方を変えた仁 (per capita 仁・比例仁・modiclus・disruption nucleolus・anti-nucleolus) を求める。
//! anti-nucleolus 以外は、一般化した逐次 LP で求める。
//!
//! 仁は「提携の不満 (超過) のベクトルを辞書式に最小化する配分」だった。不満をアフィン関数
//!
//! ```text
//! f_k(x) = (c_k - a_k . x) / w_k    (w_k > 0)
//! ```
//!
//! に一般化しても、逐次 LP (Kopelowitz 方式) はそのまま使える。
//!
//! - 第 k 段では、未確定の不満の最大値 `t` を最小化する: `c_k - a_k . x <= w_k t`。
//! - 最適値 `t*` を達成する不満のうち、最適解集合全体で `t*` に固定されるものを確定させる。
//!   確定の判定は、最適解集合の上で `a_k . x` を最大化する LP で行う。
//! - 確定した不満の係数 `a_k` が張る空間に入った不満は、以降は定数なので外す。
//! - 張る空間が `R^n` になれば配分は一意に決まる。不満が尽きても `R^n` にならなければ、
//!   辞書式最小は一意ではないのでエラーを返す。
//!
//! 不満の選び方 (CoopGame 0.2.2 の `perCapitaNucleolus`・`proportionalNucleolus`・`modiclus` と同じ):
//!
//! | 関数 | 不満 | 領域 |
//! |---|---|---|
//! | [`per_capita_nucleolus`] | `e(S, x) / abs(S)` (空でない真部分提携) | 指定 (CoopGame は配分) |
//! | [`proportional_nucleolus`] | `e(S, x) / v(S)` (`v(S) > 0` の真部分提携)。`v(S) = 0` の提携は制約 `x(S) >= 0` | 指定 (CoopGame は配分)。非負のゲームに限る |
//! | [`modiclus`] | `e(S, x) - e(T, x)` (空でない真部分提携 `S != T` の組) | 準配分 |
//! | [`disruption_nucleolus`] | `e(S, x) / b(S)`、`b(S) = v(N) - v(S) - v(N \ S)` (`b(S) > 0` の真部分提携) | コア (空でないゲームに限る) |
//!
//! disruption nucleolus (Littlechild & Vaidya 1976) は、提携 `S` の「抜ける傾向」
//! `(x(N \ S) - v(N \ S)) / (x(S) - v(S))` の大きい順に辞書式に最小化した配分である。
//! コアの上では `x(N \ S) = v(N) - x(S)` なので、これは `(x(S) - v(S)) / b(S)` を小さい順に
//! 辞書式に最大化すること、すなわち上の不満を辞書式に最小化することと同じになる (CoopGame と同じ定式化)。
//!
//! anti-prenucleolus は、超過を小さい順に並べたベクトルを準配分の上で辞書式に最大化した配分である。
//! 双対ゲーム `v*(S) = v(N) - v(N \ S)` では `e_{v*}(S, x) = -e_v(N \ S, x)` なので、
//! anti-prenucleolus は双対ゲームのプレ仁に一致する (Funaki & Meinhardt 2006)。
//! プレ仁は一般に双対ゲームのプレ仁と一致しないので、anti-prenucleolus はプレ仁とは別の解である。
//! anti-nucleolus は、同じ最大化を anti-imputation の集合 `{x : x(N) = v(N), x_i >= M_i}`
//! (`M_i = v(N) - v(N \ {i})`、双対ゲームの配分集合) の上で行った配分で、双対ゲームの仁に一致する。
//!
//! 不満の数は per capita 仁・比例仁で `2^n`、modiclus で約 `4^n` なので、明示的に全ての行を LP に入れる。

use microlp::{Problem, Variable};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::check_imputation_set;
use crate::game::{Coalition, ExplicitGame, default_tolerance};
use crate::linalg::{Span, indicator};
use crate::lp::{self, Cmp, Counter};
use crate::solution::Guarantee;

/// [`modiclus`] が扱うプレイヤー数の上限 (不満の数は `(2^n - 2)(2^n - 3)`)。
pub const MAX_MODICLUS_PLAYERS: usize = 7;

/// 不満 `f(x) = (constant - coefficients . x) / weight`。`weight` は正。
#[derive(Clone, Debug, PartialEq)]
pub struct Complaint {
    pub coefficients: Vec<f64>,
    pub constant: f64,
    pub weight: f64,
}

impl Complaint {
    pub fn value(&self, x: &[f64]) -> f64 {
        (self.constant - dot(&self.coefficients, x)) / self.weight
    }
}

/// 配分の集合: `sum x = total`、`x_i >= lower_i` (下限がある場合)、`a . x >= b` の制約。
#[derive(Clone, Debug, PartialEq)]
pub struct Feasible {
    pub total: f64,
    pub lower: Option<Vec<f64>>,
    /// `(a, b)` で `a . x >= b`。
    pub constraints: Vec<(Vec<f64>, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct LexicographicResult {
    pub allocation: Vec<f64>,
    /// 各段の不満の最大値 (非増加列)。
    pub levels: Vec<f64>,
    pub lp_solves: usize,
    /// 逐次 LP は [`Guarantee::Exact`]。
    pub guarantee: Guarantee,
}

/// 不満のベクトルを辞書式に最小化する配分を求める。
pub fn lexicographic_minimum(
    players: usize,
    feasible: &Feasible,
    complaints: &[Complaint],
    tolerance: f64,
) -> Result<LexicographicResult> {
    if players == 0 {
        return Err(Error::InvalidArgument("プレイヤーが 0 人".into()));
    }
    if complaints.iter().any(|c| {
        c.coefficients.len() != players
            || !c.weight.is_finite()
            || c.weight <= 0.0
            || !c.constant.is_finite()
    }) {
        return Err(Error::InvalidArgument(
            "不満の係数の長さがプレイヤー数と異なるか、重みが正でない".into(),
        ));
    }
    let mut solver = Lexicographic {
        n: players,
        feasible,
        complaints,
        tolerance,
        open: (0..complaints.len()).collect(),
        fixed: Vec::new(),
        span: Span::new(players),
        counter: Counter::default(),
    };
    solver.span.insert(&vec![1.0; players]);
    solver.drop_determined();
    solver.run()
}

struct Lexicographic<'a> {
    n: usize,
    feasible: &'a Feasible,
    complaints: &'a [Complaint],
    tolerance: f64,
    /// 未確定の不満の番号。
    open: Vec<usize>,
    /// 確定した不満と、その値。
    fixed: Vec<(usize, f64)>,
    span: Span,
    counter: Counter,
}

impl Lexicographic<'_> {
    fn run(mut self) -> Result<LexicographicResult> {
        let mut levels = Vec::new();
        let mut allocation = match self.open.is_empty() {
            true => self.any_feasible()?,
            false => Vec::new(),
        };
        while !self.open.is_empty() {
            let (level, x) = self.minimize_max_complaint()?;
            levels.push(level);
            self.fix_tight(level, &x)?;
            self.drop_determined();
            allocation = x;
        }
        if !self.span.is_full() && !self.is_unique()? {
            return Err(Error::InvalidArgument(
                "不満の係数が R^n を張らず、辞書式最小の配分が一意に定まらない".into(),
            ));
        }
        Ok(LexicographicResult {
            allocation,
            levels,
            lp_solves: self.counter.solves,
            guarantee: Guarantee::Exact,
        })
    }

    /// 係数が確定済みの空間に入った不満を外す (実行可能集合の上で定数になる)。
    fn drop_determined(&mut self) {
        let span = &self.span;
        let complaints = self.complaints;
        self.open
            .retain(|&k| !span.contains(&complaints[k].coefficients));
    }

    fn add_domain(&self, problem: &mut Problem, objective: Option<&[f64]>) -> Vec<Variable> {
        let x: Vec<Variable> = (0..self.n)
            .map(|i| {
                let lower = self
                    .feasible
                    .lower
                    .as_ref()
                    .map_or(f64::NEG_INFINITY, |lower| lower[i]);
                let coefficient = objective.map_or(0.0, |a| a[i]);
                problem.add_var(coefficient, (lower, f64::INFINITY))
            })
            .collect();
        lp::add(
            problem,
            x.iter().map(|&var| (var, 1.0)).collect(),
            Cmp::Eq,
            self.feasible.total,
        );
        for (a, b) in &self.feasible.constraints {
            lp::add(problem, terms(&x, a), Cmp::Ge, *b);
        }
        for &(k, level) in &self.fixed {
            let complaint = &self.complaints[k];
            // c - a . x = w * level
            lp::add(
                problem,
                terms(&x, &complaint.coefficients),
                Cmp::Eq,
                complaint.constant - complaint.weight * level,
            );
        }
        x
    }

    fn any_feasible(&mut self) -> Result<Vec<f64>> {
        let mut problem = lp::minimize();
        let x = self.add_domain(&mut problem, None);
        let solution = self
            .counter
            .solve(&problem)?
            .ok_or_else(|| Error::InvalidArgument("配分の集合が空".into()))?;
        Ok(x.iter().map(|&var| solution[var]).collect())
    }

    /// 配分の集合 (確定した等式を含む) が 1 点かを、各座標の最大・最小で確かめる。
    fn is_unique(&mut self) -> Result<bool> {
        for i in 0..self.n {
            let mut direction = vec![0.0; self.n];
            direction[i] = 1.0;
            let mut range = [0.0; 2];
            for (slot, sign) in [1.0, -1.0].into_iter().enumerate() {
                let mut problem = lp::maximize();
                let objective: Vec<f64> = direction.iter().map(|d| sign * d).collect();
                let x = self.add_domain(&mut problem, Some(&objective));
                self.add_open_rows(&mut problem, &x, Row::AtMost(f64::INFINITY));
                match self.counter.solve_bounded(&problem)? {
                    Some(solution) => range[slot] = sign * solution.objective(),
                    None => return Ok(false),
                }
            }
            if range[0] - range[1] > 10.0 * self.tolerance {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn add_open_rows(&self, problem: &mut Problem, x: &[Variable], row: Row) {
        for &k in &self.open {
            let complaint = &self.complaints[k];
            // c - a . x <= w t  <=>  a . x + w t >= c
            let mut row_terms = terms(x, &complaint.coefficients);
            let rhs = match row {
                Row::WithLevel(t) => {
                    row_terms.push((t, complaint.weight));
                    complaint.constant
                }
                Row::AtMost(bound) if bound.is_infinite() => continue,
                Row::AtMost(bound) => complaint.constant - complaint.weight * bound,
            };
            lp::add(problem, row_terms, Cmp::Ge, rhs);
        }
    }

    fn minimize_max_complaint(&mut self) -> Result<(f64, Vec<f64>)> {
        let mut problem = lp::minimize();
        let x = self.add_domain(&mut problem, None);
        let t = problem.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
        self.add_open_rows(&mut problem, &x, Row::WithLevel(t));
        let solution = self
            .counter
            .solve_bounded(&problem)?
            .ok_or_else(|| Error::Numerical("不満の最大値が下に有界でない".into()))?;
        Ok((solution[t], x.iter().map(|&var| solution[var]).collect()))
    }

    /// 不満の最大値が `level` 以下の配分の中で `a . x` を最大化する。
    fn maximize_direction(&mut self, a: &[f64], level: f64) -> Result<(f64, Vec<f64>)> {
        let mut problem = lp::maximize();
        let x = self.add_domain(&mut problem, Some(a));
        // 最適値の丸め誤差で実行不可能にならないよう、許容誤差の 1% だけ緩める。
        self.add_open_rows(&mut problem, &x, Row::AtMost(level + 0.01 * self.tolerance));
        let solution = self
            .counter
            .solve_bounded(&problem)?
            .ok_or_else(|| Error::Numerical("確定判定の LP が非有界".into()))?;
        Ok((
            solution.objective(),
            x.iter().map(|&var| solution[var]).collect(),
        ))
    }

    fn fix_tight(&mut self, level: f64, x: &[f64]) -> Result<()> {
        let tight: Vec<usize> = self
            .open
            .iter()
            .copied()
            .filter(|&k| (self.complaints[k].value(x) - level).abs() <= self.tolerance)
            .collect();
        let mut not_fixed = vec![false; tight.len()];
        let mut progressed = false;
        for position in 0..tight.len() {
            let k = tight[position];
            let complaint = &self.complaints[k];
            if not_fixed[position] || self.span.contains(&complaint.coefficients) {
                continue;
            }
            let fixed_value = complaint.constant - complaint.weight * level;
            let (max_value, witness) = self.maximize_direction(&complaint.coefficients, level)?;
            if max_value <= fixed_value + self.tolerance * complaint.weight {
                self.fixed.push((k, level));
                self.span.insert(&complaint.coefficients);
                progressed = true;
            } else {
                // witness で不満が level 未満になる不満も確定していない。
                for (later, flag) in tight.iter().zip(not_fixed.iter_mut()).skip(position + 1) {
                    if self.complaints[*later].value(&witness) < level - self.tolerance {
                        *flag = true;
                    }
                }
            }
        }
        if !progressed {
            return Err(Error::Numerical(format!(
                "不満の最大値 {level} の段で確定する不満が見つからない (許容誤差 {} を見直す)",
                self.tolerance
            )));
        }
        // 確定した不満は外す。
        let fixed: Vec<usize> = self.fixed.iter().map(|&(k, _)| k).collect();
        self.open.retain(|k| !fixed.contains(k));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Row {
    WithLevel(Variable),
    AtMost(f64),
}

fn terms(x: &[Variable], a: &[f64]) -> Vec<(Variable, f64)> {
    x.iter()
        .zip(a)
        .filter(|(_, coefficient)| **coefficient != 0.0)
        .map(|(&var, &coefficient)| (var, coefficient))
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(p, q)| p * q).sum()
}

fn proper_coalitions(game: &ExplicitGame) -> impl Iterator<Item = Coalition> {
    (1..game.grand().0).map(Coalition)
}

fn feasible_set(game: &ExplicitGame, domain: Domain) -> Feasible {
    Feasible {
        total: game.value(game.grand()),
        lower: (domain == Domain::Imputation).then(|| game.singleton_values()),
        constraints: Vec::new(),
    }
}

/// 超過の complaint `e(S, x) / weight`。
fn excess_complaint(game: &ExplicitGame, coalition: Coalition, weight: f64) -> Complaint {
    Complaint {
        coefficients: indicator(coalition, game.players()),
        constant: game.value(coalition),
        weight,
    }
}

/// 全ての空でない真部分提携の超過 (重み 1) を不満にした場合。仁 (プレ仁) に一致する。
#[cfg(test)]
fn nucleolus_by_complaints(game: &ExplicitGame, domain: Domain) -> Result<LexicographicResult> {
    check_imputation_set(game, domain, default_tolerance(game))?;
    let complaints: Vec<Complaint> = proper_coalitions(game)
        .map(|s| excess_complaint(game, s, 1.0))
        .collect();
    lexicographic_minimum(
        game.players(),
        &feasible_set(game, domain),
        &complaints,
        default_tolerance(game),
    )
}

/// per capita 仁: 超過を提携の人数で割った `e(S, x) / abs(S)` を辞書式に最小化する。
pub fn per_capita_nucleolus(game: &ExplicitGame, domain: Domain) -> Result<LexicographicResult> {
    check_imputation_set(game, domain, default_tolerance(game))?;
    let complaints: Vec<Complaint> = proper_coalitions(game)
        .map(|s| excess_complaint(game, s, s.len() as f64))
        .collect();
    lexicographic_minimum(
        game.players(),
        &feasible_set(game, domain),
        &complaints,
        default_tolerance(game),
    )
}

/// disruption nucleolus (Littlechild & Vaidya 1976): 提携の抜ける傾向を辞書式に最小化するコアの点。
///
/// コアが空のゲームではエラーを返す。`b(S) = v(N) - v(S) - v(N \ S) = 0` の提携は、コアの上で
/// `x(S) = v(S)` に固定されるので不満にしない。
pub fn disruption_nucleolus(game: &ExplicitGame) -> Result<LexicographicResult> {
    let n = game.players();
    let tolerance = default_tolerance(game);
    if !crate::nucleolus::has_nonempty_core(game)? {
        return Err(Error::InvalidArgument(
            "disruption nucleolus はコアが空でないゲームに限る".into(),
        ));
    }
    let total = game.value(game.grand());
    let mut feasible = Feasible {
        total,
        lower: None,
        constraints: Vec::new(),
    };
    let mut complaints = Vec::new();
    for s in proper_coalitions(game) {
        feasible.constraints.push((indicator(s, n), game.value(s)));
        let gap = total - game.value(s) - game.value(Coalition(game.grand().0 & !s.0));
        if gap > tolerance {
            complaints.push(excess_complaint(game, s, gap));
        }
    }
    lexicographic_minimum(n, &feasible, &complaints, tolerance)
}

/// anti-prenucleolus: 超過を小さい順に並べたベクトルを準配分の上で辞書式に最大化した配分。
/// 双対ゲームのプレ仁として求める (Funaki & Meinhardt 2006)。`levels` は双対ゲームの超過の段。
pub fn anti_prenucleolus(game: &ExplicitGame) -> Result<crate::nucleolus::NucleolusResult> {
    crate::nucleolus::prenucleolus(&game.dual())
}

/// anti-nucleolus: anti-imputation の集合 `{x(N) = v(N), x_i >= v(N) - v(N \ {i})}` の上での anti-prenucleolus。
/// 双対ゲームの仁として求める。この集合が空 (`sum_i (v(N) - v(N \ {i})) > v(N)`) ならエラーを返す。
pub fn anti_nucleolus(game: &ExplicitGame) -> Result<crate::nucleolus::NucleolusResult> {
    let utopia: f64 = crate::compromise::utopia_payoffs(game).iter().sum();
    let total = game.value(game.grand());
    if utopia > total + default_tolerance(game) {
        return Err(Error::InvalidArgument(format!(
            "anti-imputation の集合が空: 理想の支払い v(N) - v(N \\ {{i}}) の和 {utopia} が v(N) = {total} を超える"
        )));
    }
    crate::nucleolus::nucleolus(&game.dual())
}

/// 比例仁: 超過を提携の値で割った `e(S, x) / v(S)` を辞書式に最小化する。
///
/// 非負のゲーム (`v(S) >= 0`) に限る。`v(S) = 0` の提携は不満にせず、制約 `x(S) >= 0` にする
/// (CoopGame と同じ扱い)。
pub fn proportional_nucleolus(game: &ExplicitGame, domain: Domain) -> Result<LexicographicResult> {
    check_imputation_set(game, domain, default_tolerance(game))?;
    if game.values().iter().any(|v| *v < 0.0) {
        return Err(Error::InvalidArgument(
            "比例仁は非負のゲーム (全ての v(S) >= 0) に限る".into(),
        ));
    }
    let mut feasible = feasible_set(game, domain);
    let mut complaints = Vec::new();
    for s in proper_coalitions(game) {
        let value = game.value(s);
        if value > 0.0 {
            complaints.push(excess_complaint(game, s, value));
        } else {
            feasible
                .constraints
                .push((indicator(s, game.players()), 0.0));
        }
    }
    lexicographic_minimum(
        game.players(),
        &feasible,
        &complaints,
        default_tolerance(game),
    )
}

/// modiclus (修正仁): 超過の差 `e(S, x) - e(T, x)` の組を辞書式に最小化する準配分。
///
/// `S`, `T` は空でない真部分提携で `S != T` (CoopGame 0.2.2 の `modiclus` と同じ組)。
pub fn modiclus(game: &ExplicitGame) -> Result<LexicographicResult> {
    let n = game.players();
    if n > MAX_MODICLUS_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_MODICLUS_PLAYERS,
        });
    }
    let coalitions: Vec<Coalition> = proper_coalitions(game).collect();
    let mut complaints = Vec::with_capacity(coalitions.len() * coalitions.len());
    for &s in &coalitions {
        for &t in &coalitions {
            if s == t {
                continue;
            }
            // e(S) - e(T) = v(S) - v(T) - (1_S - 1_T) . x
            let coefficients = indicator(s, n)
                .iter()
                .zip(indicator(t, n))
                .map(|(a, b)| a - b)
                .collect();
            complaints.push(Complaint {
                coefficients,
                constant: game.value(s) - game.value(t),
                weight: 1.0,
            });
        }
    }
    lexicographic_minimum(
        n,
        &feasible_set(game, Domain::Preimputation),
        &complaints,
        default_tolerance(game),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generators, nucleolus};

    fn assert_close(actual: &[f64], expected: &[f64], tolerance: f64) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() <= tolerance, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn unit_weights_give_the_nucleolus() {
        for kind in 1..=4 {
            for n in [3, 5, 7] {
                let game = generators::bnf(kind, n, 2).unwrap();
                let tolerance = 1e-6 * game.max_abs_value().max(1.0);
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    let expected = match nucleolus::nucleolus_with(
                        &game,
                        nucleolus::Options::new(&game, domain),
                    ) {
                        Ok(result) => result.allocation,
                        Err(Error::EmptyImputationSet) => continue,
                        Err(err) => panic!("{err}"),
                    };
                    let actual = nucleolus_by_complaints(&game, domain).unwrap().allocation;
                    assert_close(&actual, &expected, tolerance);
                }
            }
        }
    }

    #[test]
    fn proportional_rejects_negative_games() {
        let game = ExplicitGame::from_lex(&[-1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0]).unwrap();
        assert!(proportional_nucleolus(&game, Domain::Imputation).is_err());
    }

    #[test]
    fn non_spanning_complaints_are_rejected() {
        // 比例仁で v(S) > 0 が N だけなら、配分集合全体が辞書式最小になる。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]).unwrap();
        let err = proportional_nucleolus(&game, Domain::Imputation).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err}");
    }

    #[test]
    fn disruption_nucleolus_matches_coopgame_example() {
        // CoopGame 0.2.2 の disruptionNucleolus のヘルプの例 (掲載値は小数 6 桁に丸めた値)。
        let game = ExplicitGame::from_lex(&[
            0.0, 0.0, 0.0, 0.0, 2.0, 3.0, 4.0, 1.0, 3.0, 2.0, 8.0, 11.0, 6.5, 9.5, 14.0,
        ])
        .unwrap();
        let x = disruption_nucleolus(&game).unwrap().allocation;
        for (a, e) in x.iter().zip([3.193548, 4.754839, 2.129032, 3.922581]) {
            assert!((a - e).abs() < 1e-6, "{x:?}");
        }
        // コアが空のゲームは対象外。
        let empty = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 80.0, 80.0, 80.0, 90.0]).unwrap();
        assert!(disruption_nucleolus(&empty).is_err());
    }

    #[test]
    fn anti_prenucleolus_of_bankruptcy_example() {
        // Funaki & Meinhardt (2006) Example 4.1: E = 150、d = (60, 80, 120)。
        // 破産ゲーム v のプレ仁と、その双対 w = v* の anti-prenucleolus はともに (30, 40, 80)。
        let v = ExplicitGame::from_lex(&[0.0, 0.0, 10.0, 30.0, 70.0, 90.0, 150.0]).unwrap();
        let w = ExplicitGame::from_lex(&[60.0, 80.0, 120.0, 140.0, 150.0, 150.0, 150.0]).unwrap();
        assert_eq!(v.dual().values(), w.values());
        assert_eq!(w.dual().values(), v.values());
        let expected = [30.0, 40.0, 80.0];
        let close = |x: &[f64]| x.iter().zip(&expected).all(|(a, e)| (a - e).abs() < 1e-6);
        assert!(close(
            &crate::nucleolus::prenucleolus(&v).unwrap().allocation
        ));
        assert!(close(&anti_prenucleolus(&w).unwrap().allocation));
        assert!(close(&anti_nucleolus(&w).unwrap().allocation));
        // w のプレ仁は (30, 40, 80) ではない (プレ仁は双対をとると一般に変わる)。
        assert!(!close(
            &crate::nucleolus::prenucleolus(&w).unwrap().allocation
        ));
    }

    #[test]
    fn anti_prenucleolus_maximizes_the_smallest_excess() {
        // 最小の超過は、anti-prenucleolus で最大になる (定義の第 1 段)。プレ仁や Shapley 値と比べる。
        for seed in 0..5 {
            let game = crate::generators::bnf(1, 5, seed).unwrap();
            let min_excess = |x: &[f64]| {
                proper_coalitions(&game)
                    .map(|s| crate::surplus::excess(&game, s, x))
                    .fold(f64::INFINITY, f64::min)
            };
            let anti = anti_prenucleolus(&game).unwrap().allocation;
            let pre = crate::nucleolus::prenucleolus(&game).unwrap().allocation;
            let shapley = crate::values::shapley(&game);
            assert!(min_excess(&anti) >= min_excess(&pre) - 1e-6);
            assert!(min_excess(&anti) >= min_excess(&shapley) - 1e-6);
        }
    }
}
