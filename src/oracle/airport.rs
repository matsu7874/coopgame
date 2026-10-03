//! 空港ゲーム: 提携の費用は、提携の中で最も大きい施設を必要とする人の費用 `c(S) = max_{i in S} c_i`。
//!
//! 滑走路の長さ、共用設備の容量など「最も大きい要求に合わせて作れば全員が使える」費用の分担を表す。
//!
//! - Shapley 値: 費用の増分を、その増分を必要とする人数で等分した和 (Littlechild & Owen 1973)。`O(n log n)`。
//! - 仁: 節約ゲーム `s(S) = sum_{i in S} c_i - max_{i in S} c_i` は凸 (優モジュラ) なので、
//!   凸ゲームの手法 ([`crate::convex`]) で全提携を列挙せずに求める。
//!   費用 `max` は劣モジュラ (`max(S ∪ T) = max(max S, max T)`、`max(S ∩ T) <= min(max S, max T)`) なので、
//!   加法的な項からそれを引いた節約ゲームは優モジュラになる。
//!   Littlechild (1974) は仁の簡単な表現を与えたが、本文を確認できないためその式は実装していない。
//!
//! この型の [`SetFunction`] の値は節約ゲームの値である (費用は [`AirportGame::cost`])。

use super::{PlayerSet, SetFunction};
use crate::convex;
use crate::error::{Error, Result};
use crate::structure::{ConvexGame, Proven};

#[derive(Clone, Debug, PartialEq)]
pub struct AirportGame {
    costs: Vec<f64>,
}

impl AirportGame {
    /// 各人が必要とする施設の費用 (非負の有限値)。
    pub fn new(costs: Vec<f64>) -> Result<AirportGame> {
        if costs.is_empty() || costs.iter().any(|c| !(c.is_finite() && *c >= 0.0)) {
            return Err(Error::InvalidArgument(
                "費用は 1 つ以上の非負の有限値".into(),
            ));
        }
        Ok(AirportGame { costs })
    }

    pub fn costs(&self) -> &[f64] {
        &self.costs
    }

    /// 提携の費用 `max_{i in S} c_i`。
    pub fn cost(&self, coalition: &PlayerSet) -> f64 {
        coalition
            .members()
            .map(|i| self.costs[i])
            .fold(0.0, f64::max)
    }

    /// Shapley 値による費用の分担 (Littlechild & Owen 1973)。費用の小さい順に、各段の増分を
    /// その段以上の費用を持つ人数で等分して足す。
    pub fn shapley_costs(&self) -> Vec<f64> {
        let n = self.costs.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| self.costs[a].total_cmp(&self.costs[b]));
        let mut shares = vec![0.0; n];
        let (mut previous, mut accumulated) = (0.0, 0.0);
        for (position, &i) in order.iter().enumerate() {
            accumulated += (self.costs[i] - previous) / (n - position) as f64;
            previous = self.costs[i];
            shares[i] = accumulated;
        }
        shares
    }

    /// 仁による費用の分担 (凸ゲームの手法で節約ゲームの仁を求め、費用に直す)。
    pub fn nucleolus_costs(&self) -> Result<Vec<f64>> {
        let savings = convex::nucleolus(self)?;
        Ok(crate::cost::shares_from_savings(
            &self.costs,
            &savings.allocation,
        ))
    }
}

impl SetFunction for AirportGame {
    fn players(&self) -> usize {
        self.costs.len()
    }

    /// 節約 `sum_{i in S} c_i - max_{i in S} c_i`。
    fn value(&self, coalition: &PlayerSet) -> f64 {
        let total: f64 = coalition.members().map(|i| self.costs[i]).sum();
        total - self.cost(coalition)
    }
}

impl ConvexGame for AirportGame {
    type Proof = Proven;
}
