//! 重み付き投票ゲームのオラクル。
//!
//! `v(S) = 1` (重みの和が基準 `q` 以上) / `0` (未満) なので、超過は勝利提携で `1 - x(S)`、
//! 敗北提携で `-x(S)` になる。重みの和を `q` で打ち切った値を状態とする動的計画法で、
//! 各状態について `x(S)` の小さい提携を上位 `k` 個求め、超過の大きい順に並べる
//! (計算量は `O(n q k log k)`、重みが整数で小さいときに有効)。
//! `k` 個を使い切ったら `k` を倍にして計算し直す。同じ超過の提携は提携の全順序で並べるので、
//! 計算し直しても既に返した列の順序は変わらない。
//!
//! 重み付き投票ゲームの仁を重みについて擬多項式時間で計算するアルゴリズムは Pashkovich (2022) が示した。
//! それ以前の Elkind & Pasechnik (2009) のアルゴリズムは、仁を計算する保証がないと後続研究で指摘されている。
//! ここでの動的計画法は、制約生成で違反する提携を探すためのもので、計算量の保証はない。

use super::{OracleGame, PlayerSet, SetFunction};
use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedVotingGame {
    weights: Vec<u64>,
    quota: u64,
}

impl WeightedVotingGame {
    /// 動的計画法の状態数 `q + 1` が大きすぎないよう、基準の上限を設ける。
    pub const MAX_QUOTA: u64 = 1_000_000;

    pub fn new(weights: Vec<u64>, quota: u64) -> Result<WeightedVotingGame> {
        if quota > Self::MAX_QUOTA {
            return Err(Error::InvalidArgument(format!(
                "基準 {quota} は上限 {} を超える",
                Self::MAX_QUOTA
            )));
        }
        Ok(WeightedVotingGame { weights, quota })
    }

    pub fn weights(&self) -> &[u64] {
        &self.weights
    }

    pub fn quota(&self) -> u64 {
        self.quota
    }

    fn weight(&self, coalition: &PlayerSet) -> u64 {
        coalition.members().map(|i| self.weights[i]).sum()
    }

    /// 超過の大きい順 (同じなら提携の昇順) に、真部分提携を最大 `k` 個返す。
    fn top(&self, x: &[f64], k: usize) -> Vec<(PlayerSet, f64)> {
        let n = self.weights.len();
        let q = self.quota as usize;
        let keep = k + 2; // 空提携と全体提携を除いても k 個残るように
        let by_sum = |a: &(f64, PlayerSet), b: &(f64, PlayerSet)| {
            a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1))
        };
        // states[c]: 打ち切った重みの和が c の部分集合のうち、x(S) の小さい上位 keep 個
        let mut states: Vec<Vec<(f64, PlayerSet)>> = vec![Vec::new(); q + 1];
        states[0].push((0.0, PlayerSet::empty(n)));
        for (i, (&weight, &payoff)) in self.weights.iter().zip(x).enumerate() {
            let mut next: Vec<Vec<(f64, PlayerSet)>> = vec![Vec::new(); q + 1];
            for (c, list) in states.iter().enumerate() {
                let reached = (c + weight as usize).min(q);
                for (sum, set) in list {
                    next[c].push((*sum, set.clone()));
                    let mut with = set.clone();
                    with.insert(i);
                    next[reached].push((sum + payoff, with));
                }
            }
            for list in &mut next {
                list.sort_by(by_sum);
                list.truncate(keep);
            }
            states = next;
        }
        let mut result: Vec<(PlayerSet, f64)> = Vec::new();
        for (c, list) in states.into_iter().enumerate() {
            let value = if c == q { 1.0 } else { 0.0 };
            for (sum, set) in list {
                if set.is_proper() {
                    result.push((set, value - sum));
                }
            }
        }
        result.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        result.truncate(k);
        result
    }
}

impl SetFunction for WeightedVotingGame {
    fn players(&self) -> usize {
        self.weights.len()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if self.weight(coalition) >= self.quota {
            1.0
        } else {
            0.0
        }
    }
}

struct VotingExcess<'a> {
    game: &'a WeightedVotingGame,
    x: &'a [f64],
    k: usize,
    list: Vec<(PlayerSet, f64)>,
    index: usize,
    exhausted: bool,
}

impl Iterator for VotingExcess<'_> {
    type Item = (PlayerSet, f64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.list.len() {
            if self.exhausted {
                return None;
            }
            self.k *= 2;
            self.list = self.game.top(self.x, self.k);
            self.exhausted = self.list.len() < self.k;
            if self.index >= self.list.len() {
                return None;
            }
        }
        self.index += 1;
        Some(self.list[self.index - 1].clone())
    }
}

impl OracleGame for WeightedVotingGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        Box::new(VotingExcess {
            game: self,
            x,
            k: 32,
            list: Vec::new(),
            index: 0,
            exhausted: false,
        })
    }

    fn scale(&self) -> f64 {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    #[test]
    fn matches_explicit_voting_game() {
        let weights = vec![4, 1, 3, 2, 2, 5, 1];
        let quota = 10;
        let oracle = WeightedVotingGame::new(weights.clone(), quota).unwrap();
        let explicit = generators::weighted_voting(
            &weights.iter().map(|w| *w as f64).collect::<Vec<_>>(),
            quota as f64,
        )
        .unwrap();
        let x = [0.2, -0.1, 0.15, 0.1, 0.05, 0.3, 0.3];
        let from_oracle: Vec<(PlayerSet, f64)> = oracle.excess_order(&x).collect();
        let from_explicit: Vec<(PlayerSet, f64)> = explicit.excess_order(&x).collect();
        // 全 126 個の真部分提携を、k を倍にしながら全て返す。
        assert_eq!(from_oracle.len(), 126);
        for ((_, a), (_, b)) in from_oracle.iter().zip(&from_explicit) {
            assert!((a - b).abs() < 1e-12);
        }
        for (set, e) in &from_oracle {
            assert!((oracle.value(set) - set.sum(&x) - e).abs() < 1e-12);
        }
    }
}
