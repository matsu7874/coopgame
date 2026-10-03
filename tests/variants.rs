//! per capita 仁・比例仁・modiclus と交渉集合を、文献の値・手計算・定理と突き合わせる。
//!
//! CoopGame 0.2.2 との数値の照合は `scripts/compare/check_variants.py` で行う (R が必要なのでテストには含めない)。

mod common;

use common::assert_close;
use coopgame::generators::{self, SplitMix64};
use coopgame::kernel::{self, TransferOptions};
use coopgame::{Domain, ExplicitGame, bargaining, nucleolus, properties, values, variants};

/// CoopGame の `modiclus` のヘルプにある 4 人ゲームの値 (4.25, 5.25, 5.75, 5.75)。
#[test]
fn modiclus_documented_example() {
    let game = ExplicitGame::from_lex(&[
        0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
    ])
    .unwrap();
    let result = variants::modiclus(&game).unwrap();
    assert_close(
        &result.allocation,
        &[4.25, 5.25, 5.75, 5.75],
        1e-9,
        "modiclus",
    );
}

/// Young (1985) の費用分担の例 (CoopGame の `perCapitaNucleolus` のヘルプが引用)。
/// 費用 C = (15, 20, 55, 35, 61, 65, 78) の節約ゲームは v(13) = 9, v(23) = 10, v(N) = 12、他は 0。
///
/// 手計算: per capita 超過は e(1) = -x1, e(23)/2 = (x1 - 2)/2 などで、
/// 最大値は -x1 と (x1 - 2)/2 が釣り合う x1 = 2/3 で -2/3 になる。
/// 次の段では -(x1 + x2)/2 と (x2 - 3)/2 が釣り合う x2 = 7/6 で -11/12。残りが x3 = 61/6。
#[test]
fn per_capita_nucleolus_young_1985() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.0, 9.0, 10.0, 12.0]).unwrap();
    let result = variants::per_capita_nucleolus(&game, Domain::Imputation).unwrap();
    assert_close(
        &result.allocation,
        &[2.0 / 3.0, 7.0 / 6.0, 61.0 / 6.0],
        1e-9,
        "per capita",
    );
    assert_close(&result.levels, &[-2.0 / 3.0, -11.0 / 12.0], 1e-9, "段");
}

/// 定和ゲーム (v(S) + v(N \ S) = v(N)) では、modiclus はプレ仁に一致する (Sudhölter 1997 の要旨)。
/// ランダムな 30 ゲームで確かめる。
#[test]
fn modiclus_equals_prenucleolus_for_constant_sum_games() {
    let mut rng = SplitMix64::new(1997);
    for case in 0..30 {
        let n = 3 + (rng.next_u64() % 3) as usize;
        let full = (1u64 << n) - 1;
        let mut values = vec![0.0; full as usize];
        values[full as usize - 1] = 10.0;
        for mask in 1..full {
            let complement = full ^ mask;
            if mask < complement {
                let value = rng.range(0, 1000) as f64 / 100.0;
                values[mask as usize - 1] = value;
                values[complement as usize - 1] = 10.0 - value;
            }
        }
        let game = ExplicitGame::from_binary(&values).unwrap();
        let modiclus = variants::modiclus(&game).unwrap().allocation;
        let prenucleolus = nucleolus::prenucleolus(&game).unwrap().allocation;
        assert_close(&modiclus, &prenucleolus, 1e-7, &format!("case {case}"));
    }
}

/// 凸ゲームでは modiclus はコアに属する (Sudhölter 1997 の要旨)。
#[test]
fn modiclus_lies_in_core_of_convex_games() {
    for seed in 0..20 {
        let n = 3 + (seed % 4) as usize;
        let game = generators::random_convex(n, seed).unwrap();
        let x = variants::modiclus(&game).unwrap().allocation;
        let tolerance = 1e-6 * game.max_abs_value().max(1.0);
        assert!(
            properties::is_in_core(&game, &x, tolerance),
            "seed {seed}: {x:?}"
        );
    }
}

