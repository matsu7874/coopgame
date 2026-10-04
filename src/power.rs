//! 単純ゲームの投票力指数。
//!
//! 単純ゲームは特性関数が 0 (否決) か 1 (可決) の値だけを取るゲームで、`v(S) = 1` の提携を勝利提携と呼ぶ。
//! 重み付き投票ゲームは [`crate::oracle::tabulate`] で明示ゲームにしてから渡す。
//! Shapley–Shubik 指数は [`crate::values::shapley`]、正規化 Banzhaf 指数は
//! [`crate::values::banzhaf`] と [`crate::values::normalize`] で求める。
//!
//! - 決定票 (swing): 勝利提携 `S` から `i` が抜けると負ける場合、`i` は `S` で決定票を持つ。
//!   `S` で決定票を持つ人の集合を `C(S)` と書く。
//! - 最小勝利提携: 誰が抜けても負ける勝利提携 (`C(S) = S`)。
//!
//! | 指数 | 定義 |
//! |---|---|
//! | Johnston 指数 (Johnston 1978) | 勝利提携 `S` ごとに `C(S)` の各人へ `1/|C(S)|` を配った和を、全員の和が 1 になるよう割る |
//! | Deegan–Packel 指数 (Deegan & Packel 1978) | 最小勝利提携を一様に 1 つ選び、そのメンバーで等分したときの期待値 |
//! | Public Good 指数 (Holler 1982) | `i` を含む最小勝利提携の数を、全員の和が 1 になるよう割る |
//! | Coleman の阻止力 (Coleman 1971) | `i` が決定票を持つ勝利提携の数 / 勝利提携の数 |
//! | Coleman の発議力 (Coleman 1971) | `i` が加わると勝つ敗北提携の数 / 敗北提携の数 (分子は決定票の数に等しい) |
//! | Coleman の集団の行動力 (Coleman 1971) | 勝利提携の数 / `2^n` |

use crate::coalition::Coalition;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;

/// 単純ゲームの勝利提携と決定票の集計。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SimpleGame {
    players: usize,
    /// `wins[mask]`: 提携 `mask` が勝つか。
    wins: Vec<bool>,
    /// 勝利提携。
    pub winning: Vec<Coalition>,
    /// 最小勝利提携。
    pub minimal_winning: Vec<Coalition>,
    /// `swings[i]`: `i` が決定票を持つ勝利提携の数。
    pub swings: Vec<u64>,
}

impl SimpleGame {
    /// 明示ゲームを単純ゲームとして読む。値が 0 と 1 以外の提携があればエラーを返す。
    pub fn new(game: &ExplicitGame) -> Result<SimpleGame> {
        let n = game.players();
        let mut wins = vec![false; 1usize << n];
        for mask in 1..1u64 << n {
            let value = game.value(Coalition(mask));
            wins[mask as usize] = if value == 1.0 {
                true
            } else if value == 0.0 {
                false
            } else {
                return Err(Error::InvalidArgument(format!(
                    "単純ゲームではない: 提携 {mask:#b} の値が {value} (0 か 1 のみ)"
                )));
            };
        }
        let mut winning = Vec::new();
        let mut minimal_winning = Vec::new();
        let mut swings = vec![0u64; n];
        for mask in 1..1u64 << n {
            if !wins[mask as usize] {
                continue;
            }
            let coalition = Coalition(mask);
            let mut critical = 0;
            for i in coalition.players() {
                if !wins[(mask & !(1 << i)) as usize] {
                    swings[i] += 1;
                    critical += 1;
                }
            }
            if critical == coalition.len() {
                minimal_winning.push(coalition);
            }
            winning.push(coalition);
        }
        Ok(SimpleGame {
            players: n,
            wins,
            winning,
            minimal_winning,
            swings,
        })
    }

    pub fn players(&self) -> usize {
        self.players
    }

    /// `S` で決定票を持つ人の集合 `C(S)`。
    fn critical(&self, coalition: Coalition) -> Coalition {
        Coalition(
            coalition
                .players()
                .filter(|&i| !self.wins[(coalition.0 & !(1 << i)) as usize])
                .fold(0, |mask, i| mask | 1 << i),
        )
    }

    fn require_winning(&self) -> Result<()> {
        if self.winning.is_empty() {
            return Err(Error::InvalidArgument("勝利提携がない".into()));
        }
        Ok(())
    }

    /// Johnston 指数 (和は 1)。
    pub fn johnston(&self) -> Result<Vec<f64>> {
        self.require_winning()?;
        let mut raw = vec![0.0; self.players];
        for &coalition in &self.winning {
            let critical = self.critical(coalition);
            let share = 1.0 / critical.len() as f64;
            for i in critical.players() {
                raw[i] += share;
            }
        }
        normalized(raw)
    }

    /// Deegan–Packel 指数 (和は 1)。
    pub fn deegan_packel(&self) -> Result<Vec<f64>> {
        self.require_winning()?;
        let count = self.minimal_winning.len() as f64;
        let mut index = vec![0.0; self.players];
        for coalition in &self.minimal_winning {
            let share = 1.0 / (coalition.len() as f64 * count);
            for i in coalition.players() {
                index[i] += share;
            }
        }
        Ok(index)
    }

