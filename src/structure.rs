//! ゲームの性質を型で表し、性質に基づく手法の保証を決める。
//!
//! 性質を持つことの根拠は 3 通りある。
//!
//! | 根拠 | 例 | 証明の種類 ([`ProofKind`]) | 結果の保証 |
//! |---|---|---|---|
//! | 型が構造的に持つ | [`crate::oracle::bankruptcy::BankruptcyGame`]、[`crate::oracle::graph::InducedSubgraphGame`] | [`Proven`] | [`Guarantee::Proven`] |
//! | 計算で確認した | [`ConvexChecked::new`] (全提携で優モジュラ性を判定) | [`Proven`] | [`Guarantee::Proven`] |
//! | 利用者が宣言した | [`Assume::convex`] | [`Assumed`] | [`Guarantee::Assumed`] |
//!
//! 性質に基づく手法 (例: [`crate::convex::nucleolus`]) は、証明の種類に応じて戻り値の型が変わる。
//! [`Proven`] なら結果をそのまま返し、[`Assumed`] なら [`Unverified`] に包んで返す。
//! したがって、宣言しただけの性質に基づく結果を、保証付きの結果と取り違えることはない
//! (コンパイル時に区別される)。

use std::marker::PhantomData;

use crate::game::ExplicitGame;
use crate::guarantee::{Guarantee, Property, Unverified};
use crate::oracle::{PlayerSet, SetFunction};
use crate::properties;

/// 性質の根拠の種類。結果の型と保証を決める。
pub trait ProofKind {
    /// 結果 `T` を返すときの型。
    type Output<T>;
    /// 性質 `property` に基づく手法の結果の保証。
    fn guarantee(property: Property) -> Guarantee;
    fn wrap<T>(value: T, property: Property) -> Self::Output<T>;
}

/// 性質が成り立つことが分かっている (構造、または計算による確認)。
pub enum Proven {}

/// 利用者が性質を宣言しただけ。
pub enum Assumed {}

impl ProofKind for Proven {
    type Output<T> = T;

    fn guarantee(property: Property) -> Guarantee {
        Guarantee::Proven(property)
    }

    fn wrap<T>(value: T, _property: Property) -> T {
        value
    }
}

impl ProofKind for Assumed {
    type Output<T> = Unverified<T>;

    fn guarantee(property: Property) -> Guarantee {
        Guarantee::Assumed(property)
    }

    fn wrap<T>(value: T, property: Property) -> Unverified<T> {
        Unverified::new(value, property)
    }
}

/// 凸ゲーム (優モジュラ: `v(S ∪ T) + v(S ∩ T) >= v(S) + v(T)`)。
///
/// 凸ゲームでは、カーネルは仁の 1 点に一致し (Maschler, Peleg & Shapley 1971)、
/// 最大余剰は劣モジュラ関数の最小化で求められる。[`crate::convex`] の手法が使える。
pub trait ConvexGame: SetFunction {
    type Proof: ProofKind;
}

/// 全提携の値の表を持ち、凸であることを計算で確かめたゲーム。
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexChecked {
    game: ExplicitGame,
}

impl ConvexChecked {
    /// 凸なら包んで返す。凸でなければ元のゲームを `Err` で返す。判定は `n^2 2^n`。
    pub fn new(game: ExplicitGame) -> std::result::Result<ConvexChecked, ExplicitGame> {
        if properties::is_convex(&game) {
            Ok(ConvexChecked { game })
        } else {
            Err(game)
        }
    }

    pub fn game(&self) -> &ExplicitGame {
        &self.game
    }
}

impl SetFunction for ConvexChecked {
    fn players(&self) -> usize {
        self.game.players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        SetFunction::value(&self.game, coalition)
    }
}

impl ConvexGame for ConvexChecked {
    type Proof = Proven;
}

/// 凸性を表す印 ([`Assume`] の型引数)。
pub enum Convexity {}

/// 利用者が性質 `P` を宣言したゲーム。性質は確かめない。
///
/// 性質に基づく手法を、性質が分からないゲームで試すために使う。結果は [`Unverified`] で返る。
pub struct Assume<P, G> {
    game: G,
    marker: PhantomData<fn() -> P>,
}

impl<G> Assume<Convexity, G> {
    /// `game` が凸であると宣言する (確かめない)。
    pub fn convex(game: G) -> Assume<Convexity, G> {
        Assume {
            game,
            marker: PhantomData,
        }
    }
}

impl<P, G> Assume<P, G> {
    pub fn inner(&self) -> &G {
        &self.game
    }

    pub fn into_inner(self) -> G {
        self.game
    }
}

impl<P, G: SetFunction> SetFunction for Assume<P, G> {
    fn players(&self) -> usize {
        self.game.players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        self.game.value(coalition)
    }
}

impl<G: SetFunction> ConvexGame for Assume<Convexity, G> {
    type Proof = Assumed;
}
