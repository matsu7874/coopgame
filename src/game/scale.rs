//! 値の大きさ (スケール) の扱い: 許容誤差の基準と、解く前の正規化。
//!
//! ゲームの値を `c` 倍 (`c > 0`) すると、仁・カーネルなどの解も `c` 倍になる。
//! 解の計算がこの性質を保つよう、次の 2 つをそろえて使う。
//!
//! - 許容誤差は値の大きさに比例させる ([`positive_scale`] に係数を掛ける)。下限を設けないので、
//!   値の小さいゲームで許容誤差が値を上回ることはない。
//! - LP (microlp は絶対誤差で判定する) や反復の停止条件を使う手法は、ゲームの値を 2 のべき乗
//!   ([`normalizer`]) で割って最大絶対値を `[1, 2)` にそろえてから解き、結果を元の大きさに戻す。
//!   2 のべき乗による乗除は (オーバーフロー・アンダーフローしない限り) 丸め誤差を生まないので、
//!   正規化したゲームは元のゲームと同じ値を持つ。倍率が 2 のべき乗なら、結果もちょうど同じ倍率になる。

use crate::game::oracle::Separation;
use crate::game::{ExplicitGame, PlayerSet, SetFunction};

/// 値の大きさ `magnitude` (最大絶対値など) を、許容誤差の基準にする正の値にする。0 (全ての値が 0) なら 1。
pub(crate) fn positive_scale(magnitude: f64) -> f64 {
    if magnitude > 0.0 && magnitude.is_finite() {
        magnitude
    } else {
        1.0
    }
}

/// 正規化の除数: `magnitude` 以下で最大の 2 のべき乗。`magnitude` が 0 なら 1。
///
/// 割った後の最大絶対値は `[1, 2)` に入る。
pub(crate) fn normalizer(magnitude: f64) -> f64 {
    let magnitude = positive_scale(magnitude);
    if magnitude < f64::MIN_POSITIVE {
        // 非正規化数は指数部を持たない。最小の正規化数で割っても丸めは起きない。
        return f64::MIN_POSITIVE;
    }
    // 指数部だけを残すと、magnitude 以下で最大の 2 のべき乗になる。
    f64::from_bits(magnitude.to_bits() & (0x7ff << 52))
}

impl ExplicitGame {
    /// 全ての値を `divisor` で割ったゲーム。
    pub(crate) fn divided_by(&self, divisor: f64) -> ExplicitGame {
        ExplicitGame {
            players: self.players(),
            values: self.values().iter().map(|v| v / divisor).collect(),
        }
    }

    /// 最大絶対値が `[1, 2)` に入るよう 2 のべき乗で割ったゲームと、その除数。
    pub(crate) fn normalized(&self) -> (ExplicitGame, f64) {
        let divisor = normalizer(self.max_abs_value());
        (self.divided_by(divisor), divisor)
    }
}

/// 値を `divisor` で割ったゲーム (全提携を列挙しないゲームの正規化)。
pub(crate) struct Scaled<'a, G: ?Sized> {
    game: &'a G,
    divisor: f64,
}

impl<'a, G: ?Sized> Scaled<'a, G> {
    pub fn new(game: &'a G, divisor: f64) -> Scaled<'a, G> {
        Scaled { game, divisor }
    }
}

impl<G: SetFunction + ?Sized> SetFunction for Scaled<'_, G> {
    fn players(&self) -> usize {
        self.game.players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        self.game.value(coalition) / self.divisor
    }
}

/// 正規化したゲームの超過は元のゲームの超過を `divisor` で割ったもの。
/// 配分としきい値を元の大きさに戻して元のオラクルに問い合わせる。
impl<G: Separation + ?Sized> Separation for Scaled<'_, G> {
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        let original: Vec<f64> = x.iter().map(|v| v * self.divisor).collect();
        self.game
            .violated(&original, bound * self.divisor, skip, limit)
            .into_iter()
            .map(|(coalition, excess)| (coalition, excess / self.divisor))
            .collect()
    }

    fn value_scale(&self) -> f64 {
        self.game.value_scale() / self.divisor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizer_is_largest_power_of_two_below() {
        assert_eq!(normalizer(0.0), 1.0);
        assert_eq!(normalizer(1.0), 1.0);
        assert_eq!(normalizer(1.99), 1.0);
        assert_eq!(normalizer(2.0), 2.0);
        assert_eq!(normalizer(920000.0), 524288.0);
        assert_eq!(normalizer(3e-7), 2f64.powi(-22));
        assert_eq!(normalizer(1e-310), f64::MIN_POSITIVE);
        for magnitude in [1e-300, 3e-7, 0.7, 5.0, 1e7, 1e300] {
            let scaled = magnitude / normalizer(magnitude);
            assert!((1.0..2.0).contains(&scaled), "{magnitude}");
        }
    }

    #[test]
    fn normalized_game_keeps_values_exactly() {
        let game = crate::generators::bnf(1, 4, 0).unwrap();
        let big = game.map_values(|_, v| v * 1e6).unwrap();
        let (normalized, divisor) = big.normalized();
        assert!((1.0..2.0).contains(&normalized.max_abs_value()));
        for (a, b) in normalized.values().iter().zip(big.values()) {
            assert_eq!(a * divisor, *b);
        }
    }
}
