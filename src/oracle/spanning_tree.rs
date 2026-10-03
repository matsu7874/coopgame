//! 最小全域木ゲーム: 供給元 (頂点 0) とプレイヤー (頂点 1..=n) を結ぶ費用の分担 (Bird 1976)。
//!
//! 提携 `S` の費用 `c(S)` は、供給元と `S` の頂点だけを使う最小全域木の費用 (辺の費用は対称な行列で与える)。
//!
//! - Bird 規則: 全員の最小全域木を Prim 法で供給元から作り、各プレイヤーは自分を木につないだ辺の費用を払う。
//!   この分担はコアに属する (Bird 1976)。`O(n^2)`。
//! - 仁: 計算は一般に NP 困難 (Faigle, Kern & Kuipers 1998) なので、専用の高速な手法はない。
//!   明示ベクトルにして ([`crate::oracle::tabulate`]) 逐次 LP で求める (30 人まで)。
//!
//! この型の [`SetFunction`] の値は節約ゲーム `s(S) = sum_{i in S} c({i}) - c(S)` の値である。

use super::{PlayerSet, SetFunction, tabulate};
use crate::error::{Error, Result};
use crate::nucleolus;

#[derive(Clone, Debug, PartialEq)]
pub struct SpanningTreeGame {
    /// `(n + 1) x (n + 1)` の対称な費用行列。頂点 0 が供給元、頂点 `i + 1` がプレイヤー `i`。
    costs: Vec<Vec<f64>>,
}

impl SpanningTreeGame {
    pub fn new(costs: Vec<Vec<f64>>) -> Result<SpanningTreeGame> {
        let size = costs.len();
        if size < 2 || costs.iter().any(|row| row.len() != size) {
            return Err(Error::InvalidArgument(
                "費用行列は (プレイヤー数 + 1) の正方行列".into(),
            ));
        }
        for (u, row) in costs.iter().enumerate() {
            for (v, &c) in row.iter().enumerate() {
                if !(c.is_finite() && c >= 0.0) || (c - costs[v][u]).abs() > 1e-12 {
                    return Err(Error::InvalidArgument(format!(
                        "費用 ({u}, {v}) が非負の有限値でないか、対称でない"
                    )));
                }
            }
        }
        Ok(SpanningTreeGame { costs })
    }

    /// 供給元と `members` (プレイヤー番号) を Prim 法でつなぐ。
    /// 返り値は、各メンバーを木につないだ辺の費用 (`members` と同じ順) と、その合計。
    fn prim(&self, members: &[usize]) -> (Vec<f64>, f64) {
        let m = members.len();
        let mut in_tree = vec![false; m];
        // best[k]: メンバー k を今の木につなぐ最小の費用 (最初は供給元から)。
        let mut best: Vec<f64> = members.iter().map(|&i| self.costs[0][i + 1]).collect();
        let mut paid = vec![0.0; m];
        let mut total = 0.0;
        for _ in 0..m {
            let k = (0..m)
                .filter(|&k| !in_tree[k])
                .min_by(|&a, &b| best[a].total_cmp(&best[b]).then(a.cmp(&b)))
                .expect("残りのメンバーがいる");
            in_tree[k] = true;
            paid[k] = best[k];
            total += best[k];
            for other in 0..m {
                if !in_tree[other] {
                    let c = self.costs[members[k] + 1][members[other] + 1];
                    if c < best[other] {
                        best[other] = c;
                    }
                }
            }
        }
        (paid, total)
    }

    /// 提携の費用 (供給元と提携の頂点だけを使う最小全域木の費用)。
    pub fn cost(&self, coalition: &PlayerSet) -> f64 {
        let members: Vec<usize> = coalition.members().collect();
        self.prim(&members).1
    }

    /// Bird 規則による費用の分担 (コアに属する)。
    pub fn bird_rule(&self) -> Vec<f64> {
        let members: Vec<usize> = (0..self.costs.len() - 1).collect();
        self.prim(&members).0
    }

    /// 仁による費用の分担 (全提携の表を作って逐次 LP で求める)。
    pub fn nucleolus_costs(&self) -> Result<Vec<f64>> {
        let savings = nucleolus::nucleolus(&tabulate(self)?)?.allocation;
        Ok(crate::cost::shares_from_savings(
            &self.costs[0][1..],
            &savings,
        ))
    }
}

impl SetFunction for SpanningTreeGame {
    fn players(&self) -> usize {
        self.costs.len() - 1
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        let standalone: f64 = coalition.members().map(|i| self.costs[0][i + 1]).sum();
        standalone - self.cost(coalition)
    }
}
