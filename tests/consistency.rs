//! 独立した基準どうしで計算結果を突き合わせる。
//!
//! - 仁・プレ仁は Kohlberg 基準を満たす。
//! - 仁はカーネルに、プレ仁はプレカーネルに属する。
//! - 凸ゲームではカーネルは仁の 1 点なので、transfer scheme の結果が仁と一致する。

use coopgame::kernel::{self, TransferOptions};
use coopgame::nucleolus::{self, NucleolusResult};
use coopgame::{Domain, ExplicitGame, generators, kohlberg};

fn random_games() -> Vec<(String, ExplicitGame)> {
    let mut games = Vec::new();
    for kind in 1..=4 {
        // タイプ 3 は n = 3 だと v({i}) も 1 になりうるため、配分集合が空になる。
        let smallest = if kind == 3 { 4 } else { 3 };
        for n in smallest..=6 {
            for seed in 0..3 {
                games.push((
                    format!("type{kind} n={n} seed={seed}"),
                    generators::bnf(kind, n, seed).unwrap(),
                ));
            }
        }
    }
    games.push(("type5 n=7".into(), generators::bnf(5, 7, 0).unwrap()));
    games.push(("type5 n=8".into(), generators::bnf(5, 8, 0).unwrap()));
    games
}

fn solve(game: &ExplicitGame, domain: Domain) -> NucleolusResult {
    match domain {
        Domain::Imputation => nucleolus::nucleolus(game).unwrap(),
        Domain::Preimputation => nucleolus::prenucleolus(game).unwrap(),
    }
}

#[test]
fn nucleoli_satisfy_kohlberg_and_lie_in_kernel() {
    for (name, game) in random_games() {
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let result = solve(&game, domain);
            let x = &result.allocation;
            let report = kohlberg::verify(&game, x, domain).unwrap();
            assert!(
                report.satisfied,
                "{name} {domain:?}: {:?} {:?}",
                x, report.reason
            );
            let tolerance = 1e-6 * game.max_abs_value().max(1.0);
            assert!(
                kernel::is_in_kernel(&game, x, domain, tolerance),
                "{name} {domain:?}: violation {}",
                kernel::kernel_violation(&game, x, domain)
            );
            assert!(
                result.levels.windows(2).all(|w| w[1] <= w[0] + 1e-9),
                "{name}"
            );
        }
    }
}

#[test]
fn kohlberg_rejects_perturbed_nucleolus() {
    for (name, game) in random_games() {
        let mut x = nucleolus::nucleolus(&game).unwrap().allocation;
        let n = x.len();
        // 総和を保ったまま、下限に張り付いていない 2 人の間で動かす。
        let singles = game.singleton_values();
        let Some(j) = (0..n).find(|&j| x[j] > singles[j] + 1e-3) else {
            continue;
        };
        let i = (j + 1) % n;
        let step = 1e-3 * game.max_abs_value().max(1.0);
        x[i] += step;
        x[j] -= step;
        let report = kohlberg::verify(&game, &x, Domain::Imputation).unwrap();
        assert!(!report.satisfied, "{name}: {x:?}");
    }
}

#[test]
fn transfer_scheme_reaches_kernel() {
    for (name, game) in random_games() {
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let options = TransferOptions::for_game(&game);
            let result = kernel::kernel_point(&game, domain, None, options).unwrap();
            assert!(result.converged, "{name} {domain:?}: {result:?}");
            assert!(
                kernel::is_in_kernel(&game, &result.allocation, domain, 10.0 * options.tolerance),
                "{name} {domain:?}"
            );
        }
    }
}

#[test]
fn kernel_equals_nucleolus_for_convex_games() {
    for n in 3..=7 {
        for seed in 0..5 {
            let game = generators::random_convex(n, seed).unwrap();
            let nucleolus = nucleolus::nucleolus(&game).unwrap().allocation;
            let options = TransferOptions::for_game(&game);
            let kernel = kernel::kernel_point(&game, Domain::Imputation, None, options).unwrap();
            assert!(kernel.converged, "n={n} seed={seed}");
            for (a, b) in kernel.allocation.iter().zip(&nucleolus) {
                assert!(
                    (a - b).abs() < 1e-5 * game.max_abs_value().max(1.0),
                    "n={n} seed={seed}: {:?} vs {nucleolus:?}",
                    kernel.allocation
                );
            }
        }
    }
}

#[test]
fn cost_game_via_negation() {
    // 空港ゲーム (滑走路の費用 1, 2, 3): c(S) = max_{i in S} cost_i。
    // 費用ゲームの仁は -(-c の仁)。
    // 第 1 段で {1} と {2,3} の超過 x_1 - 1 と -x_1 を釣り合わせて x_1 = 1/2、
    // 第 2 段で {1,2} と {1,3} の超過 x_2 - 3/2 と -x_2 を釣り合わせて x_2 = 3/4。
    let costs = [1.0, 2.0, 3.0];
    let values: Vec<f64> = (1u64..8)
        .map(|mask| {
            (0..3)
                .filter(|i| mask >> i & 1 == 1)
                .map(|i| costs[i])
                .fold(0.0, f64::max)
        })
        .collect();
    let cost = ExplicitGame::from_binary(&values).unwrap();
    let x: Vec<f64> = nucleolus::nucleolus(&cost.negated())
        .unwrap()
        .allocation
        .iter()
        .map(|v| -v)
        .collect();
    for (a, e) in x.iter().zip([0.5, 0.75, 1.75]) {
        assert!((a - e).abs() < 1e-9, "{x:?}");
    }
}

#[test]
fn constraint_generation_matches_full_lp() {
    use coopgame::nucleolus::{Method, Options};
    for (name, game) in random_games() {
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let mut options = Options::new(&game, domain);
            options.method = Method::Full;
            let full = nucleolus::nucleolus_with(&game, options).unwrap();
            options.method = Method::ConstraintGeneration;
            let lazy = nucleolus::nucleolus_with(&game, options).unwrap();
            let scale = game.max_abs_value().max(1.0);
            for (a, b) in full.allocation.iter().zip(&lazy.allocation) {
                assert!((a - b).abs() < 1e-6 * scale, "{name} {domain:?}");
            }
        }
    }
}
