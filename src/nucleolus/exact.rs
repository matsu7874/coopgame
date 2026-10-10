//! 有理数だけで仁・プレ仁を求める (逐次 LP を有理数の単体法で解く)。
//!
//! 浮動小数点の逐次 LP ([`crate::nucleolus`]) と同じ手順を、全て有理数で行う。
//!
//! 1. 未確定の提携の最大超過 `t` を最小化する (制約生成: 違反する提携を行として足しながら解き直す)。
//! 2. 最大超過 `t*` を達成する提携のうち、最適解集合全体で超過が `t*` に固定されるものを確定させる。
//!    確定の判定は、最大超過が `t*` 以下の配分の中で `x(S)` を最大化する LP で行う。
//! 3. 確定した提携の特性ベクトルが `R^n` を張るまで繰り返す。
//!
//! 等号・不等号の判定に許容誤差を使わないので、結果は厳密な仁である。
//! 有理数の演算は浮動小数点より桁違いに遅いので、[`MAX_EXACT_PLAYERS`] 人までに限る。

use num_traits::Zero;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::coalition::Coalition;
use crate::game::exact::{ExactGame, Rational, format_rational};
use crate::rational::simplex::{self, Outcome};
use crate::rational::{ExactEchelon, Inserted, indicator};

/// 厳密なソルバーが扱うプレイヤー数の上限。
pub const MAX_EXACT_PLAYERS: usize = 10;

/// 厳密な仁 (プレ仁)。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ExactNucleolus {
    pub allocation: Vec<Rational>,
    /// 各段の最大超過 `t*`。
    pub levels: Vec<Rational>,
    /// 解いた有理数の LP の数。
    pub lp_solves: usize,
}

/// 有理数で仁 (`Domain::Imputation`) またはプレ仁を求める。
pub fn nucleolus(game: &ExactGame, domain: Domain) -> Result<ExactNucleolus> {
    let n = game.players();
    if n == 0 || n > MAX_EXACT_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_EXACT_PLAYERS,
        });
    }
    let grand = Coalition::grand(n);
    if domain == Domain::Imputation {
        let lower = (0..n).fold(Rational::zero(), |acc, i| {
            acc + game.value(Coalition::singleton(i))
        });
        if &lower > game.value(grand) {
            return Err(Error::EmptyImputationSet);
        }
    }
    if n == 1 {
        return Ok(ExactNucleolus {
            allocation: vec![game.value(grand).clone()],
            levels: Vec::new(),
            lp_solves: 0,
        });
    }
    let mut solver = Solver::new(game, domain);
    solver.run()
}

/// LP の種類。
#[derive(Clone)]
enum Goal {
    /// 最大超過 `t` を最小化する。
    MinimizeLevel,
    /// 最大超過が `level` 以下の配分で `x(target)` を最大化する。
    MaximizeCoalition { target: Coalition, level: Rational },
}

struct Solver<'a> {
    game: &'a ExactGame,
    domain: Domain,
    n: usize,
    /// 確定した提携と `x(S)` の値 (全体提携を含む)。
    fixed: Vec<(Coalition, Rational)>,
    echelon: ExactEchelon,
    /// 超過が確定した (特性ベクトルが確定済みの空間に入る) 提携。
    determined: Vec<bool>,
    /// LP に入れる不等式の提携。
    pool: Vec<Coalition>,
    in_pool: Vec<bool>,
    lp_solves: usize,
}

impl<'a> Solver<'a> {
    fn new(game: &'a ExactGame, domain: Domain) -> Solver<'a> {
        let n = game.players();
        let grand = Coalition::grand(n);
        let size = 1usize << n;
        let mut solver = Solver {
            game,
            domain,
            n,
            fixed: vec![(grand, game.value(grand).clone())],
            echelon: ExactEchelon::default(),
            determined: vec![false; size],
            pool: Vec::new(),
            in_pool: vec![false; size],
            lp_solves: 0,
        };
        solver.echelon.insert(indicator(grand, n), n);
        solver.determined[0] = true;
        solver.determined[grand.index()] = true;
        // 1 人提携とその補集合があれば LP は有界になる。
        for i in 0..n {
            let single = Coalition::singleton(i);
            solver.remember(single);
            solver.remember(Coalition(grand.0 & !single.0));
        }
        solver.mark_determined();
        solver
    }

    fn remember(&mut self, coalition: Coalition) {
        if !self.in_pool[coalition.index()] {
            self.in_pool[coalition.index()] = true;
            self.pool.push(coalition);
        }
    }

    fn in_span(&self, coalition: Coalition) -> bool {
        self.echelon
            .reduce(indicator(coalition, self.n))
            .iter()
            .all(Zero::is_zero)
    }

    fn mark_determined(&mut self) {
        for mask in 1..self.determined.len() {
            if !self.determined[mask] && self.in_span(Coalition(mask as u64)) {
                self.determined[mask] = true;
            }
        }
    }

    fn open(&self) -> impl Iterator<Item = Coalition> + '_ {
        (1..self.determined.len())
            .filter(|&mask| !self.determined[mask])
            .map(|mask| Coalition(mask as u64))
    }

