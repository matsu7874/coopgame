//! 超過の大きい順に提携を読む処理の時間を、人数と読む個数を変えて測る。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example bench_excess_order -- 14,16,18,20,22 7 16
//! ```
//!
//! 引数は `[人数,...] [繰り返し回数] [仁・カーネルを測る最大人数]`。各項目は繰り返しの中央値を出す。
//! ゲームは `generators::random_superadditive`、配分は `kernel::equal_surplus_division`。
//!
//! - oracle_take: `OracleGame::excess_order(x).take(k)` を読み切る (`ExplicitGame`)
//! - surplus_take: `surplus::excess_order(game, x)` の先頭 `k` 個を読む
//! - nucleolus: `nucleolus::oracle::nucleolus` に `ExplicitGame` を渡す (違反する提携を超過の大きい順に探す)
//! - max_surplus: `surplus::max_surplus`
//! - kernel: `kernel::kernel_point` (transfer scheme。反復ごとに最大余剰を求める)

use std::hint::black_box;
use std::time::Instant;

use coopgame::game::oracle::OracleGame;
use coopgame::kernel::{TransferOptions, equal_surplus_division, kernel_point};
use coopgame::nucleolus::oracle;
use coopgame::{Domain, generators, surplus};

struct Args {
    sizes: Vec<usize>,
    reps: usize,
    max_solver_n: usize,
}

/// 省略した引数は既定値 (14,16,18,20、5 回、16 人) にする。
fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sizes = args
        .first()
        .map_or("14,16,18,20", String::as_str)
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect();
    let reps = args.get(1).map_or(5, |s| s.parse().expect("繰り返し回数"));
    let max_solver_n = args.get(2).map_or(16, |s| s.parse().expect("最大人数"));
    Args {
        sizes,
        reps,
        max_solver_n,
    }
}

/// `f` を `reps` 回実行した時間の中央値 (秒)。
fn median_seconds(reps: usize, mut f: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..reps)
        .map(|_| {
            let started = Instant::now();
            f();
            started.elapsed().as_secs_f64()
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

fn main() {
    let Args {
        sizes,
        reps,
        max_solver_n,
    } = parse_args();
    println!("item,n,k,seconds");
    for &n in &sizes {
        let game = generators::random_superadditive(n, n as u64).expect("ゲーム");
        let x = equal_surplus_division(&game);
        let total = (1usize << n) - 2;
        for k in [1, 10, 100, total] {
            let label = if k == total {
                "all".to_string()
            } else {
                k.to_string()
            };
            let seconds = median_seconds(reps, || {
                let sum: f64 = game.excess_order(&x).take(k).map(|(_, e)| e).sum();
                black_box(sum);
            });
            println!("oracle_take,{n},{label},{seconds:.6}");
            // `into_iter` は、`surplus::excess_order` が `Vec` を返していた版でも同じ例で測るため。
            #[allow(clippy::useless_conversion)]
            let seconds = median_seconds(reps, || {
                let sum: f64 = surplus::excess_order(&game, &x)
                    .into_iter()
                    .take(k)
                    .map(|(_, e)| e)
                    .sum();
                black_box(sum);
            });
            println!("surplus_take,{n},{label},{seconds:.6}");
        }
        let seconds = median_seconds(reps, || {
            black_box(surplus::max_surplus(&game, &x));
        });
        println!("max_surplus,{n},,{seconds:.6}");
        if n <= max_solver_n {
            let seconds = median_seconds(reps, || {
                black_box(oracle::nucleolus(&game).expect("仁"));
            });
            println!("nucleolus,{n},,{seconds:.6}");
            let seconds = median_seconds(reps, || {
                let options = TransferOptions::for_game(&game);
                black_box(
                    kernel_point(&game, Domain::Imputation, None, options).expect("カーネル"),
                );
            });
            println!("kernel,{n},,{seconds:.6}");
        }
    }
}
