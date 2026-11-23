//! 誘導部分グラフゲーム: プレイヤーは頂点で、提携の値は提携内で完結する辺の重みの和。
//!
//! 辺の重みが非負なら凸 (優モジュラ) である。
//! `v(S ∪ T) + v(S ∩ T) - v(S) - v(T)` は、`S \ T` と `T \ S` の間の辺の重みの和になるため。
//! 構築時に非負を確かめるので、この型は [`ConvexGame`] を構造的に (`Proven`) 満たす。
//! Lean による証明: リポジトリの `proofs/` の `inducedSubgraph_supermodular`。

use crate::error::{Error, Result};
use crate::game::{PlayerSet, SetFunction};
use crate::properties::{ConvexGame, Proven, Sealed};

#[derive(Clone, Debug, PartialEq)]
pub struct InducedSubgraphGame {
    players: usize,
    /// `(u, v, 重み)`。
    edges: Vec<(usize, usize, f64)>,
}

impl InducedSubgraphGame {
    /// 辺の重みは非負の有限値、端点は `players` 未満で異なる頂点。
    ///
    /// 同じ辺 (向きだけが違う `(u, v)` と `(v, u)` も同じ辺) を 2 回書くとエラーにする
    /// (重みを黙って足すと、書き間違いが値の違いとして紛れるため)。
    pub fn new(players: usize, edges: Vec<(usize, usize, f64)>) -> Result<InducedSubgraphGame> {
        if players == 0 {
            return Err(Error::InvalidArgument("プレイヤーがいない".into()));
        }
        let mut seen = std::collections::HashSet::with_capacity(edges.len());
        for &(u, v, w) in &edges {
            if !seen.insert((u.min(v), u.max(v))) {
                return Err(Error::InvalidArgument(format!(
                    "辺 ({u}, {v}) が重複している"
                )));
            }
            if u >= players || v >= players || u == v {
                return Err(Error::InvalidArgument(format!("辺 ({u}, {v}) が不正")));
            }
            if !(w.is_finite() && w >= 0.0) {
                return Err(Error::InvalidArgument(format!(
                    "辺 ({u}, {v}) の重み {w} が非負の有限値でない"
                )));
            }
        }
        Ok(InducedSubgraphGame { players, edges })
    }

    pub fn edges(&self) -> &[(usize, usize, f64)] {
        &self.edges
    }
}

impl SetFunction for InducedSubgraphGame {
    fn players(&self) -> usize {
        self.players
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        self.edges
            .iter()
            .filter(|(u, v, _)| coalition.contains(*u) && coalition.contains(*v))
            .map(|(_, _, w)| w)
            .sum()
    }
}

impl Sealed for InducedSubgraphGame {}

impl ConvexGame for InducedSubgraphGame {
    type Proof = Proven;
}