    fn excess(&self, coalition: Coalition, x: &[Rational]) -> Rational {
        coalition
            .players()
            .fold(self.game.value(coalition).clone(), |acc, i| acc - &x[i])
    }

    fn run(&mut self) -> Result<ExactNucleolus> {
        let mut levels = Vec::new();
        let mut allocation = Vec::new();
        while self.echelon.rank() < self.n {
            let (level, x) = self.minimize_level()?;
            self.fix_tight(&level, &x)?;
            self.mark_determined();
            levels.push(level);
            allocation = x;
        }
        Ok(ExactNucleolus {
            allocation,
            levels,
            lp_solves: self.lp_solves,
        })
    }

    /// 列: 配分 (配分集合なら `x_i = v({i}) + w_i`、準配分なら `x_i = p_i - q_i`)、
    /// `t = t_+ - t_-` (最小化のときだけ)、不等式ごとのスラック。
    /// `rows` に入れた提携の不等式と確定した等式で LP を作って解き、配分と `t` を返す。
    fn solve(&mut self, goal: &Goal, rows: &[Coalition]) -> Result<(Vec<Rational>, Rational)> {
        let n = self.n;
        let one = Rational::from_integer(1.into());
        let x_columns = match self.domain {
            Domain::Imputation => n,
            Domain::Preimputation => 2 * n,
        };
        let has_level = matches!(goal, Goal::MinimizeLevel);
        let level_columns = if has_level { 2 } else { 0 };
        let columns = x_columns + level_columns + rows.len();
        // x(S) の係数と定数 (配分集合なら sum v({i}))。
        let coalition_row = |coalition: Coalition| -> (Vec<Rational>, Rational) {
            let mut row = vec![Rational::zero(); columns];
            let mut constant = Rational::zero();
            for i in coalition.players() {
                match self.domain {
                    Domain::Imputation => {
                        row[i] = one.clone();
                        constant += self.game.value(Coalition::singleton(i));
                    }
                    Domain::Preimputation => {
                        row[2 * i] = one.clone();
                        row[2 * i + 1] = -one.clone();
                    }
                }
            }
            (row, constant)
        };
        let mut a: Vec<Vec<Rational>> = Vec::new();
        let mut b: Vec<Rational> = Vec::new();
        for (coalition, value) in &self.fixed {
            let (row, constant) = coalition_row(*coalition);
            a.push(row);
            b.push(value - constant);
        }
        for (k, &coalition) in rows.iter().enumerate() {
            // x(S) + t - s = v(S)、または x(S) - s = v(S) - level
            let (mut row, constant) = coalition_row(coalition);
            let mut rhs = self.game.value(coalition) - constant;
            if has_level {
                row[x_columns] = one.clone();
                row[x_columns + 1] = -one.clone();
            } else if let Goal::MaximizeCoalition { level, .. } = goal {
                rhs -= level;
            }
            row[x_columns + level_columns + k] = -one.clone();
            a.push(row);
            b.push(rhs);
        }
        let mut cost = vec![Rational::zero(); columns];
        match goal {
            Goal::MinimizeLevel => {
                cost[x_columns] = one.clone();
                cost[x_columns + 1] = -one.clone();
            }
            Goal::MaximizeCoalition { target, .. } => {
                let (row, _) = coalition_row(*target);
                for (c, r) in cost.iter_mut().zip(&row) {
                    *c = -r.clone();
                }
            }
        }
        self.lp_solves += 1;
        let solution = match simplex::minimize(&a, &b, &cost) {
            Outcome::Optimal { solution, .. } => solution,
            Outcome::Infeasible => {
                return Err(Error::Numerical("有理数の LP が実行不可能".into()));
            }
            Outcome::Unbounded => {
                return Err(Error::Numerical("有理数の LP が非有界".into()));
            }
        };
        let x: Vec<Rational> = (0..n)
            .map(|i| match self.domain {
                Domain::Imputation => self.game.value(Coalition::singleton(i)) + &solution[i],
                Domain::Preimputation => &solution[2 * i] - &solution[2 * i + 1],
            })
            .collect();
        let t = if has_level {
            &solution[x_columns] - &solution[x_columns + 1]
        } else {
            Rational::zero()
        };
        Ok((x, t))
    }

