//! 検証 (`verify::check`・`certify`・`Unverified::verify`) の結論と、エラーを返す条件を確かめる。
//!
//! - 不正な入力 (有限でない配分、負・NaN の許容誤差) は、止まらずにエラーになる。
//! - 値の大きい整数ゲームの仁も、厳密な検証に合格する。
//! - 厳密な仁から許容誤差以内の点は合格し、許容誤差を超える点は合格しない。
//! - 基準の仁を求める LP が失敗しても、エラーではなく結論 (否定・未決) を返す。
//! - 仁の配分集合が空なら否定する。

use std::sync::mpsc;
use std::time::Duration;

use coopgame::game::exact::{ExactGame, Rational};
use coopgame::nucleolus;
use coopgame::properties::Assume;
use coopgame::verify::{self, Check, Verified, VerifyOptions};
use coopgame::{Domain, Error, ExplicitGame, Guarantee};

fn r(numerator: i64, denominator: i64) -> Rational {
    Rational::new(numerator.into(), denominator.into())
}

/// `f` を別のスレッドで実行し、10 秒以内に返らなければ失敗にする (止まらない不具合をテストの失敗にするため)。
fn within_seconds<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(f());
    });
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("10 秒以内に返らなかった")
}

fn majority() -> ExplicitGame {
    ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap()
}

/// 値が 1e7 程度の 5 人の整数ゲーム。逐次 LP の仁 (`nucleolus::nucleolus`) は数値的に失敗することがある。
fn large_integer_game() -> ExplicitGame {
    ExplicitGame::from_binary(&[
        0.0, 0.0, 20000005.0, 0.0, 20000002.0, 20000003.0, 30000003.0, 0.0, 20000000.0, 20000000.0,
        30000006.0, 20000000.0, 30000005.0, 30000005.0, 40000005.0, 0.0, 20000000.0, 20000004.0,
        30000006.0, 20000002.0, 30000003.0, 30000002.0, 40000005.0, 20000003.0, 30000004.0,
        30000004.0, 40000006.0, 30000006.0, 40000001.0, 40000004.0, 50000002.0,
    ])
    .unwrap()
}

/// [`large_integer_game`] の仁 (有理数で求めた値)。
fn large_integer_nucleolus() -> Vec<Rational> {
    vec![
        r(40000001, 4),
        r(40000001, 4),
        r(79999993, 8),
        r(40000005, 4),
        r(80000009, 8),
    ]
}

fn to_f64(x: &[Rational]) -> Vec<f64> {
    use coopgame::num_traits::ToPrimitive;
    x.iter().map(|v| v.to_f64().unwrap()).collect()
}

// ---------------------------------------------------------------- 不正な入力

#[test]
fn certify_rejects_non_finite_allocations_without_hanging() {
    for x in [
        [f64::NAN, 2.0, 2.0],
        [f64::INFINITY, f64::NEG_INFINITY, 4.0],
    ] {
        let result =
            within_seconds(move || verify::certify(&majority(), &x, Domain::Preimputation));
        assert!(
            matches!(result, Err(Error::InvalidArgument(_))),
            "{x:?}: {result:?}"
        );
        let result = within_seconds(move || {
            let exact = ExactGame::from_explicit(&majority()).unwrap();
            verify::certify_exact(&exact, &x, Domain::Imputation)
        });
        assert!(
            matches!(result, Err(Error::InvalidArgument(_))),
            "{x:?}: {result:?}"
        );
    }
}

#[test]
fn recover_allocation_rejects_invalid_tolerances_without_hanging() {
    for tolerance in [-1.0, f64::NAN, f64::INFINITY] {
        let result = within_seconds(move || {
            let game = majority();
            let exact = ExactGame::from_explicit(&game).unwrap();
            verify::recover_allocation(
                &game,
                &exact,
                &[2.0, 2.0, 0.0],
                Domain::Imputation,
                tolerance,
            )
        });
        assert!(
            matches!(result, Err(Error::InvalidArgument(_))),
            "{tolerance}: {result:?}"
        );
    }
    let result = within_seconds(|| {
        let game = majority();
        let exact = ExactGame::from_explicit(&game).unwrap();
        verify::recover_allocation(
            &game,
            &exact,
            &[f64::NAN, 2.0, 2.0],
            Domain::Imputation,
            1e-6,
        )
    });
    assert!(
        matches!(result, Err(Error::InvalidArgument(_))),
        "{result:?}"
    );
    // 正しい入力では復元できる。
    let game = majority();
    let exact = ExactGame::from_explicit(&game).unwrap();
    let recovered =
        verify::recover_allocation(&game, &exact, &[2.0, 2.0, 0.0], Domain::Imputation, 0.0)
            .unwrap();
    assert_eq!(recovered, Some(vec![r(2, 1), r(2, 1), r(0, 1)]));
}

