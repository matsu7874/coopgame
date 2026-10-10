//! 同じゲームの仁を、手法を変えて求め、結果と保証を比べる例。
//!
//! - `coopgame::nucleolus`: 浮動小数点の逐次 LP による仁 (`nucleolus`)・プレ仁 (`prenucleolus`)・
//!   最小コア (`least_core`)・コアが空でないかの判定 (`has_nonempty_core`)
//! - `coopgame::nucleolus::exact`: 有理数の逐次 LP。許容誤差を使わない (`coopgame::game::exact::ExactGame`)
//! - `coopgame::nucleolus::convex`: 凸ゲームの手法。凸と確認したゲーム (`properties::ConvexChecked`) と、
//!   凸と宣言しただけのゲーム (`properties::Assume::convex`) で、結果の型が変わる
//! - `coopgame::nucleolus::variants`: 不満の定義を変えた仁 (per capita 仁・比例仁・modiclus・disruption nucleolus)
//!
//! ```bash
//! cargo run --example nucleolus_methods
//! ```

use coopgame::game::exact::{ExactGame, format_rational};
use coopgame::nucleolus::{self, convex, exact, variants};
use coopgame::properties::{self, Assume, ConvexChecked};
use coopgame::solution::{Guarantee, Property, Solution, Unverified};
use coopgame::verify::{Verified, VerifyOptions};
use coopgame::{Domain, ExplicitGame, generators};

fn main() -> coopgame::Result<()> {
    // 共通の 3 人ゲーム (辞書式順)。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    lp_methods(&game)?;
    exact_method(&game)?;
    variant_methods(&game)?;
    convex_methods(&game)?;
    Ok(())
}

/// 浮動小数点の逐次 LP による仁・プレ仁・最小コア。
fn lp_methods(game: &ExplicitGame) -> coopgame::Result<()> {
    println!("== 逐次 LP (浮動小数点) ==");
    let result = nucleolus::nucleolus(game)?;
    println!(
        "仁:       {:.4?} 保証 {}",
        result.allocation, result.guarantee
    );
    assert!(close(&result.allocation, &[2.0, 4.0, 6.0], 1e-6));
    assert_eq!(result.guarantee, Guarantee::Exact);

    // プレ仁は、個人合理性 (x_i >= v({i})) を課さずに求める。
    // このゲームではコアが空でないので、仁と一致する。
    let pre = nucleolus::prenucleolus(game)?;
    println!("プレ仁:   {:.4?}", pre.allocation);
    assert!(close(&pre.allocation, &[2.0, 4.0, 6.0], 1e-6));

    // 最小コア: 全ての提携の超過を epsilon 以下にする配分のうち、epsilon が最小のもの。
    let least = nucleolus::least_core(game, Domain::Imputation)?;
    println!(
        "最小コア: epsilon {:.4}、配分 {:.4?}",
        least.epsilon, least.allocation
    );
    assert!((least.epsilon + 2.0).abs() < 1e-6);

    // epsilon が 0 以下であることは、コアが空でないことと同じである。
    let nonempty = nucleolus::has_nonempty_core(game)?;
    println!("コアが空でない: {nonempty}");
    assert!(nonempty);
    println!();
    Ok(())
}

/// 有理数の逐次 LP。値を有理数のまま持ち、許容誤差を使わずに解く。
fn exact_method(game: &ExplicitGame) -> coopgame::Result<()> {
    println!("== 逐次 LP (有理数) ==");
    // 浮動小数点の値を、2 進数の値どおりに有理数にする (整数の値は誤差なく移る)。
    let game = ExactGame::from_explicit(game)?;
    let result = exact::nucleolus(&game, Domain::Imputation)?;
    let allocation = result
        .allocation
        .iter()
        .map(format_rational)
        .collect::<Vec<_>>()
        .join(", ");
    let levels = result
        .levels
        .iter()
        .map(format_rational)
        .collect::<Vec<_>>()
        .join(", ");
    println!("仁: {allocation}");
    println!("各段の最大超過: {levels}");
    assert_eq!(allocation, "2, 4, 6");
    assert_eq!(
        result.levels.first().map(format_rational).as_deref(),
        Some("-2")
    );
    println!();
    Ok(())
}

