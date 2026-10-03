//! 提携構造と事前の連合のもとでの解 (Aumann–Drèze 値、Owen 値、提携構造つきの仁) を、
//! 定義から導ける性質で確かめる。

mod common;

use common::assert_close;
use coopgame::partition::{self, CoalitionStructure, aumann_dreze, owen, quotient_game};
use coopgame::{Coalition, Domain, generators, nucleolus, values};

fn structures(n: usize) -> Vec<CoalitionStructure> {
    vec![
        CoalitionStructure::grand(n),
        CoalitionStructure::singletons(n),
        CoalitionStructure::new(n, &[vec![0, 1, 2], vec![3, 4], vec![5]]).unwrap(),
        CoalitionStructure::new(n, &[vec![0, 3], vec![1, 4, 5], vec![2]]).unwrap(),
    ]
}

/// Owen 値: 連合が全員 1 つ、または全員単独なら Shapley 値。各連合の和は商ゲームの Shapley 値 (商ゲームの性質)。
#[test]
fn owen_value_properties() {
    for seed in 0..10 {
        let game = generators::bnf(1 + (seed % 2) as u8, 6, seed).unwrap();
        let shapley = values::shapley(&game);
        let tolerance = 1e-9 * game.max_abs_value().max(1.0);
        assert_close(
            &owen(&game, &CoalitionStructure::grand(6)).unwrap(),
            &shapley,
            tolerance,
            "全員 1 つ",
        );
        assert_close(
            &owen(&game, &CoalitionStructure::singletons(6)).unwrap(),
            &shapley,
            tolerance,
            "全員単独",
        );
        for unions in &structures(6)[2..] {
            let value = owen(&game, unions).unwrap();
            let quotient = values::shapley(&quotient_game(&game, unions).unwrap());
            let sums: Vec<f64> = unions
                .blocks()
                .iter()
                .map(|b| b.players().map(|i| value[i]).sum())
                .collect();
            assert_close(
                &sums,
                &quotient,
                tolerance,
                &format!("seed {seed} 商ゲーム"),
            );
        }
    }
}

/// Aumann–Drèze 値: 全員 1 つなら Shapley 値、全員単独なら v({i})、各ブロックで和が v(B)。
#[test]
fn aumann_dreze_value_properties() {
    for seed in 0..10 {
        let game = generators::bnf(4, 6, seed).unwrap();
        let tolerance = 1e-9 * game.max_abs_value().max(1.0);
        assert_close(
            &aumann_dreze(&game, &CoalitionStructure::grand(6)).unwrap(),
            &values::shapley(&game),
            tolerance,
            "全員 1 つ",
        );
        assert_close(
            &aumann_dreze(&game, &CoalitionStructure::singletons(6)).unwrap(),
            &game.singleton_values(),
            tolerance,
            "全員単独",
        );
        for structure in &structures(6)[2..] {
            let value = aumann_dreze(&game, structure).unwrap();
            for &block in structure.blocks() {
                let sum: f64 = block.players().map(|i| value[i]).sum();
                assert!((sum - game.value(block)).abs() <= tolerance);
            }
        }
    }
}

/// 提携構造つきの仁: 全員 1 つなら仁、全員単独なら v({i})。各ブロックで x(B) = v(B)。
#[test]
fn structured_nucleolus_properties() {
    for seed in 0..10 {
        let game = generators::bnf(1, 6, seed).unwrap();
        let tolerance = 1e-6 * game.max_abs_value().max(1.0);
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let grand = partition::nucleolus(&game, &CoalitionStructure::grand(6), domain).unwrap();
            let expected =
                nucleolus::nucleolus_with(&game, nucleolus::Options::new(&game, domain)).unwrap();
            assert_close(
                &grand.allocation,
                &expected.allocation,
                tolerance,
                "全員 1 つ",
            );
            let singles =
                partition::nucleolus(&game, &CoalitionStructure::singletons(6), domain).unwrap();
            assert_close(
                &singles.allocation,
                &game.singleton_values(),
                tolerance,
                "全員単独",
            );
            for structure in &structures(6)[2..] {
                let Ok(result) = partition::nucleolus(&game, structure, domain) else {
                    continue;
                };
                for &block in structure.blocks() {
                    let sum: f64 = block.players().map(|i| result.allocation[i]).sum();
                    assert!((sum - game.value(block)).abs() <= tolerance);
                }
                if domain == Domain::Imputation {
                    for i in 0..6 {
                        assert!(
                            result.allocation[i] >= game.value(Coalition::singleton(i)) - tolerance
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn rejects_invalid_structures() {
    assert!(CoalitionStructure::new(3, &[vec![0, 1]]).is_err());
    assert!(CoalitionStructure::new(3, &[vec![0, 1], vec![1, 2]]).is_err());
    assert!(CoalitionStructure::new(3, &[vec![0, 1, 2], vec![]]).is_err());
    assert!(CoalitionStructure::new(3, &[vec![0, 3], vec![1, 2]]).is_err());
}
