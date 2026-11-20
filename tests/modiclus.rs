//! modiclus を、文献の数値例と、定義とは別の 2 つの特徴づけ (独立した基準) と突き合わせる。
//!
//! modiclus は、空提携と全体提携を含む全ての提携の組 `(S, T)` の超過の差 (bi-excess) `e(S, x) - e(T, x)` を
//! 準配分の上で辞書式に最小化した配分である (Sudhölter & Peleg, "Nucleoli as maximizers of collective
//! satisfaction functions", Working Paper 262 の式 (4.1))。
//! Rosenmüller & Sudhölter, "Cartels via the modiclus" (Working Paper 320) によれば、modiclus は
//!
//! - 双対被覆ゲーム `w(S + T*) = max{v(S) + v*(T), v(T) + v*(S)}` (プレイヤーは `N` とその複製 `N*`) の
//!   プレ仁を元のプレイヤーに射影したものに等しく、
//! - bi-excess `e(R, x, v) + e(T, x, v*)` が `alpha` 以上の組の族が、空でない全ての `alpha` で平衡になる
//!   ただ 1 つの準配分である (Theorem 2.4)。
//!
//! プレ仁は `nucleolus::prenucleolus` で、平衡性は LP で確かめるので、`variants::modiclus` (bi-excess の逐次 LP)
//! とは独立に計算できる。

mod common;

use common::assert_close;
use coopgame::generators::SplitMix64;
use coopgame::nucleolus::{self, variants};
use coopgame::{Coalition, ExplicitGame, values};

/// `x` で、bi-excess `e(R, x, v) + e(T, x, v*)` が `alpha` 以上の組 `(R, T)` の族 `{1_R + 1_T}` が、
/// 空でない全ての `alpha` で平衡か (正の重み `lambda` で `sum lambda (1_R + 1_T)` が `1_N` の正の倍になるか)。
/// Rosenmüller & Sudhölter (Working Paper 320) の Theorem 2.4 では、これが `x` が modiclus であることと同値。
fn bi_excess_families_are_balanced(game: &ExplicitGame, x: &[f64]) -> bool {
    let n = game.players();
    let full = (1u64 << n) - 1;
    let total = game.value(game.grand());
    let sum = |s: u64| Coalition(s).sum(x);
    let excess = |s: u64| game.value(Coalition(s)) - sum(s);
    // e(T, x, v*) = v(N) - v(N \ T) - x(T)
    let dual_excess = |t: u64| total - game.value(Coalition(full & !t)) - sum(t);
    let mut pairs: Vec<(f64, Vec<f64>)> = (0..=full)
        .flat_map(|r| (0..=full).map(move |t| (r, t)))
        .map(|(r, t)| {
            let vector = (0..n)
                .map(|i| ((r >> i & 1) + (t >> i & 1)) as f64)
                .collect();
            (excess(r) + dual_excess(t), vector)
        })
        .collect();
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
    let tolerance = 1e-9 * game.max_abs_value().max(1.0);
    let mut end = 0;
    while end < pairs.len() {
        // 値が許容誤差以内で等しい組をまとめて族に加える。
        let alpha = pairs[end].0;
        while end < pairs.len() && pairs[end].0 >= alpha - tolerance {
            end += 1;
        }
        let mut problem = microlp::Problem::new(microlp::OptimizationDirection::Minimize);
        let weights: Vec<microlp::Variable> = (0..end)
            .map(|_| problem.add_var(0.0, (1.0, f64::INFINITY)))
            .collect();
        let scale = problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY));
        for i in 0..n {
            let mut terms: Vec<(microlp::Variable, f64)> = weights
                .iter()
                .zip(&pairs[..end])
                .filter(|(_, (_, vector))| vector[i] != 0.0)
                .map(|(&weight, (_, vector))| (weight, vector[i]))
                .collect();
            terms.push((scale, -1.0));
            problem.add_constraint(terms, microlp::ComparisonOp::Eq, 0.0);
        }
        if problem.solve().is_err() {
            return false;
        }
    }
    true
}

/// 双対被覆ゲームのプレ仁。前半 `n` 成分が元のプレイヤー、後半 `n` 成分が複製の取り分。
fn dual_cover_prenucleolus(game: &ExplicitGame) -> Vec<f64> {
    let n = game.players();
    let full = (1u64 << n) - 1;
    let total = game.value(game.grand());
    // v(∅) = 0、v*(S) = v(N) - v(N \ S) (v*(∅) = 0)。
    let value = |s: u64| game.value(Coalition(s));
    let dual = |s: u64| total - value(full & !s);
    let values: Vec<f64> = (1..(1u64 << (2 * n)))
        .map(|mask| {
            let (s, t) = (mask & full, mask >> n);
            f64::max(value(s) + dual(t), value(t) + dual(s))
        })
        .collect();
    let cover = ExplicitGame::from_binary(&values).unwrap();
    nucleolus::prenucleolus(&cover).unwrap().into_allocation()
}

