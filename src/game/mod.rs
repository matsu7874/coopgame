//! ゲームの表現: 提携、特性関数、全提携の値の表。
//!
//! - [`Coalition`] は 64 人までの提携 (ビット集合)、[`PlayerSet`] は任意の人数の提携
//! - [`SetFunction`] は提携の値を返すゲーム。全ての表現が実装する最小の能力
//! - [`ExplicitGame`] は全提携の値を長さ `2^n` のベクトルで持つゲーム (30 人まで)
//! - [`exact::ExactGame`] は全提携の値を有理数で持つゲーム
//! - [`oracle`] は全提携を列挙せずに仁を求めるための能力 ([`oracle::Separation`]、[`oracle::OracleGame`])

pub(crate) mod allocation;
pub(crate) mod coalition;
pub mod exact;
pub mod oracle;
mod player_set;
pub(crate) mod subsets;

pub use allocation::Domain;
pub(crate) use allocation::excesses;
pub use coalition::{Coalition, binary_to_lex, format_coalition, player_name};
pub use player_set::{PlayerSet, SetFunction, value_scale};

use crate::error::{Error, Result};

/// 明示ベクトルで扱えるプレイヤー数の上限。`2^30` 個の `f64` で約 8GB になる。
pub const MAX_PLAYERS: usize = 30;

/// 特性関数をビット順のベクトルで保持する TU ゲーム。
#[derive(Clone, Debug, PartialEq)]
pub struct ExplicitGame {
    players: usize,
    /// 長さ `2^n`。`values[0]` は空提携で常に 0。
    values: Vec<f64>,
}

impl ExplicitGame {
    /// ビット順に並んだ長さ `2^n - 1` のベクトル(空提携を除く)から作る。
    pub fn from_binary(values: &[f64]) -> Result<ExplicitGame> {
        let players = coalition::players_from_len(values.len())?;
        check_players(players)?;
        if let Some(pos) = values.iter().position(|v| !v.is_finite()) {
            return Err(Error::InvalidArgument(format!(
                "{} 番目の値が有限でない",
                pos + 1
            )));
        }
        let mut all = Vec::with_capacity(values.len() + 1);
        all.push(0.0);
        all.extend_from_slice(values);
        Ok(ExplicitGame {
            players,
            values: all,
        })
    }

    /// 辞書式順に並んだ長さ `2^n - 1` のベクトルから作る。
    pub fn from_lex(values: &[f64]) -> Result<ExplicitGame> {
        ExplicitGame::from_binary(&coalition::lex_to_binary(values)?)
    }

    /// `value(S)` を全ての空でない提携で評価して作る。
    pub fn from_fn(
        players: usize,
        mut value: impl FnMut(Coalition) -> f64,
    ) -> Result<ExplicitGame> {
        check_players(players)?;
        let values: Vec<f64> = (1..(1u64 << players))
            .map(|mask| value(Coalition(mask)))
            .collect();
        ExplicitGame::from_binary(&values)
    }

    /// 値を返すだけのゲームから、全提携の値の表を作る (総当たり、30 人まで)。
    ///
    /// どんなゲームにも、明示ベクトル版の全ての手法 (逐次 LP の仁、カーネル全体、厳密な検証など) を
    /// 適用するための入口。特性関数を `2^n - 1` 回評価する。
    pub fn tabulate<G: SetFunction + ?Sized>(game: &G) -> Result<ExplicitGame> {
        let n = game.players();
        if n == 0 || n > MAX_PLAYERS {
            return Err(Error::TooManyPlayers {
                players: n,
                max: MAX_PLAYERS,
            });
        }
        let mut set = PlayerSet::full(n);
        ExplicitGame::from_fn(n, |coalition| {
            set.assign(coalition);
            game.value(&set)
        })
    }

    /// `members` の人だけの部分ゲーム。プレイヤーは `members` の昇順に 0 から番号を振り直す。
    pub fn subgame(&self, members: Coalition) -> Result<ExplicitGame> {
        let members: Vec<usize> = members.players().collect();
        ExplicitGame::from_fn(members.len(), |sub| {
            self.value(Coalition::embed(&members, sub.0))
        })
    }

    /// 各提携の値を `f(S, v(S))` に置き換えたゲーム。
    pub fn map_values(&self, mut f: impl FnMut(Coalition, f64) -> f64) -> Result<ExplicitGame> {
        ExplicitGame::from_fn(self.players, |coalition| {
            f(coalition, self.value(coalition))
        })
    }

