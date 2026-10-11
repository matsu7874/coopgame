//! オラクル型ゲームの仁を、独立した基準と突き合わせる。
//!
//! - 破産ゲーム: 仁はタルムード則に一致する (Aumann & Maschler 1985)。全提携を列挙できない n = 100 まで確かめる。
//! - 重み付き投票ゲーム: 小さい n では明示ベクトル版の仁と一致する。大きい n では対称性などで確かめる。

mod common;

use common::assert_close;
use std::time::Instant;

use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::generators::{self, SplitMix64};
use coopgame::nucleolus::oracle as oracle_nucleolus;
use coopgame::{Domain, nucleolus};

#[test]
fn bankruptcy_oracle_matches_talmud_rule_for_large_n() {
    let mut rng = SplitMix64::new(2026);
    // n = 100 は 1 問あたり 5-15 秒かかるので 1 問だけ解く。
    for (n, cases) in [(5, 2), (10, 2), (20, 2), (40, 2), (70, 2), (100, 1)] {
        for case in 0..cases {
            let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 100) as f64).collect();
            let total: f64 = claims.iter().sum();
            let estate = (rng.next_f64() * total).round();
            let game = BankruptcyGame::new(estate, claims.clone()).unwrap();
            let started = Instant::now();
            let result = oracle_nucleolus::nucleolus(&game).unwrap();
            let expected = common::talmud_rule(estate, &claims);
            assert_close(
                &result.allocation,
                &expected,
                1e-6 * total,
                &format!("n={n} case={case} E={estate} ({:.2?})", started.elapsed()),
            );
        }
    }
}

#[test]
fn bankruptcy_oracle_matches_explicit_solver() {
    let mut rng = SplitMix64::new(7);
    for n in 3..=10 {
        let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 50) as f64).collect();
        let total: f64 = claims.iter().sum();
        let estate = (0.6 * total).round();
        let oracle = BankruptcyGame::new(estate, claims.clone()).unwrap();
        let explicit = generators::bankruptcy(estate, &claims).unwrap();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let expected =
                nucleolus::nucleolus_with(&explicit, nucleolus::Options::new(&explicit, domain))
                    .unwrap()
                    .allocation;
            let actual = oracle_nucleolus::nucleolus_with(
                &oracle,
                domain,
                oracle_nucleolus::default_tolerance(&oracle),
            )
            .unwrap()
            .allocation;
            assert_close(
                &actual,
                &expected,
                1e-6 * total,
                &format!("n={n} {domain:?}"),
            );
        }
    }
}

#[test]
fn voting_oracle_matches_explicit_solver() {
    let mut rng = SplitMix64::new(11);
    for n in 3..=11 {
        for case in 0..3 {
            let weights: Vec<u64> = (0..n).map(|_| rng.range(1, 10)).collect();
            let quota = weights.iter().sum::<u64>() / 2 + 1;
            let oracle = WeightedVotingGame::new(weights.clone(), quota).unwrap();
            let explicit = generators::weighted_voting(
                &weights.iter().map(|w| *w as f64).collect::<Vec<_>>(),
                quota as f64,
            )
            .unwrap();
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let expected = nucleolus::nucleolus_with(
                    &explicit,
                    nucleolus::Options::new(&explicit, domain),
                )
                .unwrap()
                .allocation;
                let actual = oracle_nucleolus::nucleolus_with(&oracle, domain, 1e-7)
                    .unwrap()
                    .allocation;
                assert_close(
                    &actual,
                    &expected,
                    1e-6,
                    &format!("n={n} case={case} w={weights:?} {domain:?}"),
                );
            }
        }
    }
}

/// 全員の重みが等しい多数決ゲームの仁は、対称性から等分になる (仁は一意で、プレイヤーの入れ替えで不変)。
#[test]
fn symmetric_majority_game_splits_equally() {
    // 同じ超過の提携が多く、制約生成で追加する行が急増するため、n は小さめにする
    // (n = 19 で約 2,200 行、2 秒)。
    for n in [9, 15, 19] {
        let game = WeightedVotingGame::new(vec![1; n], (n / 2 + 1) as u64).unwrap();
        let result = oracle_nucleolus::nucleolus(&game).unwrap();
        assert_close(
            &result.allocation,
            &vec![1.0 / n as f64; n],
            1e-6,
            &format!("n={n}"),
        );
    }
}

/// 1 人の重みだけで基準に達する独裁者ゲームでは、独裁者以外は誰とも組む価値がない。
/// コアは独裁者が全てを得る 1 点なので、仁もその点になる (コアが空でなければ仁はコアに属する)。
#[test]
fn dictator_game_gives_everything_to_dictator() {
    let n = 40;
    let mut weights = vec![1; n];
    weights[7] = 100;
    let game = WeightedVotingGame::new(weights, 100).unwrap();
    let result = oracle_nucleolus::nucleolus(&game).unwrap();
    let mut expected = vec![0.0; n];
    expected[7] = 1.0;
    assert_close(&result.allocation, &expected, 1e-6, "独裁者ゲーム");
}
