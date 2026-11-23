//! ベンチマークとテスト用のゲーム生成器。
//!
//! [`bnf`] は blrzsvrzs/nucleolus (Benedek ら) の `gen_game` と同じ分布でゲームを作る。
//! 乱数生成器が異なるため、同じ seed でも値は一致しない。

use std::cmp::Ordering;

use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::game::exact_sum::sum_sign;

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
    check_size(n)?;
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
///
/// 重みの和と基準は、f64 の 2 進数の値どおりの有理数として比べる (浮動小数点数の和の丸めで勝敗が変わらない)。
/// 10 進数の重み (`0.7`、`0.1` など) は 2 進数では丸められた値になるので、10 進数どおりに比べたいときは
/// 整数の重みと基準に直して渡す (`[0.7, 0.1, 0.2]` と基準 `0.8` なら `[7, 1, 2]` と `8`)。
///
/// 重みと基準は有限の値 (NaN は比較が常に偽になり、勝利提携のないゲームを黙って作るため拒否する)。
/// 重みは 1 つ以上で非負、基準は 0 より大きく重みの和以下
/// ([`crate::games::voting::WeightedVotingGame::new`] と同じ検査)。
/// 基準の範囲は、単純ゲームの定義の「空提携は負ける (`v(∅) = 0`)」と「全体提携は勝つ (`v(N) = 1`)」から導いた検査である。
/// 基準が 0 以下だと空提携も勝利提携になり、重みの和を超えると全ての提携の値が 0 になる。
pub fn weighted_voting(weights: &[f64], quota: f64) -> Result<ExplicitGame> {
    if !quota.is_finite() || !weights.iter().all(|w| w.is_finite()) {
        return Err(Error::InvalidArgument("重みと基準は有限の値".into()));
    }
    if weights.is_empty() {
        return Err(Error::InvalidArgument("重みが 1 つもない".into()));
    }
    if let Some(i) = weights.iter().position(|&w| w < 0.0) {
        return Err(Error::InvalidArgument(format!(
            "重み {} (プレイヤー {i}) が負",
            weights[i]
        )));
    }
    if quota <= 0.0 {
        return Err(Error::InvalidArgument(format!(
            "基準 {quota} は 0 より大きい値 (0 以下だと空提携が勝利提携になる)"
        )));
    }
    let game = ExplicitGame::from_fn(weights.len(), |coalition| {
        let mut terms: Vec<f64> = coalition.members().map(|i| weights[i]).collect();
        terms.push(-quota);
        if sum_sign(&terms) != Ordering::Less {
            1.0
        } else {
            0.0
        }
    })?;
    // 全体提携が勝てるか (基準が重みの和以下か) は、各提携と同じ和の比較で判定する。
    if game.value(game.grand()) == 0.0 {
        return Err(Error::InvalidArgument(format!(
            "基準 {quota} が重みの和を超える (全体提携も勝てず、全ての提携の値が 0 になる)"
        )));
    }
    Ok(game)
}

/// 重みと基準を有理数で与える重み付き投票ゲーム: 重みの和が `quota` 以上なら 1、そうでなければ 0。
///
/// 10 進数の重みを [`crate::game::exact::parse_rational`] で値どおりに読んで渡すと、10 進数どおりに勝敗が決まる
/// (`[7/10, 1/10, 1/5]` と基準 `4/5` では `{0, 1}` が勝つ)。入力の検査は [`weighted_voting`] と同じ。
pub fn weighted_voting_exact(
    weights: &[crate::game::exact::Rational],
    quota: &crate::game::exact::Rational,
) -> Result<ExplicitGame> {
    use crate::game::exact::{Rational, format_rational};
    use num_traits::Zero;
    if weights.is_empty() {
        return Err(Error::InvalidArgument("重みが 1 つもない".into()));
    }
    if let Some(i) = weights.iter().position(|w| w < &Rational::zero()) {
        return Err(Error::InvalidArgument(format!(
            "重み {} (プレイヤー {i}) が負",
            format_rational(&weights[i])
        )));
    }
    if quota <= &Rational::zero() {
        return Err(Error::InvalidArgument(format!(
            "基準 {} は 0 より大きい値 (0 以下だと空提携が勝利提携になる)",
            format_rational(quota)
        )));
    }
    if &weights.iter().sum::<Rational>() < quota {
        return Err(Error::InvalidArgument(format!(
            "基準 {} が重みの和を超える (全体提携も勝てず、全ての提携の値が 0 になる)",
            format_rational(quota)
        )));
    }
    ExplicitGame::from_fn(weights.len(), |coalition| {
        let weight: Rational = coalition.members().map(|i| &weights[i]).sum();
        if &weight >= quota { 1.0 } else { 0.0 }
    })
}

