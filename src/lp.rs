//! microlp の薄いラッパー。解いた LP の数を数える。

use microlp::{ComparisonOp, OptimizationDirection, Problem, Solution, Variable};

use crate::error::{Error, Result};
use crate::game::Coalition;

pub(crate) use microlp::ComparisonOp as Cmp;

#[derive(Default)]
pub(crate) struct Counter {
    pub solves: usize,
}

impl Counter {
    pub fn solve(&mut self, problem: &Problem) -> Result<Option<Solution>> {
        self.solves += 1;
        match problem.solve() {
            Ok(outcome) => outcome
                .into_solution()
                .map(Some)
                .map_err(|_| Error::Lp("LP の求解が中断された".into())),
            Err(microlp::Error::Infeasible) => Ok(None),
            Err(err) => Err(Error::Lp(err.to_string())),
        }
    }
}

impl Counter {
    /// 非有界なら `None` を返す。実行不可能はエラーにする。
    pub fn solve_bounded(&mut self, problem: &Problem) -> Result<Option<Solution>> {
        self.solves += 1;
        match problem.solve() {
            Ok(outcome) => outcome
                .into_solution()
                .map(Some)
                .map_err(|_| Error::Lp("LP の求解が中断された".into())),
            Err(microlp::Error::Unbounded) => Ok(None),
            Err(microlp::Error::Infeasible) => Err(Error::Numerical("LP が実行不可能".into())),
            Err(err) => Err(Error::Lp(err.to_string())),
        }
    }
}

impl Counter {
    /// 数学的に実行可能かつ有界な LP を解く。ソルバーが実行不可能・非有界と答えたら、
    /// 数値的な失敗として [`Error::Numerical`] を返す (`what` は失敗した LP の説明)。
    pub fn solve_feasible(&mut self, problem: &Problem, what: &str) -> Result<Solution> {
        self.solves += 1;
        match problem.solve() {
            Ok(outcome) => outcome
                .into_solution()
                .map_err(|_| Error::Lp("LP の求解が中断された".into())),
            Err(microlp::Error::Infeasible) => Err(Error::Numerical(format!(
                "{what}は常に実行可能だが、LP ソルバーが実行不可能と答えた"
            ))),
            Err(microlp::Error::Unbounded) => Err(Error::Numerical(format!(
                "{what}は常に有界だが、LP ソルバーが非有界と答えた"
            ))),
            Err(err) => Err(Error::Lp(err.to_string())),
        }
    }
}

/// 解いた LP に行を追加し、前回の解から解き直す。
pub(crate) fn add_to_solution(
    solution: Solution,
    terms: Vec<(Variable, f64)>,
    op: ComparisonOp,
    rhs: f64,
) -> Result<Solution> {
    solution
        .add_constraint(terms, op, rhs)
        .map_err(|err| match err {
            microlp::Error::Infeasible => {
                Error::Numerical("行の追加で LP が実行不可能になった".into())
            }
            err => Error::Lp(err.to_string()),
        })?
        .into_solution()
        .map_err(|_| Error::Lp("LP の求解が中断された".into()))
}

pub(crate) fn minimize() -> Problem {
    Problem::new(OptimizationDirection::Minimize)
}

pub(crate) fn maximize() -> Problem {
    Problem::new(OptimizationDirection::Maximize)
}

/// `x(S)` の線形式。
pub(crate) fn coalition_expr(x: &[Variable], coalition: Coalition) -> Vec<(Variable, f64)> {
    coalition.members().map(|i| (x[i], 1.0)).collect()
}

pub(crate) fn add(problem: &mut Problem, terms: Vec<(Variable, f64)>, op: ComparisonOp, rhs: f64) {
    problem.add_constraint(terms, op, rhs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solve_feasible_reports_infeasible_and_unbounded_as_numerical_failures() {
        // x >= 1 かつ x <= 0 (実行不可能)。
        let mut infeasible = maximize();
        let x = infeasible.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
        add(&mut infeasible, vec![(x, 1.0)], Cmp::Ge, 1.0);
        add(&mut infeasible, vec![(x, 1.0)], Cmp::Le, 0.0);
        let mut counter = Counter::default();
        let err = counter
            .solve_feasible(&infeasible, "異議の LP ")
            .unwrap_err();
        assert!(
            matches!(&err, Error::Numerical(m) if m.contains("実行不可能")),
            "{err}"
        );
        // x を最大化、上限なし (非有界)。
        let mut unbounded = maximize();
        let x = unbounded.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
        add(&mut unbounded, vec![(x, 1.0)], Cmp::Ge, 0.0);
        let err = counter
            .solve_feasible(&unbounded, "異議の LP ")
            .unwrap_err();
        assert!(
            matches!(&err, Error::Numerical(m) if m.contains("非有界")),
            "{err}"
        );
        assert_eq!(counter.solves, 2);
    }
}
