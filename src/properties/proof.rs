//! 性質を型で表すラッパー (説明は [`super`] のモジュール文書)。

use std::marker::PhantomData;

use crate::game::{ExplicitGame, PlayerSet, SetFunction};
use crate::properties;
use crate::solution::{Guarantee, Property, Unverified};

mod sealed {
    /// [`super::ProofKind`] と [`super::ConvexGame`] の上位トレイト。
    ///
    /// 公開しないモジュールに置くので、クレートの外からは名前を書けず、実装もできない。
    /// 性質を持つと主張できる型は、このクレートで実装したものだけになる。
    pub trait Sealed {}
}

pub(crate) use sealed::Sealed;

/// 性質の根拠の種類。性質に基づく手法の結果の型と保証を決める。
///
/// 根拠は [`Proven`] (型の構造、または計算で確かめた) と [`Assumed`] (利用者の宣言) の 2 つだけで、
/// クレートの外では実装できない。保証 ([`Guarantee`]) はライブラリが確かめた事実からだけ付くので、
/// 利用者が独自の根拠を定義して保証を名乗ることはできない。
///
/// ```compile_fail,E0277
/// use coopgame::properties::ProofKind;
/// use coopgame::{Guarantee, Property};
///
/// enum Fake {}
///
/// impl ProofKind for Fake {
///     type Output<T> = T;
///     fn guarantee(_property: Property) -> Guarantee {
///         Guarantee::Certified
///     }
///     fn wrap<T>(value: T, _property: Property) -> T {
///         value
///     }
/// }
/// ```
pub trait ProofKind: Sealed {
    /// 結果 `T` を返すときの型。
    type Output<T>;
    /// 性質 `property` に基づく手法の結果の保証。
    fn guarantee(property: Property) -> Guarantee;
    /// 結果を [`ProofKind::Output`] の型にする。
    fn wrap<T>(value: T, property: Property) -> Self::Output<T>;
}

/// 性質が成り立つことが分かっている (構造、または計算による確認)。
pub enum Proven {}

/// 利用者が性質を宣言しただけ。
pub enum Assumed {}

impl Sealed for Proven {}

impl Sealed for Assumed {}

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
/// 最大余剰は劣モジュラ関数の最小化で求められる。[`crate::nucleolus::convex`] の手法が使える。
///
/// 凸だと主張できるのは次の型だけで、クレートの外では実装できない。
/// 凸でないゲームを [`Proven`] として扱うと、凸ゲームの手法が返す誤った配分に `proven(convex)` が付くためである。
///
/// | 型 | 根拠 | [`ConvexGame::Proof`] |
/// |---|---|---|
/// | [`BankruptcyGame`]、[`InducedSubgraphGame`]、[`AirportGame`] | 型の構造から凸 | [`Proven`] |
/// | [`ConvexChecked`] | 全提携で優モジュラ性を計算で確かめた | [`Proven`] |
/// | [`Assume`] ([`Assume::convex`]) | 利用者が宣言した (確かめない) | [`Assumed`] |
///
/// 利用者が定義したゲームを凸として扱うには、次のどちらかを使う。
///
/// - [`Assume::convex`] で包む。結果は [`Unverified`] で返るので、[`Unverified::verify`] で検証してから使う。
/// - 全提携の表 ([`ExplicitGame::tabulate`]、30 人まで) を作り、[`ConvexChecked::new`] で凸であることを確かめる。
///
/// ```
/// use coopgame::nucleolus::convex;
/// use coopgame::properties::{Assume, ConvexChecked};
/// use coopgame::verify::{Verified, VerifyOptions};
/// use coopgame::{ExplicitGame, Guarantee, PlayerSet, Property, SetFunction};
///
/// /// 提携の値が人数の 2 乗のゲーム (凸)。
/// struct Square(usize);
///
/// impl SetFunction for Square {
///     fn players(&self) -> usize {
///         self.0
///     }
///     fn value(&self, coalition: &PlayerSet) -> f64 {
///         (coalition.len() * coalition.len()) as f64
///     }
/// }
///
/// // 宣言して求め、検証してから使う。
/// let unverified = convex::nucleolus(&Assume::convex(Square(4)))?;
/// assert_eq!(unverified.peek().guarantee(), Guarantee::Assumed(Property::Convex));
/// let Verified::Certified(solution) = unverified.verify(&Square(4), VerifyOptions::default())? else {
///     panic!("凸ゲームなので合格する");
/// };
/// assert_eq!(solution.guarantee(), Guarantee::Certified);
///
/// // 全提携の表で凸であることを確かめる。
/// let checked = ConvexChecked::new(ExplicitGame::tabulate(&Square(4))?).expect("凸");
/// let solution = convex::nucleolus(&checked)?;
/// assert_eq!(solution.guarantee(), Guarantee::Proven(Property::Convex));
/// assert!(solution.allocation().iter().all(|x| (x - 4.0).abs() < 1e-9));
/// # Ok::<(), coopgame::Error>(())
/// ```
///
/// 利用者の型に `ConvexGame` を実装することはできない。
///
/// ```compile_fail,E0277
/// use coopgame::properties::{ConvexGame, Proven};
/// use coopgame::{PlayerSet, SetFunction};
///
/// /// 3 人の多数決ゲーム (凸でない)。
/// struct Majority;
///
/// impl SetFunction for Majority {
///     fn players(&self) -> usize {
///         3
///     }
///     fn value(&self, coalition: &PlayerSet) -> f64 {
///         if coalition.len() >= 2 { 1.0 } else { 0.0 }
///     }
/// }
///
/// impl ConvexGame for Majority {
///     type Proof = Proven;
/// }
/// ```
///
/// [`BankruptcyGame`]: crate::games::bankruptcy::BankruptcyGame
/// [`InducedSubgraphGame`]: crate::games::graph::InducedSubgraphGame
/// [`AirportGame`]: crate::games::airport::AirportGame
pub trait ConvexGame: SetFunction + Sealed {
    /// 凸性の根拠 ([`Proven`] か [`Assumed`])。
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

impl Sealed for ConvexChecked {}

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

impl<G: SetFunction> Sealed for Assume<Convexity, G> {}

impl<G: SetFunction> ConvexGame for Assume<Convexity, G> {
    type Proof = Assumed;
}
