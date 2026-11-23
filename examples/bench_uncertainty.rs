//! 値の不確かさの分析 (`analysis::uncertainty`) で、仁を既定の手順 (`Tolerance::Auto`) で解く場合と、
//! 比 `1e-7` の許容誤差で 1 回だけ解く場合 (`Tolerance::Relative(1e-7)`) の時間を比べる。CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example bench_uncertainty > data/bench/uncertainty.csv
//! ```
//!
//! 列:
//! - `analysis`: `uncertainty` (区間ゲームの標本の仁の分布。CLI と Python の既定の標本数 200) か、
//!   `influence` (仁の超過の大きい上位 2 段の提携と全体提携の感度。差分の幅は `1e-4 max|v(S)|`。CLI と Python の既定)
//! - `family`: `bnf1` は `generators::bnf(1, n, 1)`。`integer-1e9` は、値が `1e9 |S| + 0..6` (1 人提携は 0) の
//!   整数ゲーム (超過の段の間隔が 1 以下で、比 `1e-7` の許容誤差では段を分けられない)
//! - `players`: 人数
//! - `relative`: `uncertainty` の区間の幅 (各値の前後 `relative` 倍)。`influence` では空
//! - `samples`: `uncertainty` の標本数。`influence` では空
//! - `mode`: `fixed` は比 `1e-7` の許容誤差で 1 回だけ解く、`auto` は既定の手順
//! - `solves`: 仁を解いた回数
//! - `seconds`: 分析全体の秒 (3 回実行した最小)
//! - `refined`・`rational`・`approximate`: 解き直した解・有理数の逐次 LP で求めた解・保証が `approximate` の解の数

use std::time::Instant;

use coopgame::analysis::uncertainty::{self, IntervalGame};
use coopgame::generators::{self, SplitMix64};
use coopgame::nucleolus::{self, NucleolusResult, Options, Tolerance};
use coopgame::{Domain, ExplicitGame, Guarantee, Result};

#[derive(Default)]
struct Tally {
    solves: usize,
    refined: usize,
    rational: usize,
    approximate: usize,
}

/// 許容誤差の決め方 `tolerance` で仁を解き、解き方の内訳を `tally` に数える。
fn solve(
    tolerance: Tolerance,
    tally: &mut Tally,
) -> impl FnMut(&ExplicitGame) -> Result<NucleolusResult> + '_ {
    move |game| {
        let mut options = Options::new(Domain::Imputation);
        options.tolerance = tolerance;
        let result = nucleolus::nucleolus_with(game, options)?;
        tally.solves += 1;
        tally.refined += usize::from(result.refinements() > 0);
        tally.rational += usize::from(result.is_rational());
        tally.approximate += usize::from(result.guarantee() == Guarantee::Approximate);
        Ok(result)
    }
}

fn modes() -> [(&'static str, Tolerance); 2] {
    [
        ("fixed", Tolerance::Relative(1e-7)),
        ("auto", Tolerance::Auto),
    ]
}

fn print(
    analysis: &str,
    family: &str,
    n: usize,
    setting: &str,
    mode: &str,
    seconds: f64,
    t: &Tally,
) {
    println!(
        "{analysis},{family},{n},{setting},{mode},{},{seconds:.3},{},{},{}",
        t.solves, t.refined, t.rational, t.approximate
    );
}

/// 計測の繰り返し回数。秒は最小の値を出す (他の処理と重なったときの揺れを除く)。
const REPEATS: usize = 3;

/// `run` を [`REPEATS`] 回実行し、最小の秒と最後の内訳を返す。
fn best_of(mut run: impl FnMut(&mut Tally)) -> (f64, Tally) {
    let mut best = f64::INFINITY;
    let mut tally = Tally::default();
    for _ in 0..REPEATS {
        tally = Tally::default();
        let started = Instant::now();
        run(&mut tally);
        best = best.min(started.elapsed().as_secs_f64());
    }
    (best, tally)
}

fn measure_uncertainty(family: &str, game: &ExplicitGame, relative: f64, samples: usize) {
    let interval = IntervalGame::around(game, relative).expect("有限の値");
    for (mode, tolerance) in modes() {
        let (seconds, tally) = best_of(|tally| {
            let result =
                uncertainty::interval_monte_carlo(&interval, samples, 0, solve(tolerance, tally));
            assert!(result.is_ok(), "{family}: {result:?}");
        });
        let setting = format!("{relative:e},{samples}");
        print(
            "uncertainty",
            family,
            game.players(),
            &setting,
            mode,
            seconds,
            &tally,
        );
    }
}

fn measure_influence(family: &str, game: &ExplicitGame) {
    let delta = 1e-4 * game.max_abs_value().max(1.0);
    for (mode, tolerance) in modes() {
        let (seconds, tally) = best_of(|tally| {
            let mut solver = solve(tolerance, tally);
            let x = solver(game).expect("解ける");
            let key = uncertainty::key_coalitions(game, x.allocation(), 2).expect("配分の長さ");
            let result = uncertainty::influence(game, &key, delta, solver);
            assert!(result.is_ok(), "{family}: {result:?}");
        });
        print(
            "influence",
            family,
            game.players(),
            ",",
            mode,
            seconds,
            &tally,
        );
    }
}

/// 値が `1e9 |S| + 0..6` (1 人提携は 0) の `n` 人の整数ゲーム。
fn integer_game(n: usize, seed: u64) -> ExplicitGame {
    let mut rng = SplitMix64::new(seed);
    let values: Vec<f64> = (1usize..1 << n)
        .map(|mask| {
            let size = mask.count_ones() as f64;
            if size == 1.0 {
                0.0
            } else {
                1e9 * size + (rng.next_u64() % 7) as f64
            }
        })
        .collect();
    ExplicitGame::from_binary(&values).expect("有限の値")
}

fn main() {
    println!(
        "analysis,family,players,relative,samples,mode,solves,seconds,refined,rational,approximate"
    );
    for n in [5, 8, 10, 12] {
        let game = generators::bnf(1, n, 1).expect("BNF タイプ 1");
        measure_uncertainty("bnf1", &game, 0.1, 200);
        measure_influence("bnf1", &game);
    }
    for n in [4, 6, 8] {
        measure_uncertainty("integer-1e9", &integer_game(n, 1), 0.1, 200);
    }
    // 感度は値を少しだけ動かすので、超過の段の間隔が 1 以下のまま残り、既定の手順は有理数の逐次 LP に切り替わりうる
    // (8 人では 1 回に数秒かかるので、6 人まで)。
    for n in [4, 6] {
        measure_influence("integer-1e9", &integer_game(n, 1));
    }
    // 区間が狭く (各値 ±1e-3 程度)、超過の段の間隔が 1 以下のまま残る標本。
    for n in [4, 6] {
        measure_uncertainty("integer-1e9", &integer_game(n, 1), 1e-12, 200);
    }
}
