//! サンプリングした提携に制限した仁・最小コア (データ評価向け)。
//!
//! 人数が数百になるゲーム (例: 提携の値が「その部分データで学習したモデルの精度」のデータ評価) では、
//! 全提携の超過を扱えない。そこで提携を `M` 組サンプリングし、サンプルした提携の超過だけを
//! 辞書式に最小化した配分 (制限したゲームの仁) を近似として使う。最小コアについては、
//! サンプルした提携に制限して解く方法がデータ評価で提案されている (Yan & Procaccia 2021)。
//!
//! - サンプルは提携 `S` とその補集合 `N \ S` の組で取る (各人を確率 1/2 で含める)。
//!   1 人提携とその補集合も必ず含める。補集合が揃っていると、仁の逐次 LP が常に有界になる。
//! - 制限したゲームの仁は [`crate::nucleolus::oracle`] で解く (サンプルした提携だけを超過の大きい順に返す)。
//! - 全提携をサンプルにすれば、真の仁に一致する。

use std::collections::HashMap;

use super::oracle;
use crate::Domain;
use crate::error::{Error, Result};
use crate::game::oracle::OracleGame;
use crate::game::{PlayerSet, SetFunction};
use crate::nucleolus::{LeastCore, NucleolusResult};
use crate::rng::SplitMix64;
use crate::solution::Guarantee;
use crate::sorted::{LazySorted, by_excess_desc};

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidArgument(format!(
            "提携の値 {value} が有限の数ではない"
        )))
    }
}

/// サンプルした提携とその値だけを持つゲーム。
#[derive(Clone, Debug)]
pub struct SampledGame {
    players: usize,
    /// サンプルした真部分提携 (重複なし)。
    coalitions: Vec<PlayerSet>,
    values: HashMap<PlayerSet, f64>,
    grand_value: f64,
    scale: f64,
}

impl SampledGame {
    /// 1 人提携とその補集合に加え、ランダムな提携とその補集合を `pairs` 組サンプルする。
    pub fn sample<G: SetFunction + ?Sized>(
        game: &G,
        pairs: usize,
        seed: u64,
    ) -> Result<SampledGame> {
        let n = game.players();
        if n < 2 {
            return Err(Error::InvalidArgument("2 人以上が必要".into()));
        }
        let mut rng = SplitMix64::new(seed);
        let mut coalitions: Vec<PlayerSet> = Vec::new();
        for i in 0..n {
            let single = PlayerSet::singleton(n, i);
            coalitions.push(single.complement());
            coalitions.push(single);
        }
        for _ in 0..pairs {
            let mut set = PlayerSet::empty(n);
            for i in 0..n {
                if rng.next_u64() & 1 == 1 {
                    set.insert(i);
                }
            }
            if set.is_proper() {
                coalitions.push(set.complement());
                coalitions.push(set);
            }
        }
        SampledGame::from_coalitions(game, coalitions)
    }

    /// 指定した提携 (補集合も加える) をサンプルとするゲーム。1 人提携とその補集合は必ず加える。
    ///
    /// 値が有限の数でない提携があればエラーを返す (NaN のまま LP に渡すと止まらないため)。
    pub(crate) fn from_coalitions<G: SetFunction + ?Sized>(
        game: &G,
        coalitions: Vec<PlayerSet>,
    ) -> Result<SampledGame> {
        let n = game.players();
        let mut values: HashMap<PlayerSet, f64> = HashMap::new();
        let mut unique: Vec<PlayerSet> = Vec::new();
        let add =
            |set: PlayerSet, values: &mut HashMap<PlayerSet, f64>, unique: &mut Vec<PlayerSet>| {
                if set.universe() != n {
                    return Err(Error::InvalidArgument("提携の人数がゲームと異なる".into()));
                }
                if set.is_proper() && !values.contains_key(&set) {
                    let value = finite(game.value(&set))?;
                    values.insert(set.clone(), value);
                    unique.push(set);
                }
                Ok(())
            };
        for i in 0..n {
            let single = PlayerSet::singleton(n, i);
            add(single.complement(), &mut values, &mut unique)?;
            add(single, &mut values, &mut unique)?;
        }
        for set in coalitions {
            add(set.complement(), &mut values, &mut unique)?;
            add(set, &mut values, &mut unique)?;
        }
        let grand_value = finite(game.value(&PlayerSet::grand(n)))?;
        let scale = values
            .values()
            .fold(grand_value.abs(), |acc, v| acc.max(v.abs()));
        Ok(SampledGame {
            players: n,
            coalitions: unique,
            values,
            grand_value,
            scale,
        })
    }

