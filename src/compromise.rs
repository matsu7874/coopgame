//! 理想の支払い (utopia payoff) を基準にした解: tau 値と Gately 点。
//!
//! - 理想の支払い `M_i = v(N) - v(N \ {i})`: 全体提携に対する `i` の限界貢献。
//!   `i` がこれより多くを求めると、他の全員は `i` を除いた提携を作った方が得になる。
//! - 最小の権利 `m_i = max_{S containing i} (v(S) - sum_{j in S \ {i}} M_j)`:
//!   `S` の他のメンバーに理想の支払いを渡したとき、`i` に残る額の最大値。
//! - tau 値 (Tijs 1981): `m` と `M` を結ぶ線分上で効率的な点 `tau = m + alpha (M - m)`。
//!   準平衡 (quasi-balanced、`m <= M` かつ `sum m <= v(N) <= sum M`) なゲームでだけ定義する。
//! - Gately 点 (Gately 1974): 各人の「抜ける傾向」`d_i(x) = (x(N \ {i}) - v(N \ {i})) / (x_i - v({i}))`
//!   を全員で等しくする効率的な配分。`x(N) = v(N)` を使うと `d_i(x) = (M_i - x_i) / (x_i - v({i}))` なので、
//!   解は `x_i = v({i}) + t (M_i - v({i}))`、`t = (v(N) - sum v({j})) / sum (M_j - v({j}))` になる。
//!   これが配分 (`x_i >= v({i})`) になるのは、`v(N) > sum v({j})` で、`M_i - v({i})` の符号が全員でそろう
//!   (0 を含んでよいが全員 0 ではない) 場合である (Staudacher & Anwander 2019)。それ以外はエラーを返す。

use crate::coalition::Coalition;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;

/// 理想の支払い `M_i = v(N) - v(N \ {i})`。
pub fn utopia_payoffs(game: &ExplicitGame) -> Vec<f64> {
    let grand = game.grand();
    let total = game.value(grand);
    (0..game.players())
        .map(|i| total - game.value(Coalition(grand.0 & !(1 << i))))
        .collect()
}

/// 最小の権利 `m_i = max_{S containing i} (v(S) - sum_{j in S \ {i}} M_j)`。
pub fn minimal_rights(game: &ExplicitGame) -> Vec<f64> {
    let n = game.players();
    let utopia = utopia_payoffs(game);
    let mut rights = vec![f64::NEG_INFINITY; n];
    for mask in 1..1u64 << n {
        let coalition = Coalition(mask);
        let value = game.value(coalition);
        let utopia_sum: f64 = coalition.players().map(|j| utopia[j]).sum();
        for i in coalition.players() {
            // v(S) - sum_{j in S \ {i}} M_j
            let remainder = value - (utopia_sum - utopia[i]);
            if remainder > rights[i] {
                rights[i] = remainder;
            }
        }
    }
    rights
}

fn tolerance(game: &ExplicitGame) -> f64 {
    1e-9 * game.max_abs_value().max(1.0)
}

/// tau 値 (Tijs 1981)。準平衡でないゲームでは定義されないのでエラーを返す。
pub fn tau_value(game: &ExplicitGame) -> Result<Vec<f64>> {
    let utopia = utopia_payoffs(game);
    let rights = minimal_rights(game);
    let tol = tolerance(game);
    let total = game.value(game.grand());
    if let Some(i) = (0..game.players()).find(|&i| rights[i] > utopia[i] + tol) {
        return Err(Error::InvalidArgument(format!(
            "準平衡でない: プレイヤー {} の最小の権利 {} が理想の支払い {} を超える",
            i + 1,
            rights[i],
            utopia[i]
        )));
    }
    let (low, high): (f64, f64) = (rights.iter().sum(), utopia.iter().sum());
    if total < low - tol || total > high + tol {
        return Err(Error::InvalidArgument(format!(
            "準平衡でない: v(N) = {total} が最小の権利の和 {low} と理想の支払いの和 {high} の間にない"
        )));
    }
    if high - low <= tol {
        // m = M なので線分は 1 点。
        return Ok(rights);
    }
    let alpha = (total - low) / (high - low);
    Ok(rights
        .iter()
        .zip(&utopia)
        .map(|(m, big_m)| m + alpha * (big_m - m))
        .collect())
}

