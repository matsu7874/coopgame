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

use std::collections::HashSet;

use microlp::{Problem, Solution, Variable};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::PlayerSet;
use crate::game::allocation::check_imputation_set;
use crate::game::oracle::Separation;
use crate::linalg::Span;
use crate::lp::{self, Cmp, Counter};
use crate::nucleolus::{LeastCore, NucleolusResult};
use crate::solution::Guarantee;

/// ゲームの値の大きさに合わせた既定の許容誤差。
pub fn default_tolerance<G: Separation + ?Sized>(game: &G) -> f64 {
    1e-7 * game.value_scale().max(1.0)
}

pub fn nucleolus<G: Separation + ?Sized>(game: &G) -> Result<NucleolusResult> {
    nucleolus_with(game, Domain::Imputation, default_tolerance(game))
}

pub fn prenucleolus<G: Separation + ?Sized>(game: &G) -> Result<NucleolusResult> {
    nucleolus_with(game, Domain::Preimputation, default_tolerance(game))
}

pub fn nucleolus_with<G: Separation + ?Sized>(
    game: &G,
    domain: Domain,
    tolerance: f64,
) -> Result<NucleolusResult> {
    Sequential::new(game, domain, tolerance)?.run()
}

pub fn least_core<G: Separation + ?Sized>(game: &G, domain: Domain) -> Result<LeastCore> {
    let mut solver = Sequential::new(game, domain, default_tolerance(game))?;
    let (epsilon, allocation) = solver.minimize_max_excess()?;
    Ok(LeastCore {
        epsilon,
        allocation,
        guarantee: Guarantee::Exact,
    })
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
    domain: Domain,
    tolerance: f64,
    n: usize,
    lower: Vec<f64>,
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
        let grand = PlayerSet::full(n);
        let lower: Vec<f64> = (0..n)
            .map(|i| game.value(&PlayerSet::from_players(n, &[i])))
            .collect();
        check_imputation_set(game, domain, tolerance)?;
        let mut span = Span::new(n);
        span.insert(&grand.indicator());
        let mut solver = Sequential {
            game,
            domain,
            tolerance,
            n,
            lower,
            fixed: vec![(grand.clone(), game.value(&grand))],
            span,
            counter: Counter::default(),
            rows_added: 0,
            pool: Vec::new(),
            in_pool: HashSet::new(),
        };
        // 1 人提携とその補集合があれば、最初の LP は有界になる。
        for i in 0..n {
            let single = PlayerSet::from_players(n, &[i]);
            solver.remember(single.complement());
            solver.remember(single);
        }
        Ok(solver)
    }

    fn run(&mut self) -> Result<NucleolusResult> {
        if self.n == 1 {
            return Ok(NucleolusResult {
                allocation: vec![self.game.value(&PlayerSet::full(1))],
                levels: Vec::new(),
                lp_solves: 0,
                rows_added: 0,
                guarantee: Guarantee::Exact,
            });
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
            guarantee: Guarantee::Exact,
        })
    }

    fn remember(&mut self, coalition: PlayerSet) {
        if coalition.is_proper() && !self.in_pool.contains(&coalition) {
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
                let lower = match self.domain {
                    Domain::Imputation => self.lower[i],
                    Domain::Preimputation => f64::NEG_INFINITY,
                };
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
            let fixed_sum = self.game.value(&target) - level;
            let (max_sum, witness) = self.maximize_coalition(&target, level)?;
            if max_sum <= fixed_sum + self.tolerance {
                self.span.insert(&target.indicator());
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
                            crate::nucleolus::Options::new(&game, domain),
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
}