/// 双対被覆ゲームのプレ仁の射影と一致することを確かめ、射影を返す。
fn assert_matches_dual_cover(game: &ExplicitGame, label: &str) -> Vec<f64> {
    let n = game.players();
    let tolerance = 1e-7 * game.max_abs_value().max(1.0);
    let cover = dual_cover_prenucleolus(game);
    // 基準の確認: 双対被覆ゲームは N と N* を入れ替えても同じなので、プレ仁の 2 つの半分は等しい。
    assert_close(
        &cover[..n],
        &cover[n..],
        tolerance,
        &format!("{label}: 対称性"),
    );
    let modiclus = variants::modiclus(game).unwrap().into_allocation();
    assert_close(&modiclus, &cover[..n], tolerance, label);
    modiclus
}

/// Meinhardt の Table 4.1 のゲーム v (ビット順で 0, 0, 16/3, 0, 4, 0, 8, 0, 1, 0, 8, 0, 8, 0, 16)。
/// 論文の脚注 (MatTuGames による計算): modiclus (8, 53/12, 37/12, 1/2)、
/// プレ仁 (44/9, 4, 32/9, 32/9)、Shapley 値 (247/36, 121/36, 113/36, 95/36)。
#[test]
fn modiclus_of_meinhardt_table_4_1() {
    let game = ExplicitGame::from_binary(&[
        0.0,
        0.0,
        16.0 / 3.0,
        0.0,
        4.0,
        0.0,
        8.0,
        0.0,
        1.0,
        0.0,
        8.0,
        0.0,
        8.0,
        0.0,
        16.0,
    ])
    .unwrap();
    let modiclus = assert_matches_dual_cover(&game, "Table 4.1");
    assert_close(
        &modiclus,
        &[8.0, 53.0 / 12.0, 37.0 / 12.0, 0.5],
        1e-9,
        "modiclus",
    );
    assert_close(
        nucleolus::prenucleolus(&game).unwrap().allocation(),
        &[44.0 / 9.0, 4.0, 32.0 / 9.0, 32.0 / 9.0],
        1e-9,
        "プレ仁",
    );
    assert_close(
        &values::shapley(&game),
        &[247.0 / 36.0, 121.0 / 36.0, 113.0 / 36.0, 95.0 / 36.0],
        1e-12,
        "Shapley 値",
    );
}

/// CoopGame 0.2.2 の `modiclus` のヘルプにある 4 人ゲーム (辞書式順)。
/// CoopGame は空提携と全体提携を組に入れないので (4.25, 5.25, 5.75, 5.75) を返すが、
/// 定義どおりの modiclus は (13/3, 16/3, 17/3, 17/3) で、双対被覆ゲームのプレ仁の射影と一致する。
#[test]
fn modiclus_of_coopgame_help_example() {
    let game = ExplicitGame::from_lex(&[
        0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
    ])
    .unwrap();
    let modiclus = assert_matches_dual_cover(&game, "CoopGame のヘルプの例");
    assert_close(
        &modiclus,
        &[13.0 / 3.0, 16.0 / 3.0, 17.0 / 3.0, 17.0 / 3.0],
        1e-9,
        "modiclus",
    );
    assert!(bi_excess_families_are_balanced(&game, &modiclus));
    assert!(!bi_excess_families_are_balanced(
        &game,
        &[4.25, 5.25, 5.75, 5.75]
    ));
}

/// ランダムな 3・4 人ゲーム (負の値、1 人提携の値が 0 のもの、優加法的でないものを含む) で、
/// 双対被覆ゲームのプレ仁の射影と一致する。
#[test]
fn modiclus_equals_projected_prenucleolus_of_dual_cover_game() {
    let mut rng = SplitMix64::new(1996);
    for case in 0..40 {
        let n = 3 + case % 2;
        let zero_singletons = case % 3 == 0;
        let game = ExplicitGame::from_fn(n, |s| {
            if zero_singletons && s.len() == 1 {
                0.0
            } else {
                rng.range(0, 20) as f64 - 5.0
            }
        })
        .unwrap();
        assert_matches_dual_cover(&game, &format!("case {case}: {:?}", game.values()));
    }
}

/// modiclus は、bi-excess の上位の組の族が全て平衡になる準配分である (Theorem 2.4 の形の確認)。
/// 少しずらした準配分では、どこかの族が平衡でなくなる。
#[test]
fn modiclus_satisfies_balancedness_of_bi_excess_families() {
    let mut rng = SplitMix64::new(320);
    for case in 0..20 {
        let n = 3 + case % 2;
        let game = ExplicitGame::from_fn(n, |_| rng.range(0, 20) as f64 - 5.0).unwrap();
        let x = variants::modiclus(&game).unwrap().into_allocation();
        assert!(
            bi_excess_families_are_balanced(&game, &x),
            "case {case}: {:?} {x:?}",
            game.values()
        );
        for (from, to) in [(0, 1), (1, 2), (2, 0)] {
            let mut moved = x.clone();
            moved[from] -= 0.01;
            moved[to] += 0.01;
            assert!(
                !bi_excess_families_are_balanced(&game, &moved),
                "case {case}: {moved:?}"
            );
        }
    }
}
