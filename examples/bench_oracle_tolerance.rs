//! オラクル版の仁を、許容誤差 (`max|v(S)|` に対する比) を変えて解いたときの時間と追加した行を測る。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example bench_oracle_tolerance
//! ```
//!
//! ゲームは `examples/large_games.rs` と同じ 40 人の重み付き投票ゲーム (200 票の 3 人と 1 票の 37 人、基準 401)。
//! 仁は 200 票の 3 人が 1/3 ずつ、1 票の 37 人が 0 で、`max_error` はこの配分との差の最大値。

use std::time::Instant;

use coopgame::Domain;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::nucleolus::oracle;

fn main() {
    let mut weights = vec![200, 200, 200];
    weights.extend(std::iter::repeat_n(1, 37));
    let game = WeightedVotingGame::new(weights, 401).unwrap();
    let mut expected = vec![1.0 / 3.0; 3];
    expected.extend(std::iter::repeat_n(0.0, 37));
    println!("ratio,seconds,levels,lp_solves,rows_added,max_error");
    for ratio in [1e-7, 1e-8, 1e-9] {
        // 投票ゲームの値の最大絶対値は 1 なので、比がそのまま許容誤差になる。
        let started = Instant::now();
        let result = oracle::nucleolus_with(&game, Domain::Imputation, ratio);
        let seconds = started.elapsed().as_secs_f64();
        match result {
            Ok(r) => {
                let error = r
                    .allocation()
                    .iter()
                    .zip(&expected)
                    .map(|(a, e)| (a - e).abs())
                    .fold(0.0, f64::max);
                println!(
                    "{ratio:e},{seconds:.4},{},{},{},{error:e}",
                    r.levels().len(),
                    r.lp_solves(),
                    r.rows_added()
                );
            }
            Err(err) => println!("{ratio:e},{seconds:.4},,,,error:{err}"),
        }
    }
}
