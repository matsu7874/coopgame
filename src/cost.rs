//! 費用ゲーム: 費用 `c(S)` (提携 `S` が単独で負担する費用) の分担。
//!
//! 費用の分担 `y` (各人の負担、`y(N) = c(N)`) は、節約ゲーム
//! `s(S) = sum_{i in S} c({i}) - c(S)` (協力で節約できる額) の配分 `x` と `y_i = c({i}) - x_i` で対応する。
//! 節約ゲームの個人合理性 `x_i >= 0` は「単独より多くは払わない」`y_i <= c({i})` に、
//! コア `x(S) >= s(S)` は「どの提携も単独の費用より多くは払わない」`y(S) <= c(S)` に対応する。
//! 仁・カーネルなど超過に基づく解は、加法的な項を足しても (戦略的に同値な変換で) 対応が保たれるので、
//! 節約ゲームで求めて費用に戻せばよい。Shapley 値は線形なので費用ゲームで直接求める。

use crate::error::Result;
use crate::game::ExplicitGame;
use crate::{nucleolus, values};

/// 全提携の費用を持つ費用ゲーム。
#[derive(Clone, Debug, PartialEq)]
pub struct CostGame {
    costs: ExplicitGame,
}

impl CostGame {
    /// 費用 `c(S)` の明示ベクトル (形式は [`ExplicitGame`] と同じ) から作る。
    pub fn new(costs: ExplicitGame) -> CostGame {
        CostGame { costs }
    }

    pub fn costs(&self) -> &ExplicitGame {
        &self.costs
    }

    pub fn players(&self) -> usize {
        self.costs.players()
    }

    /// 単独の費用 `c({i})`。
    pub fn standalone(&self) -> Vec<f64> {
        self.costs.singleton_values()
    }

    /// 節約ゲーム `s(S) = sum_{i in S} c({i}) - c(S)`。
    pub fn savings_game(&self) -> Result<ExplicitGame> {
        let standalone = self.standalone();
        self.costs.map_values(|coalition, cost| {
            coalition.players().map(|i| standalone[i]).sum::<f64>() - cost
        })
    }

    /// 節約ゲームの配分 `x` を費用の分担 `y_i = c({i}) - x_i` に直す。
    pub fn to_costs(&self, savings: &[f64]) -> Vec<f64> {
        shares_from_savings(&self.standalone(), savings)
    }

    /// 費用の分担 `y` を節約ゲームの配分 `x_i = c({i}) - y_i` に直す。
    pub fn to_savings(&self, costs: &[f64]) -> Vec<f64> {
        self.to_costs(costs)
    }

    /// 仁による費用の分担 (誰も単独より多くは払わない範囲で、最も不満な提携の不満を辞書式に最小化)。
    pub fn nucleolus(&self) -> Result<Vec<f64>> {
        let savings = self.savings_game()?;
        Ok(self.to_costs(&nucleolus::nucleolus(&savings)?.allocation))
    }

    /// Shapley 値による費用の分担 (費用ゲームの Shapley 値そのもの)。
    pub fn shapley(&self) -> Vec<f64> {
        values::shapley(&self.costs)
    }
}

/// 節約ゲームの配分 `x` を費用の分担 `y_i = c_i - x_i` に直す (`c_i` は単独の費用)。
/// 逆向き (費用の分担から節約の配分) も同じ式になる。
pub(crate) fn shares_from_savings(standalone: &[f64], savings: &[f64]) -> Vec<f64> {
    standalone.iter().zip(savings).map(|(c, x)| c - x).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn savings_round_trip_and_young_example() {
        // Young (1985) の費用 (CoopGame の costSharingGameVector の例): 節約ゲームは
        // v(13) = 9, v(23) = 10, v(N) = 12、他は 0。
        let costs = ExplicitGame::from_lex(&[15.0, 20.0, 55.0, 35.0, 61.0, 65.0, 78.0]).unwrap();
        let game = CostGame::new(costs);
        let savings = game.savings_game().unwrap();
        let expected = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.0, 9.0, 10.0, 12.0]).unwrap();
        assert_eq!(savings, expected);
        let shares = game.nucleolus().unwrap();
        assert!((shares.iter().sum::<f64>() - 78.0).abs() < 1e-9);
        let back = game.to_savings(&shares);
        let nucleolus = nucleolus::nucleolus(&savings).unwrap().allocation;
        for (a, b) in back.iter().zip(&nucleolus) {
            assert!((a - b).abs() < 1e-9);
        }
        // Shapley 値の費用分担は、節約ゲームの Shapley 値を費用に直したものと一致する (線形性)。
        let direct = game.shapley();
        let via_savings = game.to_costs(&values::shapley(&savings));
        for (a, b) in direct.iter().zip(&via_savings) {
            assert!((a - b).abs() < 1e-9);
        }
    }
}