    /// 制約生成で解く。違反する提携 (超過がしきい値を超える未確定の提携) を全て足して解き直す。
    fn solve_generating(
        &mut self,
        goal: Goal,
        seed: &[Coalition],
    ) -> Result<(Vec<Rational>, Rational)> {
        for &coalition in seed {
            self.remember(coalition);
        }
        loop {
            let rows: Vec<Coalition> = self
                .pool
                .iter()
                .copied()
                .filter(|c| !self.determined[c.index()])
                .collect();
            let (x, t) = self.solve(&goal, &rows)?;
            let bound = match &goal {
                Goal::MinimizeLevel => t.clone(),
                Goal::MaximizeCoalition { level, .. } => level.clone(),
            };
            let violated: Vec<Coalition> = self
                .open()
                .filter(|c| !self.in_pool[c.index()] && self.excess(*c, &x) > bound)
                .collect();
            if violated.is_empty() {
                return Ok((x, t));
            }
            for coalition in violated {
                self.remember(coalition);
            }
        }
    }

    fn minimize_level(&mut self) -> Result<(Rational, Vec<Rational>)> {
        let (x, t) = self.solve_generating(Goal::MinimizeLevel, &[])?;
        Ok((t, x))
    }

    fn fix_tight(&mut self, level: &Rational, x: &[Rational]) -> Result<()> {
        let tight: Vec<Coalition> = self
            .open()
            .filter(|c| &self.excess(*c, x) == level)
            .collect();
        let mut witnesses: Vec<Vec<Rational>> = Vec::new();
        let mut progressed = false;
        for target in tight {
            if self.in_span(target) || witnesses.iter().any(|y| &self.excess(target, y) < level) {
                continue;
            }
            let complement = Coalition(Coalition::grand(self.n).0 & !target.0);
            let (y, _) = self.solve_generating(
                Goal::MaximizeCoalition {
                    target,
                    level: level.clone(),
                },
                &[target, complement],
            )?;
            // x(target) の最大値が v(target) - t* なら、最適解集合全体で超過が t* に固定される。
            if &self.excess(target, &y) == level {
                let value = self.game.value(target) - level;
                if let Inserted::Independent =
                    self.echelon.insert(indicator(target, self.n), self.n)
                {
                    progressed = true;
                }
                self.fixed.push((target, value));
            } else {
                witnesses.push(y);
            }
        }
        if !progressed {
            return Err(Error::Numerical(format!(
                "最大超過 {} の段で確定する提携が見つからない",
                format_rational(level)
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(a: i64, b: i64) -> Rational {
        Rational::new(a.into(), b.into())
    }

    #[test]
    fn literature_examples_exactly() {
        // Ferguson の 3 人ゲーム: 仁 = プレ仁 = (8/3, 2/3, 5/3)。
        let values: Vec<Rational> = [-1, 0, 1, 3, 4, 2, 5].iter().map(|v| q(*v, 1)).collect();
        let game = ExactGame::from_lex(values).unwrap();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let result = nucleolus(&game, domain).unwrap();
            assert_eq!(result.allocation, vec![q(8, 3), q(2, 3), q(5, 3)]);
            assert_eq!(result.levels[0], q(-1, 3));
        }
        // Peleg & Sudhölter Example 5.5.12: プレ仁 (3, 3, -4)、仁 (1, 1, 0)。
        let values: Vec<Rational> = [0, 0, 0, 10, 0, 0, 2].iter().map(|v| q(*v, 1)).collect();
        let game = ExactGame::from_lex(values).unwrap();
        assert_eq!(
            nucleolus(&game, Domain::Preimputation).unwrap().allocation,
            vec![q(3, 1), q(3, 1), q(-4, 1)]
        );
        assert_eq!(
            nucleolus(&game, Domain::Imputation).unwrap().allocation,
            vec![q(1, 1), q(1, 1), q(0, 1)]
        );
    }

    #[test]
    fn rejects_empty_imputation_set_and_large_games() {
        let values: Vec<Rational> = [1, 1, 1].iter().map(|v| q(*v, 1)).collect();
        let game = ExactGame::from_binary(values).unwrap();
        assert!(matches!(
            nucleolus(&game, Domain::Imputation),
            Err(Error::EmptyImputationSet)
        ));
        assert!(nucleolus(&game, Domain::Preimputation).is_ok());
    }

    #[test]
    fn excess_is_never_compared_with_tolerance() {
        // 浮動小数点では等しくならない 1/3 の段を含むゲームでも厳密に求まる。
        let values: Vec<Rational> = vec![
            q(1, 3),
            q(1, 7),
            q(2, 3),
            q(1, 11),
            q(5, 7),
            q(3, 5),
            q(2, 1),
        ];
        let game = ExactGame::from_binary(values).unwrap();
        let result = nucleolus(&game, Domain::Preimputation).unwrap();
        let total = result
            .allocation
            .iter()
            .fold(Rational::zero(), |acc, v| acc + v);
        assert_eq!(total, q(2, 1));
        let report =
            crate::verify::kohlberg_exact(&game, &result.allocation, Domain::Preimputation);
        assert!(report.satisfied, "{:?}", report.reason);
    }
}
