//! 全ての行を LP に入れる解 (per capita 仁・比例仁・disruption nucleolus・modiclus) と交渉集合の判定の、
//! 人数ごとの時間と最大メモリ。プレイヤー数の上限 (`MAX_*_PLAYERS`) の根拠 (`docs/performance.md`)。
//!
//! ```bash
//! cargo build --release --example bench_limits
//! target/release/examples/bench_limits > data/bench/variant-limits.csv   # 全ての組 (十数分)
//! target/release/examples/bench_limits per-capita bnf1 17                 # 1 件だけ (最大メモリは測らない)
//! ```
//!
//! 全ての組を測るときは、1 件ずつ子プロセスで実行し、最大メモリ (最大常駐セットサイズ) を
//! `/usr/bin/time` で測る (macOS は `-l`、Linux は `-v`)。
//!
//! 上限を決めるために、上限より 1 人多い人数も測る。そのままでは `TooManyPlayers` になる
//! (`status` 列が `too_many_players`) ので、`src/nucleolus/variants.rs` と `src/bargaining.rs` の
//! `MAX_*_PLAYERS` を一時的に大きくしたビルドで測る。
//!
//! ゲーム:
//! - `bnf1`・`bnf2`・`bnf4`: BNF タイプ 1, 2, 4 (seed 0)
//! - `convex`: `generators::random_convex` (seed 0)。disruption nucleolus はコアが空でないゲームに限るのでこれだけを使う
//! - `majority`・`quota2`: 重み 1 の投票ゲームで、基準が過半数と 2。交渉集合の判定は仁 (均等配分) で行う。
//!   超過が正の提携が多く、全ての異議の LP を解く (均等配分は交渉集合に属する)

use std::process::Command;
use std::time::Instant;

use coopgame::nucleolus::{self, variants};
use coopgame::{Domain, ExplicitGame, bargaining, generators};

/// (解, ゲーム, 人数)。
const CASES: &[(&str, &str, &[usize])] = &[
    ("per-capita", "bnf1", &[16, 17, 18]),
    ("per-capita", "bnf2", &[16, 17, 18]),
    ("per-capita", "bnf4", &[16, 17, 18]),
    ("per-capita", "convex", &[16, 17, 18]),
    ("proportional", "bnf1", &[16, 17, 18]),
    ("proportional", "bnf2", &[16, 17, 18]),
    ("proportional", "bnf4", &[16, 17, 18]),
    ("proportional", "convex", &[16, 17, 18]),
    ("disruption", "convex", &[15, 16, 17]),
    ("modiclus", "bnf1", &[7]),
    ("modiclus", "bnf2", &[7]),
    ("modiclus", "bnf4", &[7]),
    ("modiclus", "convex", &[7]),
    ("bargaining", "majority", &[10, 11, 12]),
    ("bargaining", "quota2", &[10, 11]),
];

fn game(name: &str, n: usize) -> coopgame::Result<ExplicitGame> {
    let voting = |quota: f64| generators::weighted_voting(&vec![1.0; n], quota);
    match name {
        "bnf1" => generators::bnf(1, n, 0),
        "bnf2" => generators::bnf(2, n, 0),
        "bnf4" => generators::bnf(4, n, 0),
        "convex" => generators::random_convex(n, 0),
        "majority" => voting((n / 2 + 1) as f64),
        "quota2" => voting(2.0),
        _ => Err(coopgame::Error::InvalidArgument(format!("ゲーム {name}"))),
    }
}

/// 1 件を実行し、`秒 LP の回数 状態` を返す。
fn run(method: &str, game_name: &str, n: usize) -> coopgame::Result<String> {
    let game = game(game_name, n)?;
    let started = Instant::now();
    let solves = match method {
        "per-capita" => {
            variants::per_capita_nucleolus(&game, Domain::Imputation).map(|r| r.lp_solves())
        }
        "proportional" => {
            variants::proportional_nucleolus(&game, Domain::Imputation).map(|r| r.lp_solves())
        }
        "disruption" => variants::disruption_nucleolus(&game).map(|r| r.lp_solves()),
        "modiclus" => variants::modiclus(&game).map(|r| r.lp_solves()),
        "bargaining" => {
            let x = nucleolus::nucleolus(&game)?.into_allocation();
            let tolerance = 10.0 * coopgame::game::default_tolerance(&game);
            bargaining::check(&game, &x, Domain::Imputation, tolerance).map(|r| {
                assert!(r.is_member(), "仁は交渉集合に属する");
                r.lp_solves
            })
        }
        _ => return Err(coopgame::Error::InvalidArgument(format!("解 {method}"))),
    };
    let seconds = started.elapsed().as_secs_f64();
    Ok(match solves {
        Ok(solves) => format!("{seconds:.3} {solves} ok"),
        Err(err) => format!("{seconds:.3} 0 {}", status(&err)),
    })
}

fn status(err: &coopgame::Error) -> &'static str {
    match err {
        coopgame::Error::TooManyPlayers { .. } => "too_many_players",
        _ => "error",
    }
}

/// `/usr/bin/time` の出力から最大常駐セットサイズ (MB) を読む。
fn max_rss_mb(stderr: &str) -> Option<f64> {
    stderr.lines().find_map(|line| {
        let line = line.trim();
        if let Some(bytes) = line.strip_suffix("maximum resident set size") {
            // macOS (`-l`): バイト
            return bytes.trim().parse::<f64>().ok().map(|b| b / 1048576.0);
        }
        // Linux (`-v`): キロバイト
        let kilobytes = line.strip_prefix("Maximum resident set size (kbytes):")?;
        kilobytes.trim().parse::<f64>().ok().map(|k| k / 1024.0)
    })
}

fn main() -> coopgame::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if let [_, method, game_name, n] = args.as_slice() {
        let n = n
            .parse()
            .map_err(|_| coopgame::Error::InvalidArgument(format!("人数 {n}")))?;
        println!("{}", run(method, game_name, n)?);
        return Ok(());
    }
    let flag = if cfg!(target_os = "macos") {
        "-l"
    } else {
        "-v"
    };
    let me = std::env::current_exe().expect("実行ファイルのパス");
    println!("method,game,n,seconds,max_rss_mb,lp_solves,status");
    for (method, game_name, sizes) in CASES {
        for n in sizes.iter() {
            let output = Command::new("/usr/bin/time")
                .arg(flag)
                .arg(&me)
                .args([method.to_string(), game_name.to_string(), n.to_string()])
                .output()
                .expect("/usr/bin/time を実行できない");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let fields: Vec<&str> = stdout.split_whitespace().collect();
            let rss = max_rss_mb(&stderr).expect("最大常駐セットサイズを読めない");
            match fields.as_slice() {
                [seconds, solves, status] => {
                    println!("{method},{game_name},{n},{seconds},{rss:.0},{solves},{status}")
                }
                _ => panic!("{method} {game_name} {n} が失敗した: {stderr}"),
            }
        }
    }
    Ok(())
}
