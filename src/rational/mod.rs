//! 有理数の線形代数と単体法 (厳密な仁の計算と検証が共有する部品)。

pub(crate) mod simplex;

use num_traits::{One, Zero};

use crate::game::Coalition;
use crate::game::exact::Rational;

/// 有理数の行を行階段形 (既約) で保持する。行の最後の成分を右辺として扱える。
#[derive(Clone, Debug, Default)]
pub(crate) struct ExactEchelon {
    /// (ピボット列, ピボットを 1 にした行)
    pub(crate) rows: Vec<(usize, Vec<Rational>)>,
}

/// 行を加えた結果。
pub(crate) enum Inserted {
    /// 新しい独立な行として加えた。
    Independent,
    /// 既存の行から従う (右辺も整合)。
    Dependent,
    /// 係数は既存の行から従うが、右辺が矛盾する。
    Inconsistent,
}

impl ExactEchelon {
    pub(crate) fn reduce(&self, mut row: Vec<Rational>) -> Vec<Rational> {
        for (pivot, existing) in &self.rows {
            if !row[*pivot].is_zero() {
                let factor = row[*pivot].clone();
                for (a, e) in row.iter_mut().zip(existing) {
                    *a -= &factor * e;
                }
            }
        }
        row
    }

    /// `coefficients` 列の係数と、続く右辺 (あれば) からなる行を加える。
    pub(crate) fn insert(&mut self, row: Vec<Rational>, coefficients: usize) -> Inserted {
        let reduced = self.reduce(row);
        let Some(pivot) = (0..coefficients).find(|&j| !reduced[j].is_zero()) else {
            return if reduced[coefficients..].iter().all(Zero::is_zero) {
                Inserted::Dependent
            } else {
                Inserted::Inconsistent
            };
        };
        let scale = reduced[pivot].clone();
        let normalized: Vec<Rational> = reduced.iter().map(|a| a / &scale).collect();
        for (_, existing) in &mut self.rows {
            if !existing[pivot].is_zero() {
                let factor = existing[pivot].clone();
                for (e, a) in existing.iter_mut().zip(&normalized) {
                    *e -= &factor * a;
                }
            }
        }
        self.rows.push((pivot, normalized));
        Inserted::Independent
    }

    pub(crate) fn rank(&self) -> usize {
        self.rows.len()
    }
}

pub(crate) fn indicator(coalition: Coalition, n: usize) -> Vec<Rational> {
    (0..n)
        .map(|i| {
            if coalition.contains(i) {
                Rational::one()
            } else {
                Rational::zero()
            }
        })
        .collect()
}
