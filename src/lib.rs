//! TU 協力ゲームの仁・プレ仁・カーネル・プレカーネルを計算し、検証するライブラリ。
//!
//! - ゲームは特性関数を長さ `2^n` のベクトルで保持する([`ExplicitGame`])。
//!   提携はビット集合 [`Coalition`] で表し、プレイヤー `i` (0 始まり) はビット `i` に対応する。
//! - 仁・プレ仁は逐次 LP (Kopelowitz 方式) で計算する([`nucleolus`])。
//! - カーネル・プレカーネルの 1 点は Maschler/Stearns の transfer scheme で求める([`kernel`])。
//! - 小さいゲームのカーネル・プレカーネル全体は多面体の和集合として求める([`kernel_set`])。
//! - 全提携を列挙しない大きいゲームは、超過の大きい順に提携を返すオラクルとして扱う([`oracle`])。
//! - 破産ゲームの仁は閉じた形の解 (タルムード則) で求められる([`bankruptcy`])。
//! - 人数の多いゲーム (データ評価など) では、サンプリングした提携に制限した仁で近似する([`sampled`])。
//! - Shapley 値・Banzhaf 値は厳密計算とサンプリング近似を用意する([`values`])。
//! - 浮動小数点で求めた仁は、有理数で厳密に検証できる([`exact`])。
//! - 使い方は、古典的な論文の結果を再現する[チュートリアル](https://github.com/matsu7874/coopgame/tree/main/docs/tutorial)で学べる。
//! - 計算結果は Kohlberg 基準([`kohlberg`])と最大余剰の釣り合い条件([`kernel::kernel_violation`])で検証できる。

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod auto;
pub mod bankruptcy;
pub mod bargaining;
pub mod coalition;
pub mod convex;
pub mod cost;
pub mod error;
pub mod exact;
pub mod explain;
pub mod game;
pub mod generators;
pub mod guarantee;
#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
pub mod io;
pub mod kernel;
pub mod kernel_set;
pub mod kohlberg;
pub(crate) mod linalg;
mod lp;
pub mod nucleolus;
pub mod oracle;
pub mod partition;
pub mod plot;
pub mod properties;
pub mod sampled;
pub mod search;
pub mod solution;
pub mod structure;
pub(crate) mod submodular;
pub mod surplus;
pub mod uncertainty;
pub mod values;
pub mod variants;
pub mod verify;

// `docs/` のコード例を `cargo test --doc` で実行する。
#[cfg(doctest)]
mod docs {
    #[doc = include_str!("../docs/tutorial/01-representing-games.md")]
    struct Tutorial01;
    #[doc = include_str!("../docs/tutorial/02-voting-power.md")]
    struct Tutorial02;
    #[doc = include_str!("../docs/tutorial/03-fair-division.md")]
    struct Tutorial03;
    #[doc = include_str!("../docs/tutorial/04-stability-and-verification.md")]
    struct Tutorial04;
    #[doc = include_str!("../README.md")]
    struct Readme;
}

pub use num_bigint;
pub use num_rational;
pub use num_traits;

pub use coalition::Coalition;
pub use error::{Error, Result};
pub use game::{CharacteristicFunction, ExplicitGame};
pub use guarantee::{Guarantee, Property, Unverified};
pub use solution::{Concept, Solution};

/// 配分を探す領域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// 効率性 `x(N) = v(N)` のみを課す(プレ仁・プレカーネル)。
    Preimputation,
    /// 効率性に加えて個人合理性 `x_i >= v({i})` を課す(仁・カーネル)。
    Imputation,
}

/// 特性関数の値の大きさに合わせた既定の許容誤差。
pub fn default_tolerance(game: &ExplicitGame) -> f64 {
    1e-7 * game.max_abs_value().max(1.0)
}
