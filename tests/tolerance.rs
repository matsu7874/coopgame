//! 逐次 LP の許容誤差の決め方 (`nucleolus::Tolerance`) を確かめる。
//!
//! - 既定 (`Tolerance::Auto`) は、解いた配分が Kohlberg 基準を細かい許容誤差でも満たすかを確かめる。
//!   満たさなければ許容誤差を下げて解き直し、それでも満たさなければ、8 人以下は有理数の逐次 LP で求め、
//!   9 人以上は保証を `approximate` にする。値が 1e9 程度までの整数のゲームで、有理数の仁と一致する。
//!   確かめない人数 (21 人以上) では、LP を解いて求めた配分の保証を `approximate` にする。
//! - 許容誤差を指定すると (`Absolute`・`Relative`)、その許容誤差で 1 回だけ解く。
//! - 最初に解いた配分が Kohlberg 基準を細かい許容誤差でも満たすゲームでは、既定でも 1 回だけ解く。

mod common;

use common::max_abs_difference;
use coopgame::game::exact::ExactGame;
use coopgame::nucleolus::{self, Options, Tolerance, exact};
use coopgame::num_traits::ToPrimitive;
use coopgame::verify;
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

/// 最初に解いた配分が Kohlberg 基準を満たすゲームでは、既定でも比 1e-7 の許容誤差で 1 回だけ解き、指定した場合と同じ結果を返す。
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
/// 既定は、解いた配分が Kohlberg 基準を満たさないと分かれば、8 人以下では有理数の逐次 LP に切り替えて求める
/// (段の間隔が確認の許容誤差より小さく、満たさないと分からないゲームもある。その場合は切り替えない)。
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

/// 有理数の逐次 LP に自動では切り替えない人数 (9 人以上) で Kohlberg 基準を満たさなければ、最後に解けた配分を
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

/// 値が `scale * |S| + 0..6` で、1 人提携の値も `scale + 0..6`、全体提携の値が 1 人提携の値の和より
/// `1..6` だけ大きい整数ゲーム。仁では多くのプレイヤーが下限 `x_i = v({i})` の近くにいる。
fn games_near_lower_bounds(
    scale: f64,
    sizes: &[usize],
    count: usize,
    seed: u64,
) -> Vec<ExplicitGame> {
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
            let full = (1usize << n) - 1;
            let mut values: Vec<f64> = (1..=full)
                .map(|mask| scale * mask.count_ones() as f64 + (next() % 7) as f64)
                .collect();
            let singles: f64 = (0..n).map(|i| values[(1 << i) - 1]).sum();
            values[full - 1] = singles + (1 + next() % 6) as f64;
            ExplicitGame::from_binary(&values).unwrap()
        })
        .collect()
}

