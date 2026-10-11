//! coopgame の最初の一歩。ゲームを作り、仁を求め、結果の保証と手法を読む。
//!
//! - `coopgame::game`: 特性関数を辞書式順のベクトルから作る (`ExplicitGame::from_lex`)。提携は `Coalition`
//! - `coopgame::auto::AutoNucleolus`: ゲームの型から、保証のある手法のうち最も速いものを選んで仁を求める
//! - `coopgame::solution`: 結果 (`Solution`) は配分・保証の種類 (`Guarantee`)・手法の名前を持つ
//! - `coopgame::games`: 破産ゲーム (`bankruptcy::BankruptcyGame`)・重み付き投票ゲーム (`voting::WeightedVotingGame`)
//!
//! 同じ `nucleolus_auto()` を呼んでも、ゲームの型によって選ばれる手法と保証が変わる。
//!
//! ```bash
//! cargo run --example quickstart
//! ```

use coopgame::auto::AutoNucleolus;
use coopgame::game::{Coalition, ExplicitGame, format_coalition};
use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::solution::{Guarantee, Property, Solution};

fn main() -> coopgame::Result<()> {
    // 1. 3 人ゲームを、辞書式順の特性関数から作る。
    //    並びは v({1}), v({2}), v({3}), v({1, 2}), v({1, 3}), v({2, 3}), v({1, 2, 3})。
    //    関数に渡すプレイヤーの番号は 0 始まりで、format_coalition は 1 始まりで表示する。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    println!("== 3 人ゲーム ==");
    for players in [&[0, 1][..], &[0, 2], &[1, 2], &[0, 1, 2]] {
        let coalition = Coalition::from_players(players);
        println!(
            "v({}) = {}",
            format_coalition(coalition, None),
            game.value(coalition)
        );
    }

    // 2. 仁を求める。全提携の表を持つゲームでは、逐次 LP で定義どおりに解く。
    let solution = game.nucleolus_auto()?;
    print_solution(&solution);
    assert!(close(&solution.allocation, &[2.0, 4.0, 6.0], 1e-6));
    assert_eq!(solution.method, "sequential-lp");
    assert_eq!(solution.guarantee, Guarantee::Exact);

    // 3. 破産問題 (遺産 200、請求 100, 200, 300)。
    //    破産ゲームの型を使うと、仁を閉じた形 (タルムード則) で求める。
    //    保証は「破産ゲームという性質のもとで正しい手法」を表す proven(bankruptcy) になる。
    println!("== 破産問題 ==");
    let bankruptcy = BankruptcyGame::new(200.0, vec![100.0, 200.0, 300.0])?;
    let solution = bankruptcy.nucleolus_auto()?;
    print_solution(&solution);
    assert!(close(&solution.allocation, &[50.0, 75.0, 75.0], 1e-6));
    assert_eq!(solution.method, "talmud-rule");
    assert_eq!(solution.guarantee, Guarantee::Proven(Property::Bankruptcy));

    // 4. 重み付き投票ゲーム [51; 35, 20, 15, 15, 15]。
    //    投票ゲームの型を使うと、全提携の表を作らずに、違反する提携を探しながら逐次 LP を解く。
    //    定義どおりに解くので、保証は exact になる。
    println!("== 重み付き投票ゲーム ==");
    let voting = WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?;
    let solution = voting.nucleolus_auto()?;
    print_solution(&solution);
    assert!(close(
        &solution.allocation,
        &[0.375, 0.25, 0.125, 0.125, 0.125],
        1e-6
    ));
    assert_eq!(solution.method, "oracle-constraint-generation");
    assert_eq!(solution.guarantee, Guarantee::Exact);
    // 同じゲームを全提携の表に直して解いた仁と一致する。
    let table = ExplicitGame::tabulate(&voting)?;
    assert!(close(
        &solution.allocation,
        &table.nucleolus_auto()?.allocation,
        1e-6
    ));
    Ok(())
}

/// 結果の配分・手法・保証を表示する。
fn print_solution(solution: &Solution) {
    println!("仁: {:.4?}", solution.allocation);
    println!("手法: {}", solution.method);
    println!(
        "保証: {} (仮定に依存しない: {})",
        solution.guarantee,
        solution.guarantee.is_reliable()
    );
    println!();
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
