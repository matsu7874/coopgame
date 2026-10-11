//! 同じ 3 人ゲームで、いろいろな解を並べて比べる。
//!
//! 3 人ゲーム `[0, 0, 0, 4, 6, 8, 12]` (辞書式順) で、次の解を求め、安定性の指標
//! (最大の不満・コアに入るか・離脱で得をする提携の数) を 1 つの表にする。
//!
//! 扱うモジュール:
//!
//! - `coopgame::values`: Shapley 値 (`shapley`)・Banzhaf 値 (`banzhaf`)・solidarity 値 (`solidarity`)
//! - `coopgame::compromise`: tau 値 (`tau_value`)・Gately 点 (`gately_point`)
//! - `coopgame::kernel`: カーネルの 1 点 (`kernel_point`) とカーネル全体 (`kernel_set`)
//! - `coopgame::nucleolus`: 仁 (`nucleolus`)
//! - `coopgame::analysis::explain`: 配分の比較 (`compare`・`render_comparison`)
//!
//! ```bash
//! cargo run --example compare_solutions
//! ```

use coopgame::analysis::explain;
use coopgame::kernel::{self, SetOptions, TransferOptions};
use coopgame::{Domain, ExplicitGame, compromise, nucleolus, values};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;

    // 限界貢献に基づく値。全提携を走査して求める。
    let shapley = values::shapley(&game);
    let banzhaf = values::banzhaf(&game);
    let solidarity = values::solidarity(&game);

    // 理想の支払い M_i = v(N) - v(N \ {i}) を基準にした妥協解。
    let tau = compromise::tau_value(&game)?;
    let gately = compromise::gately_point(&game)?;

    // 超過に基づく解。仁は逐次 LP、カーネルの 1 点は transfer scheme で求める。
    let nucleolus = nucleolus::nucleolus(&game)?.allocation;
    let kernel_point = kernel::kernel_point(
        &game,
        Domain::Imputation,
        None,
        TransferOptions::for_game(&game),
    )?;

    // カーネル全体を多面体の和集合として求める (6 人まで)。
    let kernel_set = kernel::kernel_set(&game, Domain::Imputation, SetOptions::for_game(&game))?;
    println!("カーネル全体: 多面体 {} 個", kernel_set.pieces.len());
    for (i, piece) in kernel_set.pieces.iter().enumerate() {
        println!(
            "  多面体 {}: 次元 {}、含まれる点 {:.4?}",
            i + 1,
            piece.dimension,
            piece.point
        );
    }
    println!();

    // 各解の値を確かめる (docs/examples.md の値)。
    assert!(close(&shapley, &[3.0, 4.0, 5.0], 1e-9));
    assert!(close(&banzhaf, &[3.5, 4.5, 5.5], 1e-9));
    assert!(close(&solidarity, &[11.0 / 3.0, 4.0, 13.0 / 3.0], 1e-9));
    assert!(close(&tau, &[2.5, 3.75, 5.75], 1e-9));
    assert!(close(&gately, &[8.0 / 3.0, 4.0, 16.0 / 3.0], 1e-9));
    assert!(kernel_point.converged);
    assert!(close(&kernel_point.allocation, &[2.0, 4.0, 6.0], 1e-6));
    assert!(close(&nucleolus, &[2.0, 4.0, 6.0], 1e-6));
    // カーネル全体は 0 次元の多面体 1 つ、つまり仁の 1 点である。
    assert_eq!(kernel_set.pieces.len(), 1);
    assert_eq!(kernel_set.pieces[0].dimension, 0);
    assert!(close(&kernel_set.pieces[0].point, &[2.0, 4.0, 6.0], 1e-6));

    // 名前と配分の組にして compare に渡す。
    let candidates: Vec<(String, Vec<f64>)> = [
        ("Shapley 値", shapley),
        ("Banzhaf 値", banzhaf),
        ("solidarity 値", solidarity),
        ("tau 値", tau),
        ("Gately 点", gately),
        ("カーネルの 1 点", kernel_point.allocation),
        ("仁", nucleolus),
    ]
    .into_iter()
    .map(|(name, x)| (name.to_string(), x))
    .collect();

    // 各配分の合計も並べる。Banzhaf 値だけ v(N) = 12 と一致しない。
    println!("各解の配分と合計:");
    for (name, x) in &candidates {
        println!("  {name}: {x:.4?} 合計 {:.4}", x.iter().sum::<f64>());
    }
    println!();

    // 安定性の指標で比べる。表の列はタブ区切りで、プレイヤーは 1 始まりで表示される。
    let rows = explain::compare(&game, &candidates)?;
    println!("安定性の比較:");
    print!("{}", explain::render_comparison(&rows, None));

    // 比較の結果を確かめる (docs/examples.md の値)。
    // 最大の不満 (空提携と全体提携を除く提携の超過の最大値)。
    // 仁は最大の不満を最小にするので、合計が v(N) の解の中で最も小さい。
    let expected_max_excess = [-1.0, -2.0, -1.0 / 3.0, -1.5, -4.0 / 3.0, -2.0, -2.0];
    for (row, expected) in rows.iter().zip(expected_max_excess) {
        assert!(
            (row.max_excess - expected).abs() < 1e-6,
            "{}: {}",
            row.name,
            row.max_excess
        );
        assert_eq!(row.blocking_coalitions, 0, "{}", row.name);
    }
    // Banzhaf 値は合計が 13.5 で効率性を満たさないので、コアに入らない。他はどれもコアに入る。
    for row in &rows {
        assert_eq!(row.in_core, row.name != "Banzhaf 値", "{}", row.name);
    }
    Ok(())
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
