//! 通信グラフで協力が制限されたゲームと Myerson 値 (Myerson 1977)。
//!
//! プレイヤーを頂点とする無向グラフ `g` があり、辺で直接・間接につながった人どうしだけが協力できるとする。
//! グラフ制限ゲーム `v^g(S) = sum_{C in S/g} v(C)` (`S/g` は `S` が誘導する部分グラフの連結成分) の
//! Shapley 値が Myerson 値である。

use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::game::coalition::Coalition;
use crate::values;

/// 辺の一覧を、各頂点の隣接頂点のビット集合に直す。
fn adjacency(players: usize, edges: &[(usize, usize)]) -> Result<Vec<u64>> {
    let mut neighbors = vec![0u64; players];
    for &(a, b) in edges {
        if a >= players || b >= players || a == b {
            return Err(Error::InvalidArgument(format!(
                "辺 ({a}, {b}) の端点が範囲外か、自己ループ"
            )));
        }
        neighbors[a] |= 1 << b;
        neighbors[b] |= 1 << a;
    }
    Ok(neighbors)
}

/// `mask` が誘導する部分グラフの連結成分。
fn components(mask: u64, neighbors: &[u64]) -> Vec<Coalition> {
    let mut rest = mask;
    let mut parts = Vec::new();
    while rest != 0 {
        let start = rest.trailing_zeros() as usize;
        let mut component = 1u64 << start;
        let mut frontier = component;
        while frontier != 0 {
            let v = frontier.trailing_zeros() as usize;
            frontier &= frontier - 1;
            let fresh = neighbors[v] & mask & !component;
            component |= fresh;
            frontier |= fresh;
        }
        rest &= !component;
        parts.push(Coalition(component));
    }
    parts
}

/// グラフ制限ゲーム `v^g(S) = sum_{C in S/g} v(C)`。`edges` はプレイヤー番号 (0 始まり) の組。
pub fn graph_restricted(game: &ExplicitGame, edges: &[(usize, usize)]) -> Result<ExplicitGame> {
    let n = game.players();
    let neighbors = adjacency(n, edges)?;
    let restricted: Vec<f64> = (1..1u64 << n)
        .map(|mask| {
            components(mask, &neighbors)
                .into_iter()
                .map(|c| game.value(c))
                .sum()
        })
        .collect();
    ExplicitGame::from_binary(&restricted)
}

/// Myerson 値: グラフ制限ゲームの Shapley 値。
pub fn myerson(game: &ExplicitGame, edges: &[(usize, usize)]) -> Result<Vec<f64>> {
    Ok(values::shapley(&graph_restricted(game, edges)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    fn assert_close(actual: &[f64], expected: &[f64]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-9, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn complete_graph_gives_shapley_value() {
        let game = generators::bnf(1, 5, 2).unwrap();
        let edges: Vec<(usize, usize)> = (0..5)
            .flat_map(|a| (a + 1..5).map(move |b| (a, b)))
            .collect();
        assert_close(&myerson(&game, &edges).unwrap(), &values::shapley(&game));
    }

    #[test]
    fn line_graph_three_player_majority() {
        // 3 人多数決 (2 人以上で 1) を線 0 - 1 - 2 に制限すると、0 と 2 は 1 を通さないと協力できない。
        // v^g: {0,1} = {1,2} = 1、{0,2} = 0、N = 1。中央の 1 の Shapley 値は 2/3、両端は 1/6。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let restricted = graph_restricted(&game, &[(0, 1), (1, 2)]).unwrap();
        assert_eq!(restricted.value(Coalition::from_players(&[0, 2])), 0.0);
        assert_close(
            &myerson(&game, &[(0, 1), (1, 2)]).unwrap(),
            &[1.0 / 6.0, 2.0 / 3.0, 1.0 / 6.0],
        );
    }

    #[test]
    fn glove_game_on_a_path_matches_myerson_package_example() {
        // Python の myerson パッケージの Get Started の例: 手袋ゲームを道 1 - 2 - 3 に制限すると (1/2, 1/2, 0)。
        let glove = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0]).unwrap();
        assert_close(
            &myerson(&glove, &[(0, 1), (1, 2)]).unwrap(),
            &[0.5, 0.5, 0.0],
        );
    }

    #[test]
    fn isolated_player_receives_standalone_value() {
        // 辺のない頂点は誰とも協力できないので、Myerson 値は v({i})。
        let game = generators::bnf(1, 4, 9).unwrap();
        let x = myerson(&game, &[(0, 1), (1, 2)]).unwrap();
        assert!((x[3] - game.value(Coalition::singleton(3))).abs() < 1e-9);
    }

    #[test]
    fn invalid_edges_are_rejected() {
        let game = generators::bnf(1, 3, 0).unwrap();
        assert!(myerson(&game, &[(0, 3)]).is_err());
        assert!(myerson(&game, &[(1, 1)]).is_err());
    }
}
