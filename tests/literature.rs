//! 文献で答えが示されている例を再現する。
//!
//! 一次資料を直接確認できなかった例は、引用元を明記し、答えを手計算でも導いた
//! (導出は各テストのコメントと docs/literature-cases.md)。

use coopgame::generators;
use coopgame::kernel_set::{SetOptions, kernel_set};
use coopgame::{Domain, ExplicitGame, kohlberg, nucleolus};

mod common;
use common::talmud_rule;

/// 文献の値との一致 (許容誤差 1e-7)。
fn assert_close(actual: &[f64], expected: &[f64], label: &str) {
    common::assert_close(actual, expected, 1e-7, label);
}

/// (プレ)仁を計算し、期待値との一致と Kohlberg 基準を確かめる。
fn check_nucleolus(game: &ExplicitGame, domain: Domain, expected: &[f64], label: &str) {
    let result = match domain {
        Domain::Imputation => nucleolus::nucleolus(game),
        Domain::Preimputation => nucleolus::prenucleolus(game),
    }
    .unwrap();
    assert_close(&result.allocation, expected, label);
    assert!(
        kohlberg::verify(game, expected, domain).unwrap().satisfied,
        "{label}"
    );
}

/// Peleg & Sudhölter (2007), Example 5.5.12 (p. 96)。CoopGame の `prenucleolus` のヘルプが引用。
/// v(12) = 10、v(N) = 2、他は 0 (辞書式順で 0,0,0,10,0,0,2)。
///
/// 手計算: 対称性から x1 = x2 = a、x3 = 2 - 2a。e(12) = 10 - 2a と e(3) = 2a - 2 が
/// 最大の超過を争い、釣り合う a = 3 でプレ仁 (3, 3, -4)。このとき e(13) = e(23) = 1 < 4。
/// 仁は x3 >= 0 から a <= 1 で、e(12) = 8 + x3 を最小にする x3 = 0、
/// 残りは e(1) = e(13) = -x1, e(2) = e(23) = -x2 を釣り合わせて (1, 1, 0)。
#[test]
fn peleg_sudholter_example_5_5_12() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 2.0]).unwrap();
    check_nucleolus(&game, Domain::Preimputation, &[3.0, 3.0, -4.0], "プレ仁");
    check_nucleolus(&game, Domain::Imputation, &[1.0, 1.0, 0.0], "仁");
}

/// Ferguson, T. S., Game Theory (UCLA の講義ノート) の協力ゲームの章。CoopGame のテスト 49.1 が引用
/// (https://www.math.ucla.edu/~tom/Game_Theory/coal.pdf)。
/// v(1) = -1, v(2) = 0, v(3) = 1, v(12) = 3, v(13) = 4, v(23) = 2, v(N) = 5。
///
/// 手計算: e(12) + e(13) + e(23) = (x3 - 2) + (x2 - 1) + (x1 - 3) = -1 は配分によらない。
/// 3 つの最大値は 3 つを -1/3 にそろえたときに最小で、その点は一意に (8/3, 2/3, 5/3)。
/// 1 人提携の超過 (-11/3, -2/3, -2/3) はそれより小さく、配分集合にも入る。
#[test]
fn ferguson_three_player_example() {
    let game = ExplicitGame::from_lex(&[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0]).unwrap();
    let expected = [8.0 / 3.0, 2.0 / 3.0, 5.0 / 3.0];
    check_nucleolus(&game, Domain::Imputation, &expected, "仁");
    check_nucleolus(&game, Domain::Preimputation, &expected, "プレ仁");
}

