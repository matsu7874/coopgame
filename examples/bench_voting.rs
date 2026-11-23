//! 重み付き投票ゲームのオラクル版の仁の、人数ごとの時間と最大メモリ (`docs/performance.md` の投票ゲームの計測)。
//!
//! ```bash
//! cargo build --release --example bench_voting
//! target/release/examples/bench_voting > data/bench/voting-oracle-after.csv   # 全ての組
//! target/release/examples/bench_voting veto3 20                                # 1 件だけ (最大メモリは測らない)
//! ```
//!
//! 全ての組を測るときは、1 件ずつ子プロセスで実行し、最大メモリ (最大常駐セットサイズ) を
//! `/usr/bin/time` で測る (macOS は `-l`、Linux は `-v`)。
//! 1 件が [`TIME_LIMIT_SECONDS`] 秒を超えるか、常駐セットサイズが [`RSS_LIMIT_MB`] MB を超えたら止め
//! (`status` 列が `timeout`・`rss_limit`)、同じ種類のそれより大きい人数は測らない (`skipped`)。
//!
//! ゲーム (n はプレイヤー数):
//! - `veto3`: 重み 1000 が 3 人と 1 が `n - 3` 人、基準 `3000 + floor((n - 3) / 2) + 1`。
//!   3 人が拒否権を持ち、仁は (1/3, 1/3, 1/3, 0, ..., 0)。仁が 2 段で決まる
//! - `veto2`: 重み 1000 が 2 人と 1 が `n - 2` 人、基準 `2000 + floor((n - 2) / 2) + 1`。仁は (1/2, 1/2, 0, ..., 0)
//! - `majority`: 全員の重みが 1 で基準が過半数。仁は等分 1/n
//! - `apex`: 重み `n - 2` の 1 人と 1 の `n - 1` 人、基準 `n - 1`。差の列は空
//! - `random10`・`random100`: 重みが 1-10・1-100 の一様整数 (seed は `1000 n`)、基準は過半数。差の列は空
//! - `random100q23`・`random100q90`: `random100` と同じ重みで、基準を重みの和の 2/3・9/10 (切り捨て) + 1 に
//!   したもの。9/10 では重みの大きい何人かが拒否権を持ち、仁が 2 段以上で決まることがある。差の列は空

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use coopgame::games::voting::WeightedVotingGame;
use coopgame::generators::SplitMix64;
use coopgame::nucleolus::oracle;

/// 1 件の時間の上限 (秒)。
const TIME_LIMIT_SECONDS: u64 = 120;
/// 1 件の最大常駐セットサイズの上限 (MB)。
const RSS_LIMIT_MB: u64 = 6144;

/// (ゲーム, 人数)。
const CASES: &[(&str, &[usize])] = &[
    ("veto3", &[16, 18, 20, 22, 24, 26, 28, 30, 35, 40, 50, 60]),
    ("veto2", &[16, 18, 20, 22, 24, 26, 28, 30, 40, 50]),
    ("majority", &[9, 15, 21, 25, 31]),
    ("apex", &[10, 15, 20, 25, 30]),
    ("random10", &[10, 15, 20, 25, 30]),
    ("random100", &[10, 15, 20, 25, 30]),
    ("random100q23", &[10, 15, 20, 25, 30]),
    ("random100q90", &[10, 15, 20, 25, 30]),
];

/// ゲームと、分かっていれば仁。
fn game(kind: &str, n: usize) -> (WeightedVotingGame, Option<Vec<f64>>) {
    let big = |count: usize| {
        let mut weights = vec![1u64; n];
        weights[..count].fill(1000);
        let quota = 1000 * count as u64 + ((n - count) / 2 + 1) as u64;
        let mut expected = vec![0.0; n];
        expected[..count].fill(1.0 / count as f64);
        (
            WeightedVotingGame::new(weights, quota).unwrap(),
            Some(expected),
        )
    };
    let random = |max: u64, numerator: u64, denominator: u64| {
        let mut rng = SplitMix64::new(1000 * n as u64);
        let weights: Vec<u64> = (0..n).map(|_| rng.range(1, max)).collect();
        let quota = weights.iter().sum::<u64>() * numerator / denominator + 1;
        (WeightedVotingGame::new(weights, quota).unwrap(), None)
    };
    match kind {
        "veto3" => big(3),
        "veto2" => big(2),
        "majority" => (
            WeightedVotingGame::new(vec![1; n], (n / 2 + 1) as u64).unwrap(),
            Some(vec![1.0 / n as f64; n]),
        ),
        "apex" => {
            let mut weights = vec![1u64; n];
            weights[0] = n as u64 - 2;
            (
                WeightedVotingGame::new(weights, n as u64 - 1).unwrap(),
                None,
            )
        }
        "random10" => random(10, 1, 2),
        "random100" => random(100, 1, 2),
        "random100q23" => random(100, 2, 3),
        "random100q90" => random(100, 9, 10),
        other => panic!("未知のゲーム {other}"),
    }
}

