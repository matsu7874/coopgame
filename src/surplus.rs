//! 超過 `e(S, x) = v(S) - x(S)` と最大余剰 `s_ij(x)`。
//!
//! 配分 `x` の長さはプレイヤー数と同じでなければならない。異なるとパニックする。

use crate::game::{Coalition, ExplicitGame, excess_table};
use crate::sorted::{LazySorted, by_excess_desc};

/// 空でない全ての提携の超過 `(S, e(S, x))`。提携はビット順 (`S.0` の昇順) に並ぶ。
pub fn excesses(game: &ExplicitGame, x: &[f64]) -> Vec<(Coalition, f64)> {
    excess_table(game, x)
        .into_iter()
        .enumerate()
        .skip(1)
        .map(|(mask, e)| (Coalition(mask as u64), e))
        .collect()
}

/// 真部分提携 (空でも全体でもない提携) の超過を、大きい順に返す。同じ超過の提携はビット順。
///
/// 読んだ分だけ並べ替えるので、先頭の数個だけ読むなら全体を並べ替えるより速い。
pub fn excess_order(
    game: &ExplicitGame,
    x: &[f64],
) -> impl Iterator<Item = (Coalition, f64)> + use<> {
    let n = game.players();
    let mut order = excesses(game, x);
    order.pop(); // 全体提携
    LazySorted::new(order, 4 * n * n, by_excess_desc)
}

pub fn excess(game: &ExplicitGame, coalition: Coalition, x: &[f64]) -> f64 {
    game.value(coalition) - coalition.sum(x)
}

/// 最大余剰 `s_ij(x) = max { e(S, x) : i in S, j not in S }` の `n x n` 行列。`s[i][j]` が `s_ij`。
/// 対角成分は `-inf`。
pub fn max_surplus(game: &ExplicitGame, x: &[f64]) -> Vec<Vec<f64>> {
    let n = game.players();
    max_surplus_flat(game, x)
        .chunks(n)
        .map(<[f64]>::to_vec)
        .collect()
}

/// [`max_surplus`] を行優先の平らな配列で返す。
///
/// 超過の大きい提携から順に見て、最初に `i in S, j not in S` を満たした提携の超過を
/// `s_ij` とする。全ての組は上位の少数の提携で埋まるため、読んだ分だけ並べ替える [`LazySorted`] で走査する。
pub(crate) fn max_surplus_flat(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    let n = game.players();
    let excess = excess_table(game, x);
    let full = (1u32 << n) - 1;
    let mut surplus = vec![f64::NEG_INFINITY; n * n];
    let mut remaining = n * (n - 1);
    // 真部分提携(空提携と全体提携を除く)を超過の降順に見る。
    let order = LazySorted::new((1..full).collect(), 4 * n * n, |a: &u32, b: &u32| {
        excess[*b as usize].total_cmp(&excess[*a as usize])
    });
    for mask in order {
        if remaining == 0 {
            break;
        }
        let e = excess[mask as usize];
        let inside = Coalition(u64::from(mask));
        let outside = inside.complement(n);
        for i in inside.members() {
            let row = &mut surplus[i * n..(i + 1) * n];
            for j in outside.members() {
                if row[j] == f64::NEG_INFINITY {
                    row[j] = e;
                    remaining -= 1;
                }
            }
        }
    }
    surplus
}

/// 定義どおり全提携を走査して最大余剰を求める(テストでの照合用)。
#[cfg(test)]
fn max_surplus_naive(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    let n = game.players();
    let excess = excess_table(game, x);
    let full = (1usize << n) - 1;
    let mut surplus = vec![f64::NEG_INFINITY; n * n];
    for (mask, &e) in excess.iter().enumerate().take(full).skip(1) {
        let inside = Coalition(mask as u64);
        let outside = inside.complement(n);
        for i in inside.members() {
            for j in outside.members() {
                surplus[i * n + j] = surplus[i * n + j].max(e);
            }
        }
    }
    surplus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_sort_matches_definition() {
        use crate::generators;
        for kind in 1..=4 {
            for n in 4..=9 {
                let game = generators::bnf(kind, n, 3).unwrap();
                let x = crate::kernel::equal_surplus_division(&game);
                assert_eq!(
                    max_surplus_flat(&game, &x),
                    max_surplus_naive(&game, &x),
                    "type {kind} n={n}"
                );
            }
        }
    }

    #[test]
    fn surplus_of_three_player_game() {
        // v({0, 1}) = 4、それ以外の真部分提携は 0、v(N) = 4
        let game = ExplicitGame::from_binary(&[0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 4.0]).unwrap();
        let x = [1.0, 3.0, 0.0];
        let s = max_surplus(&game, &x);
        // s_02: 0 を含み 2 を含まない提携 {0}, {0, 1} -> max(-1, 0) = 0
        assert_eq!(s[0][2], 0.0);
        // s_20: 2 を含み 0 を含まない提携 {2}, {1, 2} -> max(0, -3) = 0
        assert_eq!(s[2][0], 0.0);
        // s_01: {0}, {0, 2} -> max(-1, -1) = -1
        assert_eq!(s[0][1], -1.0);
        // s_10: {1}, {1, 2} -> max(-3, -3) = -3
        assert_eq!(s[1][0], -3.0);
        assert_eq!(s[0][0], f64::NEG_INFINITY);
    }

    #[test]
    fn excesses_list_nonempty_coalitions_in_bit_order() {
        let game = ExplicitGame::from_binary(&[0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 4.0]).unwrap();
        let all = excesses(&game, &[1.0, 3.0, 0.0]);
        assert_eq!(all.len(), 7);
        assert_eq!(all[0], (Coalition::from_players(&[0]), -1.0));
        assert_eq!(all[2], (Coalition::from_players(&[0, 1]), 0.0));
        assert_eq!(all[6], (Coalition::grand(3), 0.0));
    }
}
