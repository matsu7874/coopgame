//! 協力に制約がある場合の解の例。
//!
//! 1. 提携構造 (`partition`): プレイヤーがグループに分かれて協力する場合の
//!    Aumann–Drèze 値・提携構造つきの仁と、グループを事前の連合とみなす Owen 値・商ゲーム。
//! 2. 通信グラフ (`communication`): 辺でつながった人どうしだけが協力できる場合の Myerson 値。
//!    グラフの形 (一列・星形・完全グラフ) で値がどう変わるかを比べる。
//!
//! ```bash
//! cargo run --example cooperation_structures
//! ```
//!
//! 関数に渡すプレイヤーの番号は 0 始まりである。表示は `format_coalition` に合わせて 1 始まりにする。

use coopgame::communication;
use coopgame::game::{format_coalition, player_name};
use coopgame::partition::{self, CoalitionStructure};
use coopgame::{Coalition, Domain, ExplicitGame, values};

fn main() -> coopgame::Result<()> {
    // 共通のサンプル入力の 3 人ゲーム (辞書式順)。
    // v({1, 2}) = 4、v({1, 3}) = 6、v({2, 3}) = 8、v(N) = 12。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let shapley = values::shapley(&game);
    println!("3 人ゲーム [0, 0, 0, 4, 6, 8, 12] (辞書式順)");
    println!(
        "制約のない Shapley 値: {}",
        format_allocation(&shapley, None)
    );
    assert!(close(&shapley, &[3.0, 4.0, 5.0], 1e-9));

    structures(&game, &shapley)?;
    graphs(&game, &shapley)?;
    Ok(())
}

/// 提携構造 {1, 2} と {3} のもとでの解。
fn structures(game: &ExplicitGame, shapley: &[f64]) -> coopgame::Result<()> {
    println!("\n== 提携構造 ==");
    // ブロックは 0 始まりの番号で渡す。{1, 2} は [0, 1]、{3} は [2]。
    let blocks = CoalitionStructure::new(3, &[vec![0, 1], vec![2]])?;
    let listed: Vec<String> = blocks
        .blocks()
        .iter()
        .map(|b| format_coalition(*b, None))
        .collect();
    println!("グループ: {}", listed.join(" "));

    // Aumann–Drèze 値: 各グループの中の Shapley 値。{1, 2} は v({1, 2}) = 4 を等分し、{3} は単独の値 0。
    let aumann_dreze = partition::aumann_dreze(game, &blocks)?;
    println!(
        "Aumann–Drèze 値: {}",
        format_allocation(&aumann_dreze, None)
    );
    assert!(close(&aumann_dreze, &[2.0, 2.0, 0.0], 1e-6));

    // 提携構造つきの仁: 各グループで x(B) = v(B) を課した仁。
    // 超過 6 - x_1 と 8 - x_2 が等しく (どちらも 5) なる (1, 3, 0) になる。
    let nucleolus = partition::nucleolus(game, &blocks, Domain::Imputation)?;
    println!(
        "提携構造つきの仁: {} (保証 {})",
        format_allocation(&nucleolus.allocation, None),
        nucleolus.guarantee
    );
    assert!(close(&nucleolus.allocation, &[1.0, 3.0, 0.0], 1e-6));

    // Owen 値: グループを事前の連合とみなし、全員で v(N) = 12 を分ける。
    let owen = partition::owen(game, &blocks)?;
    println!("Owen 値: {}", format_allocation(&owen, None));
    assert!(close(&owen, &[3.5, 4.5, 4.0], 1e-6));

    // 商ゲーム: 連合を 1 人とみなした 2 人ゲーム。v_B({連合 1}) = 4、v_B({連合 2}) = 0、v_B(全体) = 12。
    // 商ゲームの Shapley 値は (8, 4) で、Owen 値の連合ごとの和 (3.5 + 4.5, 4) に一致する。
    let quotient = partition::quotient_game(game, &blocks)?;
    let quotient_shapley = values::shapley(&quotient);
    let union_sums: Vec<f64> = blocks
        .blocks()
        .iter()
        .map(|b| b.players().map(|i| owen[i]).sum())
        .collect();
    println!(
        "商ゲームの値 (ビット順): {:?}",
        &quotient.values()[1..] // 先頭は空提携の値 0
    );
    println!("商ゲームの Shapley 値: {quotient_shapley:.4?}");
    println!("Owen 値の連合ごとの和: {union_sums:.4?}");
    assert!(close(&quotient.values()[1..], &[4.0, 0.0, 12.0], 1e-9));
    assert!(close(&quotient_shapley, &[8.0, 4.0], 1e-6));
    assert!(close(&union_sums, &quotient_shapley, 1e-6));

    // 全員で 1 つのグループなら、Aumann–Drèze 値も Owen 値も Shapley 値に戻る。
    let grand = CoalitionStructure::grand(3);
    assert!(close(
        &partition::aumann_dreze(game, &grand)?,
        shapley,
        1e-9
    ));
    assert!(close(&partition::owen(game, &grand)?, shapley, 1e-9));
    println!(
        "全員で 1 つのグループなら、Aumann–Drèze 値も Owen 値も Shapley 値 (3, 4, 5) になる。"
    );
    Ok(())
}

