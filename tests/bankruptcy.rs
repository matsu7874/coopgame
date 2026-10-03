//! 公開 API のタルムード則 (`bankruptcy::talmud_rule`) を、独立した 3 つの方法と突き合わせる。
//!
//! 1. テスト用に二分法で書いた別実装 (`tests/common`)
//! 2. LP で求めた仁を有理数で検証・復元した厳密な仁 (分数として完全一致)
//! 3. オラクル版の LP の仁 (明示ベクトルでは扱えない n)

mod common;

use coopgame::bankruptcy::{constrained_equal_awards, constrained_equal_losses, talmud_rule};
use coopgame::exact::{self, Rational};
use coopgame::generators::{self, SplitMix64};
use coopgame::oracle::bankruptcy::BankruptcyGame;
use coopgame::oracle::nucleolus as oracle_nucleolus;
use coopgame::{Domain, nucleolus};

fn random_problem(rng: &mut SplitMix64, n: usize) -> (u64, Vec<u64>) {
    let claims: Vec<u64> = (0..n).map(|_| rng.range(1, 100)).collect();
    let estate = rng.range(0, claims.iter().sum());
    (estate, claims)
}

#[test]
fn matches_independent_bisection_implementation() {
    let mut rng = SplitMix64::new(85);
    for case in 0..1000 {
        let n = 1 + (rng.next_u64() % 12) as usize;
        let (estate, claims) = random_problem(&mut rng, n);
        let claims: Vec<f64> = claims.iter().map(|d| *d as f64).collect();
        let actual = talmud_rule(estate as f64, &claims).unwrap();
        let expected = common::talmud_rule(estate as f64, &claims);
        for (a, e) in actual.iter().zip(&expected) {
            assert!((a - e).abs() < 1e-9, "case {case}: E={estate} d={claims:?}");
        }
        assert!((actual.iter().sum::<f64>() - estate as f64).abs() < 1e-9);
    }
}

#[test]
fn equals_certified_nucleolus_exactly() {
    let mut rng = SplitMix64::new(1985);
    for case in 0..60 {
        let n = 2 + (rng.next_u64() % 7) as usize;
        let (estate, claims) = random_problem(&mut rng, n);
        let game = generators::bankruptcy(
            estate as f64,
            &claims.iter().map(|d| *d as f64).collect::<Vec<_>>(),
        )
        .unwrap();
        let x = nucleolus::nucleolus(&game).unwrap().allocation;
        let report = exact::certify(&game, &x, Domain::Imputation).unwrap();
        assert!(report.satisfied, "case {case}: {:?}", report.reason);
        let exact_claims: Vec<Rational> = claims
            .iter()
            .map(|d| Rational::from_integer((*d).into()))
            .collect();
        let talmud = talmud_rule(Rational::from_integer(estate.into()), &exact_claims).unwrap();
        assert_eq!(
            report.allocation, talmud,
            "case {case}: E={estate} d={claims:?}"
        );
    }
}

#[test]
fn matches_oracle_lp_for_large_n() {
    let mut rng = SplitMix64::new(60);
    let (estate, claims) = random_problem(&mut rng, 60);
    let claims: Vec<f64> = claims.iter().map(|d| *d as f64).collect();
    let game = BankruptcyGame::new(estate as f64, claims.clone()).unwrap();
    let closed_form = game.nucleolus().unwrap();
    let lp = oracle_nucleolus::nucleolus(&game).unwrap().allocation;
    for (a, b) in closed_form.iter().zip(&lp) {
        assert!((a - b).abs() < 1e-6 * claims.iter().sum::<f64>());
    }
}

#[test]
fn equal_awards_and_losses_are_feasible() {
    let mut rng = SplitMix64::new(3);
    for _ in 0..200 {
        let (estate, claims) = random_problem(&mut rng, 8);
        let claims: Vec<f64> = claims.iter().map(|d| *d as f64).collect();
        for allocation in [
            constrained_equal_awards(estate as f64, &claims).unwrap(),
            constrained_equal_losses(estate as f64, &claims).unwrap(),
        ] {
            assert!((allocation.iter().sum::<f64>() - estate as f64).abs() < 1e-9);
            assert!(
                allocation
                    .iter()
                    .zip(&claims)
                    .all(|(x, d)| *x >= -1e-12 && *x <= d + 1e-12)
            );
        }
    }
}
