//! 全提携を列挙せずに仁を求めるための、ゲームの能力。
//!
//! 明示ベクトル ([`ExplicitGame`]) は `2^n` 個の値を持つため 30 人程度が上限である。
//! ここでは、提携の値 ([`SetFunction`]) に加えて「超過 `v(S) - x(S)` の大きい順に提携を返す」機能だけをゲームに求める。
//! 仁の制約生成 ([`crate::nucleolus::oracle`]) は、違反する提携をこの順序で探すので、全提携を列挙しない。
//!
//! - [`Separation`] は違反する提携を返せるゲーム (仁の制約生成が必要とする最小の能力)
//! - [`OracleGame`] は超過の大きい順に提携を返せるゲーム。自動で [`Separation`] も満たす

use std::collections::HashSet;

use crate::game::{Coalition, ExplicitGame, PlayerSet, SetFunction};

/// 超過の大きい順に提携を返せるゲーム。
pub trait OracleGame: SetFunction {
    /// 真部分提携 (空でも全体でもない提携) を超過 `v(S) - x(S)` の非増加順に返す。
    /// 返す値は超過で、同じ超過の提携の順序は実装が決めてよい。
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a>;

    /// 特性関数の値の大きさの目安 (許容誤差の基準)。
    fn scale(&self) -> f64;
}

/// 分離オラクル: 配分 `x` で超過がしきい値を超える提携を返せるゲーム。
///
/// 仁の制約生成 ([`crate::nucleolus::oracle`]) が必要とする最小の能力。[`OracleGame`] (超過の大きい順に全て返せる)
/// を実装したゲームは、一括実装により自動でこれも満たす。超過順の列挙はできないが、
/// 違反する提携なら効率よく見つけられる構造のゲームは、これだけを実装すればよい。
pub trait Separation: SetFunction {
    /// 超過 `v(S) - x(S)` が `bound` より大きく、`skip(S)` が偽の真部分提携を、最大 `limit` 個返す。
    ///
    /// 返す組は `(提携, 超過)`。超過の大きいものを優先して返すのが望ましいが、必須ではない。
    /// 条件を満たす提携が 1 つでもあれば、空でない列を返さなければならない
    /// (空の列は「違反する提携はない」という主張として扱う)。
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)>;

    /// 特性関数の値の大きさの目安 (許容誤差の基準)。
    fn value_scale(&self) -> f64;
}

/// 超過順に列挙できるゲームは、先頭から条件を満たすものを集めれば分離オラクルになる。
impl<G: OracleGame + ?Sized> Separation for G {
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        let mut found = Vec::new();
        for (coalition, excess) in self.excess_order(x) {
            if excess <= bound || found.len() >= limit {
                break;
            }
            if !skip(&coalition) {
                found.push((coalition, excess));
            }
        }
        found
    }

    fn value_scale(&self) -> f64 {
        self.scale()
    }
}

/// 明示ゲームは全提携の超過を並べ替えて返す (オラクルの正しさを確かめる基準)。
impl OracleGame for ExplicitGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        let n = ExplicitGame::players(self);
        let full = (1u64 << n) - 1;
        let excess = crate::surplus::excesses(self, x);
        let mut order: Vec<u64> = (1..full).collect();
        order.sort_by(|a, b| {
            excess[*b as usize]
                .total_cmp(&excess[*a as usize])
                .then(a.cmp(b))
        });
        Box::new(order.into_iter().map(move |mask| {
            (
                PlayerSet::from_coalition(n, Coalition(mask)),
                excess[mask as usize],
            )
        }))
    }

    fn scale(&self) -> f64 {
        self.max_abs_value()
    }
}

/// 超過の非増加順の 2 つの列を合わせ、同じ提携は最初 (大きい方) だけを残す。
///
/// 提携の超過が 2 つの値の最大値で決まり、それぞれの列がその片方の値を返すとき、
/// 合わせた列は真の超過の非増加順になる (同じ提携は大きい方の値で先に現れるため)。
pub(crate) struct MaxMerge<A, B>
where
    A: Iterator<Item = (PlayerSet, f64)>,
    B: Iterator<Item = (PlayerSet, f64)>,
{
    first: std::iter::Peekable<A>,
    second: std::iter::Peekable<B>,
    seen: HashSet<PlayerSet>,
}

impl<A, B> MaxMerge<A, B>
where
    A: Iterator<Item = (PlayerSet, f64)>,
    B: Iterator<Item = (PlayerSet, f64)>,
{
    pub fn new(first: A, second: B) -> MaxMerge<A, B> {
        MaxMerge {
            first: first.peekable(),
            second: second.peekable(),
            seen: HashSet::new(),
        }
    }
}

impl<A, B> Iterator for MaxMerge<A, B>
where
    A: Iterator<Item = (PlayerSet, f64)>,
    B: Iterator<Item = (PlayerSet, f64)>,
{
    type Item = (PlayerSet, f64);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let take_first = match (self.first.peek(), self.second.peek()) {
                (None, None) => return None,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (Some(a), Some(b)) => a.1 >= b.1,
            };
            let item = if take_first {
                self.first.next()
            } else {
                self.second.next()
            }?;
            if self.seen.insert(item.0.clone()) {
                return Some(item);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_excess_order_is_sorted() {
        let game = crate::generators::bnf(1, 5, 2).unwrap();
        let x = crate::kernel::equal_surplus_division(&game);
        let order: Vec<(PlayerSet, f64)> = game.excess_order(&x).collect();
        assert_eq!(order.len(), 30);
        assert!(order.windows(2).all(|w| w[0].1 >= w[1].1));
        for (set, e) in &order {
            assert!((SetFunction::value(&game, set) - set.sum(&x) - e).abs() < 1e-12);
        }
    }
}
