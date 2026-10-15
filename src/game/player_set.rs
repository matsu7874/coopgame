//! 任意の人数の提携 ([`PlayerSet`]) と、提携の値を返すゲーム ([`SetFunction`])。

use std::fmt;

use crate::error::{Error, Result};
use crate::game::coalition::write_members;
use crate::game::{Coalition, ExplicitGame};

/// 任意の人数のプレイヤーの集合。プレイヤー `i` は `words[i / 64]` のビット `i % 64` に対応する。
///
/// 64 人までの集合は [`Coalition`]。2 つの型は同じ名前の操作を持つ。
/// `Coalition` へは `Coalition::try_from(&set)` で、`Coalition` からは [`PlayerSet::from_coalition`] で変換する。
/// 表示 (`Display`) は 0 始まりの番号を `{0, 2}` の形で並べる。
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerSet {
    players: usize,
    words: Vec<u64>,
}

impl PlayerSet {
    /// `players` 人のうちの空提携。
    pub fn empty(players: usize) -> PlayerSet {
        PlayerSet {
            players,
            words: vec![0; players.div_ceil(64)],
        }
    }

    /// `players` 人全員からなる全体提携。
    pub fn grand(players: usize) -> PlayerSet {
        PlayerSet::empty(players).complement()
    }

    /// # Panics
    ///
    /// `player` が人数以上だとパニックする。
    pub fn singleton(players: usize, player: usize) -> PlayerSet {
        PlayerSet::from_players(players, &[player])
    }

    /// # Panics
    ///
    /// `members` に人数以上の番号があるとパニックする。
    pub fn from_players(players: usize, members: &[usize]) -> PlayerSet {
        let mut set = PlayerSet::empty(players);
        for &i in members {
            set.insert(i);
        }
        set
    }

    /// `players` 人のうちの、`coalition` と同じメンバーの集合。
    ///
    /// # Panics
    ///
    /// `coalition` に `players` 以上の番号があるとパニックする。
    pub fn from_coalition(players: usize, coalition: Coalition) -> PlayerSet {
        assert!(
            coalition.is_subset(Coalition::grand(players.min(64))),
            "提携 {coalition} に人数 {players} 以上の番号がある"
        );
        let mut set = PlayerSet::empty(players);
        if let Some(word) = set.words.first_mut() {
            *word = coalition.0;
        }
        set
    }

    /// 64 人以下の集合を `coalition` に置き換える (割り当てずに使い回すため)。
    pub(crate) fn assign(&mut self, coalition: Coalition) {
        debug_assert!(self.players <= 64);
        if let Some(word) = self.words.first_mut() {
            *word = coalition.0;
        }
    }

    /// 64 人以下なら [`Coalition`] に変換する。
    pub(crate) fn to_coalition(&self) -> Option<Coalition> {
        match self.words.as_slice() {
            [] => Some(Coalition::empty()),
            [word] => Some(Coalition(*word)),
            _ => None,
        }
    }

    /// 全体のプレイヤー数。
    pub(crate) fn universe(&self) -> usize {
        self.players
    }

    /// # Panics
    ///
    /// `player` が人数以上だとパニックする。
    pub fn insert(&mut self, player: usize) {
        assert!(
            player < self.players,
            "プレイヤー番号 {player} が人数 {} 以上",
            self.players
        );
        self.words[player / 64] |= 1 << (player % 64);
    }

    /// `player` を除く。人数以上の番号はメンバーでないので何もしない。
    pub fn remove(&mut self, player: usize) {
        if player < self.players {
            self.words[player / 64] &= !(1 << (player % 64));
        }
    }

    pub(crate) fn toggle(&mut self, player: usize) {
        self.words[player / 64] ^= 1 << (player % 64);
    }

