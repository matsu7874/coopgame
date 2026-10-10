//! 有理数による厳密な検証を確かめる。
//!
//! - 浮動小数点で求めた仁から厳密な配分を復元し、厳密な Kohlberg 基準に合格する。
//! - 仁を少しずらした配分は不合格になる。
//! - 文献の例の答えが分数のまま得られる。

mod common;

use coopgame::game::exact::{self, Rational};
use coopgame::generators;
use coopgame::verify;
use coopgame::{Domain, ExplicitGame, nucleolus};
use num_rational::BigRational;

fn r(numerator: i64, denominator: i64) -> Rational {
    BigRational::new(numerator.into(), denominator.into())
}

fn solve(game: &ExplicitGame, domain: Domain) -> Vec<f64> {
    match domain {
        Domain::Imputation => nucleolus::nucleolus(game),
        Domain::Preimputation => nucleolus::prenucleolus(game),
    }
    .unwrap()
    .allocation
}

#[test]
fn nucleoli_of_random_games_are_certified() {
    for kind in 1..=5 {
        for n in 3..=8 {
            for seed in 0..2 {
                let Ok(game) = generators::bnf(kind, n, seed) else {
                    continue;
                };
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    let Ok(result) = (match domain {
                        Domain::Imputation => nucleolus::nucleolus(&game),
                        Domain::Preimputation => nucleolus::prenucleolus(&game),
                    }) else {
                        continue; // 配分集合が空
                    };
                    let x = result.allocation;
                    let report = verify::certify(&game, &x, domain).unwrap();
                    let label = format!("type{kind} n={n} seed={seed} {domain:?}");
                    assert!(report.satisfied, "{label}: {:?}", report.reason);
                    let difference = verify::max_difference(&report.allocation, &x).unwrap();
                    assert!(
                        difference < 1e-6 * game.max_abs_value().max(1.0),
                        "{label}: {difference}"
                    );
                }
            }
        }
    }
}

#[test]
fn perturbed_nucleoli_are_rejected() {
    for kind in [1, 2, 4] {
        for n in 3..=7 {
            let game = generators::bnf(kind, n, 1).unwrap();
            let exact_game = exact::ExactGame::from_explicit(&game).unwrap();
            let x = solve(&game, Domain::Preimputation);
            let report = verify::certify(&game, &x, Domain::Preimputation).unwrap();
            assert!(report.satisfied);
            // 厳密な仁を 1/1000 だけ動かすと不合格。
            let mut moved = report.allocation.clone();
            moved[0] += r(1, 1000);
            moved[1] -= r(1, 1000);
            assert!(
                !verify::kohlberg_exact(&exact_game, &moved, Domain::Preimputation).satisfied,
                "type{kind} n={n}"
            );
        }
    }
}

#[test]
fn literature_answers_are_exact_fractions() {
    let cases: [(&[f64], Domain, Vec<Rational>); 6] = [
        (
            &[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 2.0],
            Domain::Preimputation,
            vec![r(3, 1), r(3, 1), r(-4, 1)],
        ),
        (
            &[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 2.0],
            Domain::Imputation,
            vec![r(1, 1), r(1, 1), r(0, 1)],
        ),
        (
            &[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0],
            Domain::Imputation,
            vec![r(8, 3), r(2, 3), r(5, 3)],
        ),
        (
            &[2.0, 6.0, 5.0, 15.0, 1.0, 18.0, 14.0],
            Domain::Imputation,
            vec![r(2, 1), r(7, 1), r(5, 1)],
        ),
        (
            &[2.0, 6.0, 5.0, 15.0, 1.0, 18.0, 14.0],
            Domain::Preimputation,
            vec![r(-1, 1), r(13, 1), r(2, 1)],
        ),
        (
            &[
                0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
            ],
            Domain::Imputation,
            vec![r(7, 2), r(9, 2), r(11, 2), r(15, 2)],
        ),
    ];
    for (values, domain, expected) in cases {
        let game = ExplicitGame::from_lex(values).unwrap();
        let report = verify::certify(&game, &solve(&game, domain), domain).unwrap();
        assert!(
            report.satisfied,
            "{values:?} {domain:?}: {:?}",
            report.reason
        );
        assert_eq!(report.allocation, expected, "{values:?} {domain:?}");
    }
    // タルムード: 遺産 100 で (100/3, 100/3, 100/3)
    let game = generators::bankruptcy(100.0, &[100.0, 200.0, 300.0]).unwrap();
    let report =
        verify::certify(&game, &solve(&game, Domain::Imputation), Domain::Imputation).unwrap();
    assert_eq!(report.allocation, vec![r(100, 3); 3]);
}

// ---------------------------------------------------------------- 有理数だけのソルバー

/// 有理数だけで求めた仁・プレ仁は、浮動小数点の LP の解を厳密に検証した配分と分数として一致する。
#[test]
fn exact_solver_matches_certified_float_solution() {
    for kind in [1, 2, 4] {
        for n in 3..=6 {
            for seed in 0..2 {
                let game = coopgame::generators::bnf(kind, n, seed).unwrap();
                let exact_game = coopgame::game::exact::ExactGame::from_explicit(&game).unwrap();
                for domain in [
                    coopgame::Domain::Imputation,
                    coopgame::Domain::Preimputation,
                ] {
                    let options = coopgame::nucleolus::Options::new(&game, domain);
                    let Ok(float) = coopgame::nucleolus::nucleolus_with(&game, options) else {
                        continue;
                    };
                    let certified =
                        coopgame::verify::certify(&game, &float.allocation, domain).unwrap();
                    assert!(certified.satisfied);
                    let exact = nucleolus::exact::nucleolus(&exact_game, domain).unwrap();
                    assert_eq!(
                        exact.allocation, certified.allocation,
                        "type{kind} n={n} seed={seed} {domain:?}"
                    );
                }
            }
        }
    }
}

/// 浮動小数点の和で作ると厳密な検証が成り立たないゲームも、有理数で構築すれば直接解ける。
#[test]
fn exact_solver_on_rationally_built_games() {
    for seed in 0..5 {
        let game = coopgame::generators::random_superadditive_exact(5, seed).unwrap();
        let result = nucleolus::exact::nucleolus(&game, coopgame::Domain::Imputation).unwrap();
        let report = coopgame::verify::kohlberg_exact(
            &game,
            &result.allocation,
            coopgame::Domain::Imputation,
        );
        assert!(report.satisfied, "seed {seed}: {:?}", report.reason);
    }
}
