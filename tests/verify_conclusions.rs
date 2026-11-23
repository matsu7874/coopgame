//! 検証 (`verify::check`・`certify`・`Unverified::verify`) の結論と、エラーを返す条件を確かめる。
//!
//! - 不正な入力 (有限でない配分、負・NaN の許容誤差) は、止まらずにエラーになる。
//! - 値の大きい整数ゲームの仁も、厳密な検証に合格する。
//! - 厳密な仁から許容誤差以内の点は合格し、許容誤差を超える点は合格しない。
//! - 基準の仁が厳密でなくても、基準の仁を求める LP が失敗しても、エラーではなく結論 (否定・未決) を返す。
//! - 仁の配分集合が空なら否定する。空かどうかの判定は、浮動小数点数のゲームでは逐次 LP と揃え、
//!   有理数のゲームでは有理数で行う。

mod common;

use std::sync::mpsc;
use std::time::Duration;

use coopgame::game::exact::{ExactGame, Rational};
use coopgame::nucleolus;
use coopgame::properties::Assume;
use coopgame::verify::{self, Check, Reference, Verified, VerifyOptions};
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

/// 値が 1e7 程度の 5 人の整数ゲーム。超過の差が小さく、逐次 LP の仁 (`nucleolus::nucleolus`) は
/// 厳密な仁 ([`large_integer_nucleolus`]) と既定の許容誤差の範囲で異なる。
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
        // 候補自身の超過の段から復元できること (基準の仁に頼らない) を確かめる。
        let report = verify::certify_exact(&exact, &to_f64(&truth), Domain::Imputation).unwrap();
        assert!(report.satisfied, "case {case}: {report:?}");
        assert_eq!(report.allocation, truth, "case {case}");
    }
}

