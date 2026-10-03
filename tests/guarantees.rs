//! 保証の種類・能力の階層・性質の宣言・凸ゲームの手法・事後検証を確かめる。

mod common;

use common::assert_close;
use coopgame::auto::AutoNucleolus;
use coopgame::convex;
use coopgame::generators::{self, SplitMix64};
use coopgame::oracle::bankruptcy::BankruptcyGame;
use coopgame::oracle::graph::InducedSubgraphGame;
use coopgame::oracle::nucleolus as oracle_nucleolus;
use coopgame::oracle::voting::WeightedVotingGame;
use coopgame::oracle::{PlayerSet, Separation, SetFunction, tabulate};
use coopgame::structure::{Assume, ConvexChecked};
use coopgame::verify::{self, Check, Verified, VerifyOptions};
use coopgame::{
    Concept, ExplicitGame, Guarantee, Property, Solution, Unverified, nucleolus, properties,
};

fn random_bankruptcy(rng: &mut SplitMix64, n: usize) -> BankruptcyGame {
    let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 100) as f64).collect();
    let estate = (rng.next_f64() * claims.iter().sum::<f64>()).round();
    BankruptcyGame::new(estate, claims).unwrap()
}

fn random_graph(rng: &mut SplitMix64, n: usize) -> InducedSubgraphGame {
    let mut edges = Vec::new();
    for u in 0..n {
        for v in u + 1..n {
            if rng.next_f64() < 0.4 {
                edges.push((u, v, rng.range(0, 20) as f64));
            }
        }
    }
    InducedSubgraphGame::new(n, edges).unwrap()
}

// ---------------------------------------------------------------- 段階 2: 能力の階層

/// 超過順の列挙を持たず、分離オラクルだけを総当たりで実装したゲーム。
struct SeparationOnly(ExplicitGame);

impl SetFunction for SeparationOnly {
    fn players(&self) -> usize {
        self.0.players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        SetFunction::value(&self.0, coalition)
    }
}

impl Separation for SeparationOnly {
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        // 超過の大きい順にはしない (分離オラクルの契約では順序は必須でない)。
        let n = self.0.players();
        let mut found = Vec::new();
        for mask in 1..(1u64 << n) - 1 {
            let set = PlayerSet::from_coalition(n, coopgame::Coalition(mask));
            let excess = self.value(&set) - set.sum(x);
            if excess > bound && !skip(&set) {
                found.push((set, excess));
                if found.len() >= limit {
                    break;
                }
            }
        }
        found
    }

    fn value_scale(&self) -> f64 {
        self.0.max_abs_value()
    }
}

#[test]
fn separation_only_game_gives_the_nucleolus() {
    for kind in [1, 2, 4] {
        for n in [4, 6, 8] {
            let game = generators::bnf(kind, n, 5).unwrap();
            let expected = nucleolus::nucleolus(&game).unwrap().allocation;
            let actual = oracle_nucleolus::nucleolus(&SeparationOnly(game.clone()))
                .unwrap()
                .allocation;
            assert_close(
                &actual,
                &expected,
                1e-6 * game.max_abs_value(),
                "分離オラクル",
            );
        }
    }
}

#[test]
fn tabulate_reproduces_explicit_game() {
    let game = generators::bnf(2, 7, 1).unwrap();
    assert_eq!(tabulate(&game).unwrap(), game);
    let mut rng = SplitMix64::new(3);
    let bankruptcy = random_bankruptcy(&mut rng, 6);
    let table = tabulate(&bankruptcy).unwrap();
    let expected = generators::bankruptcy(bankruptcy.estate(), bankruptcy.claims()).unwrap();
    assert_eq!(table, expected);
}

// ---------------------------------------------------------------- 段階 3: 性質

/// 破産ゲーム・非負の重みの誘導部分グラフゲームは、構造から凸と主張している。全提携で確かめる。
#[test]
fn structural_convexity_claims_hold() {
    let mut rng = SplitMix64::new(11);
    for _ in 0..30 {
        let n = 2 + (rng.next_u64() % 7) as usize;
        assert!(properties::is_convex(
            &tabulate(&random_bankruptcy(&mut rng, n)).unwrap()
        ));
        assert!(properties::is_convex(
            &tabulate(&random_graph(&mut rng, n)).unwrap()
        ));
    }
    assert!(InducedSubgraphGame::new(3, vec![(0, 1, -1.0)]).is_err());
}

#[test]
fn convexity_check_rejects_non_convex_games() {
    let majority = generators::weighted_voting(&[1.0, 1.0, 1.0], 2.0).unwrap();
    assert!(ConvexChecked::new(majority).is_err());
    assert!(ConvexChecked::new(generators::random_convex(5, 2).unwrap()).is_ok());
}

