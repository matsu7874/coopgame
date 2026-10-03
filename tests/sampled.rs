//! サンプリングした提携に制限した仁 (`sampled`) を、明示ベクトル版の厳密なプレ仁と突き合わせる。
//!
//! - サンプルが全提携を覆えば、近似は厳密なプレ仁に一致する。
//! - サンプルを増やすと誤差が小さくなる (seed を固定した平均で比べる)。
//! - 近似はサンプルした提携の間では最小コアの制約を満たす。

use coopgame::generators;
use coopgame::oracle::{PlayerSet, SetFunction};
use coopgame::sampled::{SampledGame, sampled_least_core, sampled_nucleolus};
use coopgame::{Domain, nucleolus};

fn relative_error(actual: &[f64], expected: &[f64], scale: f64) -> f64 {
    actual
        .iter()
        .zip(expected)
        .map(|(a, e)| (a - e).abs())
        .fold(0.0, f64::max)
        / scale
}

#[test]
fn dense_sampling_recovers_exact_prenucleolus() {
    // n = 7 の真部分提携は 126 個。2000 組あればほぼ確実に全てを引く。
    for kind in [1, 2, 4] {
        let game = generators::bnf(kind, 7, 11).unwrap();
        let exact = nucleolus::prenucleolus(&game).unwrap().allocation;
        let approx = sampled_nucleolus(&game, 2000, 5, Domain::Preimputation).unwrap();
        assert_eq!(approx.evaluations, (1 << 7) - 1, "type{kind}");
        let error = relative_error(&approx.allocation, &exact, game.max_abs_value());
        assert!(error < 1e-6, "type{kind}: {error}");
    }
}

#[test]
fn error_shrinks_with_more_samples() {
    for kind in [1, 2, 4] {
        let mut mean = [0.0; 2];
        for seed in 0..5 {
            let game = generators::bnf(kind, 10, seed).unwrap();
            let exact = nucleolus::prenucleolus(&game).unwrap().allocation;
            for (slot, pairs) in [25, 400].into_iter().enumerate() {
                let approx =
                    sampled_nucleolus(&game, pairs, 100 + seed, Domain::Preimputation).unwrap();
                mean[slot] +=
                    relative_error(&approx.allocation, &exact, game.max_abs_value()) / 5.0;
            }
        }
        assert!(mean[1] < mean[0], "type{kind}: {mean:?}");
    }
}

#[test]
fn least_core_constraints_hold_on_sampled_coalitions() {
    let game = generators::bnf(2, 12, 3).unwrap();
    let (least, evaluations) = sampled_least_core(&game, 300, 9, Domain::Preimputation).unwrap();
    let sampled = SampledGame::sample(&game, 300, 9).unwrap();
    assert_eq!(evaluations, sampled.evaluations());
    let tolerance = 1e-6 * game.max_abs_value();
    let total: f64 = least.allocation.iter().sum();
    assert!((total - SetFunction::value(&game, &PlayerSet::full(12))).abs() < tolerance);
    for set in sampled.coalitions() {
        let excess = SetFunction::value(&game, set) - set.sum(&least.allocation);
        assert!(excess <= least.epsilon + tolerance);
    }
}