    /// Public Good 指数 (Holler 指数、和は 1)。
    pub fn public_good(&self) -> Result<Vec<f64>> {
        self.require_winning()?;
        let mut counts = vec![0.0; self.players];
        for coalition in &self.minimal_winning {
            for i in coalition.players() {
                counts[i] += 1.0;
            }
        }
        normalized(counts)
    }

    /// Coleman の阻止力: `i` が決定票を持つ勝利提携の数 / 勝利提携の数。
    pub fn coleman_prevent(&self) -> Result<Vec<f64>> {
        self.require_winning()?;
        let count = self.winning.len() as f64;
        Ok(self.swings.iter().map(|&s| s as f64 / count).collect())
    }

    /// Coleman の発議力: `i` が加わると勝つ敗北提携の数 / 敗北提携の数。
    pub fn coleman_initiative(&self) -> Result<Vec<f64>> {
        let losing = (1u64 << self.players) - self.winning.len() as u64;
        if losing == 0 {
            return Err(Error::InvalidArgument("敗北提携がない".into()));
        }
        Ok(self
            .swings
            .iter()
            .map(|&s| s as f64 / losing as f64)
            .collect())
    }

    /// Coleman の集団の行動力: 勝利提携の数 / `2^n`。
    pub fn coleman_collectivity(&self) -> f64 {
        self.winning.len() as f64 / (1u64 << self.players) as f64
    }
}

fn normalized(values: Vec<f64>) -> Result<Vec<f64>> {
    let total: f64 = values.iter().sum();
    if total == 0.0 {
        return Err(Error::InvalidArgument(
            "全員の値が 0 で正規化できない".into(),
        ));
    }
    Ok(values.into_iter().map(|v| v / total).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::tabulate;
    use crate::oracle::voting::WeightedVotingGame;

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-12, "{actual:?} != {expected:?}");
        }
    }

    fn voting(weights: &[u64], quota: u64) -> SimpleGame {
        let game = tabulate(&WeightedVotingGame::new(weights.to_vec(), quota).unwrap()).unwrap();
        SimpleGame::new(&game).unwrap()
    }

    #[test]
    fn weighted_voting_3_2_1_quota_4() {
        // [4; 3, 2, 1]: 勝利提携は {0,1}, {0,2}, {0,1,2}。最小勝利提携は {0,1}, {0,2}。
        // C({0,1}) = {0,1}、C({0,2}) = {0,2}、C({0,1,2}) = {0} → 決定票 (3, 1, 1)。
        let game = voting(&[3, 2, 1], 4);
        assert_eq!(game.minimal_winning.len(), 2);
        assert_eq!(game.swings, vec![3, 1, 1]);
        // Johnston: 0 は 1/2 + 1/2 + 1 = 2、1 と 2 は 1/2 → (2, 1/2, 1/2) / 3。
        assert_close(
            &game.johnston().unwrap(),
            &[2.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0],
        );
        // Deegan–Packel: (1/2)(1/2 + 1/2) = 1/2、1 と 2 は (1/2)(1/2) = 1/4。
        assert_close(&game.deegan_packel().unwrap(), &[0.5, 0.25, 0.25]);
        // Public Good: 最小勝利提携に含まれる回数 (2, 1, 1) / 4。
        assert_close(&game.public_good().unwrap(), &[0.5, 0.25, 0.25]);
        // Coleman: 勝利提携 3、敗北提携 5 (空提携を含む)。
        assert_close(
            &game.coleman_prevent().unwrap(),
            &[1.0, 1.0 / 3.0, 1.0 / 3.0],
        );
        assert_close(&game.coleman_initiative().unwrap(), &[0.6, 0.2, 0.2]);
        assert!((game.coleman_collectivity() - 3.0 / 8.0).abs() < 1e-12);
    }

    #[test]
    fn public_good_index_can_violate_monotonicity_in_weights() {
        // Holler の指数は重みに対して単調とは限らない。[51; 35, 20, 15, 15, 15] の最小勝利提携は
        // {0,1}, {0,2,3}, {0,2,4}, {0,3,4}, {1,2,3,4} で、重み 20 の 1 は 2 個、重み 15 の 2 は 3 個に入る。
        let game = voting(&[35, 20, 15, 15, 15], 51);
        let pgi = game.public_good().unwrap();
        assert!(pgi[1] < pgi[2]);
    }

    #[test]
    fn dummy_player_has_zero_power_in_every_index() {
        // EEC 閣僚理事会 (1958): [12; 4, 4, 4, 2, 2, 1]。ルクセンブルク (5) はどの勝利提携でも決定票を持たない。
        let game = voting(&[4, 4, 4, 2, 2, 1], 12);
        assert_eq!(game.swings[5], 0);
        for index in [
            game.johnston().unwrap(),
            game.deegan_packel().unwrap(),
            game.public_good().unwrap(),
            game.coleman_prevent().unwrap(),
        ] {
            assert_eq!(index[5], 0.0);
        }
    }

    #[test]
    fn non_simple_game_is_rejected() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]).unwrap();
        assert!(SimpleGame::new(&game).is_err());
    }
}