/// CoopGame のテスト 67.1 (出典の記載なし)。v = (2, 6, 5, 15, 1, 18, 14) (辞書式順)。
///
/// 手計算: 配分集合は x1 >= 2, x2 >= 6, x3 >= 5, 和 14。
/// e(12) = x3 + 1, e(23) = x1 + 4, e(13) = x2 - 13 で、最大は max(x1 + 4, x3 + 1) >= 6。
/// x1 = 2, x3 = 5 で両方が 6 になり、x2 = 7。仁は (2, 7, 5)。
/// プレ仁: e(1) + e(23) = (2 - x1) + (x1 + 4) = 6 と e(3) + e(12) = 6 は配分によらないので
/// 最大超過は 3 以上。e(1) = e(23) = 3 から x1 = -1、e(3) = e(12) = 3 から x3 = 2、x2 = 13。
/// このとき e(2) = -7, e(13) = 0 で 3 より小さい。プレ仁は (-1, 13, 2)。
#[test]
fn coopgame_test_67_1() {
    let game = ExplicitGame::from_lex(&[2.0, 6.0, 5.0, 15.0, 1.0, 18.0, 14.0]).unwrap();
    check_nucleolus(&game, Domain::Imputation, &[2.0, 7.0, 5.0], "仁");
    check_nucleolus(&game, Domain::Preimputation, &[-1.0, 13.0, 2.0], "プレ仁");
}

/// CoopGame の `nucleolus` と `prenucleolus` のヘルプに記載された 4 人ゲームの値 (出典の記載なし)。
/// 手計算はしていないので、Kohlberg 基準で期待値そのものを検証する。
#[test]
fn coopgame_documented_four_player_example() {
    let game = ExplicitGame::from_lex(&[
        0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
    ])
    .unwrap();
    let expected = [3.5, 4.5, 5.5, 7.5];
    check_nucleolus(&game, Domain::Imputation, &expected, "仁");
    check_nucleolus(&game, Domain::Preimputation, &expected, "プレ仁");
}

/// タルムードに書かれた 3 つの事例 (請求 100, 200, 300)。
#[test]
fn talmud_cases_from_aumann_maschler() {
    let claims = [100.0, 200.0, 300.0];
    for (estate, expected) in [
        (100.0, [100.0 / 3.0, 100.0 / 3.0, 100.0 / 3.0]),
        (200.0, [50.0, 75.0, 75.0]),
        (300.0, [50.0, 100.0, 150.0]),
    ] {
        assert_close(
            &talmud_rule(estate, &claims),
            &expected,
            "タルムード則の実装",
        );
        let game = generators::bankruptcy(estate, &claims).unwrap();
        check_nucleolus(
            &game,
            Domain::Imputation,
            &expected,
            &format!("E = {estate}"),
        );
    }
}

/// ランダムな破産問題 300 個で、仁がタルムード則と一致することを確かめる。
#[test]
fn bankruptcy_nucleolus_matches_talmud_rule() {
    let mut rng = SplitMix64::new(1985);
    for case in 0..300 {
        let n = 2 + (rng.next_u64() % 6) as usize;
        let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 100) as f64).collect();
        let total: f64 = claims.iter().sum();
        let estate = (rng.next_f64() * total).round();
        let game = generators::bankruptcy(estate, &claims).unwrap();
        let expected = talmud_rule(estate, &claims);
        let actual = nucleolus::nucleolus(&game).unwrap().allocation;
        for (a, e) in actual.iter().zip(&expected) {
            assert!(
                (a - e).abs() < 1e-6 * total.max(1.0),
                "case {case}: E = {estate}, d = {claims:?}: {actual:?} != {expected:?}"
            );
        }
    }
}

/// 上の 3 人ゲームのカーネル全体を求め、答えの点がカーネルに含まれることを確かめる。
#[test]
fn literature_examples_lie_in_kernel_set() {
    let cases: [(&[f64], Domain, &[f64]); 4] = [
        (
            &[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 2.0],
            Domain::Preimputation,
            &[3.0, 3.0, -4.0],
        ),
        (
            &[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 2.0],
            Domain::Imputation,
            &[1.0, 1.0, 0.0],
        ),
        (
            &[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0],
            Domain::Imputation,
            &[8.0 / 3.0, 2.0 / 3.0, 5.0 / 3.0],
        ),
        (
            &[2.0, 6.0, 5.0, 15.0, 1.0, 18.0, 14.0],
            Domain::Imputation,
            &[2.0, 7.0, 5.0],
        ),
    ];
    for (values, domain, point) in cases {
        let game = ExplicitGame::from_lex(values).unwrap();
        let set = kernel_set(&game, domain, SetOptions::for_game(&game)).unwrap();
        assert!(
            set.contains(point, 1e-7),
            "{values:?} {domain:?}: {:?}",
            set.pieces
        );
    }
}

