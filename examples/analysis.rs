//! 配分を調べる機能 (`analysis`) の例。
//!
//! 1. 配分の説明 (`analysis::explain`): どの提携が不満を持ち、コアに入るか。
//! 2. 値の不確かさと感度 (`analysis::uncertainty`): 値が ±10% 揺れたときの仁の分布と、
//!    仁を決めている提携の値に対する感度。
//! 3. 配分集合の図 (`analysis::plot`): コア・カーネル・仁・Shapley 値を描いた SVG。
//! 4. 反例の探索 (`analysis::search`): `generators` で作ったゲームから「優加法的だが凸でない」ゲームを探し、縮小する。
//!
//! ```bash
//! cargo run --example analysis
//! cargo run --example analysis -- figure.svg   # 図を figure.svg に書く
//! ```
//!
//! 引数でパスを渡した時だけ SVG をファイルに書く。省略時は SVG の大きさと図の要素の数だけを出す。
//! 関数に渡すプレイヤーの番号は 0 始まりで、提携は `format_coalition` で 1 始まりの番号で表示する。

use coopgame::analysis::explain::{self, ExplainOptions};
use coopgame::analysis::plot::{self, PlotOptions};
use coopgame::analysis::search::{self, SearchOptions};
use coopgame::analysis::uncertainty::{self, IntervalGame};
use coopgame::game::{binary_to_lex, format_coalition};
use coopgame::{ExplicitGame, generators, nucleolus, properties};

/// コマンドライン引数から SVG の出力先を読む (省略可)。
fn svg_path() -> Option<String> {
    std::env::args().nth(1)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 共通のサンプル入力の 3 人ゲーム (辞書式順)。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let names: Vec<String> = ["A", "B", "C"].iter().map(|s| s.to_string()).collect();

    explain_allocations(&game, &names)?;
    uncertainty_and_influence(&game, &names)?;
    draw_figure(&game, &names, svg_path())?;
    find_counterexample()?;
    Ok(())
}

/// 仁 (2, 4, 6) と別の案 (6, 3, 3) を説明する。
fn explain_allocations(game: &ExplicitGame, names: &[String]) -> coopgame::Result<()> {
    println!("== 配分の説明 ==");
    let nucleolus = nucleolus::nucleolus(game)?.allocation;
    assert!(close(&nucleolus, &[2.0, 4.0, 6.0], 1e-6));

    // 仁の説明。render は名前を渡すと提携を {A, B} の形で書く。
    let report = explain::report(game, &nucleolus, ExplainOptions::default())?;
    println!("-- 仁 --");
    print!("{}", explain::render(&report, Some(names)));
    assert!(report.in_core);

    // 案 (6, 3, 3) はコアに入らない。最も不満な提携は {B, C} (番号 [1, 2]) で、超過は 8 - 6 = 2。
    let proposal = [6.0, 3.0, 3.0];
    let report = explain::report(game, &proposal, ExplainOptions::default())?;
    println!("\n-- 案 (6, 3, 3) --");
    print!("{}", explain::render(&report, Some(names)));
    let worst: Vec<usize> = report.levels[0].coalitions[0].players().collect();
    assert!(!report.in_core);
    assert!((report.max_excess - 2.0).abs() < 1e-9);
    assert_eq!(worst, vec![1, 2]);
    Ok(())
}

/// 値が ±10% 揺れたときの仁の分布と、仁を決めている提携の値に対する感度。
fn uncertainty_and_influence(game: &ExplicitGame, names: &[String]) -> coopgame::Result<()> {
    // 仁を求める関数。標本の仁・感度・基準の仁 x のすべてで使う。
    let solve = |g: &ExplicitGame| Ok(nucleolus::nucleolus(g)?.allocation);
    println!("\n== 値が不確かなときの仁 ==");
    // 各提携の値を ±10% の区間で一様に引き、50 個のゲームの仁をまとめる。seed を固定すると結果も決まる。
    let interval = IntervalGame::around(game, 0.1)?;
    let distribution = uncertainty::interval_monte_carlo(&interval, 50, 1, solve)?;
    println!("名前\t平均\t標準偏差\t5%\t95%");
    for (i, name) in names.iter().enumerate() {
        println!(
            "{name}\t{:.4}\t{:.4}\t{:.4}\t{:.4}",
            distribution.mean[i],
            distribution.std[i],
            distribution.quantiles[0].1[i],
            distribution.quantiles[2].1[i]
        );
    }
    println!(
        "解けた標本 {}、解けなかった標本 {}",
        distribution.samples, distribution.failures
    );
    assert!(close(&distribution.mean, &[2.07, 4.00, 5.90], 0.01));
    assert_eq!((distribution.samples, distribution.failures), (50, 0));

    // 仁の超過の大きい上位 2 段の提携 (仁を決めている提携) と全体提携について、
    // 値を ±delta 動かしたときの dx / dv(S) を求める。
    println!("\n== 仁の感度 ==");
    let x = solve(game)?;
    let key = uncertainty::key_coalitions(game, &x, 2)?;
    // 中心差分なので、値を動かす向きで解の形が変わる点では左右の変化率の平均になる。
    let rows = uncertainty::influence(game, &key, 1e-4, solve)?;
    println!("提携\tdx/dv (A, B, C の順)");
    for row in &rows {
        let values: Vec<String> = row.sensitivity.iter().map(|v| format!("{v:+.4}")).collect();
        println!(
            "{}\t{}",
            format_coalition(row.coalition, Some(names)),
            values.join(" ")
        );
        // 仁は x(N) = v(N) を満たすので、全体提携の値を動かすと和が 1、それ以外は和が 0 だけ動く。
        let total: f64 = row.sensitivity.iter().sum();
        let expected = if row.coalition == game.grand() {
            1.0
        } else {
            0.0
        };
        assert!((total - expected).abs() < 1e-6);
    }
    assert!(rows.iter().any(|row| row.coalition == game.grand()));
    Ok(())
}

