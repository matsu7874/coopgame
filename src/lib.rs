//! TU 協力ゲームの解 (仁・カーネル・Shapley 値など) を計算し、検証するライブラリ。
//!
//! モジュールを目的で分けると次のようになる。
//!
//! | 目的 | モジュール |
//! |---|---|
//! | ゲームの表現 | [`game`] (提携、特性関数、全提携の表、有理数のゲーム、オラクルの能力) |
//! | 特定のクラスのゲーム | [`games`] (破産・空港・最小全域木・線形生産・重み付き投票・誘導部分グラフ・費用) |
//! | 手法の自動選択 | [`auto`] (ゲームの型から、保証のある手法のうち最も速いものを選ぶ) |
//! | 解 | [`nucleolus`] (仁・プレ仁・最小コアと、その別手法)、[`kernel`]、[`values`]、[`compromise`]、[`partition`]、[`communication`]、[`power`] |
//! | 検証と性質 | [`verify`] (Kohlberg 基準、厳密な検証、事後検証)、[`bargaining`]、[`properties`]、[`solution`] (結果と保証の種類) |
//! | 分析と可視化 | [`analysis`] (説明、不確かさ、図、反例の探索)、[`surplus`] (超過と最大余剰) |
//! | 入出力と生成 | [`io`] (feature `io`)、[`generators`] |
//!
//! 同じ解を別の手法で求めるものは、解のモジュールの下に公開のサブモジュールとして置く
//! (例: [`nucleolus::exact`]・[`nucleolus::oracle`]・[`nucleolus::convex`]・[`nucleolus::sampled`])。
//! 迷ったら [`auto`] がゲームの型から保証のある手法を選ぶ。
//!
//! 依存の層 (下の行ほど上の層)。各モジュールは自分より上の行にあるモジュールだけに依存し、同じ行どうしは依存しない。
//!
//! | 段 | モジュール |
//! |---|---|
//! | 0 | `solution`、(内部) `rng` |
//! | 1 | `game` |
//! | 2 | `values`・`surplus`・`compromise`・`power`・`generators`、(内部) `lp`・`linalg`・`rational` |
//! | 3 | `properties`・`kernel`・`bargaining`・`communication`、(内部) `submodular` |
//! | 4 | `nucleolus` |
//! | 5 | `games`・`verify`・`partition`・`io`・`analysis` |
//! | 6 | `auto` |
//!
//! 使い方は、古典的な論文の結果を再現する[チュートリアル](https://github.com/matsu7874/coopgame/tree/main/docs/tutorial)で学べる。

#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod analysis;
pub mod auto;
pub mod bargaining;
pub mod communication;
pub mod compromise;
pub mod error;
pub mod game;
pub mod games;
pub mod generators;
#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
pub mod io;
pub mod kernel;
pub(crate) mod linalg;
mod lp;
pub mod nucleolus;
pub mod partition;
pub mod power;
pub mod properties;
pub(crate) mod rational;
pub(crate) mod rng;
pub mod solution;
pub(crate) mod submodular;
pub mod surplus;
pub mod values;
pub mod verify;

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
    #[doc = include_str!("../docs/examples.md")]
    struct Examples;
}

pub use num_bigint;
pub use num_rational;
pub use num_traits;

pub use error::{Error, Result};
pub use game::{Coalition, ExplicitGame, PlayerSet, SetFunction};
pub use solution::{Concept, Domain, Guarantee, Property, Solution, Unverified};
