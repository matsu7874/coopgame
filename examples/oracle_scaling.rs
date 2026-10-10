//! オラクル型ゲームの仁の計算時間を、人数を変えて測る。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example oracle_scaling -- bankruptcy 10,20,40,70,100
//! cargo run --release --example oracle_scaling -- majority 9,15,21,31
//! cargo run --release --example oracle_scaling -- voting 10,20,30,40
//! ```
//!
//! - bankruptcy: 請求 1-100 の一様整数、遺産は請求の和の一様乱数倍。タルムード則との差を出す。
//! - majority: 全員の重みが 1 の多数決ゲーム。等分 (1/n) との差を出す。
//! - voting: 重み 1-10 の一様整数、基準は過半数。

use std::time::Instant;

use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::generators::SplitMix64;
use coopgame::nucleolus::oracle as oracle_nucleolus;

#[path = "../tests/common/mod.rs"]
mod common;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let kind = args.first().map(String::as_str).unwrap_or("bankruptcy");
    let sizes: Vec<usize> = args
        .get(1)
        .map_or("10,20,40".into(), |s| s.clone())
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect();
    let seeds: u64 = args.get(2).map_or(1, |s| s.parse().expect("seed 数"));
    println!("kind,n,seed,seconds,levels,lp_solves,rows_added,max_error");
    for &n in &sizes {
        for seed in 0..seeds {
            let mut rng = SplitMix64::new(seed + 1000 * n as u64);
            let started = Instant::now();
            let (result, expected) = match kind {
                "bankruptcy" => {
                    let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 100) as f64).collect();
                    let estate = (rng.next_f64() * claims.iter().sum::<f64>()).round();
                    let game = BankruptcyGame::new(estate, claims.clone()).unwrap();
                    (
                        oracle_nucleolus::nucleolus(&game),
                        Some(common::talmud_rule(estate, &claims)),
                    )
                }
                "majority" => {
                    let game = WeightedVotingGame::new(vec![1; n], (n / 2 + 1) as u64).unwrap();
                    (
                        oracle_nucleolus::nucleolus(&game),
                        Some(vec![1.0 / n as f64; n]),
                    )
                }
                "voting" => {
                    let weights: Vec<u64> = (0..n).map(|_| rng.range(1, 10)).collect();
                    let quota = weights.iter().sum::<u64>() / 2 + 1;
                    let game = WeightedVotingGame::new(weights, quota).unwrap();
                    (oracle_nucleolus::nucleolus(&game), None)
                }
                other => panic!("未知の種類 {other}"),
            };
            let seconds = started.elapsed().as_secs_f64();
            match result {
                Ok(r) => {
                    let error = expected.map_or(String::new(), |e| {
                        let max = common::max_abs_difference(&r.allocation, &e);
                        format!("{max:e}")
                    });
                    println!(
                        "{kind},{n},{seed},{seconds:.4},{},{},{},{error}",
                        r.levels.len(),
                        r.lp_solves,
                        r.rows_added
                    );
                }
                Err(err) => println!("{kind},{n},{seed},{seconds:.4},,,,error:{err}"),
            }
        }
    }
}
