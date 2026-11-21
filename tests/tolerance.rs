//! 逐次 LP の許容誤差の決め方 (`nucleolus::Tolerance`) を確かめる。
//!
//! - 既定 (`Tolerance::Auto`) は、値が大きい整数のゲームでも有理数の仁と一致する配分を返す。
//!   必要なら許容誤差を下げて解き直し、それでも超過の段が整合しなければ有理数の逐次 LP で求める。
//! - 許容誤差を指定すると (`Absolute`・`Relative`)、その許容誤差で 1 回だけ解く。
//! - 超過の段が整合するゲームでは、既定でも 1 回だけ解く。

mod common;

use common::max_abs_difference;
use coopgame::game::exact::ExactGame;
use coopgame::nucleolus::{self, Options, Tolerance, exact};
use coopgame::num_traits::ToPrimitive;
use coopgame::{Domain, Error, ExplicitGame, Guarantee, generators};

/// 値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲーム。超過の段の間隔は 1 以下になる。
fn integer_games(scale: f64, sizes: &[usize], count: usize, seed: u64) -> Vec<ExplicitGame> {
    let mut state = seed;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    (0..count)
        .map(|case| {
            let n = sizes[case % sizes.len()];
            let values: Vec<f64> = (1usize..1 << n)
                .map(|mask| {
                    let size = mask.count_ones() as f64;
                    if size == 1.0 {
                        0.0
                    } else {
                        scale * size + (next() % 7) as f64
                    }
                })
                .collect();
            ExplicitGame::from_binary(&values).unwrap()
        })
        .collect()
}

fn rational_nucleolus(game: &ExplicitGame) -> Vec<f64> {
    let exact = ExactGame::from_explicit(game).unwrap();
    exact::nucleolus(&exact, Domain::Imputation)
        .unwrap()
        .allocation
        .iter()
        .map(|q| q.to_f64().unwrap())
        .collect()
}

fn with_tolerance(game: &ExplicitGame, tolerance: Tolerance) -> nucleolus::NucleolusResult {
    let mut options = Options::new(Domain::Imputation);
    options.tolerance = tolerance;
    nucleolus::nucleolus_with(game, options).unwrap()
}

/// 値が 1e6 から 1e9 程度の整数ゲームで、既定は有理数の仁と一致する。比 1e-7 の許容誤差で 1 回だけ解くと、
/// 超過の差が許容誤差より小さい段をまとめ、ずれるゲームがある。
#[test]
fn default_matches_rational_nucleolus_of_large_integer_games() {
    for scale in [1e6, 1e7, 1e8, 1e9] {
        let games = integer_games(scale, &[3, 4, 5, 6], 24, 3);
        let mut off_with_fixed = 0;
        for (case, game) in games.iter().enumerate() {
            let truth = rational_nucleolus(game);
            let precision = 1e-13 * game.max_abs_value();
            let auto = nucleolus::nucleolus(game).unwrap();
            assert!(
                max_abs_difference(auto.allocation(), &truth) <= precision,
                "{scale:e} case {case}: {auto:?} / {truth:?}"
            );
            assert_eq!(auto.guarantee(), Guarantee::Exact, "{scale:e} case {case}");
            let fixed = with_tolerance(game, Tolerance::Relative(1e-7));
            assert_eq!(fixed.refinements(), 0);
            assert!(!fixed.is_rational());
            assert_eq!(fixed.tolerance(), 1e-7 * game.max_abs_value());
            if max_abs_difference(fixed.allocation(), &truth) > precision {
                off_with_fixed += 1;
            }
        }
        // 既定の比で 1 回だけ解くと、ずれるゲームがある (なければ、解き直しを確かめていない)。
        assert!(off_with_fixed >= 3, "{scale:e}: {off_with_fixed}");
    }
}

