//! 凸ゲームの手法を、凸と仮定して (`properties::Assume::convex`) 一般のゲームに使い、
//! 結果を事後検証で分類する。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example assumption_study -- 4,5,6,7,8,9 10 > data/analysis/assumption-study.csv
//! ```
//!
//! `superadditive` は有理数で構築したゲーム (`generators::random_superadditive_exact`) を使い、
//! 有理数の値で検証する (浮動小数点数の和による丸め誤差で厳密な検証が成り立たなくなるのを避けるため)。
//!
//! 列:
//! - `convex`: ゲームが実際に凸か (全提携で判定)
//! - `outcome`: `certified` (厳密な検証に合格 = 仁)、`refuted` (仁でないと確定)、`undecided`、
//!   `error` (手法が止まらない・数値的に失敗)
//! - `in_prekernel`: 結果が (浮動小数点で) プレカーネル条件を満たすか
//! - `distance_nucleolus`, `distance_prenucleolus`: LP で求めた仁・プレ仁との最大絶対誤差 (`max|v|` で割る)

use std::time::Instant;

use coopgame::game::exact::ExactGame;
use coopgame::generators;
use coopgame::kernel;
use coopgame::nucleolus::convex::{self, ConvexOptions};
use coopgame::properties::Assume;
use coopgame::verify::{Verified, VerifyOptions};
use coopgame::{Domain, Error, ExplicitGame, nucleolus, properties};

#[path = "../tests/common/mod.rs"]
mod common;
use common::max_abs_difference;

const CLASSES: [&str; 6] = ["convex", "bnf1", "bnf2", "bnf4", "superadditive", "voting"];

/// 浮動小数点数のゲームと、あれば有理数で構築したゲーム。
fn generate(class: &str, n: usize, seed: u64) -> (ExplicitGame, Option<ExactGame>) {
    if class == "superadditive" {
        let exact = generators::random_superadditive_exact(n, seed).expect("生成できる");
        return (exact.to_explicit().expect("変換できる"), Some(exact));
    }
    (
        generators::by_class(class, n, seed).expect("生成できる"),
        None,
    )
}

fn distance(a: &[f64], b: Option<&Vec<f64>>, scale: f64) -> String {
    match b {
        Some(b) => format!("{:.3e}", max_abs_difference(a, b) / scale),
        None => String::new(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sizes: Vec<usize> = args
        .first()
        .map_or("4,5,6", String::as_str)
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect();
    let seeds: u64 = args.get(1).map_or(3, |s| s.parse().expect("seed 数"));
    let mut options = ConvexOptions::default();
    options.tolerance = None;
    options.max_sweeps = 2_000;
    options.max_sfm_iterations = 5_000;
    println!(
        "class,n,seed,convex,outcome,detail,in_prekernel,distance_nucleolus,distance_prenucleolus,sweeps,seconds"
    );
    for &n in &sizes {
        for class in CLASSES {
            for seed in 0..seeds {
                let (game, exact) = generate(class, n, seed);
                let is_convex = properties::is_convex(&game);
                let scale = game.max_abs_value().max(1.0);
                let reference = nucleolus::nucleolus(&game).ok().map(|r| r.allocation);
                let pre = nucleolus::prenucleolus(&game).ok().map(|r| r.allocation);
                let started = Instant::now();
                let assumed = Assume::convex(game.clone());
                let result = convex::nucleolus_with(&assumed, options);
                let seconds = started.elapsed().as_secs_f64();
                let (outcome, detail, allocation, sweeps) = match result {
                    Err(err) => {
                        let kind = match err {
                            Error::LimitExceeded(_) => "limit",
                            Error::Numerical(_) => "numerical",
                            _ => "other",
                        };
                        ("error", kind.to_string(), None, 0)
                    }
                    Ok((unverified, stats)) => {
                        let allocation = unverified.peek().allocation.clone();
                        let verdict = match &exact {
                            Some(exact) => unverified.verify_exact(exact, VerifyOptions::default()),
                            None => unverified.verify(&game, VerifyOptions::default()),
                        }
                        .expect("検証できる");
                        let (outcome, detail) = match verdict {
                            Verified::Certified(_) => ("certified", String::new()),
                            Verified::Refuted { reason, .. } => {
                                let kind = if reason.contains("v({") {
                                    "not_imputation"
                                } else {
                                    "not_nucleolus"
                                };
                                ("refuted", kind.to_string())
                            }
                            Verified::Undecided { .. } => ("undecided", String::new()),
                        };
                        (outcome, detail, Some(allocation), stats.sweeps)
                    }
                };
                let (in_prekernel, d_nucleolus, d_pre) = match &allocation {
                    Some(x) => (
                        (kernel::kernel_violation(&game, x, Domain::Preimputation) <= 1e-6 * scale)
                            .to_string(),
                        distance(x, reference.as_ref(), scale),
                        distance(x, pre.as_ref(), scale),
                    ),
                    None => (String::new(), String::new(), String::new()),
                };
                println!(
                    "{class},{n},{seed},{is_convex},{outcome},{detail},{in_prekernel},{d_nucleolus},{d_pre},{sweeps},{seconds:.4}"
                );
            }
        }
    }
}
