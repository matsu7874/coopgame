//! 解の種類・配分・保証の種類をまとめた結果 (手法によらない共通の形)。
//!
//! 結果に付く保証の種類は [`Guarantee`] を、性質を仮定して求めた結果は [`Unverified`] を参照。

mod guarantee;

pub use guarantee::{Guarantee, Property, Unverified};

use crate::game::Domain;

/// 求めた解の概念。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Concept {
    /// 仁 (配分の中で超過を辞書式に最小化)。
    Nucleolus,
    /// プレ仁 (準配分の中で超過を辞書式に最小化)。
    Prenucleolus,
}

impl Concept {
    /// 解を探す範囲。
    pub fn domain(self) -> Domain {
        match self {
            Concept::Nucleolus => Domain::Imputation,
            Concept::Prenucleolus => Domain::Preimputation,
        }
    }
}

/// 手法によらない共通の結果。
///
/// 保証 ([`Guarantee`]) はライブラリが計算した事実からだけ付くので、`Solution` を作れるのは
/// ライブラリの中だけで、フィールドは読み出しの関数で読む。任意の配分を検証するには
/// [`crate::verify::check`] に配分を直接渡す。
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    pub(crate) concept: Concept,
    pub(crate) allocation: Vec<f64>,
    pub(crate) guarantee: Guarantee,
    pub(crate) method: &'static str,
}

impl Solution {
    pub fn concept(&self) -> Concept {
        self.concept
    }

    pub fn allocation(&self) -> &[f64] {
        &self.allocation
    }

    pub fn into_allocation(self) -> Vec<f64> {
        self.allocation
    }

    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }

    /// 使った手法の名前 (例: `"sequential-lp"`、`"talmud-rule"`、`"convex-transfer"`)。
    pub fn method(&self) -> &'static str {
        self.method
    }
}
