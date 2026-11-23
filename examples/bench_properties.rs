//! 性質の判定 (`properties::is_convex`・`is_superadditive`) の時間を、許容誤差つきの浮動小数点数の判定 (0.6 までの実装) と比べる。
//!
//! 値どおりの有理数による判定は、浮動小数点数の和で符号が確かな比較はそのまま決め、等号の近くだけ厳密に和を取る。
//! 等号が多いゲーム (整数ゲーム、対称ゲーム) で遅くならないかを測る。
//!
//! ```bash
//! cargo run --release --example bench_properties -- 14,16,18,20 > data/bench/properties.csv
//! ```
//!
//! 出力の列: `game,n,property,result,float_seconds,exact_seconds`。
//! `float_seconds` は許容誤差 `1e-7 max|v|` の浮動小数点数の判定 (この例の中に再現した 0.6 までの実装)、
//! `exact_seconds` は `properties` の判定。`result` は `properties` の判定の結果。
//! `is_superadditive` は `3^n` なので、`n <= 16` だけ測る。

use std::time::Instant;

use coopgame::game::{Coalition, default_tolerance};
use coopgame::generators::{self, SplitMix64};
use coopgame::{ExplicitGame, properties};

/// 0.6 までの `is_convex` (許容誤差つきの浮動小数点数の判定)。
fn float_convex(game: &ExplicitGame) -> bool {
    let tolerance = default_tolerance(game);
    let n = game.players();
    let full = game.grand().0;
    (0..=full).all(|s| {
        (0..n).all(|i| {
            (0..n).all(|j| {
                let (bi, bj) = (1u64 << i, 1u64 << j);
                if i == j || s & (bi | bj) != 0 {
                    return true;
                }
                let without = game.value(Coalition(s | bi)) - game.value(Coalition(s));
                let with = game.value(Coalition(s | bi | bj)) - game.value(Coalition(s | bj));
                with >= without - tolerance
            })
        })
    })
}

/// 0.6 までの `is_superadditive`。
fn float_superadditive(game: &ExplicitGame) -> bool {
    let tolerance = default_tolerance(game);
    let full = game.grand().0;
    (1..=full).all(|union| {
        let mut s = (union - 1) & union;
        while s > 0 {
            let t = union & !s;
            if s < t
                && game.value(Coalition(union))
                    < game.value(Coalition(s)) + game.value(Coalition(t)) - tolerance
            {
                return false;
            }
            s = (s - 1) & union;
        }
        true
    })
}

fn seconds<T>(run: impl FnOnce() -> T) -> (T, f64) {
    let started = Instant::now();
    let value = run();
    (value, started.elapsed().as_secs_f64())
}

fn main() {
    let players: Vec<usize> = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "14,16".into())
        .split(',')
        .map(|v| v.parse().expect("人数は整数"))
        .collect();
    println!("game,n,property,result,float_seconds,exact_seconds");
    for n in players {
        let mut rng = SplitMix64::new(n as u64);
        let weights: Vec<f64> = (0..n).map(|_| (rng.next_f64() * 1e9).floor()).collect();
        // 等号の少ない凸ゲーム (|S|^2)、全ての比較が等号の加法的な整数ゲーム (値 1..n と、1e9 までの乱数)、
        // 等号の多い対称な凸ゲーム (max(|S| - 1, 0))、格子の上の凸ゲーム (random_convex)。
        let games = [
            (
                "square",
                ExplicitGame::from_fn(n, |s| (s.len() * s.len()) as f64).unwrap(),
            ),
            (
                "additive-integer",
                ExplicitGame::from_fn(n, |s| s.members().map(|i| (i + 1) as f64).sum()).unwrap(),
            ),
            (
                "additive-integer-1e9",
                ExplicitGame::from_fn(n, |s| s.members().map(|i| weights[i]).sum()).unwrap(),
            ),
            (
                "symmetric",
                ExplicitGame::from_fn(n, |s| s.len().saturating_sub(1) as f64).unwrap(),
            ),
            ("convex", generators::random_convex(n, 1).unwrap()),
        ];
        for (name, game) in &games {
            let (result, exact) = seconds(|| properties::is_convex(game));
            let (_, float) = seconds(|| float_convex(game));
            println!("{name},{n},convex,{result},{float:.4},{exact:.4}");
            if n <= 16 {
                let (result, exact) = seconds(|| properties::is_superadditive(game));
                let (_, float) = seconds(|| float_superadditive(game));
                println!("{name},{n},superadditive,{result},{float:.4},{exact:.4}");
            }
        }
    }
}
