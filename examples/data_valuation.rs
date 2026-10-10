//! サンプリングした提携による仁の近似と、データ評価への応用の実験。CSV を標準出力に出す。
//!
//! ```bash
//! # 近似の収束: 明示ゲームで、サンプル数を増やしたときの真のプレ仁との差
//! cargo run --release --example data_valuation -- convergence > convergence.csv
//! # データ評価: ラベルを反転したノイズ点を、各手法の価値で見つけられるか
//! cargo run --release --example data_valuation -- valuation 50,100 2000,10000 10 > valuation.csv
//! ```
//!
//! データ評価の設定:
//! - 訓練データ n 点、検証データ 200 点。2 クラスの 2 次元正規分布 (平均 (-1, 0) と (1, 0)、分散 1)。
//! - 訓練データのラベルを 10% 反転させてノイズ点にする。
//! - 提携 S の値 v(S) は、S の点だけを使う 1-NN 分類器の検証データでの正解率 (v(空集合) = 0)。
//! - 各手法に同じ回数だけ v を評価させ、価値の低い順にノイズ点が並ぶかを見る
//!   (AUC: ノイズ点の価値がきれいな点の価値より小さい確率、precision: 価値の低い k 点中のノイズ点の割合、k はノイズ点の数)。
//! - 1 点だけでも正解率は約 0.5 になり、1 人提携の値の和が全体の値を超えるので配分集合は空になる。
//!   そのため最小コアと仁はプレ版 (個人合理性を課さない) で求める。

use std::time::Instant;

use coopgame::game::{PlayerSet, SetFunction};
use coopgame::generators::{self, SplitMix64};
use coopgame::nucleolus::sampled::{self, SampledGame};
use coopgame::{Domain, nucleolus, values};

#[path = "../tests/common/mod.rs"]
mod common;
use common::max_abs_difference;

fn normal(rng: &mut SplitMix64) -> f64 {
    // Box-Muller
    let u = rng.next_f64().max(1e-300);
    let v = rng.next_f64();
    (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
}

struct Dataset {
    train: Vec<([f64; 2], bool)>,
    noisy: Vec<bool>,
    validation: Vec<([f64; 2], bool)>,
}

fn dataset(n: usize, seed: u64) -> Dataset {
    let mut rng = SplitMix64::new(seed);
    let point = |rng: &mut SplitMix64| {
        let label = rng.next_u64() & 1 == 1;
        let center = if label { 1.0 } else { -1.0 };
        ([center + normal(rng), normal(rng)], label)
    };
    let mut train: Vec<([f64; 2], bool)> = (0..n).map(|_| point(&mut rng)).collect();
    let validation = (0..200).map(|_| point(&mut rng)).collect();
    let mut noisy = vec![false; n];
    let flips = n / 10;
    let mut flipped = 0;
    while flipped < flips {
        let i = rng.range(0, (n - 1) as u64) as usize;
        if !noisy[i] {
            noisy[i] = true;
            train[i].1 = !train[i].1;
            flipped += 1;
        }
    }
    Dataset {
        train,
        noisy,
        validation,
    }
}

/// v(S) = S の点による 1-NN 分類器の検証データでの正解率。
struct NearestNeighbor<'a> {
    data: &'a Dataset,
    /// distances[v][t]: 検証点 v と訓練点 t の距離の 2 乗
    distances: Vec<Vec<f64>>,
}

impl<'a> NearestNeighbor<'a> {
    fn new(data: &'a Dataset) -> NearestNeighbor<'a> {
        let distances = data
            .validation
            .iter()
            .map(|(p, _)| {
                data.train
                    .iter()
                    .map(|(q, _)| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2))
                    .collect()
            })
            .collect();
        NearestNeighbor { data, distances }
    }
}

impl SetFunction for NearestNeighbor<'_> {
    fn players(&self) -> usize {
        self.data.train.len()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if coalition.is_empty() {
            return 0.0;
        }
        let members: Vec<usize> = coalition.members().collect();
        let correct = self
            .data
            .validation
            .iter()
            .zip(&self.distances)
            .filter(|((_, label), row)| {
                let nearest = members
                    .iter()
                    .min_by(|&&a, &&b| row[a].total_cmp(&row[b]))
                    .expect("空でない");
                self.data.train[*nearest].1 == *label
            })
            .count();
        correct as f64 / self.data.validation.len() as f64
    }
}

/// 価値が低いほどノイズ点らしいとしたときの AUC と precision@k。
fn detection(valuation: &[f64], noisy: &[bool]) -> (f64, f64) {
    let noisy_values: Vec<f64> = valuation
        .iter()
        .zip(noisy)
        .filter(|(_, n)| **n)
        .map(|(v, _)| *v)
        .collect();
    let clean_values: Vec<f64> = valuation
        .iter()
        .zip(noisy)
        .filter(|(_, n)| !**n)
        .map(|(v, _)| *v)
        .collect();
    let mut wins = 0.0;
    for a in &noisy_values {
        for b in &clean_values {
            wins += if a < b {
                1.0
            } else if a == b {
                0.5
            } else {
                0.0
            };
        }
    }
    let auc = wins / (noisy_values.len() * clean_values.len()) as f64;
    let mut order: Vec<usize> = (0..valuation.len()).collect();
    order.sort_by(|&a, &b| valuation[a].total_cmp(&valuation[b]));
    let k = noisy_values.len();
    let hits = order[..k].iter().filter(|&&i| noisy[i]).count();
    (auc, hits as f64 / k as f64)
}