/// 超過の段が整合するゲームでは、既定でも比 1e-7 の許容誤差で 1 回だけ解き、指定した場合と同じ結果を返す。
#[test]
fn default_solves_once_when_levels_are_consistent() {
    for seed in 0..12u64 {
        for class in [1u8, 2, 4] {
            let n = 4 + (seed % 5) as usize;
            let game = generators::bnf(class, n, seed).unwrap();
            let auto = nucleolus::nucleolus(&game).unwrap();
            assert_eq!(auto.refinements(), 0, "BNF {class} n={n} seed={seed}");
            assert!(!auto.is_rational());
            assert_eq!(auto.guarantee(), Guarantee::Exact);
            assert_eq!(auto.tolerance(), 1e-7 * game.max_abs_value());
            let fixed = with_tolerance(&game, Tolerance::Relative(1e-7));
            assert_eq!(auto, fixed, "BNF {class} n={n} seed={seed}");
        }
    }
}

/// 値が 1e12 程度の整数ゲームは、逐次 LP の許容誤差を下げても超過の段を分けられない。
/// 既定は、10 人以下なら有理数の逐次 LP に切り替えて求める。
#[test]
fn default_switches_to_rational_solver_when_lp_cannot_separate_levels() {
    let mut switched = 0;
    for (case, game) in integer_games(1e12, &[3, 4, 5], 24, 5).iter().enumerate() {
        let auto = nucleolus::nucleolus(game).unwrap();
        assert_eq!(auto.guarantee(), Guarantee::Exact, "case {case}");
        if auto.is_rational() {
            switched += 1;
            assert_eq!(auto.allocation(), rational_nucleolus(game), "case {case}");
            assert_eq!(auto.tolerance(), 0.0);
            assert_eq!(auto.refinements(), 2);
        }
    }
    assert!(switched >= 5, "{switched}");
}

/// 有理数の逐次 LP に切り替えられない人数 (11 人以上) で超過の段が整合しなければ、最後に解けた配分を
/// 保証 `approximate` で返す。許容誤差を指定した場合は、確かめずに `exact` のまま返す。
#[test]
fn default_reports_approximate_when_levels_stay_inconsistent() {
    let mut approximate = 0;
    for (case, game) in integer_games(1e10, &[11], 6, 9).iter().enumerate() {
        let auto = nucleolus::nucleolus(game).unwrap();
        assert!(!auto.is_rational(), "case {case}");
        if auto.guarantee() == Guarantee::Approximate {
            approximate += 1;
            // 許容誤差を 1 回以上下げて解き直している (最も小さい許容誤差では LP が失敗することがある)。
            assert!(auto.refinements() >= 1, "case {case}");
        }
        let fixed = with_tolerance(game, Tolerance::Relative(1e-7));
        assert_eq!(fixed.guarantee(), Guarantee::Exact, "case {case}");
    }
    assert!(approximate >= 3, "{approximate}");
}

/// 絶対値と比の指定は、同じ許容誤差なら同じ結果になる。不正な値はエラーにする。
#[test]
fn fixed_tolerance_is_absolute_or_relative() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 40.0, 30.0, 20.0, 60.0]).unwrap();
    let absolute = with_tolerance(&game, Tolerance::Absolute(6e-6));
    let relative = with_tolerance(&game, Tolerance::Relative(1e-7));
    assert_eq!(absolute, relative);
    assert_eq!(absolute.tolerance(), 6e-6);
    for tolerance in [
        Tolerance::Absolute(f64::NAN),
        Tolerance::Absolute(-1.0),
        Tolerance::Absolute(f64::INFINITY),
        Tolerance::Relative(f64::NAN),
        Tolerance::Relative(-1e-9),
        Tolerance::Relative(f64::INFINITY),
    ] {
        let mut options = Options::new(Domain::Imputation);
        options.tolerance = tolerance;
        let result = nucleolus::nucleolus_with(&game, options);
        assert!(
            matches!(result, Err(Error::InvalidArgument(_))),
            "{tolerance:?}: {result:?}"
        );
    }
}