// ---------------------------------------------------------------- Shapley 値・Banzhaf 値

use coopgame::generators::SplitMix64;
use coopgame::oracle::voting::WeightedVotingGame;
use coopgame::values;

/// 手袋ゲーム: v(S) = min(S の左手袋の数, S の右手袋の数)。
fn glove_game(left: &[usize], n: usize) -> ExplicitGame {
    let values: Vec<f64> = (1u64..(1 << n))
        .map(|mask| {
            let lefts = left.iter().filter(|&&i| mask >> i & 1 == 1).count();
            let rights = (mask.count_ones() as usize) - lefts;
            lefts.min(rights) as f64
        })
        .collect();
    ExplicitGame::from_binary(&values).unwrap()
}

/// Aumann (2010) が論じた Ibn Ezra の相続問題 (遺産 120、請求 120, 60, 40, 30)。
/// CoopGame の `shapleyValue` のヘルプが引用し、Shapley 値を (80 5/6, 20 5/6, 10 5/6, 7 1/2) と記載。
/// v(S) は S の最大の請求 (120 で頭打ち) なので、Littlechild-Owen の公式で手計算できる:
/// 7.5 = 30/4、10 5/6 = 7.5 + 10/3、20 5/6 = 10 5/6 + 20/2、80 5/6 = 20 5/6 + 60/1。
#[test]
fn ibn_ezra_inheritance_shapley_value() {
    let game = ExplicitGame::from_lex(&[
        120.0, 60.0, 40.0, 30.0, 120.0, 120.0, 120.0, 60.0, 60.0, 40.0, 120.0, 120.0, 120.0, 60.0,
        120.0,
    ])
    .unwrap();
    let expected = [80.0 + 5.0 / 6.0, 20.0 + 5.0 / 6.0, 10.0 + 5.0 / 6.0, 7.5];
    assert_close(&values::shapley(&game), &expected, "Ibn Ezra");
    assert_close(
        &common::littlechild_owen(&[120.0, 60.0, 40.0, 30.0]),
        &expected,
        "Littlechild-Owen の公式",
    );
}

/// ランダムな空港ゲーム 100 個で、Shapley 値が Littlechild & Owen (1973) の公式と一致する。
#[test]
fn airport_game_shapley_matches_littlechild_owen() {
    let mut rng = SplitMix64::new(1973);
    for case in 0..100 {
        let n = 2 + (rng.next_u64() % 9) as usize;
        let costs: Vec<f64> = (0..n).map(|_| rng.range(1, 50) as f64).collect();
        let values: Vec<f64> = (1u64..(1 << n))
            .map(|mask| {
                (0..n)
                    .filter(|i| mask >> i & 1 == 1)
                    .map(|i| costs[i])
                    .fold(0.0, f64::max)
            })
            .collect();
        let game = ExplicitGame::from_binary(&values).unwrap();
        assert_close(
            &values::shapley(&game),
            &common::littlechild_owen(&costs),
            &format!("case {case}: {costs:?}"),
        );
    }
}

/// Gambarelli (2011) の Banzhaf 値の例 (CoopGame の `banzhafValue` のヘルプが引用)。
/// v = (0, 0, 0, 1, 2, 1, 3) (辞書式順)。
/// 手計算: プレイヤー 1 の限界貢献は {}: 0, {2}: 1, {3}: 2, {2,3}: 2 で和 5、5/4 = 1.25。
/// 同様にプレイヤー 2 は 0 + 1 + 1 + 1 = 3 で 0.75、プレイヤー 3 は 0 + 2 + 1 + 2 = 5 で 1.25。
#[test]
fn gambarelli_banzhaf_value() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 2.0, 1.0, 3.0]).unwrap();
    assert_close(&values::banzhaf(&game), &[1.25, 0.75, 1.25], "Gambarelli");
}

