//! オラクル型ゲームの仁・プレ仁・最小コア (逐次 LP + 制約生成)。
//!
//! 手順は [`crate::nucleolus`] と同じだが、全提携を列挙しない。
//!
//! - 違反する提携は分離オラクル [`Separation::violated`] で探す。超過順に列挙できるゲーム
//!   ([`crate::game::oracle::OracleGame`]) は、超過の大きい順に探すことになる。
//! - 超過が確定した提携 (確定済みの提携の特性ベクトルが張る空間に入る提携) は、その場で判定して除く。
//! - 段ごとに確定させる提携は、LP に入っている行の中から探す。行を制限した LP の最適面は
//!   真の最適面を含み、制限した LP にも最適面全体で等号が成り立つ行がある (LP の双対性)。
//!   その行は真の最適面でも超過が一定なので、確定させてよい。これにより各段で次元が 1 以上増える。
//!
//! 値の大きさによらず同じ精度で解けるよう、[`Separation::value_scale`] 以下で最大の 2 のべき乗で
//! 値を割ったゲームを解き、結果を元の大きさに戻す ([`crate::nucleolus`] と同じ正規化)。
//!
//! LP の行にするのは、値が有限の提携だけである。値が分からない提携 (値が分かる提携だけを持つ
//! [`crate::io::PartialGame`] では NaN) はそのゲームの制約として存在しないので、最初の LP に入れる
//! 1 人提携とその補集合や、確定判定で目的にする提携の補集合であっても、行にしない。
//! 分離オラクルが値の有限でない提携を返したときは、契約違反として [`Error::InvalidArgument`] を返す。
//!
//! 結果の保証は、オラクルが全ての真部分提携について答えるか ([`Separation::covers_all_coalitions`]) で決まる。
//! 答えるなら [`Guarantee::Exact`]、一部の提携だけなら (サンプルした提携だけを持つ
//! [`crate::nucleolus::sampled::SampledGame`] など) 元のゲームの仁とは限らないので [`Guarantee::Approximate`]。

use std::collections::HashSet;

use microlp::{Problem, Solution, Variable};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::PlayerSet;
use crate::game::allocation::lower_bounds;
use crate::game::oracle::Separation;
use crate::game::scale::{Scaled, normalized_tolerance, normalizer, positive_scale};
use crate::linalg::Span;
use crate::lp::{self, Cmp, Counter};
use crate::nucleolus::{LeastCore, NucleolusResult};
use crate::solution::Guarantee;

/// ゲームの値の大きさに合わせた既定の許容誤差: [`Separation::value_scale`] の `1e-7` 倍
/// (値の大きさが 0 なら `1e-7`)。
pub fn default_tolerance<G: Separation + ?Sized>(game: &G) -> f64 {
    1e-7 * positive_scale(game.value_scale())
}

/// 仁。保証は、オラクルが全提携について答えれば [`Guarantee::Exact`]、そうでなければ [`Guarantee::Approximate`]。
pub fn nucleolus<G: Separation + ?Sized>(game: &G) -> Result<NucleolusResult> {
    nucleolus_with(game, Domain::Imputation, default_tolerance(game))
}

/// プレ仁。保証は [`nucleolus`] と同じ。
pub fn prenucleolus<G: Separation + ?Sized>(game: &G) -> Result<NucleolusResult> {
    nucleolus_with(game, Domain::Preimputation, default_tolerance(game))
}

/// `tolerance` はゲームの値と同じ単位。配分集合の扱いは [`crate::nucleolus::Tolerance`] の説明と同じ。
///
/// `tolerance` が有限で 0 以上でないとき、またはゲームの値の大きさで割ると有限でない (値に比べて大きすぎる) ときは
/// [`Error::InvalidArgument`] を返す。
/// 結果は許容誤差の範囲で成り立つ。保証の種類 ([`nucleolus`] と同じ) は許容誤差の大きさを反映しない。
///
/// 配分や各段の最大超過が `f64` の範囲を超える (値が `f64` の上限に近い) ときは [`Error::Numerical`] を返す。
pub fn nucleolus_with<G: Separation + ?Sized>(
    game: &G,
    domain: Domain,
    tolerance: f64,
) -> Result<NucleolusResult> {
    let divisor = normalizer(game.value_scale());
    let tolerance = normalized_tolerance(tolerance, divisor)?;
    let normalized = Scaled::new(game, divisor);
    Sequential::new(&normalized, domain, tolerance)?
        .run()?
        .scaled_by(divisor)
}

