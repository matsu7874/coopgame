//! 特定のクラスのゲーム。
//!
//! 各型は [`crate::game::SetFunction`] を実装するので、[`crate::ExplicitGame::tabulate`] で全提携の表に直せば
//! どの手法にも渡せる。構造を使う専用の手法 (閉じた形の解、オラクルによる制約生成、凸ゲームの手法) もある。
//!
//! | ゲーム | 型 | 専用の手法 |
//! |---|---|---|
//! | 破産ゲーム | [`bankruptcy::BankruptcyGame`] | タルムード則 ([`bankruptcy::talmud_rule`]) による仁、オラクル |
//! | 空港ゲーム | [`airport::AirportGame`] | Littlechild–Owen の式による Shapley 値、凸ゲームの手法による仁 |
//! | 最小全域木ゲーム | [`spanning_tree::SpanningTreeGame`] | Bird 規則 |
//! | 線形生産ゲーム | [`production::LinearProductionGame`] | Owen 配分 |
//! | 重み付き投票ゲーム | [`voting::WeightedVotingGame`] | オラクル |
//! | 誘導部分グラフゲーム | [`graph::InducedSubgraphGame`] | 凸ゲームの手法 |
//! | 費用ゲーム | [`cost::CostGame`] | 節約ゲームに直して仁・カーネルを求める |

pub mod airport;
pub mod bankruptcy;
pub mod cost;
pub mod graph;
pub mod production;
pub mod spanning_tree;
pub mod voting;
