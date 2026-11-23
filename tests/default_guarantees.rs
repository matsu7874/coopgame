//! 許容誤差を指定しない既定の経路 (オラクル版の仁) の保証を確かめる。
//!
//! 規則は既定の仁 (`nucleolus::nucleolus`、`Tolerance::Auto`) と同じ:
//!
//! - 確かめられるなら確かめ、確かめた結果だけを `exact` にする。確かめていない結果は `approximate`。
//! - 許容誤差を明示したら、その許容誤差で 1 回だけ解いて `exact` (許容誤差の範囲で成り立つ)。
//!
//! 値が大きい整数のゲームでは、比 1e-7 の許容誤差で 1 回だけ解くと、超過の差が許容誤差より小さい段をまとめて
//! 仁からずれる (`tests/tolerance.rs`)。既定の経路が、そのずれた配分を `exact` と名乗らないことを確かめる。

mod common;

use common::max_abs_difference;
use coopgame::auto::AutoNucleolus;
use coopgame::game::exact::ExactGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::nucleolus::{self, exact, oracle};
use coopgame::num_traits::ToPrimitive;
use coopgame::{Domain, ExplicitGame, Guarantee};

/// 値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲーム (`tests/tolerance.rs` と同じ形)。
fn integer_games(scale: f64, sizes: &[usize], count: usize, seed: u64) -> Vec<ExplicitGame> {
    let mut state = seed;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    (0..count)
        .map(|case| {
            let n = sizes[case % sizes.len()];
            let values: Vec<f64> = (1usize..1 << n)
                .map(|mask| {
                    let size = mask.count_ones() as f64;
                    if size == 1.0 {
                        0.0
                    } else {
                        scale * size + (next() % 7) as f64
                    }
                })
                .collect();
            ExplicitGame::from_binary(&values).unwrap()
        })
        .collect()
}

/// 監査で使った 96 個 (scale 1e6..1e9、3-6 人を 24 個ずつ)。
fn audit_games() -> Vec<ExplicitGame> {
    [1e6, 1e7, 1e8, 1e9]
        .iter()
        .flat_map(|&scale| integer_games(scale, &[3, 4, 5, 6], 24, 3))
        .collect()
}

fn rational_nucleolus(game: &ExplicitGame, domain: Domain) -> Vec<f64> {
    let exact = ExactGame::from_explicit(game).unwrap();
    exact::nucleolus(&exact, domain)
        .unwrap()
        .allocation
        .iter()
        .map(|q| q.to_f64().unwrap())
        .collect()
}

/// オラクル版の仁の既定は、20 人以下なら全提携の表を作って Kohlberg 基準で確かめ、満たさなければ
/// 既定の仁の手順で求め直す。1 回だけ解いてずれた配分を `exact` と名乗らない。
#[test]
fn oracle_default_is_checked_up_to_twenty_players() {
    let games = audit_games();
    let mut once_off = 0;
    for (case, game) in games.iter().enumerate() {
        let truth = rational_nucleolus(game, Domain::Imputation);
        let precision = 1e-13 * game.max_abs_value();
        let result = oracle::nucleolus(game).unwrap();
        assert_eq!(result.guarantee(), Guarantee::Exact, "case {case}");
        assert!(
            max_abs_difference(result.allocation(), &truth) <= precision,
            "case {case}: {result:?} / {truth:?}"
        );
        // 許容誤差を明示すると、1 回だけ解いて exact (許容誤差の範囲で成り立つ)。
        let fixed =
            oracle::nucleolus_with(game, Domain::Imputation, 1e-7 * game.max_abs_value()).unwrap();
        assert_eq!(fixed.guarantee(), Guarantee::Exact);
        if max_abs_difference(fixed.allocation(), &truth) > precision {
            once_off += 1;
        }
    }
    // 1 回だけ解くとずれるゲームの数 (docs/guarantees.md の「既定の経路ごとの保証」が引用する)。
    assert_eq!(once_off, 55);
}

/// オラクル版のプレ仁も同じ規則。
#[test]
fn oracle_default_prenucleolus_is_checked() {
    for (case, game) in integer_games(1e8, &[4, 5], 12, 11).iter().enumerate() {
        let truth = rational_nucleolus(game, Domain::Preimputation);
        let result = oracle::prenucleolus(game).unwrap();
        assert_eq!(result.guarantee(), Guarantee::Exact, "case {case}");
        assert!(
            max_abs_difference(result.allocation(), &truth) <= 1e-13 * game.max_abs_value(),
            "case {case}: {result:?} / {truth:?}"
        );
    }
}

/// 21 人以上のオラクル版の仁は確かめないので `approximate`。許容誤差を明示すれば `exact`。
/// 重み付き投票ゲームの自動選択も同じ。
#[test]
fn oracle_default_beyond_check_limit_is_approximate() {
    let n = nucleolus::MAX_AUTO_CHECK_PLAYERS + 1;
    let weights: Vec<u64> = (0..n as u64).map(|i| 1 + i % 4).collect();
    let quota = weights.iter().sum::<u64>() / 2 + 1;
    let voting = WeightedVotingGame::new(weights, quota).unwrap();
    let default = oracle::nucleolus(&voting).unwrap();
    assert_eq!(default.guarantee(), Guarantee::Approximate);
    let fixed = oracle::nucleolus_with(&voting, Domain::Imputation, 1e-7).unwrap();
    assert_eq!(fixed.guarantee(), Guarantee::Exact);
    assert_eq!(fixed.allocation(), default.allocation());
    assert_eq!(
        voting.nucleolus_auto().unwrap().guarantee(),
        Guarantee::Approximate
    );

    // 20 人の投票ゲームは確かめる。
    let weights: Vec<u64> = (0..20u64).map(|i| 1 + i % 4).collect();
    let quota = weights.iter().sum::<u64>() / 2 + 1;
    let voting = WeightedVotingGame::new(weights, quota).unwrap();
    assert_eq!(
        voting.nucleolus_auto().unwrap().guarantee(),
        Guarantee::Exact
    );
}
