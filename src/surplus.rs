//! 超過 `e(S, x) = v(S) - x(S)` と最大余剰 `s_ij(x)`。

use crate::coalition::Coalition;
use crate::game::ExplicitGame;

/// 全提携について `x(S)` を求める(ビット順、長さ `2^n`)。
pub(crate) fn coalition_sums(x: &[f64]) -> Vec<f64> {
    let size = 1usize << x.len();
    let mut sums = vec![0.0; size];
    for mask in 1..size {
        let lowest = mask.trailing_zeros() as usize;
        sums[mask] = sums[mask & (mask - 1)] + x[lowest];
    }
    sums
}

/// 全提携の超過(ビット順、長さ `2^n`)。空提携の超過は 0。
pub fn excesses(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    debug_assert_eq!(x.len(), game.players());
    coalition_sums(x)
        .iter()
        .zip(game.values())
        .map(|(sum, value)| value - sum)
        .collect()
}

pub fn excess(game: &ExplicitGame, coalition: Coalition, x: &[f64]) -> f64 {
    game.value(coalition) - coalition.players().map(|i| x[i]).sum::<f64>()
}

/// 最大余剰 `s_ij(x) = max { e(S, x) : i in S, j not in S }` の `n x n` 行列(行優先)。
/// 対角成分は `-inf`。
///
/// 超過の大きい提携から順に見て、最初に `i in S, j not in S` を満たした提携の超過を
/// `s_ij` とする。全ての組は上位の少数の提携で埋まるため、上位 `k` 個だけを部分ソートし、
/// 埋まらなければ `k` を増やす。
pub fn max_surplus(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    let n = game.players();
    let excess = excesses(game, x);
    let full = (1usize << n) - 1;
    let mut surplus = vec![f64::NEG_INFINITY; n * n];
    // 真部分提携(空提携と全体提携を除く)。
    let mut order: Vec<u32> = (1..full as u32).collect();
    let mut remaining = n * (n - 1);
    let mut scanned = 0;
    let mut k = (4 * n * n).min(order.len());
    while remaining > 0 && scanned < order.len() {
        // order[scanned..k] を超過の降順に並べる。
        let rest = &mut order[scanned..];
        let take = k - scanned;
        let by_excess_desc = |a: &u32, b: &u32| excess[*b as usize].total_cmp(&excess[*a as usize]);
        if take < rest.len() {
            rest.select_nth_unstable_by(take - 1, by_excess_desc);
        }
        rest[..take].sort_unstable_by(by_excess_desc);
        for &mask in &order[scanned..k] {
            let e = excess[mask as usize];
            let inside = Coalition(u64::from(mask));
            let outside = Coalition((full & !(mask as usize)) as u64);
            for i in inside.players() {
                let row = &mut surplus[i * n..(i + 1) * n];
                for j in outside.players() {
                    if row[j] == f64::NEG_INFINITY {
                        row[j] = e;
                        remaining -= 1;
                    }
                }
            }
            if remaining == 0 {
                break;
            }
        }
        scanned = k;
        k = (4 * k).min(order.len());
    }
    surplus
}

/// 定義どおり全提携を走査して最大余剰を求める(テストでの照合用)。
#[cfg(test)]
fn max_surplus_naive(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    let n = game.players();
    let excess = excesses(game, x);
    let full = (1usize << n) - 1;
    let mut surplus = vec![f64::NEG_INFINITY; n * n];
    for (mask, &e) in excess.iter().enumerate().take(full).skip(1) {
        let inside = Coalition(mask as u64);
        let outside = Coalition((full & !mask) as u64);
        for i in inside.players() {
            for j in outside.players() {
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
                    max_surplus(&game, &x),
                    max_surplus_naive(&game, &x),
                    "type {kind} n={n}"
                );
            }
        }
    }

    #[test]
    fn surplus_of_three_player_game() {
        // v(12)=4、それ以外の真部分提携は 0、v(N)=4
        let game = ExplicitGame::from_binary(&[0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 4.0]).unwrap();
        let x = [1.0, 3.0, 0.0];
        let s = max_surplus(&game, &x);
        // s_13: 1 を含み 3 を含まない提携 {1},{12} -> max(-1, 0) = 0
        assert_eq!(s[2], 0.0);
        // s_31: 3 を含み 1 を含まない提携 {3},{23} -> max(0, -3) = 0
        assert_eq!(s[2 * 3], 0.0);
        // s_12: {1},{13} -> max(-1, -1) = -1
        assert_eq!(s[1], -1.0);
        // s_21: {2},{23} -> max(-3, -3) = -3
        assert_eq!(s[3], -3.0);
    }
}