/// コアが空でなければ、per capita 仁・比例仁は不満の最大値が 0 以下なのでコアに属する。
#[test]
fn per_capita_and_proportional_lie_in_nonempty_core() {
    let mut checked = 0;
    for seed in 0..20 {
        for game in [
            generators::random_convex(4, seed).unwrap(),
            generators::random_convex(5, seed).unwrap(),
            generators::bnf(1, 5, seed).unwrap(),
        ] {
            if !properties::has_nonempty_core(&game).unwrap() {
                continue;
            }
            let tolerance = 1e-6 * game.max_abs_value().max(1.0);
            let per_capita = variants::per_capita_nucleolus(&game, Domain::Imputation).unwrap();
            assert!(per_capita.levels[0] <= tolerance);
            assert!(properties::is_in_core(
                &game,
                &per_capita.allocation,
                tolerance
            ));
            if game.values().iter().all(|v| *v >= 0.0) {
                let proportional =
                    variants::proportional_nucleolus(&game, Domain::Imputation).unwrap();
                assert!(properties::is_in_core(
                    &game,
                    &proportional.allocation,
                    tolerance
                ));
            }
            checked += 1;
        }
    }
    assert!(checked >= 40, "{checked}");
}

// ---------------------------------------------------------------- 交渉集合

/// カーネルは交渉集合に、プレカーネルはプレ交渉集合に含まれる (Davis & Maschler 1965)。
/// 仁とプレ仁、transfer scheme で求めたカーネルの点で確かめる。
#[test]
fn kernel_points_lie_in_bargaining_set() {
    for kind in 1..=4 {
        for n in 3..=6 {
            for seed in 0..3 {
                let game = generators::bnf(kind, n, seed).unwrap();
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    let options = nucleolus::Options::new(&game, domain);
                    let Ok(result) = nucleolus::nucleolus_with(&game, options) else {
                        continue;
                    };
                    assert!(
                        bargaining::is_in_bargaining_set(&game, &result.allocation, domain)
                            .unwrap(),
                        "type{kind} n={n} seed={seed} {domain:?} 仁"
                    );
                    let point =
                        kernel::kernel_point(&game, domain, None, TransferOptions::for_game(&game))
                            .unwrap();
                    assert!(point.converged);
                    assert!(
                        bargaining::is_in_bargaining_set(&game, &point.allocation, domain).unwrap(),
                        "type{kind} n={n} seed={seed} {domain:?} カーネル"
                    );
                }
            }
        }
    }
}

/// 凸ゲームでは交渉集合はコアに一致する (Maschler, Peleg & Shapley 1971)。
/// Shapley 値 (凸ゲームではコアに属する) は交渉集合に属し、コアの外の配分は属さない。
#[test]
fn bargaining_set_equals_core_for_convex_games() {
    let mut rng = SplitMix64::new(1971);
    let mut outside = 0;
    for seed in 0..15 {
        let n = 3 + (seed % 3) as usize;
        let game = generators::random_convex(n, seed).unwrap();
        let tolerance = 1e-6 * game.max_abs_value().max(1.0);
        let shapley = values::shapley(&game);
        assert!(properties::is_in_core(&game, &shapley, tolerance));
        assert!(bargaining::is_in_bargaining_set(&game, &shapley, Domain::Imputation).unwrap());
        // ランダムな配分 (各人に v({i}) と、残りをランダムな割合で)
        let singles = game.singleton_values();
        let surplus = game.values()[game.values().len() - 1] - singles.iter().sum::<f64>();
        for _ in 0..20 {
            // 4 乗して偏らせ、コアの外の配分も多く作る
            let weights: Vec<f64> = (0..n).map(|_| rng.next_f64().powi(4)).collect();
            let total: f64 = weights.iter().sum();
            let x: Vec<f64> = singles
                .iter()
                .zip(&weights)
                .map(|(v, w)| v + surplus * w / total)
                .collect();
            let in_core = properties::is_in_core(&game, &x, tolerance);
            let in_bargaining =
                bargaining::is_in_bargaining_set(&game, &x, Domain::Imputation).unwrap();
            assert_eq!(in_core, in_bargaining, "seed {seed}: {x:?}");
            outside += usize::from(!in_core);
        }
    }
    assert!(outside >= 100, "コアの外の配分が少なすぎる: {outside}");
}
