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

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::oracle::OracleGame;
use crate::game::{PlayerSet, SetFunction};
use crate::generators::SplitMix64;
use crate::nucleolus::oracle as oracle_nucleolus;
use crate::nucleolus::{LeastCore, NucleolusResult};
use crate::solution::Guarantee;

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
            let single = PlayerSet::from_players(n, &[i]);
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
            let single = PlayerSet::from_players(n, &[i]);
            add(single.complement(), &mut values, &mut unique)?;
            add(single, &mut values, &mut unique)?;
        }
        for set in coalitions {
            add(set.complement(), &mut values, &mut unique)?;
            add(set, &mut values, &mut unique)?;
        }
        let grand_value = finite(game.value(&PlayerSet::full(n)))?;
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
        let mut order: Vec<(PlayerSet, f64)> = self
            .coalitions
            .iter()
            .map(|set| (set.clone(), self.values[set] - set.sum(x)))
            .collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Box::new(order.into_iter())
    }

    fn scale(&self) -> f64 {
        self.scale
    }
}

/// サンプリングによる近似の結果。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SampledNucleolus {
    pub allocation: Vec<f64>,
    /// サンプルした提携での最小コアの値 (最大超過の最小値)。
    pub epsilon: f64,
    pub levels: Vec<f64>,
    /// 特性関数を評価した回数。
    pub evaluations: usize,
    /// 常に [`Guarantee::Approximate`] (全提携をサンプルした場合も、検証するまでは近似として扱う)。
    pub guarantee: Guarantee,
}

/// 提携を `pairs` 組サンプルし、制限したゲームの仁 (プレ仁) を求める。
pub fn nucleolus<G: SetFunction + ?Sized>(
    game: &G,
    pairs: usize,
    seed: u64,
    domain: Domain,
) -> Result<SampledNucleolus> {
    let sampled = SampledGame::sample(game, pairs, seed)?;
    let result: NucleolusResult = oracle_nucleolus::nucleolus_with(
        &sampled,
        domain,
        oracle_nucleolus::default_tolerance(&sampled),
    )?;
    Ok(SampledNucleolus {
        epsilon: result.levels.first().copied().unwrap_or(0.0),
        allocation: result.allocation,
        levels: result.levels,
        evaluations: sampled.evaluations(),
        guarantee: Guarantee::Approximate,
    })
}

/// 提携を `pairs` 組サンプルし、制限したゲームの最小コアの配分を 1 つ求める。
pub fn least_core<G: SetFunction + ?Sized>(
    game: &G,
    pairs: usize,
    seed: u64,
    domain: Domain,
) -> Result<(LeastCore, usize)> {
    let sampled = SampledGame::sample(game, pairs, seed)?;
    let mut least = oracle_nucleolus::least_core(&sampled, domain)?;
    least.guarantee = Guarantee::Approximate;
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
                let actual =
                    oracle_nucleolus::nucleolus_with(&sampled, domain, 1e-7 * game.max_abs_value())
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