/// 1 件を実行し、`秒 段の数 LP の回数 追加した行 差` を返す (差は仁が分からなければ `-`)。
fn run(kind: &str, n: usize) -> String {
    let (game, expected) = game(kind, n);
    let started = Instant::now();
    let result = oracle::nucleolus(&game).expect("仁を求められない");
    let seconds = started.elapsed().as_secs_f64();
    let error = expected.map_or("-".to_string(), |e| {
        let max = result
            .allocation()
            .iter()
            .zip(&e)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        format!("{max:e}")
    });
    format!(
        "{seconds:.3} {} {} {} {error}",
        result.levels().len(),
        result.lp_solves(),
        result.rows_added()
    )
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

/// `ps` で読んだ、プロセス `pid` の子プロセス (計測している本体) の常駐セットサイズ (MB)。
fn child_rss_mb(pid: u32) -> Option<u64> {
    let output = Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
        .ok()?;
    let child = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()?
        .to_string();
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &child])
        .output()
        .ok()?;
    let kilobytes: u64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    Some(kilobytes / 1024)
}

/// `/usr/bin/time` ごと子プロセスを止める。
fn stop(time: &mut Child) {
    if let Some(rss_pid) = Command::new("pgrep")
        .args(["-P", &time.id().to_string()])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
    {
        let _ = Command::new("kill").arg(rss_pid).status();
    }
    let _ = time.kill();
    let _ = time.wait();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, kind, n] = args.as_slice() {
        println!("{}", run(kind, n.parse().expect("人数")));
        return;
    }
    let flag = if cfg!(target_os = "macos") {
        "-l"
    } else {
        "-v"
    };
    let me = std::env::current_exe().expect("実行ファイルのパス");
    println!("kind,n,seconds,max_rss_mb,levels,lp_solves,rows_added,max_error,status");
    for (kind, sizes) in CASES {
        let mut stopped = false;
        for n in sizes.iter() {
            if stopped {
                println!("{kind},{n},,,,,,,skipped");
                continue;
            }
            let mut time = Command::new("/usr/bin/time")
                .arg(flag)
                .arg(&me)
                .args([kind.to_string(), n.to_string()])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("/usr/bin/time を実行できない");
            let started = Instant::now();
            let mut limit = None;
            while time.try_wait().expect("子プロセスの状態").is_none() {
                if started.elapsed() > Duration::from_secs(TIME_LIMIT_SECONDS) {
                    limit = Some("timeout");
                } else if child_rss_mb(time.id()).is_some_and(|rss| rss > RSS_LIMIT_MB) {
                    limit = Some("rss_limit");
                }
                if limit.is_some() {
                    stop(&mut time);
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            if let Some(status) = limit {
                println!("{kind},{n},,,,,,,{status}");
                stopped = true;
                continue;
            }
            let output = time.wait_with_output().expect("子プロセスの出力");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let rss = max_rss_mb(&stderr).expect("最大常駐セットサイズを読めない");
            match stdout.split_whitespace().collect::<Vec<_>>().as_slice() {
                [seconds, levels, solves, rows, error] => {
                    let error = if *error == "-" { "" } else { error };
                    println!("{kind},{n},{seconds},{rss:.0},{levels},{solves},{rows},{error},ok")
                }
                _ => panic!("{kind} {n} が失敗した: {stderr}"),
            }
        }
    }
}