/// 通信グラフの形と Myerson 値。
fn graphs(game: &ExplicitGame, shapley: &[f64]) -> coopgame::Result<()> {
    println!("\n== 通信グラフ (3 人ゲーム) ==");
    // 一列 1 - 2 - 3。辺は 0 始まりの番号の組で渡す。
    // 1 と 3 は直接つながっていないので、グラフ制限ゲームでは v({1, 3}) = 6 が 0 になる。
    let path = [(0, 1), (1, 2)];
    let restricted = communication::graph_restricted(game, &path)?;
    let pair = Coalition::from_players(&[0, 2]);
    println!(
        "一列 1 - 2 - 3 のとき v^g({}) = {} (元の値 {})",
        format_coalition(pair, None),
        restricted.value(pair),
        game.value(pair)
    );
    assert_eq!(restricted.value(pair), 0.0);

    // Myerson 値はグラフ制限ゲームの Shapley 値。間にいる 2 の取り分が 4 から 6 に増える。
    let on_path = communication::myerson(game, &path)?;
    println!(
        "Myerson 値 (一列 1 - 2 - 3): {}",
        format_allocation(&on_path, None)
    );
    assert!(close(&on_path, &[2.0, 6.0, 4.0], 1e-9));

    // 一列 2 - 1 - 3 (中央が 1)。v({2, 3}) = 8 が失われ、中央の 1 の取り分が増える。
    let centered_at_first = communication::myerson(game, &[(0, 1), (0, 2)])?;
    println!(
        "Myerson 値 (一列 2 - 1 - 3): {}",
        format_allocation(&centered_at_first, None)
    );
    assert!(close(
        &centered_at_first,
        &[17.0 / 3.0, 8.0 / 3.0, 11.0 / 3.0],
        1e-9
    ));

    // 完全グラフなら誰とでも協力できるので、Shapley 値に一致する。
    let complete = communication::myerson(game, &[(0, 1), (0, 2), (1, 2)])?;
    println!(
        "Myerson 値 (完全グラフ): {}",
        format_allocation(&complete, None)
    );
    assert!(close(&complete, shapley, 1e-9));

    // 4 人では一列と星形で形が異なる。
    // v(S) = |S| - 1 のゲームでは、グラフ制限ゲームが「S の中の辺の数」(木のとき) になり、
    // Myerson 値は各辺の値 1 を両端で等分したもの (次数 / 2) になる。
    println!("\n== 通信グラフ (4 人、v(S) = |S| - 1) ==");
    let chain = ExplicitGame::from_fn(4, |s| s.len() as f64 - 1.0)?;
    let line = communication::myerson(&chain, &[(0, 1), (1, 2), (2, 3)])?;
    let star = communication::myerson(&chain, &[(0, 1), (0, 2), (0, 3)])?;
    let all_edges: Vec<(usize, usize)> = (0..4)
        .flat_map(|a| (a + 1..4).map(move |b| (a, b)))
        .collect();
    let complete = communication::myerson(&chain, &all_edges)?;
    println!("一列 1 - 2 - 3 - 4: {}", format_allocation(&line, None));
    println!("星形 (中心 1): {}", format_allocation(&star, None));
    println!("完全グラフ: {}", format_allocation(&complete, None));
    assert!(close(&line, &[0.5, 1.0, 1.0, 0.5], 1e-9));
    assert!(close(&star, &[1.5, 0.5, 0.5, 0.5], 1e-9));
    assert!(close(&complete, &[0.75, 0.75, 0.75, 0.75], 1e-9));
    println!("つながりの多い人ほど取り分が大きい。完全グラフでは対称なので等分になる。");
    Ok(())
}

/// 配分を `名前: 値` の形で 1 行に書く。名前がなければ 1 始まりの番号を使う。
fn format_allocation(x: &[f64], names: Option<&[String]>) -> String {
    let parts: Vec<String> = x
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{}: {v:.4}", player_name(names, i)))
        .collect();
    parts.join(", ")
}

/// 2 つの列が要素ごとに `tolerance` 以内で等しいか。
fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
