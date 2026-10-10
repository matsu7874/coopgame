//! 解の種類・配分・保証の種類をまとめた結果 (手法によらない共通の形)。
//!
//! 結果に付く保証の種類は [`Guarantee`] を、性質を仮定して求めた結果は [`Unverified`] を参照。

mod guarantee;

pub use guarantee::{Guarantee, Property, Unverified};

/// 配分を探す領域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// 効率性 `x(N) = v(N)` のみを課す(プレ仁・プレカーネル)。
    Preimputation,
    /// 効率性に加えて個人合理性 `x_i >= v({i})` を課す(仁・カーネル)。
    Imputation,
}

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
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    pub concept: Concept,
    pub allocation: Vec<f64>,
    pub guarantee: Guarantee,
    /// 使った手法の名前 (例: `"sequential-lp"`、`"talmud-rule"`、`"convex-transfer"`)。
    pub method: &'static str,
}