    /// 提携 `coalition` の値だけを `value` にしたゲーム。
    pub fn with_value(&self, coalition: Coalition, value: f64) -> Result<ExplicitGame> {
        self.map_values(|s, v| if s == coalition { value } else { v })
    }

    /// 1 行に 1 つの値を書いたテキスト(BNF 実装の `v.txt` 形式、ビット順)を読む。
    /// 空行と `#` で始まる行は無視する。
    pub fn parse_lines(text: &str) -> Result<ExplicitGame> {
        ExplicitGame::from_binary(&parse_values(text)?)
    }

    /// [`ExplicitGame::parse_lines`] の辞書式順版。
    pub fn parse_lines_lex(text: &str) -> Result<ExplicitGame> {
        ExplicitGame::from_lex(&parse_values(text)?)
    }

    /// ビット順で 1 行に 1 つの値を書き出す。
    pub fn to_lines(&self) -> String {
        let mut out = String::new();
        for value in &self.values[1..] {
            out.push_str(&value.to_string());
            out.push('\n');
        }
        out
    }

    pub fn players(&self) -> usize {
        self.players
    }

    pub fn grand(&self) -> Coalition {
        Coalition::grand(self.players)
    }

    pub fn value(&self, coalition: Coalition) -> f64 {
        self.values[coalition.index()]
    }

    /// 空提携を含むビット順の全値 (長さ `2^n`)。
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// `v({i})` の列。
    pub fn singleton_values(&self) -> Vec<f64> {
        (0..self.players)
            .map(|i| self.value(Coalition::singleton(i)))
            .collect()
    }

    pub fn max_abs_value(&self) -> f64 {
        self.values.iter().fold(0.0, |acc, v| acc.max(v.abs()))
    }

    /// 双対ゲーム `v*(S) = v(N) - v(N \ S)`。
    pub fn dual(&self) -> ExplicitGame {
        let grand = self.values.len() - 1;
        let total = self.values[grand];
        ExplicitGame {
            players: self.players,
            values: (0..self.values.len())
                .map(|mask| total - self.values[grand & !mask])
                .collect(),
        }
    }

    /// 符号を反転したゲーム。費用ゲーム `c` の仁は `-(-c の仁)` で求まる。
    pub fn negated(&self) -> ExplicitGame {
        ExplicitGame {
            players: self.players,
            values: self.values.iter().map(|v| -v).collect(),
        }
    }
}

/// 特性関数の値の大きさに合わせた既定の許容誤差。
pub fn default_tolerance(game: &ExplicitGame) -> f64 {
    1e-7 * game.max_abs_value().max(1.0)
}

pub(super) fn check_players(players: usize) -> Result<()> {
    if players > MAX_PLAYERS {
        return Err(Error::TooManyPlayers {
            players,
            max: MAX_PLAYERS,
        });
    }
    Ok(())
}

/// 1 行に 1 つの数値を読む。
pub fn parse_values(text: &str) -> Result<Vec<f64>> {
    let mut values = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // `名前<TAB>値` の行 (名前付きの配分の出力) は値だけを読む。
        let line = line.rsplit('\t').next().unwrap_or(line).trim();
        // `8/3` の形の分数も受け付ける (最も近い浮動小数点数にする)。
        let value = if line.contains('/') {
            exact::parse_rational(line)
                .map(|q| exact::to_f64(&[q])[0])
                .map_err(|err| Error::Parse {
                    line: index + 1,
                    message: format!("数値として読めない: {line:?} ({err})"),
                })?
        } else {
            line.parse::<f64>().map_err(|err| Error::Parse {
                line: index + 1,
                message: format!("数値として読めない: {line:?} ({err})"),
            })?
        };
        values.push(value);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bnf_format() {
        let game = ExplicitGame::parse_lines("1\n2\n# comment\n3\n\n4\n5\n6\n7\n").unwrap();
        assert_eq!(game.players(), 3);
        assert_eq!(game.value(Coalition::from_players(&[0, 1])), 3.0);
        assert_eq!(game.value(game.grand()), 7.0);
        assert_eq!(ExplicitGame::parse_lines(&game.to_lines()).unwrap(), game);
    }

    #[test]
    fn lex_and_binary_inputs_agree() {
        // 辞書式: {1},{2},{3},{12},{13},{23},{123}
        let lex = ExplicitGame::from_lex(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]).unwrap();
        let binary = ExplicitGame::from_binary(&[1.0, 2.0, 4.0, 3.0, 5.0, 6.0, 7.0]).unwrap();
        assert_eq!(lex, binary);
    }

    #[test]
    fn reports_parse_error_line() {
        let err = ExplicitGame::parse_lines("1\nx\n3\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 2, .. }));
    }
}
