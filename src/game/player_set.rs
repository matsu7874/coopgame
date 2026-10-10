//! 任意の人数の提携 ([`PlayerSet`]) と、提携の値を返すゲーム ([`SetFunction`])。

use crate::game::{Coalition, ExplicitGame};

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
        assert!(PlayerSet::full(130).is_full());
        assert!(!PlayerSet::full(130).is_proper());
        let small = PlayerSet::from_players(5, &[1, 3]);
        assert_eq!(small.to_coalition(), Some(Coalition::from_players(&[1, 3])));
        assert_eq!(
            PlayerSet::from_coalition(5, Coalition::from_players(&[1, 3])),
            small
        );
    }
}
