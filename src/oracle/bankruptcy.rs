//! 破産ゲームのオラクル。
//!
//! `v(S) = max(0, E - d(N \ S)) = max(0, E - D + d(S))` (`D` は請求の総和) なので、
//! 超過は `max(-x(S), (E - D) + sum_{i in S} (d_i - x_i))` で、2 つの加法的な関数の最大値になる。
//! それぞれを `SubsetsByWeight` で大きい順に列挙し、`MaxMerge` で合わせる。

use super::subsets::SubsetsByWeight;
use super::{MaxMerge, OracleGame, PlayerSet, SetFunction};
use crate::error::{Error, Result};
use crate::structure::{ConvexGame, Proven};

#[derive(Clone, Debug, PartialEq)]
pub struct BankruptcyGame {
    estate: f64,
    claims: Vec<f64>,
    total: f64,
}

impl BankruptcyGame {
    pub fn new(estate: f64, claims: Vec<f64>) -> Result<BankruptcyGame> {
        if estate < 0.0 || claims.iter().any(|d| *d < 0.0 || !d.is_finite()) || !estate.is_finite()
        {
            return Err(Error::InvalidArgument("遺産と請求は非負の有限値".into()));
        }
        let total = claims.iter().sum();
        Ok(BankruptcyGame {
            estate,
            claims,
            total,
        })
    }

    pub fn claims(&self) -> &[f64] {
        &self.claims
    }

    pub fn estate(&self) -> f64 {
        self.estate
    }

    /// 仁を閉じた形の解 (タルムード則) で求める。LP を使わず `O(n log n)`。
    /// 遺産が請求の総和を超える場合はタルムード則が定義されないのでエラーを返す。
    pub fn nucleolus(&self) -> Result<Vec<f64>> {
        crate::bankruptcy::talmud_rule(self.estate, &self.claims)
    }
}

impl SetFunction for BankruptcyGame {
    fn players(&self) -> usize {
        self.claims.len()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        let inside: f64 = coalition.members().map(|i| self.claims[i]).sum();
        (self.estate - self.total + inside).max(0.0)
    }
}

/// 破産ゲームは凸である。`v(S) = max(0, (E - D) + d(S))` は、加法的な `d(S)` に
/// 凸で非減少な関数 `t -> max(0, (E - D) + t)` を合成したもので、`d_i >= 0` のときこの合成は優モジュラになる
/// (`d(S ∪ T) + d(S ∩ T) = d(S) + d(T)` かつ `d(S ∩ T) <= d(S), d(T) <= d(S ∪ T)` と、凸関数の性質から)。
impl ConvexGame for BankruptcyGame {
    type Proof = Proven;
}

impl OracleGame for BankruptcyGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        let negative: Vec<f64> = x.iter().map(|v| -v).collect();
        let gains: Vec<f64> = self.claims.iter().zip(x).map(|(d, v)| d - v).collect();
        let zero_part = SubsetsByWeight::new(&negative, 0.0);
        let positive_part = SubsetsByWeight::new(&gains, self.estate - self.total);
        Box::new(MaxMerge::new(zero_part, positive_part).filter(|(set, _)| set.is_proper()))
    }

    fn scale(&self) -> f64 {
        self.estate
            .max(self.claims.iter().cloned().fold(0.0, f64::max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    #[test]
    fn matches_explicit_bankruptcy_game() {
        let claims = vec![30.0, 70.0, 10.0, 55.0, 20.0];
        let oracle = BankruptcyGame::new(100.0, claims.clone()).unwrap();
        let explicit = generators::bankruptcy(100.0, &claims).unwrap();
        let x = [10.0, 35.0, 0.0, 40.0, 15.0];
        let from_oracle: Vec<(PlayerSet, f64)> = oracle.excess_order(&x).collect();
        let from_explicit: Vec<(PlayerSet, f64)> = explicit.excess_order(&x).collect();
        assert_eq!(from_oracle.len(), 30);
        assert!(from_oracle.windows(2).all(|w| w[0].1 >= w[1].1 - 1e-12));
        // 超過の値の列は一致し、各提携の超過も正しい。
        for ((_, a), (_, b)) in from_oracle.iter().zip(&from_explicit) {
            assert!((a - b).abs() < 1e-9);
        }
        for (set, e) in &from_oracle {
            assert!((oracle.value(set) - set.sum(&x) - e).abs() < 1e-9);
        }
    }
}
