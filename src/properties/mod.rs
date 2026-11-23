//! ゲームの性質: 判定する関数と、性質を型で表すラッパー。
//!
//! - 判定: [`is_convex`]・[`is_superadditive`]・[`is_zero_monotonic`]・[`is_in_core`]
//!   (コアが空でないかは最小コアの LP で判定するので、[`crate::nucleolus::has_nonempty_core`] にある)
//! - 型: 性質を持つ根拠 ([`Proven`]・[`Assumed`]) を型で区別し、性質に基づく手法の保証を決める
//!   ([`ConvexGame`]・[`ConvexChecked`]・[`Assume`])
//!
//! ## 性質を型で表す
//!
//! 性質を持つことの根拠は 3 通りある。
//!
//! | 根拠 | 例 | 証明の種類 ([`ProofKind`]) | 結果の保証 |
//! |---|---|---|---|
//! | 型が構造的に持つ | [`crate::games::bankruptcy::BankruptcyGame`]、[`crate::games::graph::InducedSubgraphGame`]、[`crate::games::airport::AirportGame`] | [`Proven`] | [`crate::Guarantee::Proven`] |
//! | 計算で確認した | [`ConvexChecked::new`] (全提携で優モジュラ性を判定) | [`Proven`] | [`crate::Guarantee::Proven`] |
//! | 利用者が宣言した | [`Assume::convex`] | [`Assumed`] | [`crate::Guarantee::Assumed`] |
//!
//! 性質に基づく手法 (例: [`crate::nucleolus::convex::nucleolus`]) は、証明の種類に応じて戻り値の型が変わる。
//! [`Proven`] なら結果をそのまま返し、[`Assumed`] なら [`crate::Unverified`] に包んで返す。
//! したがって、宣言しただけの性質に基づく結果を、保証付きの結果と取り違えることはない
//! (コンパイル時に区別される)。
//!
//! [`ProofKind`] と [`ConvexGame`] はクレートの外では実装できない。利用者が定義したゲームを凸として扱うには、
//! [`Assume::convex`] で包んで結果を検証するか、全提携の表を [`ConvexChecked::new`] で確かめる
//! (例は [`ConvexGame`])。

mod proof;

pub(crate) use proof::Sealed;
pub use proof::{Assume, Assumed, ConvexChecked, ConvexGame, Convexity, ProofKind, Proven};

use std::cmp::Ordering;

use crate::error::Result;
use crate::game::allocation::{ExactExcess, allocation_defect, efficient_within};
use crate::game::exact_sum::sum_sign;
use crate::game::scale::check_tolerance;
use crate::game::{Coalition, ExplicitGame};

/// 互いに素な `S, T` について `v(S ∪ T) >= v(S) + v(T)` (優加法的) か。計算量は `3^n`。
///
/// 値を f64 の 2 進数の値どおりの有理数として読み、丸めなしに判定する (許容誤差を使わない)。
/// 10 進数の値を浮動小数点数の和で作ったゲームなどで、丸めの分だけ破れる場合も `false` になる。
/// 破れの大きさを許容するときは [`is_superadditive_within`] で許容誤差を明示する。
pub fn is_superadditive(game: &ExplicitGame) -> bool {
    superadditive_within(game, 0.0)
}

/// `v(S ∪ T) >= v(S) + v(T) - tolerance` が互いに素な全ての `S, T` で成り立つか
/// (許容誤差 `tolerance` の範囲で優加法的か)。比較は値どおりの有理数で行う。
///
/// `tolerance` が有限で 0 以上でないときは [`crate::Error::InvalidArgument`]。
/// `true` でも優加法的とは限らない (厳密な判定は [`is_superadditive`])。
pub fn is_superadditive_within(game: &ExplicitGame, tolerance: f64) -> Result<bool> {
    check_tolerance(tolerance)?;
    Ok(superadditive_within(game, tolerance))
}

