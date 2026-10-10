//! ゲームの性質: 判定する関数と、性質を型で表すラッパー。
//!
//! - 判定: [`is_convex`]・[`is_superadditive`]・[`is_zero_monotonic`]・[`has_nonempty_core`]・[`is_in_core`]
//! - 型: 性質を持つ根拠 ([`Proven`]・[`Assumed`]) を型で区別し、性質に基づく手法の保証を決める
//!   ([`ConvexGame`]・[`ConvexChecked`]・[`Assume`])

mod proof;

pub use proof::{Assume, Assumed, ConvexChecked, ConvexGame, Convexity, ProofKind, Proven};

use crate::Domain;
use crate::error::Result;
use crate::game::ExplicitGame;
use crate::game::coalition::Coalition;
use crate::game::default_tolerance;
use crate::nucleolus::least_core;

/// 互いに素な `S, T` について `v(S ∪ T) >= v(S) + v(T)`。計算量は `3^n`。
pub fn is_superadditive(game: &ExplicitGame) -> bool {
    let tolerance = default_tolerance(game);
    let full = game.grand().0;
    (1..=full).all(|union| {
        // union の空でない真部分集合 s を列挙し、s と union \ s を比べる。
        let mut s = (union - 1) & union;
        while s > 0 {
            let t = union & !s;
            if s < t
                && game.value(Coalition(union))
                    < game.value(Coalition(s)) + game.value(Coalition(t)) - tolerance
            {
                return false;
            }
            s = (s - 1) & union;
        }
        true
    })
}

/// `v(S ∪ T) + v(S ∩ T) >= v(S) + v(T)`(優モジュラ)。
/// 「i の限界貢献が提携の拡大で減らない」という同値条件で `n^2 2^n` で判定する。
pub fn is_convex(game: &ExplicitGame) -> bool {
    let tolerance = default_tolerance(game);
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
                let without = game.value(Coalition(s | bi)) - game.value(Coalition(s));
                let with = game.value(Coalition(s | bi | bj)) - game.value(Coalition(s | bj));
                with >= without - tolerance
            })
        })
    })
}

/// `v(S ∪ {i}) >= v(S) + v({i})`(`i` が `S` に加わって損をしない)。
pub fn is_zero_monotonic(game: &ExplicitGame) -> bool {
    let tolerance = default_tolerance(game);
    let n = game.players();
    let full = game.grand().0;
    (1..=full).all(|s| {
        (0..n).all(|i| {
            let bit = 1u64 << i;
            s & bit != 0
                || game.value(Coalition(s | bit))
                    >= game.value(Coalition(s)) + game.value(Coalition(bit)) - tolerance
        })
    })
}

/// コアが空でないか(最小コアの `epsilon` が 0 以下か)。
pub fn has_nonempty_core(game: &ExplicitGame) -> Result<bool> {
    let least = least_core(game, Domain::Preimputation)?;
    Ok(least.epsilon <= default_tolerance(game))
}

/// `x` がコアに属するか。
pub fn is_in_core(game: &ExplicitGame, x: &[f64], tolerance: f64) -> bool {
    let total: f64 = x.iter().sum();
    if (total - game.value(game.grand())).abs() > tolerance {
        return false;
    }
    crate::surplus::excesses(game, x)
        .iter()
        .all(|e| *e <= tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    #[test]
    fn classifies_known_games() {
        let convex = generators::random_convex(4, 1).unwrap();
        assert!(is_convex(&convex));
        assert!(is_superadditive(&convex));
        assert!(is_zero_monotonic(&convex));
        assert!(has_nonempty_core(&convex).unwrap());

        // 3 人多数決: 優加法的だが凸でなく、コアは空。
        let majority = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        assert!(is_superadditive(&majority));
        assert!(!is_convex(&majority));
        assert!(!has_nonempty_core(&majority).unwrap());

        // v(12) = 5 > v(123) = 4 で優加法的でも 0-単調でもない。
        let broken = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 4.0]).unwrap();
        assert!(!is_superadditive(&broken));
        assert!(!is_zero_monotonic(&broken));
    }

    #[test]
    fn superadditive_generator() {
        for seed in 0..5 {
            let game = generators::random_superadditive(5, seed).unwrap();
            assert!(is_superadditive(&game), "seed={seed}");
        }
    }
}
