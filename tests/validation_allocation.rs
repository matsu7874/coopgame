//! 配分の検証 (長さ・NaN などの有限でない値) の回帰テスト。
//!
//! 長さの違う配分や NaN を含む配分で、release ビルドで誤った判定を返す・添字でパニックする・
//! 無限ループする、といった不具合があった。
//! 配分を受け取る判定・検証の関数は、長さがプレイヤー数と異なる配分と有限でない成分を含む配分を
//! 全て [`Error::InvalidArgument`] にする (「属さない」「不成立」と答えて誤りを隠さない)。

use coopgame::analysis::plot::{PlotOptions, imputation_figure};
use coopgame::analysis::uncertainty::{influence, monte_carlo};
use coopgame::game::exact::{ExactGame, Rational};
use coopgame::game::oracle::OracleGame;
use coopgame::verify::{self, VerifyOptions, kohlberg, kohlberg_exact};
use coopgame::{Coalition, Domain, Error, ExplicitGame, bargaining, kernel, properties};

/// 2 人の組だけが 4 を稼ぐ 3 人ゲーム (仁は (2, 2, 0))。
fn pair_game() -> ExplicitGame {
    ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap()
}

fn assert_invalid<T: std::fmt::Debug>(result: coopgame::Result<T>, expected: &str) {
    let err = result.unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    assert!(err.to_string().contains(expected), "{err}");
}

#[test]
fn nan_allocation_is_rejected_by_kernel_checks() {
    let game = pair_game();
    for domain in [Domain::Imputation, Domain::Preimputation] {
        assert_invalid(
            kernel::is_in_kernel(&game, &[f64::NAN; 3], domain, 1e-6),
            "有限でない",
        );
        assert_invalid(
            kernel::is_in_kernel(&game, &[2.0, 2.0, f64::NAN], domain, 1e-6),
            "有限でない",
        );
        assert_invalid(
            kernel::kernel_violation(&game, &[f64::NAN; 3], domain),
            "有限でない",
        );
    }
    // 許容誤差が NaN なら「属する」とも「属さない」とも判定せず、エラーにする。
    assert!(kernel::is_in_kernel(&game, &[2.0, 2.0, 0.0], Domain::Imputation, f64::NAN).is_err());
}

#[test]
fn nan_allocation_is_rejected_by_bargaining_check() {
    let game = pair_game();
    assert!(bargaining::check(&game, &[f64::NAN; 3], Domain::Imputation, 1e-6).is_err());
    assert!(
        bargaining::is_in_bargaining_set(&game, &[f64::INFINITY, 0.0, 0.0], Domain::Imputation)
            .is_err()
    );
}

#[test]
fn nan_allocation_is_rejected_by_kohlberg_and_check() {
    let game = pair_game();
    for domain in [Domain::Imputation, Domain::Preimputation] {
        assert_invalid(kohlberg(&game, &[f64::NAN; 3], domain), "有限でない");
        assert_invalid(
            verify::check(&game, &[f64::NAN; 3], domain, VerifyOptions::default()),
            "有限でない",
        );
        let exact = ExactGame::from_explicit(&game).unwrap();
        assert_invalid(
            verify::check_exact(
                &exact,
                &[f64::INFINITY; 3],
                domain,
                VerifyOptions::default(),
            ),
            "有限でない",
        );
    }
    // 長さの違いも同じ扱い。
    assert_invalid(kohlberg(&game, &[2.0, 2.0], Domain::Imputation), "長さ");
    assert_invalid(
        verify::check(
            &game,
            &[2.0, 2.0],
            Domain::Imputation,
            VerifyOptions::default(),
        ),
        "長さ",
    );
}

#[test]
fn plot_rejects_non_finite_points() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 3.0]).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut options = PlotOptions::default();
        options.kernel = false;
        options.points = vec![vec![bad, 0.0, 0.0]];
        let err = imputation_figure(&game, &options).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{bad}: {err:?}");
    }
    let mut options = PlotOptions::default();
    options.kernel = false;
    options.points = vec![vec![1.0, 1.0, 1.0]];
    assert!(imputation_figure(&game, &options).is_ok());
}

#[test]
fn core_membership_rejects_wrong_length_and_non_finite_values() {
    let game = pair_game();
    assert!(properties::is_in_core(&game, &[2.0, 2.0, 0.0], 1e-9).unwrap());
    assert_invalid(properties::is_in_core(&game, &[2.0, 2.0], 1e-9), "長さ");
    assert_invalid(
        properties::is_in_core(&game, &[2.0, 2.0, 0.0, 0.0], 1e-9),
        "長さ",
    );
    assert_invalid(
        properties::is_in_core(&game, &[f64::NAN; 3], 1e-9),
        "有限でない",
    );
}

#[test]
fn kohlberg_exact_rejects_wrong_length() {
    let game = ExactGame::from_explicit(&pair_game()).unwrap();
    let int = |v: i64| Rational::from_integer(v.into());
    for x in [
        vec![int(4)],
        vec![int(2), int(2)],
        vec![int(2), int(2), int(0), int(0)],
    ] {
        assert_invalid(kohlberg_exact(&game, &x, Domain::Imputation), "長さ");
    }
    assert!(
        kohlberg_exact(&game, &[int(2), int(2), int(0)], Domain::Imputation)
            .unwrap()
            .satisfied
    );
}

#[test]
fn kernel_checks_reject_wrong_length() {
    let game = pair_game();
    assert_invalid(
        kernel::kernel_violation(&game, &[2.0, 2.0], Domain::Imputation),
        "長さ",
    );
    assert_invalid(
        kernel::kernel_violation(&game, &[1.0; 4], Domain::Preimputation),
        "長さ",
    );
    assert_invalid(
        kernel::is_in_kernel(&game, &[2.0, 2.0], Domain::Imputation, 1e-6),
        "長さ",
    );
}

#[test]
#[should_panic(expected = "配分の長さ")]
fn explicit_excess_order_panics_on_wrong_length() {
    let game = pair_game();
    let _ = game.excess_order(&[1.0, 1.0]).count();
}

#[test]
fn monte_carlo_rejects_wrong_allocation_length() {
    let game = pair_game();
    let err = monte_carlo(3, 0, |_| Ok(game.clone()), |_| Ok(vec![1.0, 1.0])).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    // 標本ごとに人数が変わるのも不正。
    let mut calls = 0;
    let err = monte_carlo(
        2,
        0,
        |_| {
            calls += 1;
            if calls == 1 {
                Ok(pair_game())
            } else {
                ExplicitGame::from_binary(&[1.0])
            }
        },
        |g| Ok(vec![0.0; g.players()]),
    )
    .unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
}

#[test]
fn influence_rejects_empty_and_out_of_range_coalitions() {
    let game = pair_game();
    let solve = |g: &ExplicitGame| Ok(coopgame::values::shapley(g));
    let err = influence(&game, &[Coalition::empty()], 1e-3, solve).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    let err = influence(&game, &[Coalition::singleton(5)], 1e-3, solve).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    let err = influence(&game, &[Coalition::singleton(0)], 1e-3, |_| Ok(vec![0.0])).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    assert!(influence(&game, &[Coalition::singleton(0)], 1e-3, solve).is_ok());
}
