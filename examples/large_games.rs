//! 30 人を超えるゲームの仁を、全提携を列挙せずに求める例。
//!
//! 全提携の表 (`ExplicitGame`) は 30 人までしか作れない。それより大きいゲームでは、
//! ゲームの構造を使う手法で仁を求める。33 人の破産ゲーム、40 人の重み付き投票ゲーム、
//! 16 人の誘導部分グラフゲームを扱う (凸ゲームの手法は人数に弱いので、グラフは 16 人にしている)。
//!
//! - `coopgame::nucleolus::oracle`: 超過の大きい提携を返せるゲームの仁 (`nucleolus`) と最小コア (`least_core`)
//! - `coopgame::game::oracle::OracleGame`: `excess_order` で、配分に対して超過 (不満) の大きい提携を上から順に見る
//! - `coopgame::auto::AutoNucleolus`: ゲームの型から、保証のある手法を選ぶ
//! - `coopgame::games`: 破産ゲーム (`bankruptcy`)・重み付き投票ゲーム (`voting`)・誘導部分グラフゲーム (`graph`)
//!
//! debug ビルドでは 2 秒ほどかかる (大半は 40 人の重み付き投票ゲームの仁)。
//!
//! ```bash
//! cargo run --example large_games
//! ```

use std::time::Instant;

use coopgame::auto::AutoNucleolus;
use coopgame::game::oracle::OracleGame;
use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::games::graph::InducedSubgraphGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::nucleolus::oracle;
use coopgame::{Domain, Guarantee, PlayerSet, Property, SetFunction};

fn main() -> coopgame::Result<()> {
    bankruptcy()?;
    voting()?;
    graph()?;
    Ok(())
}

/// 33 人の破産ゲーム。オラクル版の仁が、タルムード則と一致することを確かめる。
fn bankruptcy() -> coopgame::Result<()> {
    println!("== 33 人の破産ゲーム ==");
    // 共通のサンプル入力 (遺産 200、請求 100, 200, 300) を 11 組並べる。
    // プレイヤー 3k+1, 3k+2, 3k+3 の請求が 100, 200, 300 で、遺産は 200 × 11 = 2200。
    let copies = 11;
    let claims: Vec<f64> = (0..3 * copies)
        .map(|i| [100.0, 200.0, 300.0][i % 3])
        .collect();
    let game = BankruptcyGame::new(200.0 * copies as f64, claims)?;
    println!("人数 {}、遺産 {}", game.players(), game.estate());

    // オラクル版の仁。違反する提携を超過の大きい順に探す逐次 LP で、全提携 (2^33 個、約 86 億) を列挙しない。
    let started = Instant::now();
    let result = oracle::nucleolus(&game)?;
    println!(
        "オラクル版の仁: {:.2?}、LP {} 回、段数 {}",
        started.elapsed(),
        result.lp_solves,
        result.levels.len()
    );
    println!("先頭の 3 人: {:.4?}", &result.allocation[..3]);

    // 破産ゲームの仁はタルムード則に一致する (閉じた形で求まる)。
    let talmud = game.nucleolus()?;
    assert!(close(&result.allocation, &talmud, 1e-6 * game.estate()));
    // どの組も、3 人の共通のサンプル入力と同じ (50, 75, 75) を受け取る。
    for chunk in talmud.chunks(3) {
        assert!(close(chunk, &[50.0, 75.0, 75.0], 1e-9));
    }
    println!("タルムード則と一致する。どの組も (50, 75, 75)");

    // auto は破産ゲームの型からタルムード則を選ぶ。
    let auto = game.nucleolus_auto()?;
    println!("auto: 手法 {}、保証 {}", auto.method, auto.guarantee);
    assert_eq!(auto.method, "talmud-rule");
    assert_eq!(auto.guarantee, Guarantee::Proven(Property::Bankruptcy));

    // 仁に対して超過の大きい提携を、上から 3 つ見る。
    println!("仁で超過の大きい提携:");
    print_top_excess(&game, &result.allocation, 3);
    println!();
    Ok(())
}

