//! 構造のあるゲームを、その構造を使う専用の解法で解く。
//!
//! 全提携の表を作らずに、ゲームの構造から閉じた形の式や LP で配分を求める。
//! 入力は docs/examples.md の「構造のあるゲーム」の各節と同じ小さい例である。
//!
//! 扱うモジュール:
//!
//! - `coopgame::games::airport`: 空港ゲーム (`AirportGame` の `shapley_costs`・`nucleolus_costs`)
//! - `coopgame::games::spanning_tree`: 最小全域木ゲーム (`SpanningTreeGame` の `bird_rule`・`nucleolus_costs`)
//! - `coopgame::games::production`: 線形生産ゲーム (`LinearProductionGame` の `shadow_prices`・`owen_allocation`)
//! - `coopgame::games::cost`: 費用ゲーム (`CostGame` の `nucleolus`・`shapley`)
//! - `coopgame::games::bankruptcy`: 破産問題の配分規則 (`talmud_rule`・`constrained_equal_awards`・
//!   `constrained_equal_losses`)。`f64` と有理数 (`coopgame::game::exact::Rational`) の両方で求める
//!
//! ```bash
//! cargo run --example structured_games
//! ```

use coopgame::game::exact::{Rational, format_rational};
use coopgame::games::airport::AirportGame;
use coopgame::games::bankruptcy::{self, BankruptcyGame};
use coopgame::games::cost::CostGame;
use coopgame::games::production::LinearProductionGame;
use coopgame::games::spanning_tree::SpanningTreeGame;
use coopgame::{ExplicitGame, PlayerSet, SetFunction, nucleolus};

fn sum(x: &[f64]) -> f64 {
    x.iter().sum()
}

/// 有理数の列を `50, 75, 75` や `200/3, ...` の形で書く。
fn format_exact(values: &[Rational]) -> String {
    values
        .iter()
        .map(format_rational)
        .collect::<Vec<_>>()
        .join(", ")
}

fn airport() -> coopgame::Result<()> {
    // 滑走路に必要な費用が 3, 6, 12 の 3 機。提携の費用は、提携の中で最も大きい費用である。
    println!("== 空港ゲーム (必要な費用 3, 6, 12)");
    let game = AirportGame::new(vec![3.0, 6.0, 12.0])?;
    let total = game.cost(&PlayerSet::full(3));
    // Shapley 値は Littlechild–Owen の式 (区間ごとの費用を、その区間を使う機で等分する)。
    let shapley = game.shapley_costs();
    // 仁は、節約ゲームが凸であることを使う凸ゲームの手法で求める。
    let nucleolus = game.nucleolus_costs()?;
    println!("  全体の費用: {total:.4}");
    println!("  Shapley 値: {shapley:.4?}");
    println!("  仁:         {nucleolus:.4?}");

    assert!((total - 12.0).abs() < 1e-9);
    assert!(close(&shapley, &[1.0, 2.5, 8.5], 1e-6));
    assert!(close(&nucleolus, &[1.5, 2.25, 8.25], 1e-6));
    assert!((sum(&shapley) - total).abs() < 1e-9 && (sum(&nucleolus) - total).abs() < 1e-6);
    Ok(())
}

fn spanning_tree() -> coopgame::Result<()> {
    // 頂点 0 が供給元、頂点 1, 2, 3 がプレイヤー 1, 2, 3 (関数の番号は 0, 1, 2)。
    // 行列の (i, j) 成分が頂点 i と j を結ぶ費用である。
    println!("== 最小全域木ゲーム (供給元 0 と 3 人)");
    let game = SpanningTreeGame::new(vec![
        vec![0.0, 4.0, 5.0, 7.0],
        vec![4.0, 0.0, 2.0, 6.0],
        vec![5.0, 2.0, 0.0, 3.0],
        vec![7.0, 6.0, 3.0, 0.0],
    ])?;
    let total = game.cost(&PlayerSet::full(3));
    // Bird 規則: 最小全域木で、各人が自分から供給元側への辺の費用を払う。コアに属する。
    let bird = game.bird_rule();
    // 仁は全提携の表で逐次 LP を解いて求める。
    let nucleolus = game.nucleolus_costs()?;
    println!("  全員をつなぐ費用: {total:.4}");
    println!("  Bird 規則: {bird:.4?}");
    println!("  仁:        {nucleolus:.4?}");

    assert!((total - 9.0).abs() < 1e-9);
    assert!(close(&bird, &[4.0, 2.0, 3.0], 1e-6));
    assert!(close(&nucleolus, &[2.5, 1.5, 5.0], 1e-6));
    Ok(())
}