    /// 特性関数を評価した回数 (サンプルした真部分提携の数 + 全体提携)。
    pub fn evaluations(&self) -> usize {
        self.coalitions.len() + 1
    }

    pub fn coalitions(&self) -> &[PlayerSet] {
        &self.coalitions
    }

    /// 配分 `x` での、サンプルした真部分提携の超過の最大値。
    fn max_excess(&self, x: &[f64]) -> f64 {
        self.coalitions
            .iter()
            .map(|set| self.values[set] - set.sum(x))
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

impl SetFunction for SampledGame {
    fn players(&self) -> usize {
        self.players
    }

    /// サンプルした提携の値。サンプルしていない真部分提携は呼ばない前提で、呼ぶとパニックする。
    fn value(&self, coalition: &PlayerSet) -> f64 {
        if coalition.is_empty() {
            return 0.0;
        }
        if coalition.is_full() {
            return self.grand_value;
        }
        *self
            .values
            .get(coalition)
            .expect("サンプルしていない提携の値は使わない (補集合も含めてサンプルしている)")
    }
}

impl OracleGame for SampledGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        let order: Vec<(PlayerSet, f64)> = self
            .coalitions
            .iter()
            .map(|set| (set.clone(), self.values[set] - set.sum(x)))
            .collect();
        let first_chunk = 4 * self.players * self.players;
        Box::new(LazySorted::new(order, first_chunk, by_excess_desc))
    }

    fn scale(&self) -> f64 {
        self.scale
    }

    /// サンプルした提携だけを返すので `false`。このゲームで求めた仁・最小コアは近似になる
    /// (全提携をサンプルした場合も、サンプルしたゲームとして扱う)。
    fn lists_all_coalitions(&self) -> bool {
        false
    }
}

/// サンプリングによる近似の結果。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SampledNucleolus {
    pub(crate) allocation: Vec<f64>,
    pub(crate) epsilon: f64,
    pub(crate) levels: Vec<f64>,
    pub(crate) evaluations: usize,
    pub(crate) guarantee: Guarantee,
}

impl SampledNucleolus {
    pub fn allocation(&self) -> &[f64] {
        &self.allocation
    }

    pub fn into_allocation(self) -> Vec<f64> {
        self.allocation
    }

    /// サンプルした提携での最小コアの値 (`domain` の配分のうちで、サンプルした提携の最大超過の最小値)。
    ///
    /// 配分集合が 1 点で LP を解かなかったとき ([`SampledNucleolus::levels`] が空) は、
    /// その点でのサンプルした提携の最大超過。
    pub fn epsilon(&self) -> f64 {
        self.epsilon
    }

    /// 各段の最適な最大超過 (サンプルした提携での値。[`NucleolusResult::levels`] と同じ)。
    pub fn levels(&self) -> &[f64] {
        &self.levels
    }

    /// 特性関数を評価した回数。
    pub fn evaluations(&self) -> usize {
        self.evaluations
    }

    /// 常に [`Guarantee::Approximate`] (全提携をサンプルした場合も、検証するまでは近似として扱う)。
    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }
}

/// 提携を `pairs` 組サンプルし、制限したゲームの仁 (プレ仁) を求める。
pub fn nucleolus<G: SetFunction + ?Sized>(
    game: &G,
    pairs: usize,
    seed: u64,
    domain: Domain,
) -> Result<SampledNucleolus> {
    let sampled = SampledGame::sample(game, pairs, seed)?;
    // SampledGame は全提携を返さないオラクルなので、保証は近似になる。
    let result: NucleolusResult =
        oracle::nucleolus_with(&sampled, domain, oracle::default_tolerance(&sampled))?;
    let epsilon = match result.levels.first() {
        Some(level) => *level,
        None => sampled.max_excess(&result.allocation),
    };
    Ok(SampledNucleolus {
        epsilon,
        allocation: result.allocation,
        levels: result.levels,
        evaluations: sampled.evaluations(),
        guarantee: result.guarantee,
    })
}