/// 不満の定義を変えた仁。どれも全提携の表を受け取り、逐次 LP で解く。
fn variant_methods(game: &ExplicitGame) -> coopgame::Result<()> {
    println!("== 不満の定義を変えた仁 ==");
    // per capita 仁: 超過を提携の人数で割る。
    let per_capita = variants::per_capita_nucleolus(game, Domain::Imputation)?;
    // 比例仁: 超過を提携の値で割る。
    let proportional = variants::proportional_nucleolus(game, Domain::Imputation)?;
    // modiclus: 提携どうしの超過の差を辞書式に最小化する。
    let modiclus = variants::modiclus(game)?;
    // disruption nucleolus: 提携の抜ける傾向を辞書式に小さくする。
    let disruption = variants::disruption_nucleolus(game)?;
    println!("per capita 仁:        {:.4?}", per_capita.allocation);
    println!("比例仁:               {:.4?}", proportional.allocation);
    println!("modiclus:             {:.4?}", modiclus.allocation);
    println!("disruption nucleolus: {:.4?}", disruption.allocation);
    assert!(close(&per_capita.allocation, &[2.0, 4.0, 6.0], 1e-6));
    assert!(close(
        &proportional.allocation,
        &[4.0 / 3.0, 4.0, 20.0 / 3.0],
        1e-6
    ));
    assert!(close(
        &modiclus.allocation,
        &[8.0 / 3.0, 14.0 / 3.0, 14.0 / 3.0],
        1e-6
    ));
    assert!(close(
        &disruption.allocation,
        &[8.0 / 3.0, 4.0, 16.0 / 3.0],
        1e-6
    ));
    println!();
    Ok(())
}

/// 凸ゲームの手法。性質の根拠によって、結果の型と保証が変わる。
fn convex_methods(shared: &ExplicitGame) -> coopgame::Result<()> {
    println!("== 凸ゲームの手法 ==");

    // 1. 凸と確認したゲーム。破産ゲーム (遺産 200、請求 100, 200, 300) は凸である。
    //    ConvexChecked::new は全提携で凸性を調べ、凸なら包んで返す。
    //    結果は Solution で、保証は proven(convex) になる。
    let bankruptcy = generators::bankruptcy(200.0, &[100.0, 200.0, 300.0])?;
    let checked = ConvexChecked::new(bankruptcy).expect("破産ゲームは凸");
    let solution: Solution = convex::nucleolus(&checked)?;
    println!(
        "破産ゲーム (ConvexChecked): {:.4?} 保証 {}",
        solution.allocation, solution.guarantee
    );
    assert!(close(&solution.allocation, &[50.0, 75.0, 75.0], 1e-6));
    assert_eq!(solution.guarantee, Guarantee::Proven(Property::Convex));

    // 共通の 3 人ゲームは凸でないので、ConvexChecked::new は元のゲームを Err で返す。
    assert!(!properties::is_convex(shared));
    assert!(ConvexChecked::new(shared.clone()).is_err());
    println!("共通の 3 人ゲームは凸でない");

    // 2. 凸と宣言しただけのゲーム。性質を確かめないので、結果は Unverified<Solution> で返る。
    //    中身を使うには verify で確かめるか、accept_unverified で検証せずに受け入れる。
    //    共通の 3 人ゲームは凸でないが、この手法の結果はたまたま仁 (2, 4, 6) になり、検証に合格する。
    let unverified: Unverified<Solution> = convex::nucleolus(&Assume::convex(shared))?;
    println!(
        "共通の 3 人ゲーム (Assume): 検証前 {:.4?} 保証 {}",
        unverified.peek().allocation,
        unverified.peek().guarantee
    );
    assert_eq!(
        unverified.peek().guarantee,
        Guarantee::Assumed(Property::Convex)
    );
    match unverified.verify(shared, VerifyOptions::default())? {
        Verified::Certified(solution) => {
            println!(
                "  検証に合格: {:.4?} 保証 {}",
                solution.allocation, solution.guarantee
            );
            assert!(close(&solution.allocation, &[2.0, 4.0, 6.0], 1e-6));
            assert_eq!(solution.guarantee, Guarantee::Certified);
        }
        other => panic!("合格するはず: {other:?}"),
    }

    // 3. 凸でないゲームを凸と宣言し、結果が否定される例。
    //    v({1}) = 1、v({2, 3}) = 3、v(N) = 3 のゲームでは、凸ゲームの手法はプレイヤー 1 に 0.5 を配る。
    //    これは v({1}) = 1 を下回るので、仁ではないと確定する。
    let game = ExplicitGame::from_lex(&[1.0, 0.0, 0.0, 1.0, 1.0, 3.0, 3.0])?;
    assert!(!properties::is_convex(&game));
    let unverified = convex::nucleolus(&Assume::convex(&game))?;
    println!(
        "凸でないゲーム (Assume): 検証前 {:.4?}",
        unverified.peek().allocation
    );
    match unverified.verify(&game, VerifyOptions::default())? {
        Verified::Refuted { solution, reason } => {
            println!("  否定された: {reason}");
            assert!(close(&solution.allocation, &[0.5, 1.25, 1.25], 1e-6));
            assert_eq!(solution.guarantee, Guarantee::Assumed(Property::Convex));
        }
        other => panic!("否定されるはず: {other:?}"),
    }
    // 同じゲームの仁を逐次 LP で求めると (1, 1, 1) になる。
    let lp = nucleolus::nucleolus(&game)?;
    println!("  逐次 LP の仁: {:.4?}", lp.allocation);
    assert!(close(&lp.allocation, &[1.0, 1.0, 1.0], 1e-6));
    Ok(())
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
