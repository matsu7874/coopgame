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
///
/// # Panics
///
/// `x` の長さがプレイヤー数と異なるとパニックする (長さの違う配分で誤った表を返さないため、release でも確かめる)。
pub(crate) fn excess_table(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
    assert_eq!(
        x.len(),
        game.players(),
        "配分の長さ {} がプレイヤー数 {} と異なる",
        x.len(),
        game.players()
    );
    let mut excess = coalition_sums(x);
    for (e, value) in excess.iter_mut().zip(game.values()) {
        *e = value - *e;
    }
    excess
}

/// 配分の長さと有限性を確かめる。不正ならその説明を返す。
pub(crate) fn allocation_defect(n: usize, x: &[f64]) -> Option<String> {
    if x.len() != n {
        return Some(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        ));
    }
    x.iter()
        .position(|v| !v.is_finite())
        .map(|i| format!("x_{i} = {} は有限でない", x[i]))
}

/// 長さ・有限性・効率性・領域条件の違反を調べる。違反があればその説明を返す。
///
/// ゲームの値や許容誤差に NaN があれば違反として扱う。
pub(crate) fn feasibility_violation<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Option<String> {
    let n = game.players();
    if let Some(defect) = allocation_defect(n, x) {
        return Some(defect);
    }
    let total: f64 = x.iter().sum();
    let grand = game.value(&PlayerSet::grand(n));
    if !at_most((total - grand).abs(), tolerance) {
        return Some(format!(
            "効率性を満たさない: x(N) = {total}, v(N) = {grand}"
        ));
    }
    if domain == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let lower = game.value(&PlayerSet::singleton(n, i));
            if !at_most(lower - tolerance, *xi) {
                return Some(format!(
                    "個人合理性を満たさない: x_{i} = {xi} < v({{{i}}}) = {lower}"
                ));
            }
        }
    }
    None
}

/// `a <= b`。どちらかが NaN なら偽 (NaN を違反として扱うため)。
fn at_most(a: f64, b: f64) -> bool {
    a <= b
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
            .map(|i| game.value(&PlayerSet::singleton(n, i)))
            .sum();
        if lower > game.value(&PlayerSet::grand(n)) + tolerance {
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
                .members()
                .map(|i| game.value(Coalition::singleton(i)))
                .sum();
            if lower > game.value(block) + tolerance {
                return Err(Error::EmptyImputationSet);
            }
        }
    }
    Ok(())
}
