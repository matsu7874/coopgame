//! 逐次 LP の許容誤差の決め方を比べる。既定の手順 (`Tolerance::Auto`) と、`max|v(S)|` の `1e-7` 倍の許容誤差で
//! 1 回だけ解く場合 (`Tolerance::Relative(1e-7)`) について、正しい仁を返した数と時間を CSV で標準出力に出す。
//!
//! ```bash
//! cargo run --release --example study_tolerance > data/analysis/tolerance-study.csv
//! ```
//!
//! 列:
//! - `family`: ゲームの種類。`integer-<scale>` は、値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲームで、
//!   超過の段の間隔が 1 以下になる。`bnf<k>` は `generators::bnf`、`superadditive-rational` は有理数で構築した
//!   優加法的なゲーム、`superadditive-float` は浮動小数点数の和で作った優加法的なゲーム、`convex` は凸ゲーム
//! - `players`: 人数の範囲
//! - `games`: 比べたゲームの数 (`criterion` が `rational` の行では、有理数の仁を求められたゲームだけを数える)
//! - `criterion`: 正しさの判定。`rational` は有理数の逐次 LP の仁との差が丸めの大きさ `4 n ε max|v|` 以下、
//!   `certify` は厳密な検証 (`verify::certify`) に合格
//! - `fixed_*`: 比 `1e-7` の許容誤差で 1 回だけ解いた場合の、正しい数・解けた数・合計の秒
//! - `auto_*`: 既定の手順の、正しい数・解けた数・合計の秒
//! - `refined`: 既定の手順が許容誤差を下げて解き直したゲームの数
//! - `rational`: 既定の手順が有理数の逐次 LP に切り替えたゲームの数
//! - `approximate`: 既定の手順が保証を `approximate` にしたゲームの数

use std::time::Instant;

use coopgame::game::exact::ExactGame;
use coopgame::nucleolus::{self, Options, Tolerance, exact};
use coopgame::num_traits::ToPrimitive;
use coopgame::verify;
use coopgame::{Domain, ExplicitGame, Guarantee, generators};

#[path = "../tests/common/mod.rs"]
mod common;
use common::max_abs_difference;

#[derive(Default)]
struct Tally {
    solved: usize,
    correct: usize,
    refined: usize,
    rational: usize,
    approximate: usize,
    seconds: f64,
}

/// 正しさの判定に使う基準。
#[derive(Clone, Copy)]
enum Criterion {
    /// 有理数の逐次 LP の仁と比べる (10 人まで。時間がかかるので 6 人以下で使う)。
    Rational,
    /// 厳密な検証に合格するかで判定する。
    Certify,
}

fn row(family: &str, players: &str, games: &[ExplicitGame], criterion: Criterion) {
    let mut tallies = [Tally::default(), Tally::default()];
    let mut evaluated = 0;
    for game in games {
        let n = game.players();
        let truth: Option<Vec<f64>> = match criterion {
            Criterion::Rational => {
                let solved = ExactGame::from_explicit(game)
                    .ok()
                    .and_then(|game| exact::nucleolus(&game, Domain::Imputation).ok());
                // 2 進数の値どおりの有理数では配分集合が空になるゲーム (和の丸めによる) は、比べる仁がないので除く。
                let Some(solved) = solved else { continue };
                Some(
                    solved
                        .allocation
                        .iter()
                        .map(|q| q.to_f64().expect("有限の値"))
                        .collect(),
                )
            }
            Criterion::Certify => None,
        };
        evaluated += 1;
        let rounding = 4.0 * n as f64 * f64::EPSILON * game.max_abs_value();
        for (tally, tolerance) in tallies
            .iter_mut()
            .zip([Tolerance::Relative(1e-7), Tolerance::Auto])
        {
            let mut options = Options::new(Domain::Imputation);
            options.tolerance = tolerance;
            let started = Instant::now();
            let result = nucleolus::nucleolus_with(game, options);
            tally.seconds += started.elapsed().as_secs_f64();
            let Ok(result) = result else { continue };
            tally.solved += 1;
            let correct = match &truth {
                Some(truth) => max_abs_difference(result.allocation(), truth) <= rounding,
                None => verify::certify(game, result.allocation(), Domain::Imputation)
                    .map(|report| report.satisfied)
                    .unwrap_or(false),
            };
            tally.correct += usize::from(correct);
            tally.refined += usize::from(result.refinements() > 0);
            tally.rational += usize::from(result.is_rational());
            tally.approximate += usize::from(result.guarantee() == Guarantee::Approximate);
        }
    }
    let [fixed, auto] = tallies;
    let criterion = match criterion {
        Criterion::Rational => "rational",
        Criterion::Certify => "certify",
    };
    println!(
        "{family},{players},{evaluated},{criterion},{},{},{:.2},{},{},{:.2},{},{},{}",
        fixed.correct,
        fixed.solved,
        fixed.seconds,
        auto.correct,
        auto.solved,
        auto.seconds,
        auto.refined,
        auto.rational,
        auto.approximate
    );
}

