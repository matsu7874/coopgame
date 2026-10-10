//! 計算結果の保証の種類。
//!
//! 同じ解 (例: 仁) でも、どう求めたかで信頼できる度合いが違う。結果に保証の種類を持たせ、
//! 性質を仮定して求めた結果は別の型 [`Unverified`] で返すことで、保証付きの結果と取り違えないようにする。
//!
//! | 種類 | 意味 |
//! |---|---|
//! | [`Guarantee::Exact`] | 定義どおりに計算した (全提携を扱う LP・総当たりなど。浮動小数点の許容誤差の範囲で) |
//! | [`Guarantee::Proven`] | ゲームが性質を持つことが分かっており (型の構造、または計算で確認)、その性質のもとで正しい専用手法で求めた |
//! | [`Guarantee::Assumed`] | 利用者が宣言した性質のもとで専用手法を使った。性質が成り立たなければ誤りうる |
//! | [`Guarantee::Certified`] | 求めた後に、有理数による厳密な検証に合格した |
//! | [`Guarantee::Approximate`] | サンプリングなどによる近似、または反復が収束しなかった |

use std::fmt;

/// 専用手法の正しさの根拠になるゲームの性質。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Property {
    /// 凸 (優モジュラ): `v(S ∪ T) + v(S ∩ T) >= v(S) + v(T)`。
    Convex,
    /// 破産ゲーム `v(S) = max(0, E - d(N \ S))` の形をしている。
    Bankruptcy,
}

impl fmt::Display for Property {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Property::Convex => "convex",
            Property::Bankruptcy => "bankruptcy",
        })
    }
}

/// 結果の保証の種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Guarantee {
    Exact,
    Proven(Property),
    Assumed(Property),
    Certified,
    Approximate,
}

impl Guarantee {
    /// 性質の仮定に依存せず、正しいと言える結果か ([`Guarantee::Assumed`] と [`Guarantee::Approximate`] 以外)。
    pub fn is_reliable(self) -> bool {
        matches!(
            self,
            Guarantee::Exact | Guarantee::Proven(_) | Guarantee::Certified
        )
    }
}

impl fmt::Display for Guarantee {
    /// `exact`、`proven(convex)`、`assumed(convex)`、`certified`、`approximate` の形で書く。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Guarantee::Exact => f.write_str("exact"),
            Guarantee::Proven(property) => write!(f, "proven({property})"),
            Guarantee::Assumed(property) => write!(f, "assumed({property})"),
            Guarantee::Certified => f.write_str("certified"),
            Guarantee::Approximate => f.write_str("approximate"),
        }
    }
}

/// 利用者が宣言した性質を仮定して求めた、検証されていない結果。
///
/// 中身を使うには、[`crate::verify`] で検証して保証付きの結果に昇格させるか、
/// [`Unverified::accept_unverified`] で「検証せずに受け入れる」と明示する。
#[derive(Clone, Debug, PartialEq)]
#[must_use = "仮定付きの結果は、検証するか accept_unverified で明示的に受け入れる"]
pub struct Unverified<T> {
    value: T,
    property: Property,
}

impl<T> Unverified<T> {
    pub(crate) fn new(value: T, property: Property) -> Unverified<T> {
        Unverified { value, property }
    }

    /// 仮定した性質。
    pub fn property(&self) -> Property {
        self.property
    }

    /// 中身を見る (検証前であることを承知のうえで)。
    pub fn peek(&self) -> &T {
        &self.value
    }

    /// 検証せずに中身を受け入れる。性質が成り立たなければ誤った結果でありうる。
    pub fn accept_unverified(self) -> T {
        self.value
    }
}