fn valuation(sizes: &[usize], budgets: &[usize], seeds: u64) {
    println!("n,seed,budget,method,auc,precision,evaluations,seconds");
    for &n in sizes {
        for seed in 0..seeds {
            let data = dataset(n, seed);
            let game = NearestNeighbor::new(&data);
            // leave-one-out は予算によらず n + 1 回
            let started = Instant::now();
            let full = PlayerSet::full(n);
            let total = game.value(&full);
            let loo: Vec<f64> = (0..n)
                .map(|i| {
                    let mut without = full.clone();
                    without.remove(i);
                    total - game.value(&without)
                })
                .collect();
            let (auc, precision) = detection(&loo, &data.noisy);
            println!(
                "{n},{seed},{},loo,{auc:.4},{precision:.4},{},{:.3}",
                n + 1,
                n + 1,
                started.elapsed().as_secs_f64()
            );
            for &budget in budgets {
                let report = |method: &str,
                              values: &[f64],
                              evaluations: usize,
                              started: Instant| {
                    let (auc, precision) = detection(values, &data.noisy);
                    println!(
                        "{n},{seed},{budget},{method},{auc:.4},{precision:.4},{evaluations},{:.3}",
                        started.elapsed().as_secs_f64()
                    );
                };
                let started = Instant::now();
                let shapley = values::shapley_sampling(&game, budget / n, seed);
                report("shapley", &shapley.values, shapley.evaluations, started);

                let started = Instant::now();
                let banzhaf = values::banzhaf_sampling(&game, budget / (n + 1), seed);
                report("banzhaf", &banzhaf.values, banzhaf.evaluations, started);

                let pairs = budget.saturating_sub(2 * n) / 2;
                let started = Instant::now();
                match sampled::least_core(&game, pairs, seed, Domain::Preimputation) {
                    Ok((least, evaluations)) => {
                        report("least_core", &least.allocation, evaluations, started)
                    }
                    Err(err) => eprintln!("least_core n={n} seed={seed} budget={budget}: {err}"),
                }
                let started = Instant::now();
                match sampled::nucleolus(&game, pairs, seed, Domain::Preimputation) {
                    Ok(result) => {
                        report("nucleolus", &result.allocation, result.evaluations, started)
                    }
                    Err(err) => eprintln!("nucleolus n={n} seed={seed} budget={budget}: {err}"),
                }
            }
        }
    }
}

/// 明示ゲームで、サンプル数を増やしたときの近似と真のプレ仁の差 (最大絶対誤差を max|v| で割った値)。
fn convergence() {
    println!("type,n,seed,pairs,evaluations,relative_error");
    for kind in [1, 2, 4] {
        for n in [10, 12] {
            for seed in 0..5 {
                let game = generators::bnf(kind, n, seed).unwrap();
                let exact = nucleolus::prenucleolus(&game).unwrap().allocation;
                let scale = game.max_abs_value().max(1.0);
                for pairs in [25, 50, 100, 200, 400, 800, 1600] {
                    let sampled = SampledGame::sample(&game, pairs, 100 + seed).unwrap();
                    let approx =
                        sampled::nucleolus(&game, pairs, 100 + seed, Domain::Preimputation)
                            .unwrap();
                    let error = max_abs_difference(&approx.allocation, &exact) / scale;
                    println!(
                        "{kind},{n},{seed},{pairs},{},{error:.6}",
                        sampled.evaluations()
                    );
                }
            }
        }
    }
}

fn parse_list(text: Option<&String>, default: &str) -> Vec<usize> {
    text.map_or(default, |s| s.as_str())
        .split(',')
        .map(|s| s.parse().expect("整数のリスト"))
        .collect()
}

/// 実行する実験 (コマンドライン引数で選ぶ)。
enum Command {
    Convergence,
    Valuation {
        sizes: Vec<usize>,
        budgets: Vec<usize>,
        seeds: u64,
    },
}

/// `convergence` か `valuation [n,...] [予算,...] [seed 数]` を読む。どちらでもなければ `None`。
fn parse_args() -> Option<Command> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("convergence") => Some(Command::Convergence),
        Some("valuation") => Some(Command::Valuation {
            sizes: parse_list(args.get(1), "50"),
            budgets: parse_list(args.get(2), "2000"),
            seeds: args.get(3).map_or(3, |s| s.parse().expect("seed 数")),
        }),
        _ => None,
    }
}

fn main() {
    match parse_args() {
        Some(Command::Convergence) => convergence(),
        Some(Command::Valuation {
            sizes,
            budgets,
            seeds,
        }) => valuation(&sizes, &budgets, seeds),
        None => {
            eprintln!("使い方: data_valuation convergence | valuation <n,...> <予算,...> <seed 数>")
        }
    }
}