/// 値が 1e12 程度の 6 人の整数ゲーム。許容誤差 (`6e6`) と超過の丸め誤差の目安 (`0.064`) の比が大きく、
/// 超過の段の間隔 (1 以下) は、目安のすぐ上の許容誤差で読まないと分かれない。
/// 厳密な仁での超過の段の間隔が目安の 2 倍を超えるゲームでは、有理数で求めた仁が合格する。
#[test]
fn nucleoli_of_integer_games_near_rounding_limit_are_certified() {
    use coopgame::Coalition;
    use coopgame::num_traits::{ToPrimitive, Zero};
    let mut state = 11u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let n = 6;
    let mut separable = 0;
    for case in 0..24 {
        let values: Vec<f64> = (1usize..1 << n)
            .map(|mask| {
                let size = mask.count_ones() as f64;
                if size == 1.0 {
                    0.0
                } else {
                    1e12 * size + (next() % 7) as f64
                }
            })
            .collect();
        let game = ExplicitGame::from_binary(&values).unwrap();
        let exact = ExactGame::from_explicit(&game).unwrap();
        let truth = nucleolus::exact::nucleolus(&exact, Domain::Imputation)
            .unwrap()
            .allocation;
        let x = to_f64(&truth);
        // 厳密な仁での超過 (有理数) の、異なる値の間隔の最小値。
        let mut excesses: Vec<Rational> = (1u64..(1 << n) - 1)
            .map(|mask| {
                let sum = (0..n)
                    .filter(|i| mask >> i & 1 == 1)
                    .fold(Rational::zero(), |sum, i| sum + &truth[i]);
                exact.value(Coalition(mask)) - sum
            })
            .collect();
        excesses.sort();
        excesses.dedup();
        let gap = excesses
            .windows(2)
            .map(|pair| (&pair[1] - &pair[0]).to_f64().unwrap())
            .fold(f64::INFINITY, f64::min);
        let magnitude = game.max_abs_value() + x.iter().map(|v| v.abs()).sum::<f64>();
        let rounding = 4.0 * n as f64 * f64::EPSILON * magnitude;
        if gap <= 2.0 * rounding {
            continue;
        }
        separable += 1;
        // 候補自身の超過の段から復元できること (基準の仁に頼らない) を確かめる。
        let report = verify::certify_exact(&exact, &x, Domain::Imputation).unwrap();
        assert!(report.satisfied, "case {case}: {report:?}");
        assert_eq!(report.allocation, truth, "case {case}");
    }
    // 半数以上のゲームは段の間隔が目安の 2 倍を超える (条件を満たすゲームがなければ、何も確かめていない)。
    assert!(separable >= 12, "{separable}");
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

// ---------------------------------------------------------------- 基準の仁が厳密でない・求められない場合

/// 値が 1e7 程度の整数ゲームでは、基準の逐次 LP が許容誤差を下げて解き直すので、基準を逐次 LP の仁だけにしても
/// (`Reference::Lp`) 厳密な仁を復元でき、許容誤差 (50) を超えて離れた点を否定する。
#[test]
fn refined_lp_reference_refutes_points_beyond_tolerance() {
    let game = large_integer_game();
    let exact = ExactGame::from_explicit(&game).unwrap();
    let truth = to_f64(&large_integer_nucleolus());
    let mut lp_only = VerifyOptions::default();
    lp_only.reference = Reference::Lp;
    for shift in [100.0, 1e4] {
        let mut x = truth.clone();
        x[0] += shift;
        x[3] -= shift;
        for options in [lp_only, VerifyOptions::default()] {
            for check in [
                verify::check(&game, &x, Domain::Imputation, options),
                verify::check_exact(&exact, &x, Domain::Imputation, options),
            ] {
                match check.unwrap() {
                    Check::Refuted(reason) => {
                        assert!(reason.contains("厳密に検証した仁"), "{shift}: {reason}");
                    }
                    other => panic!("{shift}: {other:?}"),
                }
            }
        }
    }
}

/// 浮動小数点数の和で作ったゲームでは、有理数では等しくない超過が近接し、逐次 LP の仁から厳密な配分を
/// 復元できないことがある (4 人、seed 0)。基準を逐次 LP の仁だけにする (`Reference::Lp`) と、浮動小数点の
/// Kohlberg 判定に合格した LP の仁が基準になる。基準との差が許容誤差の 100 倍以下の点は未決、それを超える点は否定する。
#[test]
fn point_is_judged_against_float_verified_reference() {
    let game = common::float_sum_superadditive(4, 0);
    let lp = nucleolus::nucleolus(&game).unwrap().into_allocation();
    assert!(
        !verify::certify(&game, &lp, Domain::Imputation)
            .unwrap()
            .satisfied
    );
    let tolerance = 1e-6 * game.max_abs_value();
    let mut lp_only = VerifyOptions::default();
    lp_only.reference = Reference::Lp;
    for (shift, refuted) in [(10.0 * tolerance, false), (1000.0 * tolerance, true)] {
        let mut x = lp.clone();
        x[0] += shift;
        x[3] -= shift;
        match verify::check(&game, &x, Domain::Imputation, lp_only).unwrap() {
            Check::Refuted(reason) if refuted => {
                assert!(
                    reason.contains("浮動小数点で検証した仁"),
                    "{shift}: {reason}"
                );
            }
            Check::Undecided(reason) if !refuted => {
                assert!(
                    reason.contains("浮動小数点で検証した仁"),
                    "{shift}: {reason}"
                );
            }
            other => panic!("{shift}: {other:?}"),
        }
    }
}

/// 基準の仁を求める逐次 LP が失敗するゲームでは、基準を逐次 LP の仁だけにするとエラーではなく未決を返す。
///
/// `v({1}) = -M`、`v({2}) = v({3}) = M`、他の提携は `-M` (`M = 1.5e308`) のゲームのプレ仁は
/// `(-5M/3, M/3, M/3)` で、`-5M/3` は f64 で表せない。逐次 LP は [`Error::Numerical`] を返す。
/// 既定 (`Reference::Auto`) では有理数の逐次 LP がプレ仁を求め、離れた点を否定する。
#[test]
fn failure_of_reference_lp_is_undecided() {
    let m = 1.5e308;
    let game = ExplicitGame::from_lex(&[-m, m, m, -m, -m, -m, -m]).unwrap();
    assert!(matches!(
        nucleolus::prenucleolus(&game),
        Err(Error::Numerical(_))
    ));
    let exact = ExactGame::from_explicit(&game).unwrap();
    let x = [-m, 0.0, 0.0];
    let mut lp_only = VerifyOptions::default();
    lp_only.reference = Reference::Lp;
    for check in [
        verify::check(&game, &x, Domain::Preimputation, lp_only),
        verify::check_exact(&exact, &x, Domain::Preimputation, lp_only),
    ] {
        match check.unwrap() {
            Check::Undecided(reason) => {
                assert!(reason.contains("基準の仁も求められない"), "{reason}");
            }
            other => panic!("未決のはず: {other:?}"),
        }
    }
    for check in [
        verify::check(&game, &x, Domain::Preimputation, VerifyOptions::default()),
        verify::check_exact(&exact, &x, Domain::Preimputation, VerifyOptions::default()),
    ] {
        match check.unwrap() {
            Check::Refuted(reason) => assert!(reason.contains("厳密に検証した仁"), "{reason}"),
            other => panic!("否定のはず: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------- 基準の仁の求め方

/// 値が 1e9 程度の整数ゲームでは、逐次 LP の仁は厳密な仁から値の単位で数個ずれ、厳密な配分を復元できない。
/// 基準を逐次 LP の仁だけにすると未決になるゲームがあるが、有理数の逐次 LP で基準を求め直せば、
/// 厳密な仁との差で結論が出る (既定の `Reference::Auto` は 8 人以下、`Reference::Exact` は 10 人以下で求め直す)。
#[test]
fn rational_reference_decides_what_lp_reference_cannot() {
    let mut state = 5u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let with_reference = |reference: Reference| {
        let mut options = VerifyOptions::default();
        options.reference = reference;
        options
    };
    assert_eq!(VerifyOptions::default().reference, Reference::Auto);
    let mut undecided_by_lp = 0;
    for case in 0..40 {
        let n = 3 + case % 4;
        let values: Vec<f64> = (1usize..1 << n)
            .map(|mask| {
                let size = mask.count_ones() as f64;
                if size == 1.0 {
                    0.0
                } else {
                    1e9 * size + (next() % 7) as f64
                }
            })
            .collect();
        let game = ExplicitGame::from_binary(&values).unwrap();
        let exact = ExactGame::from_explicit(&game).unwrap();
        let truth = nucleolus::exact::nucleolus(&exact, Domain::Imputation)
            .unwrap()
            .allocation;
        // 厳密な仁から、許容誤差 (1e-6 * max|v|、3000 以上) より十分小さい 2 だけずらした点。
        // 超過の段が崩れるので、この点自身からは厳密な配分を復元できない。
        let mut near = to_f64(&truth);
        near[0] += 2.0;
        near[1] -= 2.0;
        let by_lp = verify::check(
            &game,
            &near,
            Domain::Imputation,
            with_reference(Reference::Lp),
        )
        .unwrap();
        assert!(
            !matches!(by_lp, Check::Refuted(_)),
            "case {case}: {by_lp:?}"
        );
        if matches!(by_lp, Check::Undecided(_)) {
            undecided_by_lp += 1;
        }
        for reference in [Reference::Auto, Reference::Exact] {
            let check =
                verify::check(&game, &near, Domain::Imputation, with_reference(reference)).unwrap();
            assert_eq!(
                check,
                Check::Certified {
                    exact: truth.clone()
                },
                "case {case} {reference:?}"
            );
        }
        // 許容誤差を超えて離れた点は、厳密な仁との差で否定する。
        let mut far = to_f64(&truth);
        far[0] += 1e5;
        far[1] -= 1e5;
        let check =
            verify::check(&game, &far, Domain::Imputation, VerifyOptions::default()).unwrap();
        match check {
            Check::Refuted(reason) => {
                assert!(reason.contains("厳密に検証した仁"), "case {case}: {reason}");
            }
            other => panic!("case {case}: {other:?}"),
        }
    }
    // 逐次 LP の仁だけでは未決になるゲームがある (なければ、求め直しを確かめていない)。
    assert!(undecided_by_lp >= 10, "{undecided_by_lp}");
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

/// 10 進数の値 0.1・0.2・0.3 は、2 進数の値では和が 0.6 をわずかに超える。有理数の値では配分集合が空だが、
/// 逐次 LP は既定の許容誤差以内の差を空とみなさず、配分集合が 1 点のゲームとして解く。
/// その解を否定しない (厳密には検証できないので未決)。1 点から離れた点は個人合理性で否定する。
#[test]
fn imputation_set_empty_within_solver_tolerance_is_undecided() {
    let game = ExplicitGame::from_lex(&[0.1, 0.2, 0.3, 0.3, 0.4, 0.5, 0.6]).unwrap();
    let solved = nucleolus::nucleolus(&game).unwrap();
    let check = verify::check(
        &game,
        solved.allocation(),
        Domain::Imputation,
        VerifyOptions::default(),
    )
    .unwrap();
    match &check {
        Check::Undecided(reason) => assert!(reason.contains("配分集合"), "{reason}"),
        other => panic!("未決のはず: {other:?}"),
    }
    let check = verify::check(
        &game,
        &[0.3, 0.2, 0.1],
        Domain::Imputation,
        VerifyOptions::default(),
    )
    .unwrap();
    assert!(matches!(check, Check::Refuted(_)), "{check:?}");
}

/// `sum v({i})` が `v(N)` を超える量を変えても、逐次 LP が返した仁は否定しない。逐次 LP が配分集合を空とする
/// ゲームでは、配分集合が空であることを理由に否定する。
#[test]
fn check_agrees_with_solver_on_empty_imputation_set() {
    for excess in [0.0, 1e-16, 1e-12, 1e-9, 5e-8, 1e-6, 1e-3] {
        let grand = 3.0 - excess;
        let game = ExplicitGame::from_lex(&[1.0, 1.0, 1.0, 2.0, 2.0, 2.0, grand]).unwrap();
        match nucleolus::nucleolus(&game) {
            Ok(solved) => {
                let check = verify::check(
                    &game,
                    solved.allocation(),
                    Domain::Imputation,
                    VerifyOptions::default(),
                )
                .unwrap();
                assert!(!matches!(check, Check::Refuted(_)), "{excess}: {check:?}");
            }
            Err(Error::EmptyImputationSet) => {
                let x = [grand / 3.0; 3];
                let check =
                    verify::check(&game, &x, Domain::Imputation, VerifyOptions::default()).unwrap();
                match check {
                    Check::Refuted(reason) => {
                        assert!(reason.contains("配分集合が空"), "{excess}: {reason}");
                    }
                    other => panic!("{excess}: 否定されるはず: {other:?}"),
                }
            }
            Err(error) => panic!("{excess}: {error}"),
        }
    }
}

/// 有理数で作ったゲームは値に誤差がないので、配分集合が空かどうかを有理数で判定する。
/// `sum v({i}) = v(N)` なら 1 点の配分が仁として合格し、わずかでも超えれば否定する。
#[test]
fn rational_game_decides_empty_imputation_set_exactly() {
    let values = |grand: Rational| {
        vec![
            r(1, 10),
            r(1, 5),
            r(3, 10),
            r(3, 10),
            r(2, 5),
            r(1, 2),
            grand,
        ]
    };
    let x = [0.1, 0.2, 0.3];
    let single_point = ExactGame::from_lex(values(r(3, 5))).unwrap();
    let check = verify::check_exact(
        &single_point,
        &x,
        Domain::Imputation,
        VerifyOptions::default(),
    )
    .unwrap();
    match check {
        Check::Certified { exact } => assert_eq!(exact, vec![r(1, 10), r(1, 5), r(3, 10)]),
        other => panic!("合格するはず: {other:?}"),
    }
    let empty = ExactGame::from_lex(values(r(3, 5) - r(1, 1_000_000_000_000_000_000))).unwrap();
    let check =
        verify::check_exact(&empty, &x, Domain::Imputation, VerifyOptions::default()).unwrap();
    match check {
        Check::Refuted(reason) => assert!(reason.contains("配分集合が空"), "{reason}"),
        other => panic!("否定されるはず: {other:?}"),
    }
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
