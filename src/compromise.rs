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

use num_traits::{ToPrimitive, Zero};

use crate::error::{Error, Result};
use crate::game::exact::Rational;
use crate::game::exact_sum::DyadicScale;
use crate::game::{Coalition, ExplicitGame};

/// 理想の支払い `M` と最小の権利 `m` の計算に使う、値の和の桁の余裕 (`M_j` の差で 1 ビット、
/// 30 人分の和で 5 ビット、`v(S)` との差と `M_i` を足して 2 ビット、`Σ m` で 5 ビット、`v(N) - Σ m` で 1 ビット)。
const HEADROOM: u32 = 16;

/// 値を有理数として計算するための数。`i128` (値が共通の 2 のべきの整数倍で桁に収まるとき) か、
/// 収まらないときの有理数 ([`Rational`])。どちらも値どおりで丸めない。
trait Exact: Clone + Ord + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self> {}

impl Exact for i128 {}

impl Exact for Rational {}

/// `M` と `m` を値どおりに計算する (`value` は提携の値を `T` にする関数)。
fn bounds<T: Exact>(game: &ExplicitGame, value: impl Fn(f64) -> T) -> (Vec<T>, Vec<T>) {
    let n = game.players();
    let total = value(game.value(game.grand()));
    let utopia: Vec<T> = (0..n)
        .map(|i| total.clone() - value(game.value(Coalition::singleton(i).complement(n))))
        .collect();
    // m_i = M_i + max_{S containing i} r(S)、r(S) = v(S) - Σ_{j in S} M_j。
    let mut best: Vec<Option<T>> = vec![None; n];
    for mask in 1..1u64 << n {
        let coalition = Coalition(mask);
        let remainder = coalition
            .members()
            .fold(value(game.value(coalition)), |r, j| r - utopia[j].clone());
        for i in coalition.members() {
            if best[i].as_ref().is_none_or(|b| &remainder > b) {
                best[i] = Some(remainder.clone());
            }
        }
    }
    let rights = best
        .into_iter()
        .zip(&utopia)
        .map(|(b, m)| m.clone() + b.expect("どのプレイヤーも 1 人提携に含まれる"))
        .collect();
    (utopia, rights)
}

/// `M` と `m` を有理数で厳密に求める。値の桁の幅が `i128` に収まれば整数で、収まらなければ有理数で計算する。
fn exact_bounds(game: &ExplicitGame) -> (Vec<Rational>, Vec<Rational>) {
    match DyadicScale::new(game.values().iter().copied(), HEADROOM) {
        Some(scale) => {
            let (utopia, rights) = bounds(game, |v| scale.integer(v));
            let convert =
                |values: Vec<i128>| values.into_iter().map(|v| scale.rational(v)).collect();
            (convert(utopia), convert(rights))
        }
        None => bounds(game, |v| Rational::from_float(v).expect("ゲームの値は有限")),
    }
}

fn to_f64(values: &[Rational]) -> Vec<f64> {
    values
        .iter()
        .map(|v| v.to_f64().unwrap_or(f64::NAN))
        .collect()
}

/// 理想の支払い `M_i = v(N) - v(N \ {i})`。有理数で求めた値を最も近い f64 にして返す。
pub fn utopia_payoffs(game: &ExplicitGame) -> Vec<f64> {
    let n = game.players();
    let total = Rational::from_float(game.value(game.grand())).expect("ゲームの値は有限");
    let utopia: Vec<Rational> = (0..n)
        .map(|i| {
            &total
                - Rational::from_float(game.value(Coalition::singleton(i).complement(n)))
                    .expect("ゲームの値は有限")
        })
        .collect();
    to_f64(&utopia)
}

/// 最小の権利 `m_i = max_{S containing i} (v(S) - sum_{j in S \ {i}} M_j)`。
/// 有理数で求めた値を最も近い f64 にして返す。
pub fn minimal_rights(game: &ExplicitGame) -> Vec<f64> {
    to_f64(&exact_bounds(game).1)
}

/// tau 値 (Tijs 1981)。準平衡でないゲームでは定義されないのでエラーを返す。
///
/// 準平衡 (`m <= M` かつ `Σ m <= v(N) <= Σ M`) の判定と tau 値の計算は、ゲームの値を f64 の 2 進数の値どおりの
/// 有理数として読み、丸めなしに行う。返す値は有理数の tau 値 (和はちょうど `v(N)`) を最も近い f64 にしたもの。
/// 10 進数の値を浮動小数点数の和で作ったゲームは、10 進数では準平衡でも、丸めで準平衡でなくなることがある。
pub fn tau_value(game: &ExplicitGame) -> Result<Vec<f64>> {
    let (utopia, rights) = exact_bounds(game);
    let total = Rational::from_float(game.value(game.grand())).expect("ゲームの値は有限");
    if let Some(i) = (0..game.players()).find(|&i| rights[i] > utopia[i]) {
        return Err(Error::InvalidArgument(format!(
            "準平衡でない: プレイヤー {i} の最小の権利 {} が理想の支払い {} を超える",
            rights[i].to_f64().unwrap_or(f64::NAN),
            utopia[i].to_f64().unwrap_or(f64::NAN)
        )));
    }
    let low: Rational = rights.iter().sum();
    let high: Rational = utopia.iter().sum();
    if total < low || total > high {
        return Err(Error::InvalidArgument(format!(
            "準平衡でない: v(N) = {} が最小の権利の和 {} と理想の支払いの和 {} の間にない",
            game.value(game.grand()),
            low.to_f64().unwrap_or(f64::NAN),
            high.to_f64().unwrap_or(f64::NAN)
        )));
    }
    if high == low {
        // m <= M で和が等しいので m = M (= tau)、和は v(N)。
        return Ok(to_f64(&rights));
    }
    let alpha = (&total - &low) / (&high - &low);
    let tau: Vec<Rational> = rights
        .iter()
        .zip(&utopia)
        .map(|(m, big_m)| m + &alpha * (big_m - m))
        .collect();
    Ok(to_f64(&tau))
}