// ---------------------------------------------------------------- 段階 4: 凸ゲームの手法と自動選択

#[test]
fn convex_method_matches_talmud_and_lp() {
    let mut rng = SplitMix64::new(1971);
    for case in 0..20 {
        let n = 2 + (rng.next_u64() % 10) as usize;
        let bankruptcy = random_bankruptcy(&mut rng, n);
        // Proven の型は結果をそのまま返す。
        let solution: Solution = convex::nucleolus(&bankruptcy).unwrap();
        assert_eq!(solution.guarantee, Guarantee::Proven(Property::Convex));
        assert_close(
            &solution.allocation,
            &bankruptcy.nucleolus().unwrap(),
            1e-6 * bankruptcy.claims().iter().sum::<f64>().max(1.0),
            &format!("破産 case {case}"),
        );
        let graph = random_graph(&mut rng, n);
        let lp = nucleolus::nucleolus(&tabulate(&graph).unwrap()).unwrap();
        let solution = convex::nucleolus(&graph).unwrap();
        assert_close(
            &solution.allocation,
            &lp.allocation,
            1e-6,
            &format!("グラフ case {case}"),
        );
    }
}

#[test]
fn auto_selection_reports_method_and_guarantee() {
    let mut rng = SplitMix64::new(5);
    let bankruptcy = random_bankruptcy(&mut rng, 6);
    let solution = bankruptcy.nucleolus_auto().unwrap();
    assert_eq!(
        (solution.method, solution.guarantee),
        ("talmud-rule", Guarantee::Proven(Property::Bankruptcy))
    );

    let explicit = generators::bnf(1, 6, 0).unwrap();
    let solution = explicit.nucleolus_auto().unwrap();
    assert_eq!(solution.guarantee, Guarantee::Exact);

    let voting = WeightedVotingGame::new(vec![3, 2, 2, 1, 1], 5).unwrap();
    let solution = voting.nucleolus_auto().unwrap();
    let expected = nucleolus::nucleolus(&tabulate(&voting).unwrap()).unwrap();
    assert_close(&solution.allocation, &expected.allocation, 1e-6, "投票");
    assert_eq!(solution.guarantee, Guarantee::Exact);

    let graph = random_graph(&mut rng, 7);
    let solution = graph.nucleolus_auto().unwrap();
    assert_eq!(solution.guarantee, Guarantee::Proven(Property::Convex));

    let small = ConvexChecked::new(generators::random_convex(6, 1).unwrap()).unwrap();
    assert_eq!(small.nucleolus_auto().unwrap().method, "sequential-lp");
}

// ---------------------------------------------------------------- 段階 1, 5: 仮定付きの結果と検証

#[test]
fn assumed_result_on_convex_game_is_certified() {
    let game = generators::random_convex(6, 4).unwrap();
    // Assumed の型は Unverified で返る。
    let unverified: Unverified<Solution> =
        convex::nucleolus(&Assume::convex(game.clone())).unwrap();
    assert_eq!(unverified.property(), Property::Convex);
    assert_eq!(
        unverified.peek().guarantee,
        Guarantee::Assumed(Property::Convex)
    );
    match unverified.verify(&game, VerifyOptions::default()).unwrap() {
        Verified::Certified(solution) => {
            assert_eq!(solution.guarantee, Guarantee::Certified);
            let lp = nucleolus::nucleolus(&game).unwrap().allocation;
            assert_close(&solution.allocation, &lp, 1e-9, "昇格した配分");
        }
        other => panic!("合格するはず: {other:?}"),
    }
}

/// BNF タイプ 2 (n = 4, seed 0) は凸でない。凸と仮定するとプレ仁 (配分集合の外) に収束し、
/// 仁としては否定される (`data/analysis/assumption-study.csv` の行と同じ)。
#[test]
fn assumed_result_on_non_convex_game_is_refuted() {
    let game = generators::bnf(2, 4, 0).unwrap();
    assert!(!properties::is_convex(&game));
    let unverified = convex::nucleolus(&Assume::convex(game.clone())).unwrap();
    match unverified.verify(&game, VerifyOptions::default()).unwrap() {
        Verified::Refuted { solution, reason } => {
            assert_eq!(solution.guarantee, Guarantee::Assumed(Property::Convex));
            assert!(reason.contains("より小さい"), "{reason}");
            let pre = nucleolus::prenucleolus(&game).unwrap().allocation;
            assert_close(
                &solution.allocation,
                &pre,
                1e-6 * game.max_abs_value(),
                "プレ仁",
            );
        }
        other => panic!("否定されるはず: {other:?}"),
    }
}