/// 提携を `pairs` 組サンプルし、制限したゲームの最小コアの配分を 1 つ求める。保証は [`Guarantee::Approximate`]。
pub fn least_core<G: SetFunction + ?Sized>(
    game: &G,
    pairs: usize,
    seed: u64,
    domain: Domain,
) -> Result<(LeastCore, usize)> {
    let sampled = SampledGame::sample(game, pairs, seed)?;
    let least = oracle::least_core(&sampled, domain)?;
    Ok((least, sampled.evaluations()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;
    use crate::nucleolus;

    #[test]
    fn all_coalitions_give_exact_nucleolus() {
        for kind in [1, 2, 4] {
            let game = generators::bnf(kind, 6, 3).unwrap();
            let n = game.players();
            let all: Vec<PlayerSet> = (1u64..(1 << n) - 1)
                .map(|mask| PlayerSet::from_coalition(n, crate::Coalition(mask)))
                .collect();
            let sampled = SampledGame::from_coalitions(&game, all).unwrap();
            assert_eq!(sampled.evaluations(), (1 << n) - 1);
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let expected =
                    nucleolus::nucleolus_with(&game, nucleolus::Options::new(&game, domain))
                        .unwrap()
                        .allocation;
                let actual = oracle::nucleolus_with(&sampled, domain, 1e-7 * game.max_abs_value())
                    .unwrap()
                    .allocation;
                for (a, e) in actual.iter().zip(&expected) {
                    assert!(
                        (a - e).abs() < 1e-6 * game.max_abs_value(),
                        "type{kind} {domain:?}"
                    );
                }
            }
        }
    }

    /// 1 人提携の値の和が `v(N)` に等しいと、配分集合は 1 点 `x = (1, 1, 1)` で LP を解かない。
    /// そのときも `epsilon` は返す配分でのサンプルした提携の最大超過 (提携 {0, 1} の `5 - 2 = 3`) で、
    /// サンプルした提携での最小コアの値と一致する。
    #[test]
    fn epsilon_on_single_point_imputation_set() {
        let game = crate::ExplicitGame::from_lex(&[1.0, 1.0, 1.0, 5.0, 2.0, 2.0, 3.0]).unwrap();
        let result = nucleolus(&game, 50, 0, Domain::Imputation).unwrap();
        assert!(result.levels().is_empty());
        assert_eq!(result.allocation(), &[1.0, 1.0, 1.0]);
        assert_eq!(result.epsilon(), 3.0);
        let (least, _) = least_core(&game, 50, 0, Domain::Imputation).unwrap();
        assert!((least.epsilon() - result.epsilon()).abs() < 1e-9);
        assert_eq!(result.guarantee(), Guarantee::Approximate);
        assert_eq!(least.guarantee(), Guarantee::Approximate);
    }

    #[test]
    fn rejects_non_finite_values() {
        struct Broken;
        impl SetFunction for Broken {
            fn players(&self) -> usize {
                4
            }
            fn value(&self, coalition: &PlayerSet) -> f64 {
                if coalition.len() == 2 { f64::NAN } else { 1.0 }
            }
        }
        assert!(SampledGame::sample(&Broken, 20, 0).is_err());
    }

    #[test]
    fn samples_are_closed_under_complement() {
        let game = generators::bnf(1, 8, 0).unwrap();
        let sampled = SampledGame::sample(&game, 50, 1).unwrap();
        for set in sampled.coalitions() {
            assert!(sampled.coalitions().contains(&set.complement()));
        }
        // 1 人提携 8 個とその補集合 8 個は必ず含む。
        assert!(sampled.evaluations() > 16);
    }
}
