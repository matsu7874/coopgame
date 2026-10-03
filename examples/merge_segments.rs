//! プレカーネルの分析 (`prekernel_study` の出力) で多面体が 2 つ以上だったゲームについて、
//! 同じ直線上でつながる線分をまとめた後の多面体の数を調べ、CSV を標準出力に出す。
//!
//! ```bash
//! cargo run --release --example merge_segments -- study-n3-4.csv study-n5.csv
//! ```
//!
//! ゲームは `prekernel_study` と同じ生成器と seed で作り直す。
//! あわせて、頂点を共有する多面体どうしをつないだときの連結成分の数 (`components`) も出す。

use std::fs;

use coopgame::generators;
use coopgame::kernel_set::{KernelSet, SetOptions, kernel_set};
use coopgame::{Domain, ExplicitGame};

fn generate(class: &str, n: usize, seed: u64) -> ExplicitGame {
    generators::by_class(class, n, seed).expect("生成できる")
}

/// 頂点を共有する多面体をつないだときの連結成分の数。頂点のない多面体は 1 つの成分と数える。
fn components(set: &KernelSet, tolerance: f64) -> usize {
    let count = set.pieces.len();
    let mut parent: Vec<usize> = (0..count).collect();
    fn root(parent: &mut [usize], mut k: usize) -> usize {
        while parent[k] != k {
            k = parent[k];
        }
        k
    }
    for a in 0..count {
        for b in a + 1..count {
            let (Some(first), Some(second)) = (&set.pieces[a].vertices, &set.pieces[b].vertices)
            else {
                continue;
            };
            let shared = first.iter().any(|p| {
                second
                    .iter()
                    .any(|q| p.iter().zip(q).all(|(x, y)| (x - y).abs() <= tolerance))
            });
            if shared {
                let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
                parent[ra] = rb;
            }
        }
    }
    (0..count).filter(|&k| root(&mut parent, k) == k).count()
}

fn main() {
    println!("class,n,seed,domain,pieces,merged_pieces,merged_dimensions,components");
    for path in std::env::args().skip(1) {
        let text = fs::read_to_string(&path).expect("CSV を読める");
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().expect("見出し").split(',').collect();
        let column = |name: &str| header.iter().position(|h| *h == name).expect("列がある");
        let (class, n, seed) = (column("class"), column("n"), column("seed"));
        for line in lines {
            let fields: Vec<&str> = line.split(',').collect();
            for (domain, label, pieces) in [
                (Domain::Preimputation, "prekernel", column("pre_pieces")),
                (Domain::Imputation, "kernel", column("ker_pieces")),
            ] {
                let Ok(count) = fields[pieces].parse::<usize>() else {
                    continue;
                };
                if count < 2 {
                    continue;
                }
                let game = generate(
                    fields[class],
                    fields[n].parse().expect("人数"),
                    fields[seed].parse().expect("seed"),
                );
                let options = SetOptions::for_game(&game);
                let set = kernel_set(&game, domain, options).expect("分析で計算できたゲーム");
                assert_eq!(set.pieces.len(), count, "分析の CSV と多面体の数が一致する");
                let merged = set.merge_collinear_segments(10.0 * options.tolerance);
                let dimensions: Vec<String> = merged
                    .pieces
                    .iter()
                    .map(|piece| piece.dimension.to_string())
                    .collect();
                println!(
                    "{},{},{},{label},{count},{},{},{}",
                    fields[class],
                    fields[n],
                    fields[seed],
                    merged.pieces.len(),
                    dimensions.join(" "),
                    components(&merged, 10.0 * options.tolerance)
                );
            }
        }
    }
}
