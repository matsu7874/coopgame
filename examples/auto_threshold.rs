//! 凸と確認済みの明示ゲームで、逐次 LP と凸ゲームの手法の時間を比べる (`auto::EXPLICIT_LP_LIMIT` の根拠)。
//! `relative_difference` は 2 つの配分の最大絶対差を `max|v|` で割った値。
//!
//! ```bash
//! cargo run --release --example auto_threshold -- 8,10,12,14,16,18 3
//! ```

use std::time::Instant;

use coopgame::generators;
use coopgame::nucleolus;
use coopgame::nucleolus::convex;
use coopgame::properties::ConvexChecked;

#[path = "../tests/common/mod.rs"]
mod common;
use common::max_abs_difference;

/// コマンドライン引数。
struct Args {
    sizes: Vec<usize>,
    seeds: u64,
}

/// `[人数,...] [seed 数]` を読む。省略した引数は既定値 (8,10,12 と 3) にする。
fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sizes = args
        .first()
        .map_or("8,10,12", String::as_str)
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect();
    let seeds = args.get(1).map_or(3, |s| s.parse().expect("seed 数"));
    Args { sizes, seeds }
}

fn main() {
    let Args { sizes, seeds } = parse_args();
    println!("n,seed,lp_seconds,convex_seconds,relative_difference");
    for &n in &sizes {
        for seed in 0..seeds {
            let game = ConvexChecked::new(generators::random_convex(n, seed).unwrap())
                .expect("凸ゲームを生成する");
            let started = Instant::now();
            let lp = nucleolus::nucleolus(game.game()).unwrap().allocation;
            let lp_seconds = started.elapsed().as_secs_f64();
            let started = Instant::now();
            let solution = convex::nucleolus(&game).unwrap();
            let convex_seconds = started.elapsed().as_secs_f64();
            let scale = game.game().max_abs_value().max(1.0);
            let difference = max_abs_difference(&lp, &solution.allocation) / scale;
            println!("{n},{seed},{lp_seconds:.4},{convex_seconds:.4},{difference:.2e}");
        }
    }
}
