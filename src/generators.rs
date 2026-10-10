//! ベンチマークとテスト用のゲーム生成器。
//!
//! [`bnf`] は blrzsvrzs/nucleolus (Benedek ら) の `gen_game` と同じ分布でゲームを作る。
//! 乱数生成器が異なるため、同じ seed でも値は一致しない。

use crate::error::{Error, Result};
use crate::game::ExplicitGame;

pub use crate::rng::SplitMix64;

/// BNF 実装のゲームタイプ 1-5。
///
/// 1. `v({i}) = 0`、`2 <= |S| < n` は `[1, 100|S|]`、`v(N)` は `[100(n-2), 100n]` の一様整数
/// 2. `v({i}) = 0`、それ以外は `[1, 50n]` の一様整数
/// 3. `|S| < n-2` は 0、`n-2 <= |S| <= n-1` は確率 0.9 で 1、`v(N) = 1`
///    (`n = 3` では `v({i})` も 1 になりうるため、配分集合が空になりやすい)
/// 4. `v({i}) = 0`、それ以外は `[1, n]` の一様整数
/// 5. 重み付き投票ゲーム (`n >= 7`)。`w_1..w_5 = floor((n-3)/2)`、残りは 1、`q = 4 w_1 + n - 4`
pub fn bnf(kind: u8, n: usize, seed: u64) -> Result<ExplicitGame> {
    if n < 2 {
        return Err(Error::InvalidArgument("プレイヤー数は 2 以上".into()));
    }
    let n64 = n as u64;
    let mut rng = SplitMix64::new(seed);
    let full = (1u64 << n) - 1;
    let values: Vec<f64> = match kind {
        1 => (1..=full)
            .map(|mask| {
                let size = mask.count_ones() as u64;
                if size == 1 {
                    0.0
                } else if mask == full {
                    rng.range(100 * (n64 - 2), 100 * n64) as f64
                } else {
                    rng.range(1, 100 * size) as f64
                }
            })
            .collect(),
        2 => (1..=full)
            .map(|mask| {
                if mask.count_ones() == 1 {
                    0.0
                } else {
                    rng.range(1, 50 * n64) as f64
                }
            })
            .collect(),
        3 => (1..=full)
            .map(|mask| {
                let size = mask.count_ones() as usize;
                if mask == full || (size + 2 >= n && rng.next_f64() < 0.9) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect(),
        4 => (1..=full)
            .map(|mask| {
                if mask.count_ones() == 1 {
                    0.0
                } else {
                    rng.range(1, n64) as f64
                }
            })
            .collect(),
        5 => {
            if n < 7 {
                return Err(Error::InvalidArgument("タイプ 5 は n >= 7".into()));
            }
            let big = ((n - 3) / 2) as f64;
            let weights: Vec<f64> = (0..n).map(|i| if i < 5 { big } else { 1.0 }).collect();
            let quota = 4.0 * big + n as f64 - 4.0;
            return weighted_voting(&weights, quota);
        }
        _ => {
            return Err(Error::InvalidArgument(format!(
                "未知のゲームタイプ {kind}(1-5)"
            )));
        }
    };
    ExplicitGame::from_binary(&values)
}

/// 重み付き投票ゲーム: 重みの和が `quota` 以上なら 1、そうでなければ 0。
pub fn weighted_voting(weights: &[f64], quota: f64) -> Result<ExplicitGame> {
    ExplicitGame::from_fn(weights.len(), |coalition| {
        let weight: f64 = coalition.players().map(|i| weights[i]).sum();
        if weight >= quota { 1.0 } else { 0.0 }
    })
}

/// 破産ゲーム: `v(S) = max(0, E - sum_{j not in S} d_j)`。仁はタルムード則と一致する。
pub fn bankruptcy(estate: f64, claims: &[f64]) -> Result<ExplicitGame> {
    if claims.iter().any(|d| *d < 0.0) || estate < 0.0 {
        return Err(Error::InvalidArgument("請求額と遺産は非負".into()));
    }
    ExplicitGame::from_fn(claims.len(), |coalition| {
        let outside: f64 = (0..claims.len())
            .filter(|&j| !coalition.contains(j))
            .map(|j| claims[j])
            .sum();
        (estate - outside).max(0.0)
    })
}

/// 非負の Harsanyi 配当を持つ凸ゲーム。配当は `[0, 1)` の一様乱数。
///
/// 凸ゲームではカーネルと仁が一致する (Maschler, Peleg, Shapley 1971、docs/references.md の MPS1971)。
pub fn random_convex(n: usize, seed: u64) -> Result<ExplicitGame> {
    let mut rng = SplitMix64::new(seed);
    let size = 1usize << n;
    let mut values = vec![0.0; size];
    for value in values.iter_mut().skip(1) {
        *value = rng.next_f64();
    }
    // 部分集合和 (zeta 変換): v(S) = sum_{T subset S} d_T
    for bit in 0..n {
        for mask in 0..size {
            if mask >> bit & 1 == 1 {
                values[mask] += values[mask ^ (1 << bit)];
            }
        }
    }
    ExplicitGame::from_binary(&values[1..])
}

/// 優加法的なゲーム。提携ごとに `[0, |S|)` の一様乱数を基本値とし、
/// 分割して稼いだ方が大きければその値に置き換える(優加法的な包)。計算量は `3^n`。
pub fn random_superadditive(n: usize, seed: u64) -> Result<ExplicitGame> {
    let mut rng = SplitMix64::new(seed);
    let size = 1usize << n;
    let mut values = vec![0.0; size];
    for (mask, value) in values.iter_mut().enumerate().skip(1) {
        *value = rng.next_f64() * mask.count_ones() as f64;
    }
    superadditive_cover(&mut values, |a, b| a + b);
    ExplicitGame::from_binary(&values[1..])
}

/// 各提携の値を、その提携の 2 分割の値の和との最大値で置き換える (小さい提携から順に、優加法的にする)。
/// `values` はビット順で長さ `2^n` (空提携を含む)。
fn superadditive_cover<T: PartialOrd>(values: &mut [T], add: impl Fn(&T, &T) -> T) {
    for mask in 1..values.len() {
        // mask の空でない真部分集合 part と残り mask \ part の値の和の最大値。
        let mut part = (mask - 1) & mask;
        while part > 0 {
            let rest = mask & !part;
            if part < rest {
                let sum = add(&values[part], &values[rest]);
                if sum > values[mask] {
                    values[mask] = sum;
                }
            }
            part = (part - 1) & mask;
        }
    }
}

/// 分析・実験で使うゲームのクラスを名前で生成する。
///
/// `bnf1`・`bnf2`・`bnf4` ([`bnf`] のタイプ 1, 2, 4)、`superadditive` ([`random_superadditive`])、
/// `convex` ([`random_convex`])、`voting` ([`random_weighted_voting`])。
pub fn by_class(class: &str, n: usize, seed: u64) -> Result<ExplicitGame> {
    match class {
        "bnf1" => bnf(1, n, seed),
        "bnf2" => bnf(2, n, seed),
        "bnf4" => bnf(4, n, seed),
        "superadditive" => random_superadditive(n, seed),
        "convex" => random_convex(n, seed),
        "voting" => random_weighted_voting(n, seed),
        other => Err(Error::InvalidArgument(format!("未知のクラス {other:?}"))),
    }
}

/// [`random_superadditive`] と同じ乱数で、優加法的な被覆を有理数のまま計算したゲーム。
///
/// [`random_superadditive`] は `v(S) = max(v(S), v(T) + v(S \ T))` を浮動小数点数の和で計算するので、
/// 有理数では等しいはずの値の間に丸め誤差が残り、厳密な検証 ([`crate::verify::certify`]) が成り立たないことがある。
/// こちらは乱数の値を 2 進数の値どおりの有理数にしてから、和と最大値を有理数で計算する。
/// 浮動小数点の手法には [`crate::game::exact::ExactGame::to_explicit`] で変換して渡し、
/// 検証には [`crate::verify::certify_exact`] を使う。
pub fn random_superadditive_exact(n: usize, seed: u64) -> Result<crate::game::exact::ExactGame> {
    use crate::game::exact::{Rational, to_rational};
    if n == 0 || n > crate::game::MAX_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: crate::game::MAX_PLAYERS,
        });
    }
    let mut rng = SplitMix64::new(seed);
    let size = 1usize << n;
    let mut values: Vec<Rational> = vec![Rational::from_integer(0.into()); size];
    for (mask, value) in values.iter_mut().enumerate().skip(1) {
        *value = to_rational(rng.next_f64() * mask.count_ones() as f64)?;
    }
    superadditive_cover(&mut values, |a, b| a + b);
    values.remove(0);
    crate::game::exact::ExactGame::from_binary(values)
}

