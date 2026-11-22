//! 有理数だけの仁のソルバー (`nucleolus::exact::nucleolus`) の計測。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example bench_exact -- 3,4,5,6,7,8,9,10 > data/bench/exact-scaling.csv
//! ```
//!
//! BNF タイプ 1, 2, 4 (seed 1) の仁を有理数で求め、浮動小数点の LP の仁を厳密に検証した配分
//! (`verify::certify`) と分数として一致するかを `matches_certified` に出す。
//!
//! 2 つ目の引数に `integer` を付けると、値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲームの時間を測る
//! (`scale` は 1 と 1e9。仁の既定の手順が有理数の逐次 LP に切り替えるのは、この種類の値の大きいゲームである)。
//! 10 人は 1 ゲームに数分かかる。
//!
//! ```bash
//! cargo run --release --example bench_exact -- 7,8,9,10 integer > data/bench/exact-scaling-integer.csv
//! ```

use std::time::Instant;

use coopgame::game::exact::ExactGame;
use coopgame::nucleolus::{self, Options, Tolerance};
use coopgame::verify;
use coopgame::{Domain, ExplicitGame, generators};

/// コマンドライン引数 `[人数,...]` を読む。省略したら 3,4,5,6 にする。
fn parse_sizes() -> Vec<usize> {
    std::env::args()
        .nth(1)
        .unwrap_or_else(|| "3,4,5,6".to_string())
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect()
}

/// 値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲーム (`examples/study_tolerance.rs` と同じ形)。
fn integer_game(scale: f64, n: usize, seed: u64) -> ExplicitGame {
    let mut state = seed;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let values: Vec<f64> = (1usize..1 << n)
        .map(|mask| {
            let size = mask.count_ones() as f64;
            if size == 1.0 {
                0.0
            } else {
                scale * size + (next() % 7) as f64
            }
        })
        .collect();
    ExplicitGame::from_binary(&values).expect("有限の値")
}

/// 整数ゲームの有理数の仁の時間。10 人は 1 ゲームに数分かかるので、seed を 1 つにする。
fn bench_integer(sizes: &[usize]) {
    println!("type,n,seed,seconds,lp_solves,levels");
    for &n in sizes {
        let seeds = if n >= 10 { 1 } else { 2 };
        for scale in [1.0, 1e9] {
            for seed in 1..=seeds {
                let game = integer_game(scale, n, 100 + seed);
                let exact_game = ExactGame::from_explicit(&game).unwrap();
                let started = Instant::now();
                let result = nucleolus::exact::nucleolus(&exact_game, Domain::Imputation).unwrap();
                println!(
                    "integer-{scale:e},{n},{seed},{:.1},{},{}",
                    started.elapsed().as_secs_f64(),
                    result.lp_solves,
                    result.levels.len()
                );
            }
        }
    }
}

fn main() {
    let sizes = parse_sizes();
    if std::env::args().nth(2).as_deref() == Some("integer") {
        bench_integer(&sizes);
        return;
    }
    println!("type,n,seconds,lp_solves,levels,matches_certified");
    for n in sizes {
        for kind in [1u8, 2, 4] {
            let game = generators::bnf(kind, n, 1).unwrap();
            let exact_game = ExactGame::from_explicit(&game).unwrap();
            let started = Instant::now();
            let result = nucleolus::exact::nucleolus(&exact_game, Domain::Imputation).unwrap();
            let seconds = started.elapsed().as_secs_f64();
            // 浮動小数点の逐次 LP の解と比べるので、既定の比の許容誤差で 1 回だけ解く
            // (既定の設定は有理数の逐次 LP に切り替えることがある)。
            let mut options = Options::new(Domain::Imputation);
            options.tolerance = Tolerance::Relative(1e-7);
            let float = nucleolus::nucleolus_with(&game, options)
                .unwrap()
                .into_allocation();
            let certified = verify::certify(&game, &float, Domain::Imputation).unwrap();
            let matches = certified.satisfied && certified.allocation == result.allocation;
            println!(
                "{kind},{n},{seconds:.3},{},{},{matches}",
                result.lp_solves,
                result.levels.len()
            );
        }
    }
}
