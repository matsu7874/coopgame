//! 求めた配分を検証する。
//!
//! 3 人ゲーム `[0, 0, 0, 4, 6, 8, 12]` (辞書式順) で、次の 3 つの配分を同じ判定にかける。
//!
//! - 仁 (2, 4, 6)
//! - 仁を少しずらした配分 (2.5, 3.5, 6)。コアには入るが仁ではない
//! - コアの外の配分 (6, 3, 3)
//!
//! 最後に、21 人の破産ゲームで事後検証が Undecided (確定できない) になる例を示す。
//! 性質を仮定して求めた結果 (`Unverified`) の検証は `nucleolus_methods` の例で扱う。
//!
//! 扱うモジュール:
//!
//! - `coopgame::verify`: Kohlberg 基準 (`kohlberg`)、有理数での厳密な検証 (`certify`)、
//!   配分の事後検証 (`check`)
//! - `coopgame::kernel`: カーネルに入るか (`is_in_kernel`)
//! - `coopgame::bargaining`: 交渉集合に入るか (`check`)。入らなければ反論のない異議
//! - `coopgame::properties`: コアに入るか (`is_in_core`)
//! - `coopgame::surplus`: 提携ごとの超過 (`excesses`) と最大余剰 (`max_surplus`)
//!
//! ```bash
//! cargo run --example verification
//! ```

use coopgame::bargaining::{self, Objection};
use coopgame::game::default_tolerance;
use coopgame::game::exact::format_rational;
use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::kernel;
use coopgame::nucleolus;
use coopgame::properties;
use coopgame::surplus;
use coopgame::verify::{self, Check, VerifyOptions};
use coopgame::{Concept, Domain, ExplicitGame};

/// 1 つの配分に対する判定の結果。
struct Findings {
    in_core: bool,
    kohlberg: bool,
    /// 有理数での厳密な検証に合格したか。
    certified: bool,
    /// 厳密な検証で復元した有理数の配分 (表示用の文字列)。
    exact: Vec<String>,
    check: Check,
    in_kernel: bool,
    /// 反論のない異議。`None` なら交渉集合に入る。
    objection: Option<Objection>,
}