/// 値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲームを `count` 個作る。人数は `sizes` を順に使う。
fn integer_games(
    scale: f64,
    sizes: &[usize],
    count: usize,
    next: &mut impl FnMut() -> u64,
) -> Vec<ExplicitGame> {
    (0..count)
        .map(|case| {
            let n = sizes[case % sizes.len()];
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
        })
        .collect()
}

fn main() {
    let mut state = 97531u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    println!(
        "family,players,games,criterion,fixed_correct,fixed_solved,fixed_seconds,\
         auto_correct,auto_solved,auto_seconds,refined,rational,approximate"
    );
    for scale in [1.0, 1e5, 1e6, 1e7, 1e8, 1e9, 1e12] {
        let games = integer_games(scale, &[3, 4, 5, 6], 200, &mut next);
        row(
            &format!("integer-{scale:e}"),
            "3-6",
            &games,
            Criterion::Rational,
        );
    }
    for (scale, count) in [(1e7, 20), (1e9, 10)] {
        let games = integer_games(scale, &[7, 8], count, &mut next);
        row(
            &format!("integer-{scale:e}"),
            "7-8",
            &games,
            Criterion::Certify,
        );
    }
    for scale in [1.0, 1e6, 1e7, 1e8, 1e9] {
        let games = integer_games(scale, &[11, 12, 13, 14], 40, &mut next);
        row(
            &format!("integer-{scale:e}"),
            "11-14",
            &games,
            Criterion::Certify,
        );
    }
    for class in [1u8, 2, 4] {
        let family = format!("bnf{class}");
        let small: Vec<ExplicitGame> = (0..200u64)
            .filter_map(|seed| generators::bnf(class, 3 + (seed % 4) as usize, seed).ok())
            .collect();
        row(&family, "3-6", &small, Criterion::Rational);
        let medium: Vec<ExplicitGame> = (0..40u64)
            .filter_map(|seed| generators::bnf(class, 11 + (seed % 4) as usize, seed).ok())
            .collect();
        row(&family, "11-14", &medium, Criterion::Certify);
        let large: Vec<ExplicitGame> = (0..9u64)
            .filter_map(|seed| generators::bnf(class, 16 + 2 * (seed % 3) as usize, seed).ok())
            .collect();
        row(&family, "16-20", &large, Criterion::Certify);
    }
    let rational: Vec<ExplicitGame> = (0..200u64)
        .filter_map(|seed| {
            generators::random_superadditive_exact(3 + (seed % 4) as usize, seed)
                .ok()
                .and_then(|game| game.to_explicit().ok())
        })
        .collect();
    row(
        "superadditive-rational",
        "3-6",
        &rational,
        Criterion::Rational,
    );
    let float: Vec<ExplicitGame> = (0..40u64)
        .filter_map(|seed| generators::random_superadditive(11 + (seed % 4) as usize, seed).ok())
        .collect();
    row("superadditive-float", "11-14", &float, Criterion::Certify);
    let convex: Vec<ExplicitGame> = (0..40u64)
        .filter_map(|seed| generators::random_convex(11 + (seed % 4) as usize, seed).ok())
        .collect();
    row("convex", "11-14", &convex, Criterion::Certify);
    let convex: Vec<ExplicitGame> = (0..9u64)
        .filter_map(|seed| generators::random_convex(16 + 2 * (seed % 3) as usize, seed).ok())
        .collect();
    row("convex", "16-20", &convex, Criterion::Certify);
}
