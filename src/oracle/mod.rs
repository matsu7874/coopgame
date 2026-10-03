//! 全提携を列挙しないゲームの表現 (オラクル)。
//!
//! 明示ベクトル ([`crate::ExplicitGame`]) は `2^n` 個の値を持つため 30 人程度が上限である。
//! ここでは、提携の値と「超過 `v(S) - x(S)` の大きい順に提携を返す」機能だけをゲームに求める。
//! 仁の制約生成 ([`nucleolus`]) は、違反する提携をこの順序で探すので、全提携を列挙しない。
//!
//! - [`PlayerSet`] は任意の人数の提携
//! - [`SetFunction`] は提携の値を返すゲーム (Shapley 値などのサンプリングに使う)
//! - [`Separation`] は違反する提携を返せるゲーム (仁の制約生成が必要とする最小の能力)
//! - [`OracleGame`] は超過の大きい順に提携を返せるゲーム。自動で [`Separation`] も満たす
//! - [`tabulate`] は値を返すだけのゲームから全提携の値の表を作る (総当たり)

pub mod airport;
pub mod bankruptcy;
pub mod graph;
pub mod nucleolus;
pub mod production;
pub mod spanning_tree;
pub(crate) mod subsets;
pub mod voting;

use std::collections::HashSet;

use crate::coalition::Coalition;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;

/// 任意の人数の提携。プレイヤー `i` は `words[i / 64]` のビット `i % 64` に対応する。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerSet {
    players: usize,
    words: Vec<u64>,
}

impl PlayerSet {
    pub(crate) fn empty(players: usize) -> PlayerSet {
        PlayerSet {
            players,
            words: vec![0; players.div_ceil(64)],
        }
    }

    pub fn full(players: usize) -> PlayerSet {
        let mut set = PlayerSet::empty(players);
        for i in 0..players {
            set.insert(i);
        }
        set
    }

    pub fn from_players(players: usize, members: &[usize]) -> PlayerSet {
        let mut set = PlayerSet::empty(players);
        for &i in members {
            set.insert(i);
        }
        set
    }

    pub fn from_coalition(players: usize, coalition: Coalition) -> PlayerSet {
        let mut set = PlayerSet::empty(players);
        if let Some(word) = set.words.first_mut() {
            *word = coalition.0;
        }
        set
    }

    /// 64 人以下なら [`Coalition`] に変換する。
    pub(crate) fn to_coalition(&self) -> Option<Coalition> {
        match self.words.as_slice() {
            [] => Some(Coalition::EMPTY),
            [word] => Some(Coalition(*word)),
            _ => None,
        }
    }

    /// 全体のプレイヤー数。
    pub(crate) fn universe(&self) -> usize {
        self.players
    }

    pub(crate) fn insert(&mut self, player: usize) {
        debug_assert!(player < self.players);
        self.words[player / 64] |= 1 << (player % 64);
    }

    pub fn remove(&mut self, player: usize) {
        self.words[player / 64] &= !(1 << (player % 64));
    }

    pub(crate) fn toggle(&mut self, player: usize) {
        self.words[player / 64] ^= 1 << (player % 64);
    }

    pub fn contains(&self, player: usize) -> bool {
        self.words[player / 64] >> (player % 64) & 1 == 1
    }

    pub fn len(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    pub(crate) fn is_full(&self) -> bool {
        self.len() == self.players
    }

    /// 空でも全体でもない (真部分提携)。
    pub(crate) fn is_proper(&self) -> bool {
        !self.is_empty() && !self.is_full()
    }

    pub(crate) fn complement(&self) -> PlayerSet {
        let mut set = PlayerSet::full(self.players);
        for (word, own) in set.words.iter_mut().zip(&self.words) {
            *word &= !own;
        }
        set
    }

    /// メンバーを昇順に列挙する。
    pub fn members(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(index, &word)| {
            let mut rest = word;
            std::iter::from_fn(move || {
                if rest == 0 {
                    return None;
                }
                let bit = rest.trailing_zeros() as usize;
                rest &= rest - 1;
                Some(index * 64 + bit)
            })
        })
    }

    /// `x(S)`。
    pub fn sum(&self, x: &[f64]) -> f64 {
        self.members().map(|i| x[i]).sum()
    }

    /// 特性ベクトル。
    pub(crate) fn indicator(&self) -> Vec<f64> {
        (0..self.players)
            .map(|i| if self.contains(i) { 1.0 } else { 0.0 })
            .collect()
    }
}

/// 提携の値を返すゲーム。
pub trait SetFunction {
    fn players(&self) -> usize;
    /// 提携の値。空提携は 0 を返す。
    fn value(&self, coalition: &PlayerSet) -> f64;
}

/// 参照も同じゲームとして扱う (`Assume::convex(&game)` などで所有権を渡さずに包むため)。
impl<G: SetFunction + ?Sized> SetFunction for &G {
    fn players(&self) -> usize {
        (**self).players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        (**self).value(coalition)
    }
}

/// 全提携を列挙せずに得られる、特性関数の値の大きさの目安 (許容誤差の基準)。
///
/// 全体提携、1 人提携、`n - 1` 人提携の値の絶対値の最大 (1 以上)。評価は `2n + 1` 回。
pub fn value_scale<G: SetFunction + ?Sized>(game: &G) -> f64 {
    let n = game.players();
    let mut scale = game.value(&PlayerSet::full(n)).abs().max(1.0);
    for i in 0..n {
        let single = PlayerSet::from_players(n, &[i]);
        scale = scale
            .max(game.value(&single).abs())
            .max(game.value(&single.complement()).abs());
    }
    scale
}

/// 値を返すだけのゲームから、全提携の値の表 ([`ExplicitGame`]) を作る (総当たり、30 人まで)。
///
/// どんなゲームにも、明示ベクトル版の全ての手法 (逐次 LP の仁、カーネル全体、厳密な検証など) を
/// 適用するための入口。特性関数を `2^n - 1` 回評価する。
pub fn tabulate<G: SetFunction + ?Sized>(game: &G) -> Result<ExplicitGame> {
    let n = game.players();
    if n == 0 || n > crate::game::MAX_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: crate::game::MAX_PLAYERS,
        });
    }
    ExplicitGame::from_fn(n, |coalition| {
        game.value(&PlayerSet::from_coalition(n, coalition))
    })
}

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
/// 仁の制約生成 ([`nucleolus`]) が必要とする最小の能力。[`OracleGame`] (超過の大きい順に全て返せる)
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

impl SetFunction for ExplicitGame {
    fn players(&self) -> usize {
        ExplicitGame::players(self)
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        let coalition = coalition.to_coalition().expect("明示ゲームは 30 人以下");
        ExplicitGame::value(self, coalition)
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
    fn player_set_basics() {
        let mut set = PlayerSet::from_players(130, &[0, 63, 64, 129]);
        assert_eq!(set.len(), 4);
        assert!(set.contains(64) && set.contains(129) && !set.contains(1));
        assert_eq!(set.members().collect::<Vec<_>>(), vec![0, 63, 64, 129]);
        assert_eq!(set.complement().len(), 126);
        set.toggle(64);
        assert!(!set.contains(64));
        assert!(PlayerSet::full(130).is_full());
        assert!(!PlayerSet::full(130).is_proper());
        let small = PlayerSet::from_players(5, &[1, 3]);
        assert_eq!(small.to_coalition(), Some(Coalition::from_players(&[1, 3])));
        assert_eq!(
            PlayerSet::from_coalition(5, Coalition::from_players(&[1, 3])),
            small
        );
    }

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
