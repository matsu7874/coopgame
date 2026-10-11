//! 解を探した範囲・配分・保証の種類をまとめた結果 (手法によらない共通の形)。
//!
//! 結果に付く保証の種類は [`Guarantee`] を、性質を仮定して求めた結果は [`Unverified`] を参照。

mod guarantee;

pub use guarantee::{Guarantee, Property, Unverified};

use crate::game::Domain;

/// 手法によらない共通の仁の結果。`domain` が [`Domain::Imputation`] なら仁、
/// [`Domain::Preimputation`] ならプレ仁。
///
/// 保証 ([`Guarantee`]) はライブラリが計算した事実からだけ付くので、`Solution` を作れるのは
/// ライブラリの中だけで、フィールドは読み出しの関数で読む。任意の配分を検証するには
/// [`crate::verify::check`] に配分を直接渡す。
#[derive(Clone, Debug, PartialEq)]
pub struct Solution {
    pub(crate) domain: Domain,
    pub(crate) allocation: Vec<f64>,
    pub(crate) guarantee: Guarantee,
    pub(crate) method: &'static str,
}

impl Solution {
    /// 解を探した範囲 (仁は [`Domain::Imputation`]、プレ仁は [`Domain::Preimputation`])。
    pub fn domain(&self) -> Domain {
        self.domain
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
