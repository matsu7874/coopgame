//! コアが空でないかの判定 (`nucleolus::has_nonempty_core`) を、最小コアの `epsilon` の絶対値が
//! 許容誤差 (`1e-7 max|v|`) 以下のゲームで確かめる。
//!
//! どのゲームも、`epsilon` の値を手計算で求められるものを使う。

use coopgame::ExplicitGame;
use coopgame::nucleolus::{self, MAX_EXACT_CORE_PLAYERS};

/// 対称な `n` 人ゲーム: `n - 1` 人の提携の値が `(n - 1) * 1e8 + offset`、全体提携が `n * 1e8`、他は 0。
///
/// 対称なので最小コアは等分 `1e8` を含み、`n - 1` 人の提携の超過は `offset`、
/// それより小さい提携の超過は `-1e8` 以下になる。`epsilon = max(offset, -1e8)`。
/// 許容誤差は `1e-7 * n * 1e8 = 10 n` なので、`|offset| <= 10 n` なら `epsilon` は許容誤差の帯に入る。
fn symmetric_game(n: usize, offset: f64) -> ExplicitGame {
    let size = 1usize << n;
    let values: Vec<f64> = (1..size)
        .map(|mask| match mask.count_ones() as usize {
            k if k == n => n as f64 * 1e8,
            k if k == n - 1 => (n - 1) as f64 * 1e8 + offset,
            _ => 0.0,
        })
        .collect();
    ExplicitGame::from_binary(&values).unwrap()
}

#[test]
fn epsilon_slightly_above_zero_means_empty_core() {
    // 3 人: 2 人の提携が 2e8 + 1、全体が 3e8 + 1。epsilon = (2e8 + 1) - 2 (3e8 + 1) / 3 = 1/3 > 0。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 2e8 + 1.0, 2e8 + 1.0, 2e8 + 1.0, 3e8 + 1.0])
        .unwrap();
    assert!(!nucleolus::has_nonempty_core(&game).unwrap());
    for n in [3, 5, MAX_EXACT_CORE_PLAYERS] {
        assert!(
            !nucleolus::has_nonempty_core(&symmetric_game(n, 1.0)).unwrap(),
            "n = {n}"
        );
    }
}

#[test]
fn epsilon_slightly_below_zero_means_nonempty_core() {
    // 3 人: 2 人の提携が 2e8、全体が 3e8 + 1。epsilon = 2e8 - 2 (3e8 + 1) / 3 = -2/3 < 0。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 2e8, 2e8, 2e8, 3e8 + 1.0]).unwrap();
    assert!(nucleolus::has_nonempty_core(&game).unwrap());
    for n in [3, 5, MAX_EXACT_CORE_PLAYERS] {
        assert!(
            nucleolus::has_nonempty_core(&symmetric_game(n, -1.0)).unwrap(),
            "n = {n}"
        );
    }
}

/// 左手袋の持ち主 `left` 人と右手袋の持ち主 `n - left` 人の手袋ゲーム `v(S) = min(|S ∩ L|, |S ∩ R|)`。
fn glove_game(n: usize, left: usize) -> ExplicitGame {
    let values: Vec<f64> = (1..1usize << n)
        .map(|mask| {
            let l = (mask & ((1 << left) - 1)).count_ones();
            let r = (mask >> left).count_ones();
            l.min(r) as f64
        })
        .collect();
    ExplicitGame::from_binary(&values).unwrap()
}

#[test]
fn games_whose_epsilon_is_exactly_zero_have_nonempty_core() {
    // コアが空でなく、内部 (全ての真部分提携の超過が負の点) を持たないゲームは epsilon = 0。
    // 手袋ゲーム: コアは少ない側が全てを受け取る点 (同数なら線分)。
    for (n, left) in [(4, 2), (5, 2), (6, 3), (6, 4)] {
        assert!(
            nucleolus::has_nonempty_core(&glove_game(n, left)).unwrap(),
            "手袋 {left} / {n}"
        );
    }
    // 対称ゲームで n - 1 人の提携がちょうど等分の和を稼ぐ (コアは等分の 1 点)。
    for n in [3, 6, MAX_EXACT_CORE_PLAYERS] {
        assert!(nucleolus::has_nonempty_core(&symmetric_game(n, 0.0)).unwrap());
    }
    // 加法的なゲーム (コアは v({i}) の 1 点)。値は 2 進数で正確に表せる。
    let additive = ExplicitGame::from_binary(&[1.5, 2.25, 3.75]).unwrap();
    assert!(nucleolus::has_nonempty_core(&additive).unwrap());
    // 拒否権を持つプレイヤー 0 がいる単純ゲーム (コアはプレイヤー 0 が全てを受け取る点)。
    // 重み (5, 1, 1, 1, 1, 1)、quota 7。
    let n = 6;
    let weights = [5, 1, 1, 1, 1, 1];
    let values: Vec<f64> = (1..1usize << n)
        .map(|mask| {
            let weight: i32 = (0..n)
                .filter(|i| mask >> i & 1 == 1)
                .map(|i| weights[i])
                .sum();
            f64::from(u8::from(weight >= 7))
        })
        .collect();
    let veto = ExplicitGame::from_binary(&values).unwrap();
    assert!(nucleolus::has_nonempty_core(&veto).unwrap());
}

#[test]
fn epsilon_within_input_rounding_counts_as_zero() {
    // 2 人ゲーム v({0}) = 0.1, v({1}) = 0.2, v(N) = 0.3。10 進数では加法的でコアは 1 点だが、
    // f64 の値どおりの有理数では 0.1 + 0.2 = 0.3000000000000000166... > 0.3 = 0.2999999999999999888... となり、
    // epsilon は 1.4e-17 だけ正になる。入力の丸めの大きさ (4 n ε max|v| = 5.3e-16) 以下なので空でないとする。
    let decimal = ExplicitGame::from_binary(&[0.1, 0.2, 0.3]).unwrap();
    assert!(nucleolus::has_nonempty_core(&decimal).unwrap());
    let rounded_sum = ExplicitGame::from_binary(&[0.1, 0.2, 0.1 + 0.2]).unwrap();
    assert!(nucleolus::has_nonempty_core(&rounded_sum).unwrap());
    // 丸めの大きさを超える正の epsilon は、許容誤差 (1e-7 max|v|) より小さくても空と判定する。
    // v(N) = 0.3 - 1e-12 なら epsilon = 5e-13 で、丸めの大きさ 5.3e-16 を超える。
    let short = ExplicitGame::from_binary(&[0.1, 0.2, 0.3 - 1e-12]).unwrap();
    assert!(!nucleolus::has_nonempty_core(&short).unwrap());
}

#[test]
fn above_exact_limit_epsilon_within_tolerance_counts_as_nonempty() {
    // 厳密に判定する人数を超えると、epsilon (= 1) が許容誤差 (10 n) 以下なら空でないとする。
    let n = MAX_EXACT_CORE_PLAYERS + 1;
    assert!(nucleolus::has_nonempty_core(&symmetric_game(n, 1.0)).unwrap());
    // 許容誤差を超える epsilon は、人数によらず空と判定する。
    assert!(!nucleolus::has_nonempty_core(&symmetric_game(n, 100.0 * n as f64)).unwrap());
}
