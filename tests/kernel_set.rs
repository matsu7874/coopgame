//! カーネル全体の計算を、点ごとの判定・仁・transfer scheme と突き合わせる。

use coopgame::generators::{self, SplitMix64};
use coopgame::kernel::{self, TransferOptions};
use coopgame::kernel::{KernelSet, SetOptions, kernel_set};
use coopgame::{Domain, ExplicitGame, nucleolus};

fn scale(game: &ExplicitGame) -> f64 {
    game.max_abs_value().max(1.0)
}

fn solve(game: &ExplicitGame, domain: Domain) -> KernelSet {
    kernel_set(game, domain, SetOptions::for_game(game)).unwrap()
}

/// 配分集合(またはその近く)からランダムに初期点を作る。
fn random_start(game: &ExplicitGame, domain: Domain, rng: &mut SplitMix64) -> Vec<f64> {
    let n = game.players();
    let singles = game.singleton_values();
    let surplus = game.value(game.grand()) - singles.iter().sum::<f64>();
    let weights: Vec<f64> = (0..n).map(|_| rng.next_f64() + 1e-3).collect();
    let total: f64 = weights.iter().sum();
    let mut x: Vec<f64> = singles
        .iter()
        .zip(&weights)
        .map(|(v, w)| v + surplus * w / total)
        .collect();
    if domain == Domain::Preimputation {
        // 和を保って配分集合の外にも出す。
        let shift = (rng.next_f64() - 0.5) * scale(game);
        x[0] += shift;
        x[n - 1] -= shift;
    }
    x
}

/// 各多面体の頂点と頂点の重心がカーネル条件を満たし、
/// 仁と transfer scheme の到達点が和集合に含まれることを確かめる。
fn check(name: &str, game: &ExplicitGame, domain: Domain) -> KernelSet {
    let set = solve(game, domain);
    let tolerance = 1e-6 * scale(game);
    assert!(!set.pieces.is_empty(), "{name} {domain:?}: 空");
    for piece in &set.pieces {
        let vertices = piece
            .vertices
            .as_ref()
            .expect("小さいゲームでは頂点を求められる");
        assert!(!vertices.is_empty(), "{name} {domain:?}");
        for vertex in vertices {
            assert!(
                kernel::is_in_kernel(game, vertex, domain, tolerance),
                "{name} {domain:?}: 頂点 {vertex:?} がカーネル条件を満たさない (violation {})",
                kernel::kernel_violation(game, vertex, domain)
            );
        }
        let centroid: Vec<f64> = (0..game.players())
            .map(|i| vertices.iter().map(|v| v[i]).sum::<f64>() / vertices.len() as f64)
            .collect();
        assert!(
            kernel::is_in_kernel(game, &centroid, domain, tolerance),
            "{name} {domain:?}: 重心 {centroid:?}"
        );
    }

    let nucleolus = match domain {
        Domain::Imputation => nucleolus::nucleolus(game),
        Domain::Preimputation => nucleolus::prenucleolus(game),
    }
    .unwrap()
    .allocation;
    assert!(
        set.contains(&nucleolus, tolerance),
        "{name} {domain:?}: 仁 {nucleolus:?} を含まない"
    );

    let mut rng = SplitMix64::new(42);
    for _ in 0..20 {
        let start = random_start(game, domain, &mut rng);
        let point =
            kernel::kernel_point(game, domain, Some(&start), TransferOptions::for_game(game))
                .unwrap();
        if point.converged {
            assert!(
                set.contains(&point.allocation, 1e-5 * scale(game)),
                "{name} {domain:?}: transfer scheme の点 {:?} を含まない",
                point.allocation
            );
        }
    }
    set
}

#[test]
fn convex_games_have_single_point_kernel() {
    for n in 3..=5 {
        for seed in 0..3 {
            let game = generators::random_convex(n, seed).unwrap();
            let name = format!("convex n={n} seed={seed}");
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let set = check(&name, &game, domain);
                assert_eq!(set.pieces.len(), 1, "{name} {domain:?}: {:?}", set.pieces);
                assert_eq!(set.pieces[0].dimension, 0, "{name} {domain:?}");
            }
        }
    }
}

#[test]
fn random_games_sound_and_complete() {
    for kind in [1, 2, 4] {
        for n in 3..=4 {
            for seed in 0..4 {
                let game = generators::bnf(kind, n, seed).unwrap();
                let name = format!("type{kind} n={n} seed={seed}");
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    check(&name, &game, domain);
                }
            }
        }
    }
    let game = generators::bnf(3, 4, 0).unwrap();
    check("type3 n=4", &game, Domain::Imputation);
}

#[test]
fn five_player_games() {
    for kind in [1, 4] {
        let game = generators::bnf(kind, 5, 1).unwrap();
        check(&format!("type{kind} n=5"), &game, Domain::Imputation);
    }
}

/// 優加法的でコアも空でないのに、カーネルが 1 点にならない 5 人ゲーム
/// (`examples/prekernel_study.rs` の superadditive, seed 18)。
#[test]
fn superadditive_game_with_segment_kernel() {
    let game = generators::random_superadditive(5, 18).unwrap();
    assert!(coopgame::properties::is_superadditive(&game));
    assert!(coopgame::nucleolus::has_nonempty_core(&game).unwrap());
    let set = check("superadditive n=5 seed=18", &game, Domain::Imputation);
    assert!(
        set.pieces.iter().any(|piece| piece.dimension == 1),
        "{:?}",
        set.pieces
    );
    // カーネルはコアに含まれる。
    let tolerance = 1e-6 * scale(&game);
    for piece in &set.pieces {
        for vertex in piece.vertices.as_ref().unwrap() {
            assert!(coopgame::properties::is_in_core(&game, vertex, tolerance));
        }
    }
}

/// 0-単調なゲームではカーネルとプレカーネルが一致する
/// (Maschler, Peleg, Shapley 1979、docs/references.md の MPS1979)。
/// 頂点が互いの集合に含まれることで、集合として一致することを確かめる。
#[test]
fn kernel_equals_prekernel_for_zero_monotonic_games() {
    let mut games: Vec<(String, ExplicitGame)> = (0..6)
        .map(|seed| {
            (
                format!("superadditive n=4 seed={seed}"),
                generators::random_superadditive(4, seed).unwrap(),
            )
        })
        .collect();
    games.push((
        "superadditive n=5 seed=18".into(),
        generators::random_superadditive(5, 18).unwrap(),
    ));
    games.push((
        "voting n=5 seed=7".into(),
        generators::random_weighted_voting(5, 7).unwrap(),
    ));
    for (name, game) in games {
        assert!(coopgame::properties::is_zero_monotonic(&game), "{name}");
        let kernel = solve(&game, Domain::Imputation);
        let prekernel = solve(&game, Domain::Preimputation);
        let tolerance = 1e-6 * scale(&game);
        for (from, to, label) in [
            (&kernel, &prekernel, "カーネル ⊂ プレカーネル"),
            (&prekernel, &kernel, "プレカーネル ⊂ カーネル"),
        ] {
            for piece in &from.pieces {
                for vertex in piece.vertices.as_ref().unwrap() {
                    assert!(
                        to.contains(vertex, tolerance),
                        "{name}: {label} が {vertex:?} で不成立"
                    );
                }
            }
        }
    }
}