/// 手袋ゲームの Shapley 値 (CoopGame のテスト 15.2, 15.3 は出典の記載なし、15.4-15.6 は Wiese の教科書 p. 65 を引用)。
/// 手計算: 右手袋が 1 枚だけのとき、右の人は自分より前に左の人が 1 人でもいれば貢献 1 を得る。
/// 左が k 人なら、その確率は k / (k + 1)。残りを左の k 人で等分する。
#[test]
fn glove_game_shapley_values() {
    // 左 {1}、右 {2, 3}
    assert_close(
        &values::shapley(&glove_game(&[0], 3)),
        &[2.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0],
        "L={1}",
    );
    // 左 {1}、右 {2, ..., 10}: 9/10 と 1/90
    let mut expected = vec![1.0 / 90.0; 10];
    expected[0] = 0.9;
    assert_close(
        &values::shapley(&glove_game(&[0], 10)),
        &expected,
        "L={1}, 右 9 人",
    );
    // 左 {1, 2, 4}、右 {3}: 3/4 と 1/12
    assert_close(
        &values::shapley(&glove_game(&[0, 1, 3], 4)),
        &[1.0 / 12.0, 1.0 / 12.0, 0.75, 1.0 / 12.0],
        "Wiese",
    );
}

/// 重み (50, 49, 1) の重み付き投票ゲームの正規化 Banzhaf 指数 (CoopGame のテスト 07.1)。
/// 手計算 (q = 51): 勝利提携 {1,2}, {1,3}, {1,2,3} で、1 は 3 つとも決定的、2 と 3 は 1 つずつ。
#[test]
fn normalized_banzhaf_index_of_weighted_voting() {
    for (quota, expected) in [
        (1.0, [1.0 / 3.0; 3]),
        (51.0, [0.6, 0.2, 0.2]),
        (52.0, [0.5, 0.5, 0.0]),
        (100.0, [1.0 / 3.0; 3]),
    ] {
        let game = generators::weighted_voting(&[50.0, 49.0, 1.0], quota).unwrap();
        assert_close(
            &values::normalize(&values::banzhaf(&game)),
            &expected,
            &format!("q = {quota}"),
        );
    }
}

/// 101 人の多数決ゲーム (明示ベクトルでは扱えない) でサンプリング推定を確かめる。
/// Shapley 値は対称性と効率性から 1/101。Banzhaf 値は、他の 100 人のうちちょうど 50 人が
/// 賛成する確率 C(100, 50) / 2^100 (各人の決定的な確率)。
#[test]
fn sampling_on_large_majority_game() {
    let n = 101;
    let game = WeightedVotingGame::new(vec![1; n], 51).unwrap();
    let shapley = values::shapley_sampling(&game, 4_000, 3);
    let banzhaf = values::banzhaf_sampling(&game, 4_000, 4);
    let mut log_binomial = 0.0;
    for k in 0..50 {
        log_binomial += ((100 - k) as f64).ln() - ((k + 1) as f64).ln();
    }
    let exact_banzhaf = (log_binomial - 100.0 * 2f64.ln()).exp();
    for i in 0..n {
        assert!(
            (shapley.values[i] - 1.0 / n as f64).abs() < 5.0 * shapley.standard_errors[i],
            "Shapley {i}"
        );
        assert!(
            (banzhaf.values[i] - exact_banzhaf).abs() < 5.0 * banzhaf.standard_errors[i],
            "Banzhaf {i}"
        );
    }
    assert!((shapley.values.iter().sum::<f64>() - 1.0).abs() < 1e-9);
}
