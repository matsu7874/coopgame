//! ゲームの型が持つ能力と性質から、保証のある手法のうち最も速いものを選んで仁を求める。
//!
//! 選ぶのは保証のある手法 ([`Guarantee::Exact`] か [`Guarantee::Proven`]) だけである。
//! 性質を宣言しただけのゲーム ([`crate::structure::Assume`]) は [`AutoNucleolus`] を実装しないので、
//! 自動選択で仮定付きの手法が使われることはない (仮定付きで試すときは [`crate::convex`] などを直接呼ぶ)。
//!
//! | 型 | 選ぶ手法 | 保証 |
//! |---|---|---|
//! | [`ExplicitGame`] | 逐次 LP + 制約生成 | Exact |
//! | [`ConvexChecked`] | n <= [`EXPLICIT_LP_LIMIT`] は逐次 LP、それより大きいと凸ゲームの手法 | Exact / Proven(convex) |
//! | [`BankruptcyGame`] | タルムード則 (閉じた形) | Proven(bankruptcy) |
//! | [`WeightedVotingGame`] | オラクルによる制約生成 | Exact |
//! | [`InducedSubgraphGame`] | 凸ゲームの手法 | Proven(convex) |

use crate::convex;
use crate::error::Result;
use crate::game::ExplicitGame;
use crate::guarantee::{Guarantee, Property};
use crate::nucleolus;
use crate::oracle::bankruptcy::BankruptcyGame;
use crate::oracle::graph::InducedSubgraphGame;
use crate::oracle::nucleolus as oracle_nucleolus;
use crate::oracle::voting::WeightedVotingGame;
use crate::solution::{Concept, Solution};
use crate::structure::ConvexChecked;

/// 凸と確認済みの明示ゲームで、逐次 LP を使う人数の上限 (計測で決めた。`docs/guarantees.md`)。
pub const EXPLICIT_LP_LIMIT: usize = 16;

/// 保証のある手法のうち最も速いものを選んで仁を求める。
pub trait AutoNucleolus {
    fn nucleolus_auto(&self) -> Result<Solution>;
}

fn exact(allocation: Vec<f64>, method: &'static str) -> Solution {
    Solution {
        concept: Concept::Nucleolus,
        allocation,
        guarantee: Guarantee::Exact,
        method,
    }
}

impl AutoNucleolus for ExplicitGame {
    fn nucleolus_auto(&self) -> Result<Solution> {
        Ok(exact(
            nucleolus::nucleolus(self)?.allocation,
            "sequential-lp",
        ))
    }
}

impl AutoNucleolus for ConvexChecked {
    fn nucleolus_auto(&self) -> Result<Solution> {
        if self.game().players() <= EXPLICIT_LP_LIMIT {
            self.game().nucleolus_auto()
        } else {
            convex::nucleolus(self)
        }
    }
}

impl AutoNucleolus for BankruptcyGame {
    fn nucleolus_auto(&self) -> Result<Solution> {
        Ok(Solution {
            concept: Concept::Nucleolus,
            allocation: self.nucleolus()?,
            guarantee: Guarantee::Proven(Property::Bankruptcy),
            method: "talmud-rule",
        })
    }
}

impl AutoNucleolus for WeightedVotingGame {
    fn nucleolus_auto(&self) -> Result<Solution> {
        Ok(exact(
            oracle_nucleolus::nucleolus(self)?.allocation,
            "oracle-constraint-generation",
        ))
    }
}

impl AutoNucleolus for InducedSubgraphGame {
    fn nucleolus_auto(&self) -> Result<Solution> {
        convex::nucleolus(self)
    }
}
