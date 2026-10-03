//! microlp の薄いラッパー。解いた LP の数を数える。

use microlp::{ComparisonOp, OptimizationDirection, Problem, Solution, Variable};

use crate::coalition::Coalition;
use crate::error::{Error, Result};

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
    coalition.players().map(|i| (x[i], 1.0)).collect()
}

pub(crate) fn add(problem: &mut Problem, terms: Vec<(Variable, f64)>, op: ComparisonOp, rhs: f64) {
    problem.add_constraint(terms, op, rhs);
}
