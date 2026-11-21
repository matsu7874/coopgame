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

use crate::error::{Error, Result};
use crate::game::oracle::Separation;
use crate::game::{ExplicitGame, PlayerSet, SetFunction};

/// 利用者が指定した許容誤差が、有限で 0 以上かを確かめる。
///
/// NaN や負の値のまま比較に使うと、どの判定も成り立たず、原因の分からない数値的な失敗になる。
pub(crate) fn check_tolerance(tolerance: f64) -> Result<f64> {
    if tolerance.is_finite() && tolerance >= 0.0 {
        Ok(tolerance)
    } else {
        Err(Error::InvalidArgument(format!(
            "許容誤差 {tolerance:e} が 0 以上の有限の数ではない"
        )))
    }
}

/// 利用者が指定した許容誤差の比 `ratio` を確かめ、値の大きさ `magnitude` (最大絶対値など) に掛けて、
/// 値と同じ単位の許容誤差にする。
///
/// 比が有限で 0 以上でないとき、または掛けた許容誤差が有限でないときは [`Error::InvalidArgument`]
/// (掛ける前に確かめるので、エラーには利用者が指定した比を書ける)。
pub(crate) fn tolerance_from_ratio(ratio: f64, magnitude: f64) -> Result<f64> {
    if !(ratio.is_finite() && ratio >= 0.0) {
        return Err(Error::InvalidArgument(format!(
            "許容誤差の比 {ratio:e} が 0 以上の有限の数ではない"
        )));
    }
    let tolerance = ratio * positive_scale(magnitude);
    if tolerance.is_finite() {
        Ok(tolerance)
    } else {
        Err(Error::InvalidArgument(format!(
            "許容誤差の比 {ratio:e} がゲームの値の大きさに比べて大きすぎる"
        )))
    }
}

/// 利用者が指定した許容誤差を確かめ、値を `divisor` で割ったゲームに合わせて割る。
///
/// [`check_tolerance`] の条件に加えて、割った許容誤差が有限でない (ゲームの値の大きさに比べて大きすぎる) ときも
/// [`Error::InvalidArgument`] を返す。無限大の許容誤差のまま解くと、どの判定も成り立ってしまう。
pub(crate) fn normalized_tolerance(tolerance: f64, divisor: f64) -> Result<f64> {
    let normalized = check_tolerance(tolerance)? / divisor;
    if normalized.is_finite() {
        Ok(normalized)
    } else {
        Err(Error::InvalidArgument(format!(
            "許容誤差 {tolerance:e} がゲームの値の大きさに比べて大きすぎる"
        )))
    }
}

/// `players` 人のゲームで、最大絶対値が `magnitude` の値から和や差を作ったときの丸め誤差の目安
/// `4 n ε magnitude` (`ε` は [`f64::EPSILON`])。
///
/// 10 進数の入力や浮動小数点数の和で作った値は、有理数では等しいはずの量がこの程度ずれる。
/// 「ちょうど 0 か」「ちょうど等しいか」を値どおりに判定すると結論が丸めで変わる箇所では、
/// 許容誤差 (`1e-7 max|v|` など、解の精度のための幅) ではなく、この大きさと比べる。
pub(crate) fn rounding_magnitude(players: usize, magnitude: f64) -> f64 {
    4.0 * players as f64 * f64::EPSILON * magnitude
}

/// `terms` 個の値の和 `sum` から `value` を引く。差が和の丸め誤差の範囲
/// (`2 (terms + 1) ε max(|sum|, |value|)`) なら、ちょうど 0 を返す。
///
/// 費用から節約 `sum_{i in S} c({i}) - c(S)` を作るときに使う。費用が部分提携の費用の和に等しい提携の節約は
/// 0 だが、浮動小数点数の和では `-3.6e-15` のような値になり、「値が 0 の提携」「非負のゲーム」の判定を狂わせる。
pub(crate) fn difference_or_zero(sum: f64, value: f64, terms: usize) -> f64 {
    let difference = sum - value;
    let rounding = 2.0 * (terms as f64 + 1.0) * f64::EPSILON * sum.abs().max(value.abs());
    if difference.abs() <= rounding {
        0.0
    } else {
        difference
    }
}

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

    fn covers_all_coalitions(&self) -> bool {
        self.game.covers_all_coalitions()
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