// ---------------------------------------------------------------- 値の大きいゲーム

/// `v({0}) = 0, v({1}) = 1e7, v(N) = 1e7 + 2` の仁は `(1, 1e7 + 1)`。許容誤差 `1e-6 * 1e7 = 10` で段を読むと、
/// 2 人とも下限に張り付いたとみなして復元に失敗する。
#[test]
fn nucleolus_of_two_player_game_with_large_values_is_certified() {
    let game = ExplicitGame::from_binary(&[0.0, 1e7, 1e7 + 2.0]).unwrap();
    let exact = vec![r(1, 1), r(10000001, 1)];
    let report = verify::certify(&game, &[1.0, 1e7 + 1.0], Domain::Imputation).unwrap();
    assert!(report.satisfied, "{report:?}");
    assert_eq!(report.allocation, exact);
    let check = verify::check(
        &game,
        &[1.0, 1e7 + 1.0],
        Domain::Imputation,
        VerifyOptions::default(),
    )
    .unwrap();
    assert_eq!(
        check,
        Check::Certified {
            exact: exact.clone()
        }
    );

    // 許容誤差 (10) 以内の点は、厳密な仁で合格する。超える点は否定する。
    let check = verify::check(
        &game,
        &[1.5, 1e7 + 0.5],
        Domain::Imputation,
        VerifyOptions::default(),
    )
    .unwrap();
    assert_eq!(check, Check::Certified { exact });
    let check = verify::check(
        &game,
        &[21.0, 1e7 - 19.0],
        Domain::Imputation,
        VerifyOptions::default(),
    )
    .unwrap();
    assert!(matches!(check, Check::Refuted(_)), "{check:?}");
}

/// 超過の差が許容誤差 (`1e-6 * 5e7 = 50`) より小さい別の段がある。
#[test]
fn nucleolus_of_large_integer_game_is_certified() {
    let game = large_integer_game();
    let truth = large_integer_nucleolus();
    let x = to_f64(&truth);
    let report = verify::certify(&game, &x, Domain::Imputation).unwrap();
    assert!(report.satisfied, "{report:?}");
    assert_eq!(report.allocation, truth);
    let check = verify::check(&game, &x, Domain::Imputation, VerifyOptions::default()).unwrap();
    assert_eq!(check, Check::Certified { exact: truth });
}

/// 値が 1e7 程度のランダムな整数ゲーム (3-5 人) で、有理数で求めた仁が全て合格する。
#[test]
fn nucleoli_of_random_large_integer_games_are_certified() {
    let mut state = 7u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for case in 0..60 {
        let n = 3 + (next() % 3) as usize;
        let values: Vec<f64> = (1usize..1 << n)
            .map(|mask| {
                let size = mask.count_ones() as f64;
                if size == 1.0 {
                    0.0
                } else {
                    1e7 * size + (next() % 7) as f64
                }
            })
            .collect();
        let game = ExplicitGame::from_binary(&values).unwrap();
        let exact = ExactGame::from_explicit(&game).unwrap();
        let truth = nucleolus::exact::nucleolus(&exact, Domain::Imputation)
            .unwrap()
            .allocation;
        let check = verify::check_exact(
            &exact,
            &to_f64(&truth),
            Domain::Imputation,
            VerifyOptions::default(),
        )
        .unwrap();
        assert_eq!(check, Check::Certified { exact: truth }, "case {case}");
    }
}

// ---------------------------------------------------------------- 許容誤差の境目

/// 厳密な仁 `(1/2, 1/2, 3/2, 3/2, 2)` から、許容誤差 (`1e-6 * 6`) の 0.9 倍ずらした点。
/// 点からは厳密な仁を復元できないが、比べる基準の仁が厳密に検証できるので合格する。
#[test]
fn point_within_tolerance_of_exact_nucleolus_is_certified() {
    let game = ExplicitGame::from_binary(&[
        -2.0, 1.0, 3.0, -1.0, 4.0, 2.0, 1.0, 3.0, 4.0, 4.0, 5.0, 0.0, 0.0, 2.0, -2.0, -1.0, 4.0,
        -2.0, -1.0, 6.0, 1.0, 3.0, -1.0, 2.0, 6.0, 2.0, 1.0, 3.0, 5.0, -2.0, 6.0,
    ])
    .unwrap();
    let truth = vec![r(1, 2), r(1, 2), r(3, 2), r(3, 2), r(2, 1)];
    let tolerance = 1e-6 * game.max_abs_value();
    for (factor, certified) in [(0.9, true), (1.1, false)] {
        let shift = factor * tolerance;
        let x = [0.5, 0.5 + shift, 1.5 - shift, 1.5, 2.0];
        let check =
            verify::check(&game, &x, Domain::Preimputation, VerifyOptions::default()).unwrap();
        if certified {
            assert_eq!(
                check,
                Check::Certified {
                    exact: truth.clone()
                },
                "{factor}"
            );
        } else {
            assert!(matches!(check, Check::Refuted(_)), "{factor}: {check:?}");
        }
    }
}

