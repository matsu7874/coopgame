//! Kohlberg 基準による検証 (説明は [`super::kohlberg`])。

use crate::Domain;
use crate::error::Result;
use crate::game::allocation::require_allocation;
use crate::game::scale::check_tolerance;
use crate::game::{ExplicitGame, verification_tolerance};
use crate::levels::kohlberg_at;

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KohlbergReport {
    pub satisfied: bool,
    /// 不成立の場合の理由。
    pub reason: Option<String>,
    /// 平衡性を判定した超過の段の数。
    pub levels_checked: usize,
    pub lp_solves: usize,
    /// 判定に使った許容誤差 (値の単位)。
    pub tolerance: f64,
}

/// Kohlberg 基準による仁・プレ仁の検証 (浮動小数点数の判定)。
///
/// 超過が `alpha` 以上の真部分提携の族を `D(alpha, x)` とする。
///
/// - プレ仁: `x` がプレ仁であることと、空でない全ての `D(alpha, x)` が平衡
///   (全提携に正の重みを付けて特性ベクトルの和を `1_N` の正数倍にできる)であることは同値。
/// - 仁: `D0(x) = { {i} : x_i = v({i}) }` を重み 0 以上で加えてよい。
///
/// 超過の値の段ごとに平衡性を LP で判定する。族が平衡で、族の特性ベクトルと `1_N` の階数が `n` に達していれば、
/// それ以降の段も平衡になるので判定を打ち切る
/// (Lean による証明: リポジトリの `proofs/` の `PosCombination.mono_of_span_insert_eq_top`)。
/// `D0(x)` の 1 人提携は、階数には数えない。
///
/// 許容誤差は [`crate::game::verification_tolerance`] (`max|v(S)|` の `1e-11` 倍) で、
/// 和・個人合理性の差と、同じ段とみなす超過の差をこの幅まで許す ([`kohlberg_within`] で指定できる)。
///
/// 浮動小数点数の配分は仁を丸めた値なので、厳密な判定 (許容誤差 0) ではほぼ全ての配分が不成立になる。
/// そのため `satisfied` は「許容誤差の範囲で Kohlberg 基準を満たす」ことを意味し、仁であることの証明ではない。
/// 仁であることを確かめるには、厳密な配分を復元して有理数で判定する [`crate::verify::certify`]
/// (CLI の `certify`・`check`) を使う。
///
/// # Errors
///
/// `x` の長さがプレイヤー数と異なるか、有限でない成分があれば [`crate::Error::InvalidArgument`]。
pub fn kohlberg(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<KohlbergReport> {
    kohlberg_within(game, x, domain, verification_tolerance(game))
}

/// 許容誤差 `tolerance` (値の単位) を指定した [`kohlberg`]。
///
/// # Errors
///
/// `tolerance` が有限で 0 以上でないとき、`x` の長さがプレイヤー数と異なるか有限でない成分があるときは
/// [`crate::Error::InvalidArgument`]。
pub fn kohlberg_within(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<KohlbergReport> {
    check_tolerance(tolerance)?;
    require_allocation(game.players(), x)?;
    let check = kohlberg_at(game, x, domain, tolerance)?;
    Ok(KohlbergReport {
        satisfied: check.satisfied,
        reason: check.reason,
        levels_checked: check.levels_checked,
        lp_solves: check.lp_solves,
        tolerance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_nucleolus_and_rejects_other_points() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        assert!(
            kohlberg(&game, &[2.0, 2.0, 0.0], Domain::Imputation)
                .unwrap()
                .satisfied
        );
        let report = kohlberg(&game, &[1.0, 3.0, 0.0], Domain::Imputation).unwrap();
        assert!(!report.satisfied);
        let report = kohlberg(&game, &[2.0, 2.0, 1.0], Domain::Imputation).unwrap();
        assert!(report.reason.unwrap().contains("効率性"));
    }

    #[test]
    fn rejects_allocations_off_the_nucleolus_by_more_than_rounding() {
        // v({0, 1}) = 1e8、v(N) = 1e8 - 6。仁は (49999997, 49999997, 0)。
        // 以前の既定の許容誤差 (1e-6 max|v| = 100) では、個人合理性を 3 破る配分や、仁から 7 ずれた配分も合格していた。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1e8, 0.0, 0.0, 1e8 - 6.0]).unwrap();
        let report = kohlberg(&game, &[49999997.0, 49999997.0, 0.0], Domain::Imputation).unwrap();
        assert!(report.satisfied);
        assert_eq!(report.tolerance, 1e-11 * 1e8);
        for x in [
            [49999998.5, 49999998.5, -3.0],
            [49999997.0 + 7.0, 49999997.0 - 7.0, 0.0],
        ] {
            assert!(
                !kohlberg(&game, &x, Domain::Imputation).unwrap().satisfied,
                "{x:?}"
            );
            // 許容誤差を明示すれば、以前の判定になる。
            assert!(
                kohlberg_within(&game, &x, Domain::Imputation, 100.0)
                    .unwrap()
                    .satisfied,
                "{x:?}"
            );
        }
        for tolerance in [f64::NAN, -1.0, f64::INFINITY] {
            assert!(kohlberg_within(&game, &[0.0; 3], Domain::Imputation, tolerance).is_err());
        }
    }

    #[test]
    fn majority_game() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let third = 1.0 / 3.0;
        assert!(
            kohlberg(&game, &[third; 3], Domain::Preimputation)
                .unwrap()
                .satisfied
        );
        assert!(
            !kohlberg(&game, &[0.5, 0.25, 0.25], Domain::Preimputation)
                .unwrap()
                .satisfied
        );
    }
}
