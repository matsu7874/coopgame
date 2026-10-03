//! 線形生産ゲーム (Owen 1975): プレイヤーが資源を持ち寄り、線形の技術で製品を作って売る。
//!
//! 製品 `k` 種類、資源 `m` 種類。製品 1 単位に資源 `A[r][k]` を使い、価格 `p_k` で売れる。
//! プレイヤー `i` は資源 `b_i` (長さ `m`) を持つ。提携の値は
//! `v(S) = max { p . z : A z <= b(S), z >= 0 }` (`b(S)` は持ち寄った資源の和)。
//!
//! Owen 配分: 全員の LP の双対問題 `min { y . b(N) : A^T y >= p, y >= 0 }` の最適解 `y*` (資源の影の価格)
//! で、各人に持ち込んだ資源の価値 `x_i = y* . b_i` を払う。この配分はコアに属する (Owen 1975)。

use microlp::Variable;

use super::{PlayerSet, SetFunction};
use crate::error::{Error, Result};
use crate::lp::{self, Cmp, Counter};

#[derive(Clone, Debug, PartialEq)]
pub struct LinearProductionGame {
    /// `m x k` の技術行列。
    technology: Vec<Vec<f64>>,
    prices: Vec<f64>,
    /// `n x m` の資源。
    resources: Vec<Vec<f64>>,
}

impl LinearProductionGame {
    pub fn new(
        technology: Vec<Vec<f64>>,
        prices: Vec<f64>,
        resources: Vec<Vec<f64>>,
    ) -> Result<LinearProductionGame> {
        let k = prices.len();
        let m = technology.len();
        let finite_nonnegative = |v: &f64| v.is_finite() && *v >= 0.0;
        if m == 0 || k == 0 || resources.is_empty() {
            return Err(Error::InvalidArgument(
                "資源・製品・プレイヤーは 1 つ以上".into(),
            ));
        }
        if technology
            .iter()
            .any(|row| row.len() != k || !row.iter().all(finite_nonnegative))
            || !prices.iter().all(finite_nonnegative)
            || resources
                .iter()
                .any(|b| b.len() != m || !b.iter().all(finite_nonnegative))
        {
            return Err(Error::InvalidArgument(
                "技術行列は資源 x 製品、資源は各人が資源の数だけ持ち、全て非負".into(),
            ));
        }
        // 価格が正の製品が資源を使わずに作れると値が非有界になる。
        for (j, p) in prices.iter().enumerate() {
            if *p > 0.0 && technology.iter().all(|row| row[j] == 0.0) {
                return Err(Error::InvalidArgument(format!(
                    "製品 {j} は資源を使わずに作れるので値が非有界"
                )));
            }
        }
        Ok(LinearProductionGame {
            technology,
            prices,
            resources,
        })
    }

    fn pooled(&self, coalition: &PlayerSet) -> Vec<f64> {
        let mut b = vec![0.0; self.technology.len()];
        for i in coalition.members() {
            for (total, own) in b.iter_mut().zip(&self.resources[i]) {
                *total += own;
            }
        }
        b
    }

    /// 資源の影の価格 `y*` (全員の LP の双対問題の最適解)。
    pub fn shadow_prices(&self) -> Result<Vec<f64>> {
        let b = self.pooled(&PlayerSet::full(self.resources.len()));
        let mut problem = lp::minimize();
        let y: Vec<Variable> = b
            .iter()
            .map(|amount| problem.add_var(*amount, (0.0, f64::INFINITY)))
            .collect();
        for (j, price) in self.prices.iter().enumerate() {
            let terms = y
                .iter()
                .zip(&self.technology)
                .map(|(&var, row)| (var, row[j]))
                .filter(|(_, a)| *a != 0.0)
                .collect();
            lp::add(&mut problem, terms, Cmp::Ge, *price);
        }
        let solution = Counter::default()
            .solve(&problem)?
            .ok_or_else(|| Error::Numerical("双対問題が実行不可能".into()))?;
        Ok(y.iter().map(|&var| solution[var]).collect())
    }

    /// Owen 配分 `x_i = y* . b_i` (コアに属する)。
    pub fn owen_allocation(&self) -> Result<Vec<f64>> {
        let prices = self.shadow_prices()?;
        Ok(self
            .resources
            .iter()
            .map(|b| b.iter().zip(&prices).map(|(a, y)| a * y).sum())
            .collect())
    }
}

impl SetFunction for LinearProductionGame {
    fn players(&self) -> usize {
        self.resources.len()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if coalition.is_empty() {
            return 0.0;
        }
        let b = self.pooled(coalition);
        let mut problem = lp::maximize();
        let z: Vec<Variable> = self
            .prices
            .iter()
            .map(|p| problem.add_var(*p, (0.0, f64::INFINITY)))
            .collect();
        for (row, amount) in self.technology.iter().zip(&b) {
            let terms = z
                .iter()
                .zip(row)
                .map(|(&var, a)| (var, *a))
                .filter(|(_, a)| *a != 0.0)
                .collect();
            lp::add(&mut problem, terms, Cmp::Le, *amount);
        }
        Counter::default()
            .solve(&problem)
            .ok()
            .flatten()
            .map_or(f64::NAN, |solution| solution.objective())
    }
}