// ---------------------------------------------------------------- 基準の仁を求められない場合

/// 逐次 LP の仁が数値的に失敗するゲームでも、エラーではなく結論を返す。仁から許容誤差 (50) より
/// 離れた点は合格しない。
#[test]
fn failure_of_reference_lp_is_not_an_error() {
    let game = large_integer_game();
    let exact = ExactGame::from_explicit(&game).unwrap();
    let truth = to_f64(&large_integer_nucleolus());
    for shift in [100.0, 1e4] {
        let mut x = truth.clone();
        x[0] += shift;
        x[3] -= shift;
        for check in [
            verify::check(&game, &x, Domain::Imputation, VerifyOptions::default()),
            verify::check_exact(&exact, &x, Domain::Imputation, VerifyOptions::default()),
        ] {
            let check = check.unwrap();
            assert!(
                matches!(check, Check::Refuted(_) | Check::Undecided(_)),
                "{shift}: {check:?}"
            );
        }
    }
}

/// `sum v({i}) = 3 > v(N) = 3 - 5e-6` なので配分集合は空で、仁は存在しない。和と個人合理性は
/// 許容誤差 (`1e-6 * 3`) の範囲で満たす点でも否定する。
#[test]
fn empty_imputation_set_is_refuted() {
    let grand = 3.0 - 5e-6;
    let game = ExplicitGame::from_lex(&[1.0, 1.0, 1.0, 2.0, 2.0, 2.0, grand]).unwrap();
    let x = [grand / 3.0; 3];
    let check = verify::check(&game, &x, Domain::Imputation, VerifyOptions::default()).unwrap();
    match check {
        Check::Refuted(reason) => assert!(reason.contains("配分集合が空"), "{reason}"),
        other => panic!("否定されるはず: {other:?}"),
    }
    // プレ仁は存在するので、配分集合が空であることは否定の理由にならない。
    let check = verify::check(&game, &x, Domain::Preimputation, VerifyOptions::default()).unwrap();
    assert!(matches!(check, Check::Certified { .. }), "{check:?}");
}

// ---------------------------------------------------------------- Unverified::verify

/// `verify` は結果を借りるだけなので、不正な入力でエラーになっても結果は失われない。
#[test]
fn verify_keeps_the_result_on_error() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 3.0]).unwrap();
    let unverified = nucleolus::convex::nucleolus(&Assume::convex(game.clone())).unwrap();
    let four_players = ExplicitGame::from_binary(&[0.0; 15]).unwrap();
    let error = unverified
        .verify(&four_players, VerifyOptions::default())
        .unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
    match unverified.verify(&game, VerifyOptions::default()).unwrap() {
        Verified::Certified(solution) => {
            assert_eq!(solution.guarantee(), Guarantee::Certified);
            assert_eq!(solution.allocation(), &[1.0, 1.0, 1.0]);
        }
        other => panic!("合格するはず: {other:?}"),
    }
    // 検証した後も、元の結果は仮定付きのまま残る。
    assert_eq!(
        unverified.accept_unverified().guarantee(),
        Guarantee::Assumed(coopgame::Property::Convex)
    );
}

/// `verify` は渡したゲームの仁かどうかを判定する。
#[test]
fn verify_judges_against_the_given_game() {
    let convex_game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 3.0]).unwrap();
    let unverified = nucleolus::convex::nucleolus(&Assume::convex(convex_game.clone())).unwrap();
    // 同じ 3 人で、仁が (1, 1, 1) のゲーム。
    let symmetric = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 2.0, 2.0, 2.0, 3.0]).unwrap();
    assert!(matches!(
        unverified
            .verify(&symmetric, VerifyOptions::default())
            .unwrap(),
        Verified::Certified(_)
    ));
    // 全体の値が違うゲームでは否定される。
    assert!(matches!(
        unverified
            .verify(&majority(), VerifyOptions::default())
            .unwrap(),
        Verified::Refuted { .. }
    ));
}
