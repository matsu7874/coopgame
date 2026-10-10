//! 配分の分析と可視化。
//!
//! - [`explain`] は配分を説明し、比較する (不満の大きい提携、コアへの所属、安定性)。
//! - [`uncertainty`] は特性関数の値が不確かなときの配分の分布と感度を求める。
//! - [`plot`] は 3-4 人のゲームの配分集合を SVG の図にする。
//! - [`search`] は性質の反例を探し、縮小する。

pub mod explain;
pub mod plot;
pub mod search;
pub mod uncertainty;