/// Gately 点 (Gately 1974)。次の場合は抜ける傾向を等しくする配分がないか一意でないので、エラーを返す。
///
/// - `v(N) <= sum v({i})` (本質的でない)。
/// - `M_i - v({i})` の符号が人によって異なるか、全員 0。
///
/// 条件の判定と配分の計算は、ゲームの値を f64 の 2 進数の値どおりの有理数として読み、丸めなしに行う。
pub fn gately_point(game: &ExplicitGame) -> Result<Vec<f64>> {
    let n = game.players();
    let exact = |v: f64| Rational::from_float(v).expect("ゲームの値は有限");
    let total = exact(game.value(game.grand()));
    let singles: Vec<Rational> = game.singleton_values().into_iter().map(exact).collect();
    let utopia: Vec<Rational> = (0..n)
        .map(|i| &total - exact(game.value(Coalition::singleton(i).complement(n))))
        .collect();
    let surplus = &total - singles.iter().sum::<Rational>();
    if surplus <= Rational::zero() {
        return Err(Error::InvalidArgument(
            "v(N) <= sum v({i}) (本質的でない) ため Gately 点が定まらない".into(),
        ));
    }
    let gaps: Vec<Rational> = utopia.iter().zip(&singles).map(|(m, s)| m - s).collect();
    let spread: Rational = gaps.iter().sum();
    let zero = Rational::zero();
    let mixed = gaps.iter().any(|g| g > &zero) && gaps.iter().any(|g| g < &zero);
    if mixed || spread.is_zero() {
        return Err(Error::InvalidArgument(
            "M_i - v({i}) の符号が人によって異なるか全員 0 のため、Gately 点が配分として定まらない"
                .into(),
        ));
    }
    let t = surplus / spread;
    let x: Vec<Rational> = singles.iter().zip(&gaps).map(|(s, g)| s + &t * g).collect();
    Ok(to_f64(&x))
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
    fn tau_value_checks_quasi_balance_on_exact_values() {
        // 2 人ゲーム v({0}) = v({1}) = 5e11、v(N) = 1e12 - 500。M_i = 5e11 - 500 < m_i = 5e11 なので準平衡でない。
        // 以前は許容誤差 1e-9 max|v| = 1000 の範囲で受け付け、m = M とみなして和が v(N) より 500 大きい m を返していた。
        let game = ExplicitGame::from_binary(&[5e11, 5e11, 1e12 - 500.0]).unwrap();
        assert!(tau_value(&game).is_err());
        // m = M のゲーム (加法的): tau = m で、和は v(N)。
        let additive = ExplicitGame::from_binary(&[1.5, 2.25, 3.75]).unwrap();
        let tau = tau_value(&additive).unwrap();
        assert_eq!(tau, vec![1.5, 2.25]);
        // 10 進数では加法的でも、2 進数の値どおりでは 0.1 + 0.2 > 0.3 なので Σ m > v(N) になり準平衡でない。
        let decimal = ExplicitGame::from_binary(&[0.1, 0.2, 0.3]).unwrap();
        assert!(tau_value(&decimal).is_err());
        // 返す値の和は、有理数の tau 値を丸めた分しか v(N) からずれない。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 2.0, 1.0, 3.0]).unwrap();
        let tau = tau_value(&game).unwrap();
        assert!((tau.iter().sum::<f64>() - 3.0).abs() <= 4.0 * f64::EPSILON * 3.0);
    }

    #[test]
    fn exact_bounds_agree_on_integer_and_rational_paths() {
        // 桁の幅が i128 に収まらないゲーム (1e30 と 1e-30) は有理数で計算する。
        let game = ExplicitGame::from_lex(&[1e-30, 0.0, 0.0, 1e30, 1e30, 0.5, 2e30]).unwrap();
        let rational = bounds(&game, |v| Rational::from_float(v).unwrap());
        assert_eq!(exact_bounds(&game), rational);
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0]).unwrap();
        assert_eq!(
            exact_bounds(&game),
            bounds(&game, |v| Rational::from_float(v).unwrap())
        );
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
        let propensity: Vec<f64> = (0..3)
            .map(|i| {
                let others = Coalition::singleton(i).complement(3);
                let x_others = others.sum(&x);
                (x_others - game.value(others)) / (x[i] - game.value(Coalition::singleton(i)))
            })
            .collect();
        assert!((propensity[0] - propensity[1]).abs() < 1e-9);
        assert!((propensity[1] - propensity[2]).abs() < 1e-9);
    }
}
