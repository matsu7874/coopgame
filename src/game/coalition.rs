//! 提携のビット集合表現と、特性関数ベクトルの並び順(ビット順・辞書式順)の変換。
//!
//! - ビット順: 提携 `S` の位置は `sum_{i in S} 2^i` (プレイヤーは 0 始まり)。
//!   BNF 実装 (blrzsvrzs/nucleolus) や TUGLab の `binary = TRUE` と同じ並び。
//! - 辞書式順: 提携を大きさの昇順に並べ、同じ大きさの中ではメンバー列の辞書式順に並べる。
//!   CoopGame や TUGLab の既定と同じ並び。

use crate::error::{Error, Result};

/// プレイヤーの集合。プレイヤー `i` はビット `i` に対応する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Coalition(pub u64);

impl Coalition {
    pub(crate) const EMPTY: Coalition = Coalition(0);

    /// `n` 人全員からなる全体提携。
    pub fn grand(n: usize) -> Coalition {
        debug_assert!(n < 64);
        Coalition((1u64 << n) - 1)
    }

    pub fn singleton(player: usize) -> Coalition {
        Coalition(1u64 << player)
    }

    pub fn from_players(players: &[usize]) -> Coalition {
        Coalition(players.iter().fold(0, |mask, &i| mask | (1u64 << i)))
    }

    /// [`Coalition::from_players`] の、番号が `n` 人の範囲にあるかを確かめる版。
    pub fn try_from_players(n: usize, players: &[usize]) -> Result<Coalition> {
        match players.iter().find(|&&i| i >= n) {
            Some(i) => Err(Error::InvalidArgument(format!(
                "プレイヤー番号 {i} が人数 {n} 以上"
            ))),
            None => Ok(Coalition::from_players(players)),
        }
    }

    /// `members` の部分集合 (ビット `k` が `members[k]` を選ぶ) を全体の番号の提携に直す。
    pub(crate) fn embed(members: &[usize], sub: u64) -> Coalition {
        Coalition(
            members
                .iter()
                .enumerate()
                .filter(|(bit, _)| sub >> bit & 1 == 1)
                .fold(0, |mask, (_, &i)| mask | 1 << i),
        )
    }

    /// `parts` のうち `chosen` のビットで選んだ提携の和集合。
    pub(crate) fn union_of(parts: &[Coalition], chosen: u64) -> Coalition {
        Coalition(
            parts
                .iter()
                .enumerate()
                .filter(|(bit, _)| chosen >> bit & 1 == 1)
                .fold(0, |mask, (_, c)| mask | c.0),
        )
    }

    pub fn contains(self, player: usize) -> bool {
        self.0 >> player & 1 == 1
    }

    pub fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 特性関数ベクトル(ビット順)での位置。
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// メンバーを昇順に列挙する。
    pub fn players(self) -> impl Iterator<Item = usize> {
        let mut rest = self.0;
        std::iter::from_fn(move || {
            if rest == 0 {
                return None;
            }
            let player = rest.trailing_zeros() as usize;
            rest &= rest - 1;
            Some(player)
        })
    }
}

/// プレイヤーの名前 (`names` がないか足りなければ 1 始まりの番号)。
pub fn player_name(names: Option<&[String]>, player: usize) -> String {
    names
        .and_then(|names| names.get(player).cloned())
        .unwrap_or_else(|| (player + 1).to_string())
}

/// 提携を `{A, B}` の形で書く。
pub fn format_coalition(coalition: Coalition, names: Option<&[String]>) -> String {
    let members: Vec<String> = coalition.players().map(|i| player_name(names, i)).collect();
    format!("{{{}}}", members.join(", "))
}

/// 長さ `2^n - 1` のベクトルからプレイヤー数 `n` を求める。
pub(crate) fn players_from_len(len: usize) -> Result<usize> {
    let size = len.checked_add(1).ok_or(Error::InvalidLength(len))?;
    if len == 0 || !size.is_power_of_two() {
        return Err(Error::InvalidLength(len));
    }
    Ok(size.trailing_zeros() as usize)
}

/// 空でない提携を辞書式順に並べる。
pub(crate) fn lexicographic_order(n: usize) -> Vec<Coalition> {
    let mut order = Vec::with_capacity((1usize << n) - 1);
    let mut members = Vec::with_capacity(n);
    for size in 1..=n {
        push_combinations(n, size, 0, &mut members, &mut order);
    }
    order
}

fn push_combinations(
    n: usize,
    size: usize,
    first: usize,
    members: &mut Vec<usize>,
    out: &mut Vec<Coalition>,
) {
    if members.len() == size {
        out.push(Coalition::from_players(members));
        return;
    }
    let remaining = size - members.len();
    for player in first..=(n - remaining) {
        members.push(player);
        push_combinations(n, size, player + 1, members, out);
        members.pop();
    }
}

/// 辞書式順の特性関数ベクトルをビット順に並べ替える。
pub(crate) fn lex_to_binary(values: &[f64]) -> Result<Vec<f64>> {
    let n = players_from_len(values.len())?;
    let mut binary = vec![0.0; values.len()];
    for (value, coalition) in values.iter().zip(lexicographic_order(n)) {
        binary[coalition.index() - 1] = *value;
    }
    Ok(binary)
}

/// ビット順の特性関数ベクトルを辞書式順に並べ替える。
pub fn binary_to_lex(values: &[f64]) -> Result<Vec<f64>> {
    let n = players_from_len(values.len())?;
    Ok(lexicographic_order(n)
        .into_iter()
        .map(|coalition| values[coalition.index() - 1])
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexicographic_order_for_three_players() {
        let order: Vec<Vec<usize>> = lexicographic_order(3)
            .into_iter()
            .map(|s| s.players().collect())
            .collect();
        assert_eq!(
            order,
            vec![
                vec![0],
                vec![1],
                vec![2],
                vec![0, 1],
                vec![0, 2],
                vec![1, 2],
                vec![0, 1, 2]
            ]
        );
    }

    #[test]
    fn lex_binary_round_trip() {
        let lex: Vec<f64> = (1..=31).map(f64::from).collect();
        let binary = lex_to_binary(&lex).unwrap();
        assert_eq!(binary_to_lex(&binary).unwrap(), lex);
        // {1,2} (0 始まりで {0,1}) は辞書式で 6 番目、ビット順で 3 番目。
        assert_eq!(binary[2], 6.0);
    }

    #[test]
    fn rejects_invalid_length() {
        assert_eq!(players_from_len(6), Err(Error::InvalidLength(6)));
        assert_eq!(players_from_len(7), Ok(3));
    }
}