/// 仁で多くのプレイヤーが下限 `x_i = v({i})` にいるゲームでも、既定は有理数の仁と一致する。
/// 下限にいるプレイヤーの 1 人提携だけでは、Kohlberg 基準を満たすことにならない。
#[test]
fn default_matches_rational_nucleolus_when_players_sit_at_lower_bounds() {
    // 2 人: v({0}) = v({1}) = 1e10、v(N) = 2e10 + 1。仁は余りの 1 を等分する。
    let game = ExplicitGame::from_binary(&[1e10, 1e10, 2e10 + 1.0]).unwrap();
    let auto = nucleolus::nucleolus(&game).unwrap();
    assert_eq!(auto.allocation(), [1e10 + 0.5, 1e10 + 0.5], "{auto:?}");
    assert_eq!(auto.guarantee(), Guarantee::Exact);

    // 対称なゲーム v(S) = s|S| (S != N)、v(N) = s n + 1。仁は等分 s + 1/n。
    let scale = 3e9;
    for n in [3usize, 6, 8] {
        let full = (1usize << n) - 1;
        let values: Vec<f64> = (1..=full)
            .map(|mask| {
                let size = mask.count_ones() as f64;
                if mask == full {
                    scale * size + 1.0
                } else {
                    scale * size
                }
            })
            .collect();
        let game = ExplicitGame::from_binary(&values).unwrap();
        let auto = nucleolus::nucleolus(&game).unwrap();
        let truth = rational_nucleolus(&game);
        let rounding = 4.0 * n as f64 * f64::EPSILON * game.max_abs_value();
        assert!(
            max_abs_difference(auto.allocation(), &truth) <= rounding,
            "n={n}: {auto:?} / {truth:?}"
        );
        assert_eq!(auto.guarantee(), Guarantee::Exact, "n={n}");
    }

    for scale in [1e7, 1e9] {
        for (case, game) in games_near_lower_bounds(scale, &[3, 4, 5, 6], 24, 7)
            .iter()
            .enumerate()
        {
            let n = game.players();
            let auto = nucleolus::nucleolus(game).unwrap();
            let truth = rational_nucleolus(game);
            let rounding = 4.0 * n as f64 * f64::EPSILON * game.max_abs_value();
            assert!(
                max_abs_difference(auto.allocation(), &truth) <= rounding,
                "{scale:e} case {case}: {auto:?} / {truth:?}"
            );
            // 個人合理性を丸めの範囲で満たす。
            for (i, share) in auto.allocation().iter().enumerate() {
                let lower = game.value(coopgame::Coalition::singleton(i));
                assert!(*share >= lower - rounding, "{scale:e} case {case}: x_{i}");
            }
        }
    }
}

/// 有理数の逐次 LP に自動では切り替えない人数 (9 人以上) で、下限にいるプレイヤーが多い配分が
/// Kohlberg 基準を満たさなければ、保証を `approximate` にする (等分が仁の対称なゲームで、1 人に余りを寄せた配分を
/// `exact` と名乗らない)。
#[test]
fn default_does_not_call_inconsistent_levels_exact_beyond_rational_limit() {
    let (n, scale) = (nucleolus::MAX_AUTO_RATIONAL_PLAYERS + 1, 3e9);
    let full = (1usize << n) - 1;
    let values: Vec<f64> = (1..=full)
        .map(|mask| {
            let size = mask.count_ones() as f64;
            if mask == full {
                scale * size + 1.0
            } else {
                scale * size
            }
        })
        .collect();
    let game = ExplicitGame::from_binary(&values).unwrap();
    let auto = nucleolus::nucleolus(&game).unwrap();
    assert!(!auto.is_rational(), "{auto:?}");
    let equal_split = scale + 1.0 / n as f64;
    let rounding = 4.0 * n as f64 * f64::EPSILON * game.max_abs_value();
    let is_equal_split = auto
        .allocation()
        .iter()
        .all(|share| (share - equal_split).abs() <= rounding);
    assert!(
        is_equal_split || auto.guarantee() == Guarantee::Approximate,
        "{auto:?}"
    );
}