/// Gately 点 (Gately 1974)。次の場合は抜ける傾向を等しくする配分がないか一意でないので、エラーを返す。
///
/// - `v(N) <= sum v({i})` (本質的でない)。
/// - `M_i - v({i})` の符号が人によって異なるか、全員 0。
pub fn gately_point(game: &ExplicitGame) -> Result<Vec<f64>> {
    let utopia = utopia_payoffs(game);
    let singles = game.singleton_values();
    let total = game.value(game.grand());
    let tol = tolerance(game);
    let surplus = total - singles.iter().sum::<f64>();
    if surplus <= tol {
        return Err(Error::InvalidArgument(
            "v(N) <= sum v({i}) (本質的でない) ため Gately 点が定まらない".into(),
        ));
    }
    let gaps: Vec<f64> = utopia.iter().zip(&singles).map(|(m, s)| m - s).collect();
    let spread: f64 = gaps.iter().sum();
    let mixed = gaps.iter().any(|&g| g > tol) && gaps.iter().any(|&g| g < -tol);
    if mixed || spread.abs() <= tol {
        return Err(Error::InvalidArgument(
            "M_i - v({i}) の符号が人によって異なるか全員 0 のため、Gately 点が配分として定まらない"
                .into(),
        ));
    }
    let t = surplus / spread;
    Ok(singles
        .iter()
        .zip(&utopia)
        .map(|(s, m)| s + t * (m - s))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-9, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn glove_game_tau_value_gives_everything_to_the_scarce_player() {
        // 手袋ゲーム: 0 が左手袋、1, 2 が右手袋。M = (1, 0, 0)、m = (1, 0, 0) なので tau = (1, 0, 0)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0]).unwrap();
        assert_close(&utopia_payoffs(&game), &[1.0, 0.0, 0.0]);
        assert_close(&minimal_rights(&game), &[1.0, 0.0, 0.0]);
        assert_close(&tau_value(&game).unwrap(), &[1.0, 0.0, 0.0]);
    }

    #[test]
    fn three_player_tau_value_by_hand() {
        // v(i) = 0, v(12) = 4, v(13) = 6, v(23) = 8, v(N) = 12。
        // M = (12-8, 12-6, 12-4) = (4, 6, 8)。
        // m_1 = max(0, 4-6, 6-8, 12-14) = 0、m_2 = max(0, 4-4, 8-8, 12-12) = 0、m_3 = max(0, 6-4, 8-6, 12-10) = 2。
        // sum m = 2、sum M = 18、alpha = (12-2)/16 = 5/8 → tau = (5/2, 15/4, 23/4)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0]).unwrap();
        assert_close(&minimal_rights(&game), &[0.0, 0.0, 2.0]);
        assert_close(&tau_value(&game).unwrap(), &[2.5, 3.75, 5.75]);
    }

    #[test]
    fn tau_value_rejects_games_that_are_not_quasi_balanced() {
        // コアが空の対称ゲーム v(ij) = 80, v(N) = 90: M = 10 だが m = 80 - 10 = 70 > M。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 80.0, 80.0, 80.0, 90.0]).unwrap();
        assert!(tau_value(&game).is_err());
    }

    #[test]
    fn gately_point_matches_coopgame_examples() {
        // CoopGame 0.2.2 の gatelyValue のヘルプの例 (分数の例と、Gately 1974 の 3 地域の例)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 3.0, 6.0]).unwrap();
        assert_close(
            &gately_point(&game).unwrap(),
            &[18.0 / 11.0, 36.0 / 11.0, 12.0 / 11.0],
        );
        let gately =
            ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1170.0, 770.0, 210.0, 1530.0]).unwrap();
        let x = gately_point(&gately).unwrap();
        for (a, e) in x.iter().zip([827.7049, 476.5574, 225.7377]) {
            assert!((a - e).abs() < 1e-4, "{x:?}");
        }
    }

    #[test]
    fn gately_point_rejects_mixed_signs() {
        // M - v({i}) = (-1, 2, 2) の符号がそろわず、式の値 (-1, 2, 2) は個人合理性を満たさない。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 4.0, 3.0]).unwrap();
        assert!(gately_point(&game).is_err());
    }

    #[test]
    fn tau_value_matches_coopgame_example() {
        // CoopGame 0.2.2 の tauValue のヘルプの例 (Stach 2011)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 2.0, 1.0, 3.0]).unwrap();
        assert_close(&tau_value(&game).unwrap(), &[1.2, 0.6, 1.2]);
    }

    #[test]
    fn gately_point_equalizes_propensity_to_disrupt() {
        let game = ExplicitGame::from_lex(&[1.0, 0.0, 2.0, 4.0, 6.0, 8.0, 12.0]).unwrap();
        let x = gately_point(&game).unwrap();
        assert!((x.iter().sum::<f64>() - 12.0).abs() < 1e-9);
        let grand = game.grand();
        let propensity: Vec<f64> = (0..3)
            .map(|i| {
                let others = Coalition(grand.0 & !(1 << i));
                let x_others: f64 = others.players().map(|j| x[j]).sum();
                (x_others - game.value(others)) / (x[i] - game.value(Coalition::singleton(i)))
            })
            .collect();
        assert!((propensity[0] - propensity[1]).abs() < 1e-9);
        assert!((propensity[1] - propensity[2]).abs() < 1e-9);
    }
}
