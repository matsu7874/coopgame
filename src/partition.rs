//! 提携構造 (プレイヤーの分割) と事前の連合のもとでの解。
//!
//! - 提携構造 `B = {B_1, ..., B_m}` (N の分割): 各ブロックが別々に協力する状況。
//!   - Aumann–Drèze 値: 各ブロック内の部分ゲームの Shapley 値 (Aumann & Drèze 1974)。
//!   - 提携構造つきの仁: 各ブロックで `x(B_k) = v(B_k)` を課した仁。
//! - 事前の連合 (a priori unions): 全員が協力するが、交渉は連合単位で行う状況。
//!   - Owen 値: 連合の順序と連合内の順序をそれぞれ一様に選んだときの限界貢献の期待値 (Owen 1977)。
//!     `phi_i = sum_{R ⊆ M \ {k}} sum_{T ⊆ B_k \ {i}} r!(m-r-1)!/m! * t!(b-t-1)!/b! * [v(Q ∪ T ∪ {i}) - v(Q ∪ T)]`
//!     (`B_k` は i の連合、`Q` は R の連合の和、`m` は連合の数、`b = |B_k|`)。

use crate::Domain;
use crate::coalition::Coalition;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::nucleolus::{self, NucleolusResult, Options};
use crate::values;

/// N の分割。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoalitionStructure {
    players: usize,
    blocks: Vec<Coalition>,
}

impl CoalitionStructure {
    /// プレイヤー番号のブロックの列から作る。全員がちょうど 1 つのブロックに属する必要がある。
    pub fn new(players: usize, blocks: &[Vec<usize>]) -> Result<CoalitionStructure> {
        let mut seen = 0u64;
        let mut coalitions = Vec::with_capacity(blocks.len());
        for block in blocks {
            if block.is_empty() {
                return Err(Error::InvalidArgument("空のブロック".into()));
            }
            let mut mask = 0u64;
            for &i in block {
                if i >= players || seen >> i & 1 == 1 || mask >> i & 1 == 1 {
                    return Err(Error::InvalidArgument(format!(
                        "プレイヤー {i} が範囲外か、2 つ以上のブロックに属する"
                    )));
                }
                mask |= 1 << i;
            }
            seen |= mask;
            coalitions.push(Coalition(mask));
        }
        if seen != Coalition::grand(players).0 {
            return Err(Error::InvalidArgument(
                "どのブロックにも属さないプレイヤーがいる".into(),
            ));
        }
        Ok(CoalitionStructure {
            players,
            blocks: coalitions,
        })
    }

    /// 全員で 1 つのブロック。
    pub fn grand(players: usize) -> CoalitionStructure {
        CoalitionStructure {
            players,
            blocks: vec![Coalition::grand(players)],
        }
    }

    /// 全員が単独のブロック。
    pub fn singletons(players: usize) -> CoalitionStructure {
        CoalitionStructure {
            players,
            blocks: (0..players).map(Coalition::singleton).collect(),
        }
    }

    pub fn blocks(&self) -> &[Coalition] {
        &self.blocks
    }

    fn check(&self, game: &ExplicitGame) -> Result<()> {
        if game.players() != self.players {
            return Err(Error::InvalidArgument(format!(
                "提携構造の人数 {} がゲームの人数 {} と異なる",
                self.players,
                game.players()
            )));
        }
        Ok(())
    }
}

fn factorials(n: usize) -> Vec<f64> {
    let mut f = vec![1.0; n + 1];
    for k in 1..=n {
        f[k] = f[k - 1] * k as f64;
    }
    f
}

/// Aumann–Drèze 値: 各ブロック内の部分ゲームの Shapley 値。
pub fn aumann_dreze(game: &ExplicitGame, structure: &CoalitionStructure) -> Result<Vec<f64>> {
    structure.check(game)?;
    let mut value = vec![0.0; game.players()];
    for &block in structure.blocks() {
        let shapley = values::shapley(&game.subgame(block)?);
        for (i, phi) in block.players().zip(shapley) {
            value[i] = phi;
        }
    }
    Ok(value)
}

/// Owen 値 (事前の連合 `unions` のもとでの値)。
pub fn owen(game: &ExplicitGame, unions: &CoalitionStructure) -> Result<Vec<f64>> {
    unions.check(game)?;
    let m = unions.blocks().len();
    let fm = factorials(m);
    let mut value = vec![0.0; game.players()];
    for (k, &union) in unions.blocks().iter().enumerate() {
        let others: Vec<Coalition> = unions
            .blocks()
            .iter()
            .enumerate()
            .filter(|(l, _)| *l != k)
            .map(|(_, c)| *c)
            .collect();
        let members: Vec<usize> = union.players().collect();
        let b = members.len();
        let fb = factorials(b);
        for &i in &members {
            let mates: Vec<usize> = members.iter().copied().filter(|&j| j != i).collect();
            let mut total = 0.0;
            for chosen in 0u64..(1 << others.len()) {
                let r = chosen.count_ones() as usize;
                let q = Coalition::union_of(&others, chosen).0;
                let outer = fm[r] * fm[m - r - 1] / fm[m];
                for sub in 0u64..(1 << mates.len()) {
                    let t = sub.count_ones() as usize;
                    let inner = fb[t] * fb[b - t - 1] / fb[b];
                    let s = q | Coalition::embed(&mates, sub).0;
                    total += outer
                        * inner
                        * (game.value(Coalition(s | 1 << i)) - game.value(Coalition(s)));
                }
            }
            value[i] = total;
        }
    }
    Ok(value)
}

/// 提携構造のもとでの仁 (各ブロックで `x(B) = v(B)`)。
pub fn nucleolus(
    game: &ExplicitGame,
    structure: &CoalitionStructure,
    domain: Domain,
) -> Result<NucleolusResult> {
    structure.check(game)?;
    nucleolus::nucleolus_with_structure(game, structure.blocks(), Options::new(game, domain))
}

/// 連合を 1 人のプレイヤーとみなした商ゲーム `v_B(R) = v(∪_{k ∈ R} B_k)`。
pub fn quotient_game(game: &ExplicitGame, unions: &CoalitionStructure) -> Result<ExplicitGame> {
    unions.check(game)?;
    ExplicitGame::from_fn(unions.blocks().len(), |chosen| {
        game.value(Coalition::union_of(unions.blocks(), chosen.0))
    })
}