/// 40 人の重み付き投票ゲーム。auto はオラクルによる制約生成を選ぶ。
fn voting() -> coopgame::Result<()> {
    println!("== 40 人の重み付き投票ゲーム ==");
    // 大政党 3 つ (各 200 票) と小政党 37 (各 1 票)。可決には 401 票が必要である。
    let mut weights = vec![200, 200, 200];
    weights.extend(std::iter::repeat_n(1, 37));
    let game = WeightedVotingGame::new(weights, 401)?;

    // 最小コア: 全ての提携の超過を epsilon 以下にする配分のうち、epsilon が最小のもの。
    let least = oracle::least_core(&game, Domain::Imputation)?;
    println!("最小コアの epsilon: {:.4}", least.epsilon);
    // epsilon が正なので、コアは空である。
    assert!((least.epsilon - 1.0 / 3.0).abs() < 1e-6);

    let started = Instant::now();
    let auto = game.nucleolus_auto()?;
    println!(
        "auto: {:.2?}、手法 {}、保証 {}",
        started.elapsed(),
        auto.method,
        auto.guarantee
    );
    assert_eq!(auto.method, "oracle-constraint-generation");
    assert_eq!(auto.guarantee, Guarantee::Exact);
    println!("大政党 3 つ: {:.4?}", &auto.allocation[..3]);
    let small = auto.allocation[3..].iter().copied().fold(0.0, f64::max);
    println!("小政党 37 の取り分の最大: {small:.4}");
    // 仁は大政党に 1/3 ずつ、小政党に 0 を配る。
    assert!(close(&auto.allocation[..3], &[1.0 / 3.0; 3], 1e-6));
    assert!(close(&auto.allocation[3..], &[0.0; 37], 1e-6));

    println!("仁で超過の大きい提携:");
    print_top_excess(&game, &auto.allocation, 3);
    println!();
    Ok(())
}

/// 16 頂点の誘導部分グラフゲーム。凸なので、auto は凸ゲームの手法を選ぶ。
fn graph() -> coopgame::Result<()> {
    println!("== 16 人の誘導部分グラフゲーム ==");
    // 提携の値は、提携の中で完結する辺の重みの和。辺の重みが非負なので凸ゲームである。
    // 頂点 u から u+1 に重み 1-9 の辺、u+7 に重み 2 の辺を張る (番号は n で割った余り)。
    // 16 頂点では、重複する辺も自己ループも出ない。
    let n = 16;
    let edges: Vec<(usize, usize, f64)> = (0..n)
        .flat_map(|u| {
            [
                (u, (u + 1) % n, 1.0 + (u % 9) as f64),
                (u, (u + 7) % n, 2.0),
            ]
        })
        .collect();
    let game = InducedSubgraphGame::new(n, edges)?;
    let total = game.value(&PlayerSet::full(n));
    println!("辺 {} 本、v(N) = {total}", game.edges().len());

    // 凸ゲームの手法は、劣モジュラ関数の最小化で最大余剰を求めるので、全提携を列挙しない。
    // 人数の 2 乗の組を何周も回るため、オラクル版の逐次 LP より人数に弱い (100 人では数十秒以上かかる)。
    let started = Instant::now();
    let auto = game.nucleolus_auto()?;
    println!(
        "auto: {:.2?}、手法 {}、保証 {}",
        started.elapsed(),
        auto.method,
        auto.guarantee
    );
    assert_eq!(auto.method, "convex-transfer");
    assert_eq!(auto.guarantee, Guarantee::Proven(Property::Convex));
    println!("仁: {:.2?}", clean(&auto.allocation));
    let sum: f64 = auto.allocation.iter().sum();
    assert!((sum - total).abs() < 1e-6 * total);
    Ok(())
}

/// 配分 `x` に対して超過の大きい提携を、上から `count` 個表示する。
fn print_top_excess<G: OracleGame>(game: &G, x: &[f64], count: usize) {
    for (coalition, excess) in game.excess_order(x).take(count) {
        // PlayerSet を表示する公開関数がないので、要素 (1 始まり) を並べる。
        println!(
            "  超過 {excess:>9.4}  {} 人  {:?}",
            coalition.len(),
            coalition.members().map(|i| i + 1).collect::<Vec<_>>()
        );
    }
}

/// 表示用に、ごく小さい値を 0 にする (`-0.0000` と出ないように)。
fn clean(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .map(|&v| if v.abs() < 1e-9 { 0.0 } else { v })
        .collect()
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
