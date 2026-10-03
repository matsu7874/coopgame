//! 凸ゲームの仁 (`convex::nucleolus`) の計測。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example convex_scaling -- bankruptcy 10,20,40 3   # タルムード則と比べる
//! cargo run --release --example convex_scaling -- graph 10,16,24,40 3     # n <= 16 は LP の仁と比べる
//! ```
//!
//! - bankruptcy: 請求 1-100 の整数、遺産は総和の一様乱数。基準はタルムード則 (閉じた形)。
//! - graph: 誘導部分グラフゲーム。各辺を確率 0.3 で張り、重みは 0-10 の一様乱数。
//!   n <= 16 は全提携の表の LP で求めた仁を基準にする。それより大きい n は基準がないので
//!   `max_error` を空にし、ランダムな 4 組のカーネル条件を総当たりで確かめた結果 (`pair_check`) を出す
//!   (26 人を超えると総当たりできないので `not_checked`)。

use std::time::Instant;

use coopgame::convex::{self, ConvexOptions};
use coopgame::generators::SplitMix64;
use coopgame::oracle::bankruptcy::BankruptcyGame;
use coopgame::oracle::graph::InducedSubgraphGame;
use coopgame::oracle::tabulate;
use coopgame::verify::{self, Check, VerifyOptions};
use coopgame::{Concept, Guarantee, Property, Solution, nucleolus};

#[path = "../tests/common/mod.rs"]
mod common;
use common::max_abs_difference;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let kind = args.first().map(String::as_str).unwrap_or("graph");
    let sizes: Vec<usize> = args
        .get(1)
        .map_or("10,16", String::as_str)
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect();
    let seeds: u64 = args.get(2).map_or(3, |s| s.parse().expect("seed 数"));
    println!("kind,n,seed,seconds,sweeps,transfers,minimizations,evaluations,max_error,pair_check");
    for &n in &sizes {
        for seed in 0..seeds {
            let mut rng = SplitMix64::new(1000 * n as u64 + seed);
            match kind {
                "bankruptcy" => {
                    let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 100) as f64).collect();
                    let estate = (rng.next_f64() * claims.iter().sum::<f64>()).round();
                    let game = BankruptcyGame::new(estate, claims).unwrap();
                    let started = Instant::now();
                    let (solution, stats) =
                        convex::nucleolus_with(&game, ConvexOptions::default()).unwrap();
                    let seconds = started.elapsed().as_secs_f64();
                    let error =
                        max_abs_difference(&solution.allocation, &game.nucleolus().unwrap());
                    println!(
                        "bankruptcy,{n},{seed},{seconds:.3},{},{},{},{},{error:.2e},",
                        stats.sweeps, stats.transfers, stats.minimizations, stats.evaluations
                    );
                }
                "graph" => {
                    let mut edges = Vec::new();
                    for u in 0..n {
                        for v in u + 1..n {
                            if rng.next_f64() < 0.3 {
                                edges.push((u, v, (rng.next_f64() * 10.0 * 100.0).round() / 100.0));
                            }
                        }
                    }
                    let game = InducedSubgraphGame::new(n, edges).unwrap();
                    let started = Instant::now();
                    let (solution, stats) =
                        convex::nucleolus_with(&game, ConvexOptions::default()).unwrap();
                    let seconds = started.elapsed().as_secs_f64();
                    let (error, pair_check) = if n > verify::MAX_PAIR_CHECK_PLAYERS {
                        (String::new(), "not_checked".to_string())
                    } else if n <= 16 {
                        let explicit = tabulate(&game).unwrap();
                        let lp = nucleolus::nucleolus(&explicit).unwrap().allocation;
                        (
                            format!("{:.2e}", max_abs_difference(&solution.allocation, &lp)),
                            String::new(),
                        )
                    } else {
                        let candidate = Solution {
                            concept: Concept::Nucleolus,
                            allocation: solution.allocation.clone(),
                            guarantee: Guarantee::Proven(Property::Convex),
                            method: "convex-transfer",
                        };
                        let mut options = VerifyOptions::default();
                        options.pairs = 4;
                        options.seed = seed;
                        let check = verify::check(&game, &candidate, options).unwrap();
                        let label = match check {
                            Check::Certified { .. } => "certified",
                            Check::Refuted(_) => "refuted",
                            Check::Undecided(_) => "no_counterexample",
                        };
                        (String::new(), label.to_string())
                    };
                    println!(
                        "graph,{n},{seed},{seconds:.3},{},{},{},{},{error},{pair_check}",
                        stats.sweeps, stats.transfers, stats.minimizations, stats.evaluations
                    );
                }
                other => panic!("未知の種類 {other}"),
            }
        }
    }
}
