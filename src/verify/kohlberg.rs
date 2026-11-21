//! Kohlberg 基準による検証 (説明は [`super::kohlberg`])。

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::{ExplicitGame, default_tolerance};
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
}

/// Kohlberg 基準による仁・プレ仁の検証。
///
/// 超過が `alpha` 以上の真部分提携の族を `D(alpha, x)` とする。
///
/// - プレ仁: `x` がプレ仁であることと、空でない全ての `D(alpha, x)` が平衡
///   (全提携に正の重みを付けて特性ベクトルの和を `1_N` の正数倍にできる)であることは同値。
/// - 仁: `D0(x) = { {i} : x_i = v({i}) }` を重み 0 以上で加えてよい。
///
/// 超過の値の段ごとに平衡性を LP で判定する。族の階数が `n` に達して平衡なら、
/// それ以降の段も平衡になる(`1_N` が錐の内点にあるため)ので判定を打ち切る
/// (Lean による証明: リポジトリの `proofs/` の `PosCombination.mono_of_span_eq_top`)。
///
/// 既定の許容誤差で検証する。
pub fn kohlberg(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<KohlbergReport> {
    let n = game.players();
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    let check = kohlberg_at(game, x, domain, default_tolerance(game) * 10.0)?;
    Ok(KohlbergReport {
        satisfied: check.satisfied,
        reason: check.reason,
        levels_checked: check.levels_checked,
        lp_solves: check.lp_solves,
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
