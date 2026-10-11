//! 重み付き投票ゲームの投票力指数を求め、1 つの表で比べる。
//!
//! 5 党の重みが 35, 20, 15, 15, 15 で、可決に 51 票が必要な投票ゲーム `[51; 35, 20, 15, 15, 15]` を使う。
//!
//! 扱うモジュール:
//!
//! - `coopgame::games::voting`: 重み付き投票ゲーム (`WeightedVotingGame`)。
//!   `ExplicitGame::tabulate` で全提携の表に直す
//! - `coopgame::values`: Shapley–Shubik 指数 (`shapley`)、Banzhaf 指数 (`banzhaf` と `normalize`)
//! - `coopgame::power`: 単純ゲーム (`SimpleGame`) の Johnston 指数・Deegan–Packel 指数・
//!   Public Good 指数・Coleman の阻止力と発議力と集団の行動力
//!
//! ```bash
//! cargo run --example voting_power
//! ```

use coopgame::ExplicitGame;
use coopgame::game::format_coalition;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::power::SimpleGame;
use coopgame::values;

fn main() -> coopgame::Result<()> {
    // 重み付き投票ゲームは提携の値を重みの和から求めるオラクルである。
    // 全提携 (2^5 = 32 個) の表に直して、明示ゲームの関数に渡す。
    let voting = WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?;
    let game = ExplicitGame::tabulate(&voting)?;

    // 勝利提携・最小勝利提携・決定票 (抜けると否決になる勝利提携) の数を集計する。
    let simple = SimpleGame::new(&game)?;
    let n = simple.players();
    println!(
        "投票ゲーム [{}; {}]",
        voting.quota(),
        voting
            .weights()
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "勝利提携 {} 個 (全 {} 個中)",
        simple.winning.len(),
        1usize << n
    );
    let minimal: Vec<String> = simple
        .minimal_winning
        .iter()
        .map(|&c| format_coalition(c, None))
        .collect();
    println!("最小勝利提携 {} 個: {}", minimal.len(), minimal.join(" "));
    println!("各党の決定票の数: {:?}", simple.swings);
    println!();

    // 指数を求める。Shapley–Shubik 指数は Shapley 値、Banzhaf 指数は Banzhaf 値を合計 1 に正規化したもの。
    let shapley_shubik = values::shapley(&game);
    let banzhaf_raw = values::banzhaf(&game);
    let banzhaf = values::normalize(&banzhaf_raw);
    let johnston = simple.johnston()?;
    let deegan_packel = simple.deegan_packel()?;
    let public_good = simple.public_good()?;
    let prevent = simple.coleman_prevent()?;
    let initiate = simple.coleman_initiative()?;
    let collectivity = simple.coleman_collectivity();

    // 行が党、列が指数の表にする。党は 1 始まりの番号と重みで表示する。
    let columns: [(&str, &[f64]); 8] = [
        ("Shapley-Shubik", &shapley_shubik),
        ("Banzhaf (値)", &banzhaf_raw),
        ("Banzhaf (正規化)", &banzhaf),
        ("Johnston", &johnston),
        ("Deegan-Packel", &deegan_packel),
        ("Public Good", &public_good),
        ("Coleman 阻止力", &prevent),
        ("Coleman 発議力", &initiate),
    ];
    let header: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
    println!("党\t重み\t{}", header.join("\t"));
    for player in 0..n {
        let cells: Vec<String> = columns
            .iter()
            .map(|(_, index)| format!("{:.4}", index[player]))
            .collect();
        println!(
            "{}\t{}\t{}",
            player + 1,
            voting.weights()[player],
            cells.join("\t")
        );
    }
    println!();
    println!("Coleman の集団の行動力 (勝利提携の割合): {collectivity:.4}");

    // 結果を確かめる (docs/examples.md の値)。
    assert_eq!(simple.winning.len(), 13);
    assert_eq!(simple.minimal_winning.len(), 5);
    assert_eq!(simple.swings, vec![11, 5, 3, 3, 3]);
    let seventh = 7.0 / 60.0;
    assert!(close(
        &shapley_shubik,
        &[0.45, 0.2, seventh, seventh, seventh],
        1e-9
    ));
    assert!(close(
        &banzhaf_raw,
        &[0.6875, 0.3125, 0.1875, 0.1875, 0.1875],
        1e-9
    ));
    assert!(close(&banzhaf, &[0.44, 0.2, 0.12, 0.12, 0.12], 1e-9));
    let j = 11.0 / 144.0;
    assert!(close(&johnston, &[7.0 / 12.0, 0.1875, j, j, j], 1e-9));
    let d = 11.0 / 60.0;
    assert!(close(&deegan_packel, &[0.3, 0.15, d, d, d], 1e-9));
    assert!(close(
        &public_good,
        &[4.0 / 15.0, 2.0 / 15.0, 0.2, 0.2, 0.2],
        1e-9
    ));
    let p = |k: f64| k / 13.0;
    assert!(close(
        &prevent,
        &[p(11.0), p(5.0), p(3.0), p(3.0), p(3.0)],
        1e-9
    ));
    let q = |k: f64| k / 19.0;
    assert!(close(
        &initiate,
        &[q(11.0), q(5.0), q(3.0), q(3.0), q(3.0)],
        1e-9
    ));
    assert!((collectivity - 13.0 / 32.0).abs() < 1e-12);
    // 重み 15 の党の Deegan–Packel 指数・Public Good 指数は、重み 20 の党より大きい。
    assert!(deegan_packel[2] > deegan_packel[1]);
    assert!(public_good[2] > public_good[1]);
    Ok(())
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