    /// `player` がメンバーか。人数以上の番号は `false`。
    pub fn contains(&self, player: usize) -> bool {
        player < self.players && self.words[player / 64] >> (player % 64) & 1 == 1
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

    /// 2 つの集合の語ごとの演算。人数が違えばパニックする。
    fn combine(&self, other: &PlayerSet, op: impl Fn(u64, u64) -> u64) -> PlayerSet {
        assert_eq!(
            self.players, other.players,
            "人数の違う集合は組み合わせられない"
        );
        PlayerSet {
            players: self.players,
            words: self
                .words
                .iter()
                .zip(&other.words)
                .map(|(&a, &b)| op(a, b))
                .collect(),
        }
    }

    pub fn union(&self, other: &PlayerSet) -> PlayerSet {
        self.combine(other, |a, b| a | b)
    }

    pub fn intersection(&self, other: &PlayerSet) -> PlayerSet {
        self.combine(other, |a, b| a & b)
    }

    pub fn is_subset(&self, other: &PlayerSet) -> bool {
        assert_eq!(self.players, other.players, "人数の違う集合は比べられない");
        self.words
            .iter()
            .zip(&other.words)
            .all(|(a, b)| a & !b == 0)
    }

    /// この提携に入らないプレイヤーの提携。
    pub fn complement(&self) -> PlayerSet {
        let mut words: Vec<u64> = self.words.iter().map(|w| !w).collect();
        // 最後の語の、人数を超えるビットを落とす。
        if let (Some(last), rest @ 1..) = (words.last_mut(), self.players % 64) {
            *last &= (1u64 << rest) - 1;
        }
        PlayerSet {
            players: self.players,
            words,
        }
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

impl fmt::Display for PlayerSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_members(f, self.members())
    }
}

impl TryFrom<&PlayerSet> for Coalition {
    type Error = Error;

    /// 64 人以下の集合を変換する。それより多ければ [`Error::TooManyPlayers`]。
    fn try_from(set: &PlayerSet) -> Result<Coalition> {
        set.to_coalition().ok_or(Error::TooManyPlayers {
            players: set.players,
            max: 64,
        })
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
/// 全体提携、1 人提携、`n - 1` 人提携の値の絶対値の最大 (これらが全て 0 なら 1)。評価は `2n + 1` 回。
/// ゲームの値を `c` 倍すると、この値も `c` 倍になる。
pub fn value_scale<G: SetFunction + ?Sized>(game: &G) -> f64 {
    let n = game.players();
    let mut scale = game.value(&PlayerSet::grand(n)).abs();
    for i in 0..n {
        let single = PlayerSet::singleton(n, i);
        scale = scale
            .max(game.value(&single).abs())
            .max(game.value(&single.complement()).abs());
    }
    crate::game::scale::positive_scale(scale)
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
        assert!(PlayerSet::grand(130).is_full());
        assert!(!PlayerSet::grand(130).is_proper());
        let small = PlayerSet::from_players(5, &[1, 3]);
        assert_eq!(small.to_coalition(), Some(Coalition::from_players(&[1, 3])));
        assert_eq!(
            PlayerSet::from_coalition(5, Coalition::from_players(&[1, 3])),
            small
        );
    }

    #[test]
    fn same_operations_as_coalition() {
        // 64 人以下では、Coalition と同じ結果になる。
        let (a, b) = (
            Coalition::from_players(&[0, 2]),
            Coalition::from_players(&[2, 3]),
        );
        let (sa, sb) = (
            PlayerSet::from_coalition(5, a),
            PlayerSet::from_coalition(5, b),
        );
        let same = |set: PlayerSet, c: Coalition| {
            assert_eq!(Coalition::try_from(&set).unwrap(), c);
            assert_eq!(set.to_string(), c.to_string());
        };
        same(sa.union(&sb), a.union(b));
        same(sa.intersection(&sb), a.intersection(b));
        same(sa.complement(), a.complement(5));
        same(PlayerSet::grand(5), Coalition::grand(5));
        same(PlayerSet::singleton(5, 4), Coalition::singleton(4));
        same(PlayerSet::empty(5), Coalition::empty());
        assert!(sa.intersection(&sb).is_subset(&sa) && !sa.is_subset(&sb));
        assert_eq!(a.to_string(), "{0, 2}");
        assert_eq!(Coalition::empty().to_string(), "{}");
    }

    #[test]
    fn large_sets() {
        let mut set = PlayerSet::from_players(130, &[0, 64, 129]);
        assert_eq!(set.to_string(), "{0, 64, 129}");
        assert_eq!(set.complement().len(), 127);
        assert!(Coalition::try_from(&set).is_err());
        set.remove(64);
        set.insert(1);
        assert_eq!(set.members().collect::<Vec<_>>(), vec![0, 1, 129]);
    }
}