/// `Check` の結果を 1 行にする。
fn describe(check: &Check) -> String {
    match check {
        Check::Certified { exact } => format!(
            "Certified (厳密な配分 [{}])",
            exact
                .iter()
                .map(format_rational)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Check::Refuted(reason) => format!("Refuted ({reason})"),
        Check::Undecided(reason) => format!("Undecided ({reason})"),
    }
}

/// 超過の大きい順に、上位 `count` 個の提携 (空提携と全体提携を除く) を表示する。
fn print_top_excesses(game: &ExplicitGame, x: &[f64], count: usize) {
    // excess_order は、空でも全体でもない提携を超過の大きい順に返す。
    for (coalition, excess) in surplus::excess_order(game, x).into_iter().take(count) {
        println!(
            "    {coalition:<10} v(S) = {:>5.2}  超過 = {excess:>5.2}",
            game.value(coalition)
        );
    }
}

/// 最大余剰 `s_ij` の行列を表示する。行が i、列が j (対角は意味を持たない)。
fn print_max_surplus(game: &ExplicitGame, x: &[f64]) {
    for (i, row) in surplus::max_surplus(game, x).iter().enumerate() {
        println!("    {i}: {row:.2?}");
    }
}

/// 配分 `x` を同じ判定にかけ、結果を表示して返す。
fn inspect(game: &ExplicitGame, label: &str, x: &[f64]) -> coopgame::Result<Findings> {
    let tolerance = default_tolerance(game);
    println!("== {label} {x:.4?}");

    // 1. 超過の大きい提携。超過が正の提携は、抜けると得をする。
    println!("  超過の大きい提携:");
    print_top_excesses(game, x, 3);

    // 2. コアに入るか (全ての提携の超過が 0 以下か)。
    let in_core = properties::is_in_core(game, x, tolerance);
    println!("  コア: {}", if in_core { "入る" } else { "入らない" });

    // 3. Kohlberg 基準 (浮動小数点)。仁であることの必要十分条件である。
    let report = verify::kohlberg(game, x, Domain::Imputation)?;
    println!(
        "  Kohlberg 基準: {} {}",
        report.satisfied,
        report.reason.as_deref().unwrap_or("")
    );

    // 4. 有理数での厳密な検証。浮動小数点数の配分から有理数の配分を復元して判定する。
    let certified = verify::certify(game, x, Domain::Imputation)?;
    let exact: Vec<String> = certified.allocation.iter().map(format_rational).collect();
    println!(
        "  厳密な検証: {} 復元した配分 [{}] {}",
        certified.satisfied,
        exact.join(", "),
        certified.reason.as_deref().unwrap_or("")
    );

    // 5. 仁としての事後検証。結果は Certified・Refuted・Undecided のどれかになる。
    let check = verify::check(game, x, Concept::Nucleolus, VerifyOptions::default())?;
    println!("  事後検証: {}", describe(&check));

    // 6. カーネルの条件: どの 2 人の間でも最大余剰が釣り合っているか。
    let in_kernel = kernel::is_in_kernel(game, x, Domain::Imputation, tolerance);
    println!(
        "  カーネル: {}",
        if in_kernel { "入る" } else { "入らない" }
    );
    print_max_surplus(game, x);

    // 7. 交渉集合: 反論のない異議があるか。
    let objection = bargaining::check(game, x, Domain::Imputation, 10.0 * tolerance)?.objection;
    match &objection {
        None => println!("  交渉集合: 入る"),
        Some(objection) => {
            // payoff は全員分の支払いで、提携のメンバー以外は x のままである。
            let offer: Vec<String> = objection
                .coalition
                .members()
                .map(|k| format!("プレイヤー {k} に {:.4}", objection.payoff[k]))
                .collect();
            println!(
                "  交渉集合: 入らない。プレイヤー {} はプレイヤー {} に対し、提携 {} の支払い ({}) で異議を出し、プレイヤー {} は反論できない",
                objection.objector,
                objection.target,
                objection.coalition,
                offer.join("、"),
                objection.target
            );
        }
    }
    println!();

    Ok(Findings {
        in_core,
        kohlberg: report.satisfied,
        certified: certified.satisfied,
        exact,
        check,
        in_kernel,
        objection,
    })
}

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;

    // 逐次 LP で仁を求める。これが検証の基準になる。
    let nucleolus = nucleolus::nucleolus(&game)?.allocation;
    assert!(close(&nucleolus, &[2.0, 4.0, 6.0], 1e-6));

    // 結果を確かめる (docs/examples.md の値)。
    let found = inspect(&game, "仁", &nucleolus)?;
    assert!(found.in_core && found.kohlberg && found.certified);
    assert_eq!(found.exact, ["2", "4", "6"]);
    assert!(matches!(found.check, Check::Certified { .. }));
    assert!(found.in_kernel);
    assert!(found.objection.is_none());
    // 仁では 2 人の提携 3 つの超過がどれも -2 になる。
    for (coalition, excess) in surplus::excesses(&game, &nucleolus) {
        if coalition.len() == 2 {
            assert!((excess + 2.0).abs() < 1e-6, "{coalition}");
        }
    }

    let found = inspect(&game, "少しずらした配分", &[2.5, 3.5, 6.0])?;
    assert!(found.in_core);
    assert!(!found.kohlberg && !found.certified);
    assert!(matches!(found.check, Check::Refuted(_)));
    assert!(!found.in_kernel);
    // コアは交渉集合に含まれるので、コアの配分には反論のない異議がない。
    assert!(found.objection.is_none());

    let found = inspect(&game, "コアの外の配分", &[6.0, 3.0, 3.0])?;
    assert!(!found.in_core && !found.kohlberg && !found.certified);
    assert!(matches!(found.check, Check::Refuted(_)));
    assert!(!found.in_kernel);
    let objection = found.objection.expect("(6, 3, 3) には反論のない異議がある");
    let coalition: Vec<usize> = objection.coalition.members().collect();
    assert_eq!((objection.objector, objection.target), (1, 0));
    assert_eq!(coalition, vec![1, 2]);

    // Check の 3 つ目の結果 Undecided: 21 人以上では有理数の検証をせず、
    // ランダムに選んだ 1 組でカーネル条件を確かめる。反例がなければ「確定できない」になる。
    println!("== 21 人の破産ゲームのタルムード則 (仁)");
    let claims: Vec<f64> = (1..=21).map(|i| f64::from(i) * 10.0).collect();
    let bankruptcy = BankruptcyGame::new(1000.0, claims)?;
    let talmud = bankruptcy.nucleolus()?;
    let mut options = VerifyOptions::default();
    options.pairs = 1;
    let check = verify::check(&bankruptcy, &talmud, Concept::Nucleolus, options)?;
    println!("  事後検証 (1 組だけ確かめる): {}", describe(&check));
    assert!(matches!(check, Check::Undecided(_)));
    Ok(())
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