/// 21 人 (厳密な検証の上限を超える) では、組ごとのカーネル条件で否定だけができる。
#[test]
fn pair_checks_refute_but_do_not_certify_large_games() {
    let mut rng = SplitMix64::new(21);
    let game = random_bankruptcy(&mut rng, 21);
    let talmud = game.nucleolus().unwrap();
    let candidate = |allocation: Vec<f64>| Solution {
        concept: Concept::Nucleolus,
        allocation,
        guarantee: Guarantee::Exact,
        method: "test",
    };
    let mut options = VerifyOptions::default();
    options.pairs = 3;
    options.seed = 1;
    let check = verify::check(&game, &candidate(talmud.clone()), options).unwrap();
    assert!(matches!(check, Check::Undecided(_)), "{check:?}");
    // 和を保ったまま全員の配分をずらす。
    let shift: Vec<f64> = (0..21).map(|_| rng.next_f64() - 0.5).collect();
    let mean = shift.iter().sum::<f64>() / 21.0;
    let perturbed: Vec<f64> = talmud
        .iter()
        .zip(&shift)
        .map(|(x, s)| x + 2.0 * (s - mean))
        .collect();
    let check = verify::check(&game, &candidate(perturbed), options).unwrap();
    assert!(matches!(check, Check::Refuted(_)), "{check:?}");
}

#[test]
fn approximate_results_say_so() {
    let game = generators::bnf(1, 8, 2).unwrap();
    assert_eq!(
        coopgame::values::shapley_sampling(&game, 10, 0).guarantee,
        Guarantee::Approximate
    );
    assert_eq!(
        coopgame::sampled::sampled_nucleolus(&game, 50, 0, coopgame::Domain::Preimputation)
            .unwrap()
            .guarantee,
        Guarantee::Approximate
    );
    assert_eq!(
        nucleolus::nucleolus(&game).unwrap().guarantee,
        Guarantee::Exact
    );
    assert!(!Guarantee::Assumed(Property::Convex).is_reliable());
    assert_eq!(
        Guarantee::Proven(Property::Convex).to_string(),
        "proven(convex)"
    );
}

// ---------------------------------------------------------------- 有理数で構築したゲーム

/// 浮動小数点数の和で作った優加法的なゲーム (n = 4, seed 0) は、値を有理数にすると等しいはずの超過がずれ、
/// LP の仁が厳密な検証に合格しない。同じ乱数で有理数のまま構築すると合格する。
#[test]
fn rational_construction_makes_certification_possible() {
    use coopgame::exact;
    let float_game = generators::random_superadditive(4, 0).unwrap();
    let x = nucleolus::nucleolus(&float_game).unwrap().allocation;
    assert!(
        !exact::certify(&float_game, &x, coopgame::Domain::Imputation)
            .unwrap()
            .satisfied
    );

    let exact_game = generators::random_superadditive_exact(4, 0).unwrap();
    let explicit = exact_game.to_explicit().unwrap();
    let x = nucleolus::nucleolus(&explicit).unwrap().allocation;
    let report = exact::certify_exact(&exact_game, &x, coopgame::Domain::Imputation).unwrap();
    assert!(report.satisfied, "{:?}", report.reason);

    // 有理数のゲームで、凸と仮定した結果も検証できる (このゲームは凸)。
    assert!(properties::is_convex(&explicit));
    let unverified = convex::nucleolus(&Assume::convex(&explicit)).unwrap();
    match unverified
        .verify_exact(&exact_game, VerifyOptions::default())
        .unwrap()
    {
        Verified::Certified(solution) => assert_eq!(solution.guarantee, Guarantee::Certified),
        other => panic!("合格するはず: {other:?}"),
    }
}

#[test]
fn rational_values_parse_decimals_exactly() {
    use coopgame::exact::{self, ExactGame, Rational};
    let values = exact::parse_rational_values("0.1\n0.2\n0.3\n0.5\n0.6\n0.7\n# 全体\n1\n").unwrap();
    let game = ExactGame::from_lex(values).unwrap();
    let explicit = game.to_explicit().unwrap();
    let x = nucleolus::nucleolus(&explicit).unwrap().allocation;
    let report = exact::certify_exact(&game, &x, coopgame::Domain::Imputation).unwrap();
    let q = |a: i64, b: i64| Rational::new(a.into(), b.into());
    assert_eq!(report.allocation, vec![q(7, 30), q(1, 3), q(13, 30)]);
}
