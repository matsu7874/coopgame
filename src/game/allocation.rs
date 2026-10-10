//! 配分に関する基本的な計算: 提携ごとの和 `x(S)` と、配分が領域 ([`Domain`]) に入るかの判定。
//! 解の手法・検証・超過の計算が共有する。

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame};

/// 効率性と領域条件の違反を調べる。
pub(crate) fn feasibility_violation(
    game: &ExplicitGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Option<String> {
    let total: f64 = x.iter().sum();
    let grand = game.value(game.grand());
    if (total - grand).abs() > tolerance {
        return Some(format!(
            "効率性を満たさない: x(N) = {total}, v(N) = {grand}"
        ));
    }
    if domain == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let lower = game.value(Coalition::singleton(i));
            if *xi < lower - tolerance {
                return Some(format!(
                    "個人合理性を満たさない: x_{} = {xi} < v({{{}}}) = {lower}",
                    i + 1,
                    i + 1
                ));
            }
        }
    }
    None
}

/// 配分の領域が空でないかを確かめる (`Domain::Imputation` で `sum v({i}) <= v(N)`)。
pub(crate) fn check_imputation_set(
    game: &ExplicitGame,
    domain: Domain,
    tolerance: f64,
) -> Result<()> {
    check_blocks(game, &[game.grand()], domain, tolerance)
}

/// 配分の領域で、各ブロック `B` に `sum_{i in B} v({i}) <= v(B)` を確かめる (満たさなければ配分がない)。
pub(crate) fn check_blocks(
    game: &ExplicitGame,
    blocks: &[Coalition],
    domain: Domain,
    tolerance: f64,
) -> Result<()> {
    if domain == Domain::Imputation {
        for &block in blocks {
            let lower: f64 = block
                .players()
                .map(|i| game.value(Coalition::singleton(i)))
                .sum();
            if lower > game.value(block) + tolerance {
                return Err(Error::EmptyImputationSet);
            }
        }
    }
    Ok(())
}

/// 全提携について `x(S)` を求める(ビット順、長さ `2^n`)。
pub(crate) fn coalition_sums(x: &[f64]) -> Vec<f64> {
    let size = 1usize << x.len();
    let mut sums = vec![0.0; size];
    for mask in 1..size {
        let lowest = mask.trailing_zeros() as usize;
        sums[mask] = sums[mask & (mask - 1)] + x[lowest];
    }
    sums
}
