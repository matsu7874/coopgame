//! 配分に関する基本的な計算: 配分を探す領域 ([`Domain`])、提携ごとの和 `x(S)` と超過、
//! 配分が領域に入るかの判定。解の手法・検証・分析が共有する。

use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame, PlayerSet, SetFunction};

/// 配分を探す領域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// 効率性 `x(N) = v(N)` のみを課す(プレ仁・プレカーネル)。
    Preimputation,
    /// 効率性に加えて個人合理性 `x_i >= v({i})` を課す(仁・カーネル)。
    Imputation,
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

/// 全提携の超過 `e(S, x) = v(S) - x(S)` (ビット順、長さ `2^n`)。空提携の超過は 0。
pub fn excesses(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    debug_assert_eq!(x.len(), game.players());
    let mut excess = coalition_sums(x);
    for (e, value) in excess.iter_mut().zip(game.values()) {
        *e = value - *e;
    }
    excess
}

/// 効率性と領域条件の違反を調べる。違反があればその説明を返す。
pub(crate) fn feasibility_violation<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Option<String> {
    let n = game.players();
    let total: f64 = x.iter().sum();
    let grand = game.value(&PlayerSet::full(n));
    if (total - grand).abs() > tolerance {
        return Some(format!(
            "効率性を満たさない: x(N) = {total}, v(N) = {grand}"
        ));
    }
    if domain == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let lower = game.value(&PlayerSet::from_players(n, &[i]));
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
pub(crate) fn check_imputation_set<G: SetFunction + ?Sized>(
    game: &G,
    domain: Domain,
    tolerance: f64,
) -> Result<()> {
    let n = game.players();
    if domain == Domain::Imputation {
        let lower: f64 = (0..n)
            .map(|i| game.value(&PlayerSet::from_players(n, &[i])))
            .sum();
        if lower > game.value(&PlayerSet::full(n)) + tolerance {
            return Err(Error::EmptyImputationSet);
        }
    }
    Ok(())
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