fn production() -> coopgame::Result<()> {
    // 2 種類の資源から 2 種類の製品を作る。製品 1 単位に使う資源は製品 1 が (1, 2)、製品 2 が (2, 1)。
    // 価格は 3 と 4。3 人が資源 (2, 1)、(1, 2)、(3, 3) を持ち寄る。
    println!("== 線形生産ゲーム (資源 2 種類、製品 2 種類、3 人)");
    let game = LinearProductionGame::new(
        vec![vec![1.0, 2.0], vec![2.0, 1.0]], // 資源 x 製品
        vec![3.0, 4.0],
        vec![vec![2.0, 1.0], vec![1.0, 2.0], vec![3.0, 3.0]], // 人 x 資源
    )?;
    // 全員の資源 (6, 6) で作れる売上の最大。
    let value = game.value(&PlayerSet::full(3));
    // 双対 LP の最適解 (資源 1 単位の影の価格) で各人の資源を評価したものが Owen 配分である。
    let prices = game.shadow_prices()?;
    let owen = game.owen_allocation()?;
    println!("  全員での売上の最大: {value:.4}");
    println!("  影の価格: {prices:.4?}");
    println!("  Owen 配分: {owen:.4?} 合計 {:.4}", sum(&owen));

    assert!((value - 14.0).abs() < 1e-6);
    assert!(close(&prices, &[5.0 / 3.0, 2.0 / 3.0], 1e-6));
    assert!(close(&owen, &[4.0, 3.0, 7.0], 1e-6));
    assert!((sum(&owen) - value).abs() < 1e-6);
    Ok(())
}

fn cost() -> coopgame::Result<()> {
    // 共同配送の費用 c(S)。単独で 6, 6, 8、2 人で 9 ({1, 2})・11 ({1, 3})・12 ({2, 3})、3 人で 15。
    println!("== 費用ゲーム (共同配送)");
    let game = CostGame::new(ExplicitGame::from_lex(&[
        6.0, 6.0, 8.0, 9.0, 11.0, 12.0, 15.0,
    ])?);
    let standalone = game.standalone();
    // 仁は、節約ゲーム (単独の費用の合計 - 提携の費用) の仁を求めて費用に戻す。
    let nucleolus = game.nucleolus()?;
    let shapley = game.shapley();
    println!("  単独の費用: {standalone:.4?}");
    println!("  仁:         {nucleolus:.4?}");
    println!("  Shapley 値: {shapley:.4?}");

    assert!(close(
        &nucleolus,
        &[11.0 / 3.0, 14.0 / 3.0, 20.0 / 3.0],
        1e-6
    ));
    assert!(close(&shapley, &[4.0, 4.5, 6.5], 1e-6));
    // どちらも合計は全員の費用 15 で、各人は単独の費用より少なく払う。
    for x in [&nucleolus, &shapley] {
        assert!((sum(x) - 15.0).abs() < 1e-6);
        assert!(
            x.iter()
                .zip(&standalone)
                .all(|(share, alone)| share < alone)
        );
    }
    Ok(())
}

fn bankruptcy() -> coopgame::Result<()> {
    // 遺産 200 を、請求 100, 200, 300 の 3 人で分ける。
    println!("== 破産問題 (遺産 200、請求 100, 200, 300)");
    let estate = 200.0;
    let claims = [100.0, 200.0, 300.0];
    let talmud = bankruptcy::talmud_rule(estate, &claims)?;
    let cea = bankruptcy::constrained_equal_awards(estate, &claims)?;
    let cel = bankruptcy::constrained_equal_losses(estate, &claims)?;
    println!("  f64:");
    println!("    タルムード則: {talmud:.4?}");
    println!("    CEA:          {cea:.4?}");
    println!("    CEL:          {cel:.4?}");

    // 同じ規則を有理数で計算する。200 / 3 のような値も誤差なく表せる。
    let int = |k: i64| Rational::from_integer(k.into());
    let exact_estate = int(200);
    let exact_claims = [int(100), int(200), int(300)];
    let exact_talmud = bankruptcy::talmud_rule(exact_estate.clone(), &exact_claims)?;
    let exact_cea = bankruptcy::constrained_equal_awards(exact_estate.clone(), &exact_claims)?;
    let exact_cel = bankruptcy::constrained_equal_losses(exact_estate, &exact_claims)?;
    println!("  有理数:");
    println!("    タルムード則: [{}]", format_exact(&exact_talmud));
    println!("    CEA:          [{}]", format_exact(&exact_cea));
    println!("    CEL:          [{}]", format_exact(&exact_cel));

    // タルムード則は破産ゲーム v(S) = max(0, 遺産 - S の外の請求の合計) の仁に一致する。
    // 全提携の表に直して逐次 LP で求めた仁と比べる。
    let game = BankruptcyGame::new(estate, claims.to_vec())?;
    let lp = nucleolus::nucleolus(&ExplicitGame::tabulate(&game)?)?.allocation;
    println!("  破産ゲームの仁 (逐次 LP): {lp:.4?}");

    assert!(close(&talmud, &[50.0, 75.0, 75.0], 1e-6));
    assert!(close(&cea, &[200.0 / 3.0; 3], 1e-6));
    assert!(close(&cel, &[0.0, 50.0, 150.0], 1e-6));
    assert_eq!(exact_talmud, [int(50), int(75), int(75)]);
    let third = Rational::new(200.into(), 3.into());
    assert_eq!(exact_cea, [third.clone(), third.clone(), third]);
    assert_eq!(exact_cel, [int(0), int(50), int(150)]);
    assert!(close(&lp, &talmud, 1e-6));
    Ok(())
}

fn main() -> coopgame::Result<()> {
    airport()?;
    println!();
    spanning_tree()?;
    println!();
    production()?;
    println!();
    cost()?;
    println!();
    bankruptcy()
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
