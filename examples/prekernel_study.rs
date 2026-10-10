//! プレカーネル・カーネルの形とゲームの性質の関係を調べ、CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example prekernel_study -- [最大人数] [seed 数] [最小人数] > study.csv
//! ```
//!
//! 摂動実験では、真部分提携の値に `[-eps, eps] * max|v|` の一様乱数を足した
//! ゲームを作り、プレカーネルが 1 点かどうかを調べる。

use std::time::Instant;

use coopgame::generators::{self, SplitMix64};
use coopgame::kernel::{KernelSet, SetOptions, kernel_set};
use coopgame::{Domain, Error, ExplicitGame};
use coopgame::{nucleolus, properties};

const CLASSES: [&str; 6] = ["bnf1", "bnf2", "bnf4", "superadditive", "convex", "voting"];
const PERTURBATIONS: usize = 5;
const EPSILON: f64 = 1e-3;

fn generate(class: &str, n: usize, seed: u64) -> ExplicitGame {
    generators::by_class(class, n, seed).expect("生成できる")
}

/// 真部分提携の値に小さな一様乱数を足す(全体提携の値は保つ)。
fn perturb(game: &ExplicitGame, rng: &mut SplitMix64) -> ExplicitGame {
    let scale = EPSILON * game.max_abs_value().max(1.0);
    let full = game.grand().index();
    let values: Vec<f64> = game.values()[1..]
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if index + 1 == full {
                *value
            } else {
                value + (2.0 * rng.next_f64() - 1.0) * scale
            }
        })
        .collect();
    ExplicitGame::from_binary(&values).expect("長さは変わらない")
}

fn is_single_point(set: &KernelSet) -> bool {
    set.pieces.len() == 1 && set.pieces[0].dimension == 0
}

struct Shape {
    status: String,
    pieces: usize,
    max_dimension: usize,
    single: bool,
    nodes: usize,
    seconds: f64,
    set: Option<KernelSet>,
}

fn shape(game: &ExplicitGame, domain: Domain) -> Shape {
    let started = Instant::now();
    let result = kernel_set(game, domain, SetOptions::for_game(game));
    let seconds = started.elapsed().as_secs_f64();
    match result {
        Ok(set) => Shape {
            status: "ok".into(),
            pieces: set.pieces.len(),
            max_dimension: set.pieces.iter().map(|p| p.dimension).max().unwrap_or(0),
            single: is_single_point(&set),
            nodes: set.nodes,
            seconds,
            set: Some(set),
        },
        Err(err) => Shape {
            status: match err {
                Error::EmptyImputationSet => "empty_imputation".into(),
                Error::LimitExceeded(_) => "limit".into(),
                other => format!("error:{}", other.to_string().replace(',', ";")),
            },
            pieces: 0,
            max_dimension: 0,
            single: false,
            nodes: 0,
            seconds,
            set: None,
        },
    }
}

/// カーネルの全ての頂点がコアに属すか。頂点が未計算なら "unknown"。
fn kernel_in_core(game: &ExplicitGame, set: &KernelSet) -> &'static str {
    let tolerance = 1e-6 * game.max_abs_value().max(1.0);
    let mut all = true;
    for piece in &set.pieces {
        let Some(vertices) = &piece.vertices else {
            return "unknown";
        };
        all &= vertices
            .iter()
            .all(|v| properties::is_in_core(game, v, tolerance));
    }
    if all { "yes" } else { "no" }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let max_players: usize = args.first().map_or(5, |a| a.parse().expect("最大人数"));
    let seeds: u64 = args.get(1).map_or(20, |a| a.parse().expect("seed 数"));
    let min_players: usize = args.get(2).map_or(3, |a| a.parse().expect("最小人数"));

    println!(
        "class,n,seed,superadditive,convex,zero_monotonic,core_nonempty,\
         pre_status,pre_pieces,pre_max_dim,pre_single,pre_nodes,pre_seconds,\
         ker_status,ker_pieces,ker_max_dim,ker_single,ker_nodes,ker_seconds,kernel_in_core,\
         perturbed_single"
    );
    for n in min_players..=max_players {
        for class in CLASSES {
            for seed in 0..seeds {
                let game = generate(class, n, seed);
                let core_nonempty = nucleolus::has_nonempty_core(&game).unwrap_or(false);
                let pre = shape(&game, Domain::Preimputation);
                let ker = shape(&game, Domain::Imputation);
                let in_core = match (&ker.set, core_nonempty) {
                    (Some(set), true) => kernel_in_core(&game, set),
                    _ => "na",
                };
                let mut rng = SplitMix64::new(seed ^ 0x5eed);
                let perturbed_single = (0..PERTURBATIONS)
                    .filter(|_| {
                        let perturbed = perturb(&game, &mut rng);
                        kernel_set(
                            &perturbed,
                            Domain::Preimputation,
                            SetOptions::for_game(&perturbed),
                        )
                        .map(|set| is_single_point(&set))
                        .unwrap_or(false)
                    })
                    .count();
                println!(
                    "{class},{n},{seed},{},{},{},{core_nonempty},\
                     {},{},{},{},{},{:.4},\
                     {},{},{},{},{},{:.4},{in_core},\
                     {perturbed_single}",
                    properties::is_superadditive(&game),
                    properties::is_convex(&game),
                    properties::is_zero_monotonic(&game),
                    pre.status,
                    pre.pieces,
                    pre.max_dimension,
                    pre.single,
                    pre.nodes,
                    pre.seconds,
                    ker.status,
                    ker.pieces,
                    ker.max_dimension,
                    ker.single,
                    ker.nodes,
                    ker.seconds,
                );
            }
        }
    }
}