fn superadditive_within(game: &ExplicitGame, tolerance: f64) -> bool {
    let full = game.grand().0;
    (1..=full).all(|union| {
        // union の空でない真部分集合 s を列挙し、s と union \ s を比べる。
        let mut s = (union - 1) & union;
        while s > 0 {
            let t = union & !s;
            if s < t
                && sum_sign(&[
                    game.value(Coalition(union)),
                    -game.value(Coalition(s)),
                    -game.value(Coalition(t)),
                    tolerance,
                ]) == Ordering::Less
            {
                return false;
            }
            s = (s - 1) & union;
        }
        true
    })
}

/// `v(S ∪ T) + v(S ∩ T) >= v(S) + v(T)` (優モジュラ、凸) か。
/// 「i の限界貢献が提携の拡大で減らない」という同値条件で `n^2 2^n` で判定する。
///
/// 値を f64 の 2 進数の値どおりの有理数として読み、丸めなしに判定する (許容誤差を使わない)。
/// [`ConvexChecked::new`] はこの判定に合格したゲームだけを受け付ける。
/// 破れの大きさを許容するときは [`is_convex_within`] で許容誤差を明示する。
pub fn is_convex(game: &ExplicitGame) -> bool {
    convex_within(game, 0.0)
}

/// 全ての `S` と `i, j not in S` (`i != j`) で
/// `v(S ∪ {i, j}) - v(S ∪ {j}) >= v(S ∪ {i}) - v(S) - tolerance` が成り立つか
/// (許容誤差 `tolerance` の範囲で凸か)。比較は値どおりの有理数で行う。
///
/// `tolerance` が有限で 0 以上でないときは [`crate::Error::InvalidArgument`]。
/// `true` でも凸とは限らないので、凸ゲームの手法の保証 ([`ConvexChecked`]) には使わない
/// (厳密な判定は [`is_convex`])。
pub fn is_convex_within(game: &ExplicitGame, tolerance: f64) -> Result<bool> {
    check_tolerance(tolerance)?;
    Ok(convex_within(game, tolerance))
}

fn convex_within(game: &ExplicitGame, tolerance: f64) -> bool {
    let n = game.players();
    let full = game.grand().0;
    (0..=full).all(|s| {
        (0..n).all(|i| {
            (0..n).all(|j| {
                let (bi, bj) = (1u64 << i, 1u64 << j);
                if i == j || s & (bi | bj) != 0 {
                    return true;
                }
                // i の限界貢献: S に j を加えても減らない。
                sum_sign(&[
                    game.value(Coalition(s | bi | bj)),
                    -game.value(Coalition(s | bj)),
                    -game.value(Coalition(s | bi)),
                    game.value(Coalition(s)),
                    tolerance,
                ]) != Ordering::Less
            })
        })
    })
}

/// `v(S ∪ {i}) >= v(S) + v({i})` (`i` が `S` に加わって損をしない、0-単調) か。
///
/// 値を f64 の 2 進数の値どおりの有理数として読み、丸めなしに判定する (許容誤差を使わない)。
/// 破れの大きさを許容するときは [`is_zero_monotonic_within`] で許容誤差を明示する。
pub fn is_zero_monotonic(game: &ExplicitGame) -> bool {
    zero_monotonic_within(game, 0.0)
}

/// `v(S ∪ {i}) >= v(S) + v({i}) - tolerance` が成り立つか (許容誤差 `tolerance` の範囲で 0-単調か)。
/// 比較は値どおりの有理数で行う。
///
/// `tolerance` が有限で 0 以上でないときは [`crate::Error::InvalidArgument`]。
/// `true` でも 0-単調とは限らない (厳密な判定は [`is_zero_monotonic`])。
pub fn is_zero_monotonic_within(game: &ExplicitGame, tolerance: f64) -> Result<bool> {
    check_tolerance(tolerance)?;
    Ok(zero_monotonic_within(game, tolerance))
}