/// 配分集合の図。`path` があれば SVG を書く。
fn draw_figure(
    game: &ExplicitGame,
    names: &[String],
    path: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n== 配分集合の図 ==");
    // PlotOptions は #[non_exhaustive] なので既定値から作る。案 (6, 3, 3) も点として描く。
    let mut options = PlotOptions::default();
    options.names = Some(names.to_vec());
    options.points = vec![("案".to_string(), vec![6.0, 3.0, 3.0])];
    let figure = plot::imputation_figure(game, &options)?;

    println!("コアの頂点 {} 個:", figure.core_vertices.len());
    for vertex in &figure.core_vertices {
        println!("  {vertex:.4?}");
    }
    println!("カーネルの多面体 {} 個", figure.kernel_pieces.len());
    println!("描いた点:");
    for (label, x) in &figure.points {
        println!("  {label}: {x:.4?}");
    }
    // 点は仁・Shapley 値・追加の点の順。
    assert_eq!(figure.points.len(), 3);
    assert!(close(&figure.points[0].1, &[2.0, 4.0, 6.0], 1e-6));
    assert!(close(&figure.points[1].1, &[3.0, 4.0, 5.0], 1e-9));
    assert!(!figure.core_vertices.is_empty());

    match path {
        Some(path) => {
            std::fs::write(&path, &figure.svg)?;
            println!("SVG ({} バイト) を {path} に書いた", figure.svg.len());
        }
        None => println!(
            "SVG は {} バイト (引数にパスを渡すとファイルに書く)",
            figure.svg.len()
        ),
    }
    Ok(())
}

/// 優加法的だが凸でないゲームを探し、縮小する。
fn find_counterexample() -> coopgame::Result<()> {
    println!("\n== 反例の探索 ==");
    // SearchOptions は #[non_exhaustive] なので既定値から作る。
    let mut options = SearchOptions::default();
    options.attempts = 200;
    options.seed = 1;
    options.shrink_budget = 500;
    // 3 人か 4 人の優加法的なゲームを作る (generators::random_superadditive は優加法的な包を取る)。
    let found = search::search(
        |rng| {
            let n = 3 + (rng.next_u64() % 2) as usize;
            generators::random_superadditive(n, rng.next_u64())
        },
        |g| properties::is_superadditive(g) && !properties::is_convex(g),
        options,
    )?;
    let found = found.expect("試行回数 (attempts) の中で見つかる");
    let original = binary_to_lex(&found.original.values()[1..])?;
    let shrunk = binary_to_lex(&found.game.values()[1..])?;
    println!(
        "{} 番目の生成で発見 ({} 人)。縮小で {} 回変形し {} 人になった。",
        found.attempt + 1,
        found.original.players(),
        found.shrink_steps,
        found.game.players()
    );
    println!("元の値 (辞書式順): {original:.4?}");
    println!("縮小した値 (辞書式順): {shrunk:?}");
    // 縮小した後も、優加法的で凸でないことは保たれる。
    assert!(properties::is_superadditive(&found.game));
    assert!(!properties::is_convex(&found.game));
    // seed を固定しているので、縮小した結果も決まる。
    // v(N) + v({2}) = 2 < v({1, 2}) + v({2, 3}) = 3 なので凸でない。
    assert_eq!(shrunk, vec![0.0, 0.0, 0.0, 2.0, 0.0, 1.0, 2.0]);
    Ok(())
}

/// 2 つの列が要素ごとに `tolerance` 以内で等しいか。
fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
