//! 反例の探索と縮小: 「ある性質を満たさないゲーム」をランダムに探し、できるだけ小さく単純な形にする。
//!
//! 予想の検証や、論文・テストに載せる反例を作るための枠組み。性質は「反例であるか」を返す関数で与える
//! (例: 「優加法的で、カーネルが 1 点でない」)。ゲームのクラスの条件 (優加法性など) も関数に含める。
//!
//! 縮小 ([`shrink`]) は、反例であることを保ったまま次の変形を、何も変わらなくなるまで繰り返す。
//!
//! 1. プレイヤーを 1 人除く (除いた人を含まない提携だけの部分ゲームにする)。
//! 2. 提携の値を 0 にする。
//! 3. 提携の値を整数に丸める。
//! 4. 提携の値を 0 に向けて半分にする (整数なら整数の範囲で)。
//!
//! 単純さは (人数, 整数でない値の数, 値の絶対値の和) の辞書式順で比べ、より単純になる変形だけを受け入れる。

use crate::error::Result;
use crate::game::ExplicitGame;
use crate::game::coalition::Coalition;
use crate::generators::SplitMix64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SearchOptions {
    /// ランダムに生成するゲームの数の上限。
    pub attempts: usize,
    pub seed: u64,
    /// 縮小で性質を評価する回数の上限。
    pub shrink_budget: usize,
}

