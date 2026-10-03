//! 有理数だけの仁のソルバー (`exact::nucleolus::nucleolus_exact`) の計測。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example exact_scaling -- 3,4,5,6,7,8,9,10 > data/bench/exact-scaling.csv
//! ```
//!
//! BNF タイプ 1, 2, 4 (seed 1) の仁を有理数で求め、浮動小数点の LP の仁を厳密に検証した配分
//! (`exact::certify`) と分数として一致するかを `matches_certified` に出す。

use std::time::Instant;

use coopgame::exact::nucleolus::nucleolus_exact;
use coopgame::exact::{self, ExactGame};
use coopgame::{Domain, generators, nucleolus};

fn main() {
    let sizes: Vec<usize> = std::env::args()
        .nth(1)
        .map_or("3,4,5,6".to_string(), |s| s)
        .split(',')
        .map(|s| s.parse().expect("人数"))
        .collect();
    println!("type,n,seconds,lp_solves,levels,matches_certified");
    for n in sizes {
        for kind in [1u8, 2, 4] {
            let game = generators::bnf(kind, n, 1).unwrap();
            let exact_game = ExactGame::from_explicit(&game).unwrap();
            let started = Instant::now();
            let result = nucleolus_exact(&exact_game, Domain::Imputation).unwrap();
            let seconds = started.elapsed().as_secs_f64();
            let float = nucleolus::nucleolus(&game).unwrap().allocation;
            let certified = exact::certify(&game, &float, Domain::Imputation).unwrap();
            let matches = certified.satisfied && certified.allocation == result.allocation;
            println!(
                "{kind},{n},{seconds:.3},{},{},{matches}",
                result.lp_solves,
                result.levels.len()
            );
        }
    }
}