/// 下限 `x_0 = v({0})` にいるプレイヤー 0 を含む提携の超過がどれも低いゲーム。超過の高い段の提携は
/// プレイヤー 0 を含まないので、提携の特性ベクトルだけでは多くの段を読んでも `R^n` を張らない。
/// Kohlberg 基準の確認は、`1_N` も張る空間に数えて打ち切るので、少ない段で終わる。
#[test]
fn kohlberg_check_stops_early_when_a_player_stays_at_the_lower_bound() {
    let n = 12;
    let full = (1usize << n) - 1;
    let grand = 1e7 * (n - 1) as f64;
    // プレイヤー 0 を含まない提携の値。
    let without_first = |mask: usize| -> f64 {
        match mask.count_ones() as usize {
            0 | 1 => 0.0,
            size if size == n - 1 => grand + 1e8,
            size => {
                let noise = 1 + (mask as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) % 999_999;
                1e7 * size as f64 + noise as f64
            }
        }
    };
    let values: Vec<f64> = (1..=full)
        .map(|mask| {
            if mask == full {
                grand
            } else if mask & 1 == 0 {
                without_first(mask)
            } else if mask == 1 {
                0.0
            } else {
                without_first(mask & !1) - 1e9
            }
        })
        .collect();
    let game = ExplicitGame::from_binary(&values).unwrap();
    let result = nucleolus::nucleolus(&game).unwrap();
    assert!(
        result.allocation()[0].abs() <= 1e-3,
        "{}",
        result.allocation()[0]
    );
    let report = verify::kohlberg(&game, result.allocation(), Domain::Imputation).unwrap();
    assert!(report.satisfied, "{:?}", report.reason);
    assert!(
        report.levels_checked <= n,
        "{} 段を読んだ",
        report.levels_checked
    );
    // 有理数の判定も同じ条件で打ち切る。
    let exact = verify::certify(&game, result.allocation(), Domain::Imputation).unwrap();
    assert!(exact.satisfied, "{:?}", exact.reason);
    assert!(
        exact.levels_checked <= n,
        "有理数の判定が {} 段を読んだ",
        exact.levels_checked
    );
}

/// 確認する人数 ([`nucleolus::MAX_AUTO_CHECK_PLAYERS`]) を超えるゲームでは、既定の手順は Kohlberg 基準を確かめないので、
/// 1 回解いた配分を保証 `approximate` で返す。許容誤差を指定した場合は、従来どおり `exact` のまま返す。
/// 確認する人数以下では、Kohlberg 基準を満たした配分に `exact` を付ける。
#[test]
fn default_reports_approximate_beyond_check_limit() {
    let limit = nucleolus::MAX_AUTO_CHECK_PLAYERS;
    // BNF タイプ 1 は、最初に解いた配分が Kohlberg 基準を満たす (docs/performance.md の「既定の手順の効果」)。
    let checked = generators::bnf(1, limit, 3).unwrap();
    let auto = nucleolus::nucleolus(&checked).unwrap();
    assert_eq!(auto.guarantee(), Guarantee::Exact, "{limit} 人");
    assert_eq!(auto.refinements(), 0);

    let game = generators::bnf(1, limit + 1, 3).unwrap();
    for domain in [Domain::Imputation, Domain::Preimputation] {
        let auto = nucleolus::nucleolus_with(&game, Options::new(domain)).unwrap();
        assert_eq!(auto.guarantee(), Guarantee::Approximate, "{domain:?}");
        assert_eq!(auto.refinements(), 0, "{domain:?}");
        assert!(!auto.is_rational(), "{domain:?}");
        let mut options = Options::new(domain);
        options.tolerance = Tolerance::Relative(1e-7);
        let fixed = nucleolus::nucleolus_with(&game, options).unwrap();
        assert_eq!(fixed.guarantee(), Guarantee::Exact, "{domain:?}");
        // 既定の手順は比 `1e-7` で 1 回だけ解いた配分をそのまま返す。
        assert_eq!(auto.allocation(), fixed.allocation(), "{domain:?}");
        assert_eq!(auto.tolerance(), fixed.tolerance(), "{domain:?}");
    }
}

/// LP を解かずに決まる配分 (配分集合が 1 点) は、確認する人数を超えても `exact` のまま。
#[test]
fn default_keeps_exact_without_lp_beyond_check_limit() {
    let n = nucleolus::MAX_AUTO_CHECK_PLAYERS + 1;
    // 加法的なゲーム: 配分集合は `x_i = v({i}) = 1` の 1 点。
    let values: Vec<f64> = (1usize..1 << n)
        .map(|mask| mask.count_ones() as f64)
        .collect();
    let game = ExplicitGame::from_binary(&values).unwrap();
    let result = nucleolus::nucleolus(&game).unwrap();
    assert_eq!(result.lp_solves(), 0, "{result:?}");
    assert_eq!(result.guarantee(), Guarantee::Exact);
}