fn zero_monotonic_within(game: &ExplicitGame, tolerance: f64) -> bool {
    let n = game.players();
    let full = game.grand().0;
    (1..=full).all(|s| {
        (0..n).all(|i| {
            let bit = 1u64 << i;
            s & bit != 0
                || sum_sign(&[
                    game.value(Coalition(s | bit)),
                    -game.value(Coalition(s)),
                    -game.value(Coalition(bit)),
                    tolerance,
                ]) != Ordering::Less
        })
    })
}

/// `x` がコアに属するか: 効率性 `|x(N) - v(N)| <= tolerance` と、全ての提携の超過
/// `v(S) - x(S) <= tolerance` を確かめる。
///
/// 比較は、ゲームの値と `x` と `tolerance` を f64 の 2 進数の値どおりの有理数として読んで行う
/// (浮動小数点数の和の丸めで結論が変わらない)。`tolerance = 0.0` で、`x` がちょうどコアに属するかの判定になる。
/// 浮動小数点数で表した配分 (`1/3` を丸めた値など) は、和が `v(N)` にちょうど一致しないことが多いので、
/// そうした配分を「コアの点を丸めたもの」として扱うときは、その丸めの分の `tolerance` を明示する。
///
/// 長さがプレイヤー数と異なる `x` や、有限でない成分 (NaN など) を含む `x` は属さない (`Ok(false)`)。
///
/// `tolerance` が有限で 0 以上でないときは [`crate::Error::InvalidArgument`] を返す。
/// 無限大ではどの配分も属することになり、負の値ではコアの点も属さないことになるためである。
pub fn is_in_core(game: &ExplicitGame, x: &[f64], tolerance: f64) -> Result<bool> {
    check_tolerance(tolerance)?;
    if allocation_defect(game.players(), x).is_some() {
        return Ok(false);
    }
    if !efficient_within(game.value(game.grand()), x, tolerance) {
        return Ok(false);
    }
    let excess = ExactExcess::new(game, x, tolerance);
    Ok((1..game.grand().0).all(|mask| !excess.exceeds(Coalition(mask))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    #[test]
    fn is_in_core_rejects_invalid_tolerance() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        // 無限大ではどの配分も「属する」になり、負ではコアの点も「属さない」になっていた。
        for tolerance in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN, -1.0] {
            for x in [[2.0, 2.0, 0.0], [9.0, 9.0, 9.0]] {
                let result = is_in_core(&game, &x, tolerance);
                assert!(
                    matches!(result, Err(crate::Error::InvalidArgument(_))),
                    "tolerance {tolerance}: {result:?}"
                );
            }
        }
        assert!(is_in_core(&game, &[2.0, 2.0, 0.0], 0.0).unwrap());
        assert!(!is_in_core(&game, &[0.0, 2.0, 2.0], 1e-9).unwrap());
        // 不正な配分は、許容誤差が正しければ「属さない」。
        assert!(!is_in_core(&game, &[f64::NAN, 2.0, 0.0], 1e-9).unwrap());
        assert!(!is_in_core(&game, &[2.0, 2.0], 1e-9).unwrap());
    }

    #[test]
    fn classifies_known_games() {
        let convex = generators::random_convex(4, 1).unwrap();
        assert!(is_convex(&convex));
        assert!(is_superadditive(&convex));
        assert!(is_zero_monotonic(&convex));

        // 3 人多数決: 優加法的だが凸でなく、コアは空。
        let majority = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        assert!(is_superadditive(&majority));
        assert!(!is_convex(&majority));

        // v(12) = 5 > v(123) = 4 で優加法的でも 0-単調でもない。
        let broken = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 4.0]).unwrap();
        assert!(!is_superadditive(&broken));
        assert!(!is_zero_monotonic(&broken));
    }

    #[test]
    fn properties_are_decided_on_exact_values() {
        // v(N) = 1e8 - 6 < v({0, 1}) + v({2}) = 1e8。破れは 6 で、以前の許容誤差 (1e-7 max|v| = 10) より小さい。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1e8, 0.0, 0.0, 1e8 - 6.0]).unwrap();
        assert!(!is_convex(&game));
        assert!(!is_superadditive(&game));
        assert!(!is_zero_monotonic(&game));
        assert!(ConvexChecked::new(game.clone()).is_err());
        // 許容誤差を明示すれば、破れの大きさまで許す。
        assert!(is_convex_within(&game, 6.0).unwrap());
        assert!(!is_convex_within(&game, 5.0).unwrap());
        assert!(is_superadditive_within(&game, 6.0).unwrap());
        assert!(!is_superadditive_within(&game, 5.0).unwrap());
        assert!(is_zero_monotonic_within(&game, 6.0).unwrap());
        assert!(!is_zero_monotonic_within(&game, 5.0).unwrap());
        // v(N) - v({0, 1}) = 1 - 1e-8 < v({0, 2}) - v({0}) = 1。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0 - 1e-8]).unwrap();
        assert!(!is_convex(&game));
        assert!(is_superadditive(&game));
        // 10 進数では加法的だが、2 進数の値どおりでは 0.1 + 0.2 > 0.3 なので優加法的でない。
        let decimal = ExplicitGame::from_binary(&[0.1, 0.2, 0.3]).unwrap();
        assert!(!is_superadditive(&decimal));
        assert!(is_superadditive(
            &ExplicitGame::from_binary(&[0.1, 0.2, 0.1 + 0.2]).unwrap()
        ));
        for tolerance in [f64::NAN, -1.0, f64::INFINITY] {
            assert!(is_convex_within(&game, tolerance).is_err());
            assert!(is_superadditive_within(&game, tolerance).is_err());
            assert!(is_zero_monotonic_within(&game, tolerance).is_err());
        }
    }

    #[test]
    fn core_membership_is_decided_on_exact_values() {
        // 値の小さいゲーム: v({1, 2}) = 6e-12 > x({1, 2}) = 0 なのでコアに属さない
        // (絶対値の許容誤差 1e-9 では属すると判定していた)。
        let small =
            ExplicitGame::from_binary(&[0.0, 0.0, 6e-12, 0.0, 6e-12, 6e-12, 9e-12]).unwrap();
        assert!(!is_in_core(&small, &[9e-12, 0.0, 0.0], 0.0).unwrap());
        assert!(is_in_core(&small, &[9e-12, 0.0, 0.0], 6e-12).unwrap());
        // 同じ形で値が 2 進数で正確に表せるゲーム (単位 2^-40) では、均等な配分がちょうどコアに属する。
        let unit = 2f64.powi(-40);
        let exact: Vec<f64> = [0.0, 0.0, 6.0, 0.0, 6.0, 6.0, 9.0]
            .iter()
            .map(|v| v * unit)
            .collect();
        let exact = ExplicitGame::from_binary(&exact).unwrap();
        assert!(is_in_core(&exact, &[3.0 * unit; 3], 0.0).unwrap());
        assert!(!is_in_core(&exact, &[9.0 * unit, 0.0, 0.0], 0.0).unwrap());
        // 和の丸め: 0.1 + 0.2 は 2 進数の値どおりでは 0.3 と異なる。
        let decimal = ExplicitGame::from_binary(&[0.0, 0.0, 0.3]).unwrap();
        assert!(!is_in_core(&decimal, &[0.1, 0.2], 0.0).unwrap());
        assert!(is_in_core(&decimal, &[0.1, 0.2], 1e-15).unwrap());
        // 超過がちょうど許容誤差に等しい提携は属する側。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        assert!(is_in_core(&game, &[1.5, 2.0, 0.5], 0.5).unwrap());
        assert!(!is_in_core(&game, &[1.5, 2.0, 0.5], 0.25).unwrap());
    }

    #[test]
    fn superadditive_generator() {
        for seed in 0..5 {
            let game = generators::random_superadditive(5, seed).unwrap();
            assert!(is_superadditive(&game), "seed={seed}");
        }
    }
}
