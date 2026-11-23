//! 全提携を列挙せずに仁を求めるための、ゲームの能力。
//!
//! 明示ベクトル ([`ExplicitGame`]) は `2^n` 個の値を持つため 30 人程度が上限である。
//! ここでは、提携の値 ([`SetFunction`]) に加えて「超過 `v(S) - x(S)` の大きい順に提携を返す」機能だけをゲームに求める。
//! 仁の制約生成 ([`crate::nucleolus::oracle`]) は、違反する提携をこの順序で探すので、全提携を列挙しない。
//!
//! - [`Separation`] は違反する提携を返せるゲーム (仁の制約生成が必要とする最小の能力)
//! - [`OracleGame`] は超過の大きい順に提携を返せるゲーム。自動で [`Separation`] も満たす
//!
//! オラクルが全ての真部分提携について答えるかを [`Separation::covers_all_coalitions`]
//! ([`OracleGame::lists_all_coalitions`]) で問い合わせる。一部の提携しか扱わないオラクル
//! (サンプルした提携だけを持つ [`crate::nucleolus::sampled::SampledGame`] など) で求めた仁は、
//! 元のゲームの仁とは限らないので、[`crate::nucleolus::oracle`] は保証を近似 ([`crate::Guarantee::Approximate`]) にする。

use std::collections::HashSet;

use crate::game::{ExplicitGame, PlayerSet, SetFunction};

/// 超過の大きい順に提携を返せるゲーム。
pub trait OracleGame: SetFunction {
    /// 真部分提携 (空でも全体でもない提携) を超過 `v(S) - x(S)` の非増加順に返す。
    /// 返す値は超過で、同じ超過の提携の順序は実装が決めてよい。
    ///
    /// `x` の長さはプレイヤー数と同じでなければならない。異なる場合、実装はパニックしてよい
    /// (空の列を返すと「違反する提携はない」と解釈されるため、黙って続けてはならない)。
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a>;

    /// 特性関数の値の大きさの目安 (許容誤差の基準)。
    fn scale(&self) -> f64;

    /// [`Separation::violated_outside_span`] の実装。一括実装 (`impl<G: OracleGame> Separation for G`) がこれを呼ぶ。
    ///
    /// 契約は [`Separation::violated_outside_span`] と同じ。既定は [`OracleGame::excess_order`] の先頭から、
    /// `skip(S)` が偽で超過が `bound` を超える提携を集める (`fixed` は使わない)。確定済みの空間に入る提携のうち
    /// 超過の大きいものが多いゲームでは、それを 1 つずつ読み飛ばすことになるので、確定済みの空間を使って
    /// 探せるゲーム ([`crate::games::voting::WeightedVotingGame`]) は上書きする。
    fn search_outside_span(
        &self,
        x: &[f64],
        bound: f64,
        fixed: &[PlayerSet],
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        let _ = fixed;
        violated_in_order(self.excess_order(x), bound, skip, limit)
    }

    /// [`OracleGame::excess_order`] が全ての真部分提携を返すか。
    ///
    /// 既定は `true` で、これは「全ての真部分提携を超過の順に返す」という [`OracleGame::excess_order`]
    /// の契約そのものである。サンプルした提携や値が分かる提携など、一部の提携だけを返すゲームは `false` を返す。
    /// `false` のゲームで求めた仁・最小コア ([`crate::nucleolus::oracle`]) の保証は
    /// [`crate::Guarantee::Approximate`] になる。
    fn lists_all_coalitions(&self) -> bool {
        true
    }
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

    /// 確定済みの提携 `fixed` の特性ベクトルが張る空間に入らない提携に限って、
    /// [`Separation::violated`] と同じく違反する提携を返す。
    ///
    /// 仁の制約生成 ([`crate::nucleolus::oracle`]) は、超過が確定した提携 (`fixed` の特性ベクトルの線形結合で
    /// 書ける提携) を探索から除く。超過の大きい提携の多くが確定済みになる段では、`skip` で 1 つずつ除くと
    /// 指数個の提携を読むことがある。このメソッドは `fixed` を受け取るので、確定済みの空間を使って探索の段階で
    /// 除ける。
    ///
    /// 契約:
    /// - 返す提携は、超過が `bound` より大きく、`skip(S)` が偽で、`fixed` の特性ベクトルの張る空間に入らない
    ///   真部分提携。最大 `limit` 個
    /// - そのような提携が 1 つでもあれば、空でない列を返す (空の列は「違反する提携はない」という主張)
    /// - 呼び出し側は、`fixed` の張る空間に入る提携に対して `skip` が真になるようにする。
    ///   そのため、`fixed` を使わずに [`Separation::violated`] に委ねても契約を満たす
    ///
    /// 既定の実装は [`Separation::violated`] に委ねる。
    fn violated_outside_span(
        &self,
        x: &[f64],
        bound: f64,
        fixed: &[PlayerSet],
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        let _ = fixed;
        self.violated(x, bound, skip, limit)
    }

    /// 特性関数の値の大きさの目安 (許容誤差の基準)。
    fn value_scale(&self) -> f64;

    /// [`Separation::violated`] が全ての真部分提携を対象に答えるか。
    ///
    /// 既定は `true` で、これは「条件を満たす真部分提携が 1 つでもあれば返す」という
    /// [`Separation::violated`] の契約そのものである。一部の提携 (サンプルした提携など) だけを対象にするゲームは
    /// `false` を返す。`false` のゲームで求めた仁・最小コア ([`crate::nucleolus::oracle`]) の保証は
    /// [`crate::Guarantee::Approximate`] になる。
    fn covers_all_coalitions(&self) -> bool {
        true
    }
}

/// 超過順に列挙できるゲームは、先頭から条件を満たすものを集めれば分離オラクルになる。
/// 対象の提携は [`OracleGame::excess_order`] が返す提携と同じ ([`OracleGame::lists_all_coalitions`])。
impl<G: OracleGame + ?Sized> Separation for G {
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        violated_in_order(self.excess_order(x), bound, skip, limit)
    }

    fn violated_outside_span(
        &self,
        x: &[f64],
        bound: f64,
        fixed: &[PlayerSet],
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        self.search_outside_span(x, bound, fixed, skip, limit)
    }

    fn value_scale(&self) -> f64 {
        self.scale()
    }

    fn covers_all_coalitions(&self) -> bool {
        self.lists_all_coalitions()
    }
}

/// 超過の非増加順の列の先頭から、`skip(S)` が偽で超過が `bound` を超える提携を最大 `limit` 個集める。
pub(crate) fn violated_in_order(
    order: impl Iterator<Item = (PlayerSet, f64)>,
    bound: f64,
    skip: &dyn Fn(&PlayerSet) -> bool,
    limit: usize,
) -> Vec<(PlayerSet, f64)> {
    let mut found = Vec::new();
    for (coalition, excess) in order {
        if excess <= bound || found.len() >= limit {
            break;
        }
        if !skip(&coalition) {
            found.push((coalition, excess));
        }
    }
    found
}

/// 明示ゲームは全提携の超過を、読んだ分だけ並べ替えて返す (オラクルの正しさを確かめる基準)。
impl OracleGame for ExplicitGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        let n = ExplicitGame::players(self);
        Box::new(
            crate::surplus::excess_order(self, x)
                .map(move |(coalition, excess)| (PlayerSet::from_coalition(n, coalition), excess)),
        )
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