impl Default for SearchOptions {
    fn default() -> SearchOptions {
        SearchOptions {
            attempts: 1_000,
            seed: 0,
            shrink_budget: 2_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Counterexample {
    /// 縮小した反例。
    pub game: ExplicitGame,
    /// 最初に見つかった反例。
    pub original: ExplicitGame,
    /// 何番目の生成で見つかったか (0 始まり)。
    pub attempt: usize,
    /// 縮小で受け入れた変形の数。
    pub shrink_steps: usize,
}

/// `generate` でゲームを作り、`is_counterexample` が真になる最初のゲームを縮小して返す。
pub fn search(
    mut generate: impl FnMut(&mut SplitMix64) -> Result<ExplicitGame>,
    mut is_counterexample: impl FnMut(&ExplicitGame) -> bool,
    options: SearchOptions,
) -> Result<Option<Counterexample>> {
    let mut rng = SplitMix64::new(options.seed);
    for attempt in 0..options.attempts {
        let game = generate(&mut rng)?;
        if is_counterexample(&game) {
            let (shrunk, steps) = shrink(&game, &mut is_counterexample, options.shrink_budget);
            return Ok(Some(Counterexample {
                game: shrunk,
                original: game,
                attempt,
                shrink_steps: steps,
            }));
        }
    }
    Ok(None)
}

/// 単純さの尺度 (小さいほど単純)。
fn complexity(game: &ExplicitGame) -> (usize, usize, f64) {
    let values = &game.values()[1..];
    (
        game.players(),
        values.iter().filter(|v| v.fract() != 0.0).count(),
        values.iter().map(|v| v.abs()).sum(),
    )
}

fn simpler(a: &ExplicitGame, b: &ExplicitGame) -> bool {
    let (pa, fa, sa) = complexity(a);
    let (pb, fb, sb) = complexity(b);
    (pa, fa).cmp(&(pb, fb)).then(sa.total_cmp(&sb)).is_lt()
}

/// プレイヤー `removed` を除いた部分ゲーム。
pub(crate) fn without_player(game: &ExplicitGame, removed: usize) -> Option<ExplicitGame> {
    let n = game.players();
    if n <= 1 {
        return None;
    }
    game.subgame(Coalition(game.grand().0 & !(1 << removed)))
        .ok()
}

/// 値の 1 つを置き換えたゲーム。
fn with_value(game: &ExplicitGame, mask: usize, value: f64) -> Option<ExplicitGame> {
    if game.values()[mask] == value {
        return None;
    }
    game.with_value(Coalition(mask as u64), value).ok()
}

/// 反例であることを保ったまま、ゲームをより単純にする。縮小したゲームと受け入れた変形の数を返す。
///
/// 1 周でプレイヤーの除去と、各提携の値の置き換えを順に試す。受け入れた変形はその場で反映し、
/// 1 周で何も変わらなくなるか、評価回数が `budget` に達したら止める。
pub fn shrink(
    game: &ExplicitGame,
    is_counterexample: &mut impl FnMut(&ExplicitGame) -> bool,
    budget: usize,
) -> (ExplicitGame, usize) {
    let mut current = game.clone();
    let mut evaluations = 0;
    let mut steps = 0;
    let mut try_candidate = |candidate: ExplicitGame, current: &mut ExplicitGame| -> Option<bool> {
        if !simpler(&candidate, current) {
            return Some(false);
        }
        if evaluations >= budget {
            return None;
        }
        evaluations += 1;
        if is_counterexample(&candidate) {
            *current = candidate;
            return Some(true);
        }
        Some(false)
    };
    loop {
        let mut changed = false;
        // プレイヤーの除去 (除けたら同じ番号をもう一度試す)。
        let mut player = 0;
        while player < current.players() {
            let Some(candidate) = without_player(&current, player) else {
                break;
            };
            match try_candidate(candidate, &mut current) {
                None => return (current, steps),
                Some(true) => {
                    steps += 1;
                    changed = true;
                }
                Some(false) => player += 1,
            }
        }
        // 値の置き換え (0、整数への丸め、半分)。
        for mask in 1..current.values().len() {
            loop {
                let value = current.values()[mask];
                let halved = if value.fract() == 0.0 {
                    (value / 2.0).trunc()
                } else {
                    value / 2.0
                };
                let mut accepted = false;
                for replacement in [0.0, value.round(), halved] {
                    let Some(candidate) = with_value(&current, mask, replacement) else {
                        continue;
                    };
                    match try_candidate(candidate, &mut current) {
                        None => return (current, steps),
                        Some(true) => {
                            steps += 1;
                            changed = true;
                            accepted = true;
                            break;
                        }
                        Some(false) => {}
                    }
                }
                if !accepted {
                    break;
                }
            }
        }
        if !changed {
            return (current, steps);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties;

    #[test]
    fn finds_and_shrinks_non_superadditive_game() {
        // v({1, 2}) を負にして優加法性を壊したゲームを探す。縮小は局所的な最小で止まるが、
        // 反例のまま人数・値は単純になる。
        let found = search(
            |rng| {
                crate::generators::bnf(1, 5, rng.next_u64()).map(|g| {
                    let mut values = g.values()[1..].to_vec();
                    values[2] = -values[2].abs() - 1.0;
                    ExplicitGame::from_binary(&values).unwrap()
                })
            },
            |g| !properties::is_superadditive(g),
            SearchOptions::default(),
        )
        .unwrap()
        .expect("反例がある");
        assert_eq!(found.attempt, 0);
        assert!(!properties::is_superadditive(&found.game));
        assert!(found.shrink_steps > 0);
        assert!(simpler(&found.game, &found.original));
        assert!(found.game.values().iter().all(|v| v.fract() == 0.0));
    }

    #[test]
    fn shrinks_to_the_two_players_involved() {
        // 違反は v({1, 2}) < v({1}) + v({2}) だけ。他の人と値は全て除ける。
        let mut values = vec![3.0; 15];
        values[0] = 2.0; // v({1})
        values[1] = 2.0; // v({2})
        values[2] = 1.0; // v({1, 2})
        values[14] = 100.0;
        let game = ExplicitGame::from_binary(&values).unwrap();
        let mut predicate = |g: &ExplicitGame| !properties::is_superadditive(g);
        assert!(predicate(&game));
        let (shrunk, _) = shrink(&game, &mut predicate, 10_000);
        assert_eq!(shrunk.players(), 2);
        // v({1}) + v({2}) > v({1, 2}) を保つ最も単純な形は v({1}) = 1 (または v({2}) = 1)、他は 0。
        assert_eq!(shrunk.values().iter().map(|v| v.abs()).sum::<f64>(), 1.0);
    }

    #[test]
    fn subgame_drops_player() {
        let game = ExplicitGame::from_lex(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]).unwrap();
        // 2 番目の人 (番号 1) を除くと、{1} = 1, {3} = 3, {1, 3} = 5。
        let sub = without_player(&game, 1).unwrap();
        assert_eq!(sub.values(), &[0.0, 1.0, 3.0, 5.0]);
    }
}