/// 破産ゲーム: `v(S) = max(0, E - sum_{j not in S} d_j)`。仁はタルムード則と一致する。
///
/// 入力の前提は [`crate::games::bankruptcy::BankruptcyGame::new`] と同じ
/// (遺産と請求は有限かつ非負、請求の総和も有限、遺産は請求の総和以下)。
pub fn bankruptcy(estate: f64, claims: &[f64]) -> Result<ExplicitGame> {
    crate::games::bankruptcy::validate(&estate, claims)?;
    ExplicitGame::from_fn(claims.len(), |coalition| {
        let outside: f64 = (0..claims.len())
            .filter(|&j| !coalition.contains(j))
            .map(|j| claims[j])
            .sum();
        (estate - outside).max(0.0)
    })
}

/// 非負の Harsanyi 配当を持つ凸ゲーム。配当は `[0, 1)` の一様乱数を `2^-(53-n)` の格子に切り捨てた値。
///
/// 配当を格子にそろえるので、部分集合和 `v(S) = Σ_{T ⊆ S} d_T` (`2^n` 個以下の和で `2^n` 未満) は
/// 丸めなしに計算でき、値を有理数として読んでも凸である (非負の配当を持つゲームは凸)。
///
/// 凸ゲームではカーネルと仁が一致する (Maschler, Peleg, Shapley 1971、docs/references.md の MPS1971)。
pub fn random_convex(n: usize, seed: u64) -> Result<ExplicitGame> {
    check_size(n)?;
    let mut rng = SplitMix64::new(seed);
    let size = 1usize << n;
    let grid = (1u64 << (53 - n)) as f64;
    let mut values = vec![0.0; size];
    for value in values.iter_mut().skip(1) {
        *value = (rng.next_f64() * grid).floor() / grid;
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

/// [`random_superadditive`] の基本値の格子 `2^-47`。
///
/// 基本値は `[0, |S|)` (`|S| <= 30 < 2^5`) なので、格子の上の値は `2^52` 未満の整数倍になる。
/// 優加法的な包の値は分割した提携の基本値の和で、和も `|S|` 未満なので、和を丸めなしに計算できる。
const SUPERADDITIVE_GRID: f64 = (1u64 << 47) as f64;

/// 提携 `mask` の基本値: `[0, |S|)` の一様乱数を `2^-47` の格子に切り捨てた値。
fn superadditive_base(rng: &mut SplitMix64, mask: usize) -> f64 {
    (rng.next_f64() * mask.count_ones() as f64 * SUPERADDITIVE_GRID).floor() / SUPERADDITIVE_GRID
}

/// 優加法的なゲーム。提携ごとに `[0, |S|)` の一様乱数を `2^-47` の格子に切り捨てて基本値とし、
/// 分割して稼いだ方が大きければその値に置き換える (優加法的な包)。計算量は `3^n`。
///
/// 基本値を格子にそろえるので、包の和は丸めなしに計算でき、値を有理数として読んでも優加法的である
/// (有理数で同じ包を計算した [`random_superadditive_exact`] と値が一致する)。
pub fn random_superadditive(n: usize, seed: u64) -> Result<ExplicitGame> {
    check_size(n)?;
    let mut rng = SplitMix64::new(seed);
    let size = 1usize << n;
    let mut values = vec![0.0; size];
    for (mask, value) in values.iter_mut().enumerate().skip(1) {
        *value = superadditive_base(&mut rng, mask);
    }
    superadditive_cover(&mut values, |a, b| a + b);
    ExplicitGame::from_binary(&values[1..])
}

/// 全提携の表を作る前に、人数が 1 以上 [`crate::game::MAX_PLAYERS`] 以下かを確かめる
/// (`1 << n` の計算や `2^n` 個の確保より先に弾くため)。
fn check_size(n: usize) -> Result<()> {
    if n == 0 {
        return Err(Error::InvalidArgument("プレイヤー数は 1 以上".into()));
    }
    crate::game::check_players(n)
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
/// `superadditive` と `convex` は、値を有理数として読んでも名前の性質を満たす。
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

/// [`random_superadditive`] と同じ乱数で、優加法的な包を有理数のまま計算したゲーム。
///
/// 基本値は [`random_superadditive`] と同じ格子の値で、包の和は浮動小数点数でも丸めなしに計算できるので、
/// 値は [`random_superadditive`] を有理数にしたもの ([`crate::game::exact::ExactGame::from_explicit`]) と一致する。
/// 有理数の値を直接使う手法 ([`crate::verify::certify_exact`] など) に渡すときに使う。
pub fn random_superadditive_exact(n: usize, seed: u64) -> Result<crate::game::exact::ExactGame> {
    use crate::game::exact::{Rational, to_rational};
    check_size(n)?;
    let mut rng = SplitMix64::new(seed);
    let size = 1usize << n;
    let mut values: Vec<Rational> = vec![Rational::from_integer(0.into()); size];
    for (mask, value) in values.iter_mut().enumerate().skip(1) {
        *value = to_rational(superadditive_base(&mut rng, mask))?;
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
    use crate::game::exact::Rational;

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

    fn exact(game: &ExplicitGame) -> Vec<Rational> {
        game.values()
            .iter()
            .map(|v| Rational::from_float(*v).unwrap())
            .collect()
    }

    /// 有理数で `v(S ∪ T) >= v(S) + v(T)` (互いに素な S, T) を確かめる。
    fn superadditive_on_rationals(game: &ExplicitGame) -> bool {
        let v = exact(game);
        let size = v.len();
        (1..size).all(|s| (1..size).all(|t| s & t != 0 || v[s | t] >= &v[s] + &v[t]))
    }

    /// 有理数で `v(S ∪ T) + v(S ∩ T) >= v(S) + v(T)` を確かめる。
    fn convex_on_rationals(game: &ExplicitGame) -> bool {
        let v = exact(game);
        let size = v.len();
        (1..size).all(|s| (1..size).all(|t| &v[s | t] + &v[s & t] >= &v[s] + &v[t]))
    }

    #[test]
    fn generated_games_have_their_properties_on_rationals() {
        // 浮動小数点数の和で包を作っていたときは、有理数で読むと n = 5 の 300 個中 295 個で優加法性が破れていた。
        for n in 2..=7 {
            for seed in 0..20 {
                let game = random_superadditive(n, seed).unwrap();
                assert!(
                    superadditive_on_rationals(&game),
                    "superadditive n={n} seed={seed}"
                );
                let from_class = by_class("superadditive", n, seed).unwrap();
                assert_eq!(game, from_class);
                let exact_game = random_superadditive_exact(n, seed).unwrap();
                assert_eq!(
                    exact_game.values(),
                    exact(&game).as_slice(),
                    "n={n} seed={seed}"
                );
                let convex = random_convex(n, seed).unwrap();
                assert!(convex_on_rationals(&convex), "convex n={n} seed={seed}");
            }
        }
        // 最大の人数でも格子の上の和は丸めなしに計算できる (値は格子の整数倍)。
        let game = random_convex(20, 1).unwrap();
        let grid = (1u64 << (53 - 20)) as f64;
        assert!(game.values().iter().all(|v| (v * grid).fract() == 0.0));
    }

    #[test]
    fn weighted_voting_compares_sums_on_rationals() {
        // 浮動小数点数の逐次和では 1e16 + 1 + 1 が 1e16 のままになり、基準 1e16 + 2 に届かないと判定していた。
        let game = weighted_voting(&[1e16, 1.0, 1.0], 1e16 + 2.0).unwrap();
        assert_eq!(game.value(Coalition::from_players(&[0, 1, 2])), 1.0);
        assert_eq!(game.value(Coalition::from_players(&[0, 1])), 0.0);
        // f64 の重みは 2 進数の値どおりに比べる: 0.7 + 0.1 (= 0.79999999999999993...) は 0.8 (= 0.80000000000000004...) に届かない。
        let binary = weighted_voting(&[0.7, 0.1, 0.2], 0.8).unwrap();
        assert_eq!(binary.value(Coalition::from_players(&[0, 1])), 0.0);
        assert_eq!(binary.value(Coalition::from_players(&[0, 2])), 1.0);
        // 10 進数どおりに比べるには有理数で渡す。整数の重み [7, 1, 2] と基準 8 と同じゲームになる。
        let parse = |text: &str| crate::game::exact::parse_rational(text).unwrap();
        let decimal =
            weighted_voting_exact(&[parse("0.7"), parse("0.1"), parse("0.2")], &parse("0.8"))
                .unwrap();
        assert_eq!(decimal, weighted_voting(&[7.0, 1.0, 2.0], 8.0).unwrap());
        assert_eq!(decimal.value(Coalition::from_players(&[0, 1])), 1.0);
    }
}