/// 最小コア。1 人ゲームの `epsilon` は `f64::NEG_INFINITY` ([`LeastCore::epsilon`])。保証は [`nucleolus`] と同じ。
///
/// 2 人以上で、配分や `epsilon` が `f64` の範囲を超えるときは [`Error::Numerical`] を返す。
pub fn least_core<G: Separation + ?Sized>(game: &G, domain: Domain) -> Result<LeastCore> {
    let divisor = normalizer(game.value_scale());
    let normalized = Scaled::new(game, divisor);
    let tolerance = default_tolerance(game) / divisor;
    let mut solver = Sequential::new(&normalized, domain, tolerance)?;
    if solver.n == 1 {
        return Ok(LeastCore {
            epsilon: f64::NEG_INFINITY,
            allocation: vec![game.value(&PlayerSet::grand(1))],
            guarantee: solver.guarantee,
        });
    }
    let (epsilon, allocation) = solver.minimize_max_excess()?;
    let epsilon = epsilon * divisor;
    let allocation: Vec<f64> = allocation.iter().map(|v| v * divisor).collect();
    super::check_representable(&[epsilon], "最小コアの epsilon ")?;
    super::check_representable(&allocation, "配分")?;
    Ok(LeastCore {
        epsilon,
        allocation,
        guarantee: solver.guarantee,
    })
}

/// オラクルが扱う提携の範囲から決まる保証。
fn guarantee_of<G: Separation + ?Sized>(game: &G) -> Guarantee {
    if game.covers_all_coalitions() {
        Guarantee::Exact
    } else {
        Guarantee::Approximate
    }
}

#[derive(Clone, Copy)]
enum RowForm {
    /// `x(S) + t >= v(S)`
    WithLevel(Variable),
    /// `x(S) >= v(S) - bound`
    AtMost(f64),
}

struct Sequential<'a, G: Separation + ?Sized> {
    game: &'a G,
    tolerance: f64,
    /// 結果に付ける保証 ([`guarantee_of`])。
    guarantee: Guarantee,
    n: usize,
    /// 各プレイヤーの下限 (`Domain::Imputation` のみ)。
    lower: Option<Vec<f64>>,
    fixed: Vec<(PlayerSet, f64)>,
    span: Span,
    counter: Counter,
    rows_added: usize,
    pool: Vec<PlayerSet>,
    in_pool: HashSet<PlayerSet>,
}