/// 重みを `[1, 10]` の一様な整数、基準を重みの総和の過半数とする重み付き投票ゲーム。
pub fn random_weighted_voting(n: usize, seed: u64) -> Result<ExplicitGame> {
    let mut rng = SplitMix64::new(seed);
    let weights: Vec<f64> = (0..n).map(|_| rng.range(1, 10) as f64).collect();
    let quota = (weights.iter().sum::<f64>() / 2.0).floor() + 1.0;
    weighted_voting(&weights, quota)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Coalition;

    #[test]
    fn bnf_type5_is_weighted_voting() {
        let game = bnf(5, 7, 0).unwrap();
        // w = (2,2,2,2,2,1,1), q = 11
        assert_eq!(game.value(Coalition::from_players(&[0, 1, 2, 3, 4])), 0.0);
        assert_eq!(
            game.value(Coalition::from_players(&[0, 1, 2, 3, 4, 5])),
            1.0
        );
    }

    #[test]
    fn random_convex_is_supermodular() {
        let game = random_convex(4, 7).unwrap();
        let full = 1u64 << 4;
        for a in 1..full {
            for b in 1..full {
                let lhs = game.value(Coalition(a | b)) + game.value(Coalition(a & b));
                let rhs = game.value(Coalition(a)) + game.value(Coalition(b));
                assert!(lhs >= rhs - 1e-12);
            }
        }
    }
}
