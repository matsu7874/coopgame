//! 部分集合を重みの和 `offset + sum_{i in S} w_i` の大きい順に遅延列挙する。
//!
//! 正の重みを全て選んだ集合が最大で、そこから要素 `i` を反転 (入れる/外す) すると和は `|w_i|` 減る。
//! 反転する要素の集合を、反転コストの和の小さい順に列挙すればよい。
//! コストを昇順に並べた添字 `0..n` 上で、「最後の要素 `j` の次 `j + 1` を追加する」と
//! 「最後の要素 `j` を `j + 1` に置き換える」の 2 通りで展開すると、
//! 空でない全ての反転集合がちょうど 1 回ずつ、コストの非減少順に現れる。

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::PlayerSet;

/// 反転集合の候補。コストの小さい順 (同じなら生成順) に取り出すため、順序を逆にする。
struct Candidate {
    cost: f64,
    sequence: u64,
    /// コスト順の添字で、最後に追加した要素。
    last: usize,
    toggled: Vec<usize>,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then(other.sequence.cmp(&self.sequence))
    }
}

/// 全ての部分集合 (空集合・全体を含む) を和の非増加順に返す。
pub struct SubsetsByWeight {
    players: usize,
    best: PlayerSet,
    best_sum: f64,
    /// 反転コストの昇順に並べたプレイヤー。
    order: Vec<usize>,
    costs: Vec<f64>,
    heap: BinaryHeap<Candidate>,
    sequence: u64,
    started: bool,
}

impl SubsetsByWeight {
    pub fn new(weights: &[f64], offset: f64) -> SubsetsByWeight {
        let players = weights.len();
        let mut best = PlayerSet::empty(players);
        let mut best_sum = offset;
        for (i, &w) in weights.iter().enumerate() {
            if w > 0.0 {
                best.insert(i);
                best_sum += w;
            }
        }
        let mut order: Vec<usize> = (0..players).collect();
        order.sort_by(|&a, &b| {
            weights[a]
                .abs()
                .total_cmp(&weights[b].abs())
                .then(a.cmp(&b))
        });
        let costs: Vec<f64> = order.iter().map(|&i| weights[i].abs()).collect();
        SubsetsByWeight {
            players,
            best,
            best_sum,
            order,
            costs,
            heap: BinaryHeap::new(),
            sequence: 0,
            started: false,
        }
    }

    fn push(&mut self, cost: f64, last: usize, toggled: Vec<usize>) {
        self.sequence += 1;
        self.heap.push(Candidate {
            cost,
            sequence: self.sequence,
            last,
            toggled,
        });
    }
}

impl Iterator for SubsetsByWeight {
    type Item = (PlayerSet, f64);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            if self.players > 0 {
                self.push(self.costs[0], 0, vec![0]);
            }
            return Some((self.best.clone(), self.best_sum));
        }
        let candidate = self.heap.pop()?;
        let j = candidate.last;
        if j + 1 < self.players {
            let mut added = candidate.toggled.clone();
            added.push(j + 1);
            self.push(candidate.cost + self.costs[j + 1], j + 1, added);
            let mut replaced = candidate.toggled.clone();
            *replaced.last_mut().expect("空でない") = j + 1;
            self.push(
                candidate.cost - self.costs[j] + self.costs[j + 1],
                j + 1,
                replaced,
            );
        }
        let mut set = self.best.clone();
        for &k in &candidate.toggled {
            set.toggle(self.order[k]);
        }
        Some((set, self.best_sum - candidate.cost))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerates_all_subsets_in_order() {
        let weights = [3.0, -1.0, 0.5, -2.5, 2.0, 0.0];
        let all: Vec<(PlayerSet, f64)> = SubsetsByWeight::new(&weights, 1.5).collect();
        assert_eq!(all.len(), 64);
        assert!(all.windows(2).all(|w| w[0].1 >= w[1].1 - 1e-12));
        let distinct: std::collections::HashSet<_> = all.iter().map(|(s, _)| s.clone()).collect();
        assert_eq!(distinct.len(), 64);
        for (set, sum) in &all {
            let expected: f64 = 1.5 + set.members().map(|i| weights[i]).sum::<f64>();
            assert!((sum - expected).abs() < 1e-12);
        }
    }
}