impl<'a, G: Separation + ?Sized> Sequential<'a, G> {
    fn new(game: &'a G, domain: Domain, tolerance: f64) -> Result<Sequential<'a, G>> {
        let n = game.players();
        if n == 0 {
            return Err(Error::InvalidArgument("プレイヤーがいない".into()));
        }
        let grand = PlayerSet::grand(n);
        if !game.value(&grand).is_finite() {
            return Err(Error::InvalidArgument(
                "全体提携の値が有限の数ではない".into(),
            ));
        }
        if domain == Domain::Imputation
            && (0..n).any(|i| !game.value(&PlayerSet::singleton(n, i)).is_finite())
        {
            return Err(Error::InvalidArgument(
                "配分集合で解くには、1 人提携の値が全て有限の数である必要がある".into(),
            ));
        }
        let lower = lower_bounds(game, std::slice::from_ref(&grand), domain, tolerance)?;
        let mut span = Span::new(n);
        span.insert(&grand.indicator());
        let mut solver = Sequential {
            game,
            tolerance,
            guarantee: guarantee_of(game),
            n,
            lower,
            fixed: vec![(grand.clone(), game.value(&grand))],
            span,
            counter: Counter::default(),
            rows_added: 0,
            pool: Vec::new(),
            in_pool: HashSet::new(),
        };
        // 1 人提携とその補集合があれば、最初の LP は有界になる (値が分からないものは remember が除く)。
        for i in 0..n {
            let single = PlayerSet::singleton(n, i);
            solver.remember(single.complement());
            solver.remember(single);
        }
        Ok(solver)
    }

    fn run(&mut self) -> Result<NucleolusResult> {
        if self.n == 1 {
            return Ok(NucleolusResult::without_lp(
                vec![self.game.value(&PlayerSet::grand(1))],
                self.guarantee,
                self.tolerance,
            ));
        }
        if let Some(point) = self.single_point() {
            return Ok(NucleolusResult::without_lp(
                point,
                self.guarantee,
                self.tolerance,
            ));
        }
        let mut levels = Vec::new();
        let mut allocation = Vec::new();
        while !self.span.is_full() {
            let (level, x) = self.minimize_max_excess()?;
            levels.push(level);
            self.fix_tight_rows(level, &x)?;
            allocation = x;
        }
        Ok(NucleolusResult {
            allocation,
            levels,
            lp_solves: self.counter.solves,
            rows_added: self.rows_added,
            guarantee: self.guarantee,
            tolerance: self.tolerance,
            refinements: 0,
            rational: false,
        })
    }

    /// 下限の和が `v(N)` 以上 (許容誤差以内で空の配分集合を含む) なら、配分集合は 1 点
    /// `x_i = lower_i` で、それが仁 ([`crate::nucleolus`] と同じ扱い)。
    fn single_point(&self) -> Option<Vec<f64>> {
        let lower = self.lower.as_ref()?;
        let singles: f64 = (0..self.n)
            .map(|i| self.game.value(&PlayerSet::singleton(self.n, i)))
            .sum();
        (singles >= self.game.value(&PlayerSet::grand(self.n))).then(|| lower.clone())
    }

    /// 提携を LP の行の候補に加える。値が有限でない提携 (値が分からない提携) はゲームの制約として
    /// 存在しないので加えない。
    fn remember(&mut self, coalition: PlayerSet) {
        if coalition.is_proper()
            && !self.in_pool.contains(&coalition)
            && self.game.value(&coalition).is_finite()
        {
            self.in_pool.insert(coalition.clone());
            self.pool.push(coalition);
        }
    }

    fn is_determined(&self, coalition: &PlayerSet) -> bool {
        self.span.contains(&coalition.indicator())
    }

    fn terms(x: &[Variable], coalition: &PlayerSet) -> Vec<(Variable, f64)> {
        coalition.members().map(|i| (x[i], 1.0)).collect()
    }

    fn add_players(&self, problem: &mut Problem, objective: Option<&PlayerSet>) -> Vec<Variable> {
        (0..self.n)
            .map(|i| {
                let lower = self
                    .lower
                    .as_ref()
                    .map_or(f64::NEG_INFINITY, |lower| lower[i]);
                let coefficient = match objective {
                    Some(s) if s.contains(i) => 1.0,
                    _ => 0.0,
                };
                problem.add_var(coefficient, (lower, f64::INFINITY))
            })
            .collect()
    }

    fn add_fixed(&self, problem: &mut Problem, x: &[Variable]) {
        for (coalition, value) in &self.fixed {
            lp::add(problem, Self::terms(x, coalition), Cmp::Eq, *value);
        }
    }

    fn row(
        &self,
        x: &[Variable],
        form: RowForm,
        coalition: &PlayerSet,
    ) -> (Vec<(Variable, f64)>, f64) {
        let mut terms = Self::terms(x, coalition);
        let value = self.game.value(coalition);
        match form {
            RowForm::WithLevel(t) => {
                terms.push((t, 1.0));
                (terms, value)
            }
            RowForm::AtMost(bound) => (terms, value - bound),
        }
    }

    /// 行を入れて LP を解き、違反する提携を超過の大きい順に追加しながら解き直す。
    fn solve_rows(
        &mut self,
        build: impl Fn(&Self) -> (Problem, Vec<Variable>, RowForm),
        seed: &[PlayerSet],
    ) -> Result<(Solution, Vec<Variable>, RowForm)> {
        for coalition in seed {
            self.remember(coalition.clone());
        }
        let (mut problem, x, form) = build(self);
        let mut in_lp: HashSet<PlayerSet> = HashSet::new();
        for coalition in &self.pool {
            if !self.is_determined(coalition) {
                let (terms, rhs) = self.row(&x, form, coalition);
                lp::add(&mut problem, terms, Cmp::Ge, rhs);
                in_lp.insert(coalition.clone());
            }
        }
        let mut solution = self
            .counter
            .solve_bounded(&problem)?
            .ok_or_else(|| Error::Numerical("最初の LP が非有界".into()))?;
        let slack = 1e-3 * self.tolerance;
        let batch = 4 * self.n;
        loop {
            let values: Vec<f64> = x.iter().map(|&var| solution[var]).collect();
            let bound = match form {
                RowForm::WithLevel(t) => solution[t],
                RowForm::AtMost(bound) => bound,
            };
            let skip =
                |coalition: &PlayerSet| in_lp.contains(coalition) || self.is_determined(coalition);
            let violated: Vec<PlayerSet> = self
                .game
                .violated(&values, bound + slack, &skip, batch)
                .into_iter()
                .map(|(coalition, _)| coalition)
                .collect();
            if violated.is_empty() {
                return Ok((solution, x, form));
            }
            for coalition in violated {
                let (terms, rhs) = self.row(&x, form, &coalition);
                if !rhs.is_finite() {
                    return Err(Error::InvalidArgument(format!(
                        "分離オラクルが値の有限でない提携 {coalition} を返した"
                    )));
                }
                solution = lp::add_to_solution(solution, terms, Cmp::Ge, rhs)?;
                in_lp.insert(coalition.clone());
                self.remember(coalition);
                self.rows_added += 1;
            }
        }
    }

    fn minimize_max_excess(&mut self) -> Result<(f64, Vec<f64>)> {
        let build = |this: &Self| {
            let mut problem = lp::minimize();
            let x = this.add_players(&mut problem, None);
            let t = problem.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
            this.add_fixed(&mut problem, &x);
            (problem, x, RowForm::WithLevel(t))
        };
        let (solution, x, form) = self.solve_rows(build, &[])?;
        let RowForm::WithLevel(t) = form else {
            unreachable!("最大超過の最小化は WithLevel の行を使う")
        };
        Ok((solution[t], x.iter().map(|&var| solution[var]).collect()))
    }

    fn maximize_coalition(&mut self, target: &PlayerSet, level: f64) -> Result<(f64, Vec<f64>)> {
        let bound = level + 0.01 * self.tolerance;
        let build = |this: &Self| {
            let mut problem = lp::maximize();
            let x = this.add_players(&mut problem, Some(target));
            this.add_fixed(&mut problem, &x);
            (problem, x, RowForm::AtMost(bound))
        };
        // 補集合の行は x(target) の上限を直接与える。値が分からなければ remember が除く
        // (除いても、1 人提携とその補集合の行があれば LP は有界)。
        let seed = [target.clone(), target.complement()];
        let (solution, x, _) = self.solve_rows(build, &seed)?;
        Ok((
            solution.objective(),
            x.iter().map(|&var| solution[var]).collect(),
        ))
    }

    fn excess(&self, coalition: &PlayerSet, x: &[f64]) -> f64 {
        self.game.value(coalition) - coalition.sum(x)
    }

    /// LP の行のうち最大超過を達成するものを確定判定し、確定したものを等式に移す。
    ///
    /// 候補は段の LP の解 `x` で許容誤差以内にタイトな行で、`x(S)` が `v(S) - level` を許容誤差より
    /// 大きく超えられなければ確定する。確定値は `v(S) - level` ではなく `x(S)` にする。
    /// 許容誤差以内の差しかない提携は、本当はタイトでなく最適面の上で `x(S) = v(S) - level` を取れないことがあり、
    /// その値で固定すると以後の LP が実行不可能になるためである。`x(S)` で固定すれば、`x` は同じ段の以後の LP と
    /// 次の段の LP の実行可能点であり続ける。本当にタイトな提携では、2 つの値は丸めの範囲で一致する。
    fn fix_tight_rows(&mut self, level: f64, x: &[f64]) -> Result<()> {
        let tight: Vec<PlayerSet> = self
            .pool
            .iter()
            .filter(|s| {
                !self.is_determined(s) && (self.excess(s, x) - level).abs() <= self.tolerance
            })
            .cloned()
            .collect();
        let mut witnesses: Vec<Vec<f64>> = Vec::new();
        let mut progressed = false;
        for target in tight {
            if self.is_determined(&target)
                || witnesses
                    .iter()
                    .any(|y| self.excess(&target, y) < level - self.tolerance)
            {
                continue;
            }
            let tight_sum = self.game.value(&target) - level;
            let (max_sum, witness) = self.maximize_coalition(&target, level)?;
            if max_sum <= tight_sum + self.tolerance {
                self.span.insert(&target.indicator());
                let fixed_sum = target.sum(x);
                self.fixed.push((target, fixed_sum));
                progressed = true;
                if self.span.is_full() {
                    break;
                }
            } else {
                witnesses.push(witness);
            }
        }
        if !progressed {
            return Err(Error::Numerical(format!(
                "最大超過 {level} の段で確定する提携が見つからない (許容誤差 {})",
                self.tolerance
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    #[test]
    fn explicit_oracle_matches_explicit_solver() {
        for kind in [1, 2, 4] {
            for n in 3..=7 {
                for seed in 0..3 {
                    let game = generators::bnf(kind, n, seed).unwrap();
                    for domain in [Domain::Imputation, Domain::Preimputation] {
                        let expected = crate::nucleolus::nucleolus_with(
                            &game,
                            crate::nucleolus::Options::new(domain),
                        )
                        .unwrap()
                        .allocation;
                        let actual = nucleolus_with(&game, domain, default_tolerance(&game))
                            .unwrap()
                            .allocation;
                        let scale = game.max_abs_value().max(1.0);
                        for (a, e) in actual.iter().zip(&expected) {
                            assert!(
                                (a - e).abs() < 1e-6 * scale,
                                "type{kind} n={n} seed={seed} {domain:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_invalid_tolerance() {
        let game = generators::bnf(1, 4, 0).unwrap();
        let single = crate::ExplicitGame::from_lex(&[2.0]).unwrap();
        for tolerance in [f64::NAN, -1.0, f64::INFINITY, f64::NEG_INFINITY] {
            for domain in [Domain::Imputation, Domain::Preimputation] {
                for result in [
                    nucleolus_with(&game, domain, tolerance),
                    nucleolus_with(&single, domain, tolerance),
                ] {
                    assert!(
                        matches!(result, Err(Error::InvalidArgument(_))),
                        "{tolerance} {domain:?}: {result:?}"
                    );
                }
            }
        }
        // 大きい許容誤差は利用者の指定として受け付ける。
        assert!(nucleolus_with(&game, Domain::Imputation, 1e3).is_ok());
    }

    /// 値が数千万の整数で、端数の差 (1 程度) が許容誤差 (`1e-7 * max |v|`、2〜3) より小さい提携を持つゲーム。
    /// 許容誤差以内にタイトなだけの提携を `v(S) - level` で固定すると、以後の LP が実行不可能になる。
    /// 有理数の逐次 LP で求めた仁・プレ仁と、許容誤差の 2 倍以内で一致することを確かめる。
    #[test]
    fn near_tie_integer_games_match_exact_nucleolus() {
        use crate::game::exact::ExactGame;
        use num_traits::ToPrimitive;
        let games: [&[f64]; 2] = [
            &[
                -1.0, -1.0, 7444064.0, -1.0, 7444064.0, 7444065.0, 19288200.0, 0.0, 7444064.0,
                7444063.0, 19288201.0, 7444064.0, 19288201.0, 19288200.0, 22390681.0, -1.0,
                7444065.0, 7444063.0, 19288202.0, 7444063.0, 19288200.0, 19288202.0, 22390680.0,
                7444065.0, 19288201.0, 19288202.0, 22390680.0, 19288202.0, 22390680.0, 22390682.0,
                17088946.0,
            ],
            &[
                3000000.0, 2999999.0, 6644424.0, 2999999.0, 6644424.0, 6644423.0, 26266127.0,
                3000000.0, 6644422.0, 6644423.0, 26266125.0, 6644423.0, 26266127.0, 26266127.0,
                24717084.0, 3000001.0, 6644424.0, 6644424.0, 26266126.0, 6644423.0, 26266127.0,
                26266126.0, 24717083.0, 6644424.0, 26266127.0, 26266127.0, 24717083.0, 26266127.0,
                24717085.0, 24717085.0, 30350090.0,
            ],
        ];
        for values in games {
            let game = crate::ExplicitGame::from_binary(values).unwrap();
            let exact_game = ExactGame::from_explicit(&game).unwrap();
            let tolerance = default_tolerance(&game);
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let expected: Vec<f64> = crate::nucleolus::exact::nucleolus(&exact_game, domain)
                    .unwrap()
                    .allocation
                    .iter()
                    .map(|r| r.to_f64().unwrap())
                    .collect();
                let actual = nucleolus_with(&game, domain, tolerance).unwrap().allocation;
                for (a, e) in actual.iter().zip(&expected) {
                    assert!(
                        (a - e).abs() <= 2.0 * tolerance,
                        "{domain:?}: {actual:?} != {expected:?}"
                    );
                }
            }
        }
    }

    /// 値が `f64` の上限に近いと、正規化したゲームでは解けても元の大きさに戻すと溢れる。
    /// 2 人ゲーム `v({0}) = v({1}) = -1.7e308`、`v(N) = 1.7e308` の最大超過は約 `-2.55e308`。
    #[test]
    fn overflowing_results_are_numerical_errors() {
        let game = crate::ExplicitGame::from_lex(&[-1.7e308, -1.7e308, 1.7e308]).unwrap();
        assert!(matches!(prenucleolus(&game), Err(Error::Numerical(_))));
        assert!(matches!(
            least_core(&game, Domain::Preimputation),
            Err(Error::Numerical(_))
        ));
        // 値を小さくした同じ形のゲームは解ける。
        let small = crate::ExplicitGame::from_lex(&[-1.7e300, -1.7e300, 1.7e300]).unwrap();
        assert!(prenucleolus(&small).is_ok());
        assert!(least_core(&small, Domain::Preimputation).is_ok());
    }

    /// 値が有限でない提携を行にしない。分離オラクルが返したらエラー、全体提携・1 人提携 (配分集合のとき) の値が
    /// 有限でなければ LP を組む前にエラーにする。
    #[test]
    fn non_finite_values_never_become_rows() {
        /// 3 人ゲーム。`v({0, 1})` だけ NaN で、分離オラクルは NaN の提携も違反として返す。
        struct Broken {
            nan: PlayerSet,
            grand: f64,
            single: f64,
        }
        impl crate::SetFunction for Broken {
            fn players(&self) -> usize {
                3
            }
            fn value(&self, coalition: &PlayerSet) -> f64 {
                if *coalition == self.nan {
                    f64::NAN
                } else if coalition.is_full() {
                    self.grand
                } else if coalition.len() == 1 {
                    self.single
                } else {
                    1.0
                }
            }
        }
        impl Separation for Broken {
            fn violated(
                &self,
                _x: &[f64],
                _bound: f64,
                skip: &dyn Fn(&PlayerSet) -> bool,
                _limit: usize,
            ) -> Vec<(PlayerSet, f64)> {
                if skip(&self.nan) {
                    Vec::new()
                } else {
                    vec![(self.nan.clone(), f64::NAN)]
                }
            }
            fn value_scale(&self) -> f64 {
                1.0
            }
        }
        let nan = PlayerSet::from_coalition(3, crate::Coalition::from_players(&[0, 1]));
        let broken = |grand, single| Broken {
            nan: nan.clone(),
            grand,
            single,
        };
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let result = nucleolus_with(&broken(3.0, 0.0), domain, 1e-7);
            match result {
                Err(Error::InvalidArgument(message)) => {
                    assert!(message.contains("{0, 1}"), "{message}")
                }
                other => panic!("{domain:?}: {other:?}"),
            }
            assert!(matches!(
                nucleolus_with(&broken(f64::NAN, 0.0), domain, 1e-7),
                Err(Error::InvalidArgument(_))
            ));
        }
        assert!(matches!(
            nucleolus_with(&broken(3.0, f64::INFINITY), Domain::Imputation, 1e-7),
            Err(Error::InvalidArgument(_))
        ));
    }
}
