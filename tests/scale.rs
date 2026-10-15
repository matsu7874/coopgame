//! 値の大きさ (スケール) によらず同じ解が求まることと、許容誤差の境界の扱いを確かめる。
//!
//! ゲームの値を `c` 倍すると、仁・カーネル・比例仁などの解も `c` 倍になる (比例仁は不満が比なので同じ比)。
//! 値の大きさに合わせた許容誤差と、LP に渡す前の正規化がこの性質を保つことを、
//! 極端な倍率 (`1e-7` から `1e7`) と、丸めの起きない 2 のべき乗の倍率で確かめる。

mod common;

use common::{assert_close, max_abs_difference};
use coopgame::game::exact::ExactGame;
use coopgame::kernel::{self, SetOptions, TransferOptions};
use coopgame::nucleolus::{self, Method, Options, convex, exact, oracle, variants};
use coopgame::num_traits::ToPrimitive;
use coopgame::properties::ConvexChecked;
use coopgame::verify::{self, Check, VerifyOptions};
use coopgame::{Domain, Error, ExplicitGame, generators};

fn scaled(game: &ExplicitGame, factor: f64) -> ExplicitGame {
    game.map_values(|_, v| v * factor).unwrap()
}

fn exact_nucleolus(game: &ExplicitGame, domain: Domain) -> Vec<f64> {
    let exact = ExactGame::from_explicit(game).unwrap();
    exact::nucleolus(&exact, domain)
        .unwrap()
        .allocation
        .iter()
        .map(|q| q.to_f64().unwrap())
        .collect()
}

fn with_method(game: &ExplicitGame, domain: Domain, method: Method) -> Vec<f64> {
    let mut options = Options::new(game, domain);
    options.method = method;
    nucleolus::nucleolus_with(game, options)
        .unwrap_or_else(|err| panic!("{method:?} {domain:?}: {err}"))
        .into_allocation()
}

/// 確定等式から決まる `x_i` が下限 `v({i})` にちょうど乗る 5 人ゲーム (ビット順)。
const DEGENERATE: [f64; 31] = [
    0.0, 0.0, -16.0, 1.0, 21.0, -6.0, 57.0, 1.0, -8.0, -2.0, 92.0, 92.0, 32.0, 66.0, 72.0, 1.0,
    14.0, -30.0, 25.0, -33.0, -33.0, 34.0, 33.0, -27.0, 84.0, 1.0, 38.0, 88.0, 61.0, -7.0, 59.0,
];

#[test]
fn nucleolus_of_degenerate_game_at_large_scales() {
    let base = ExplicitGame::from_binary(&DEGENERATE).unwrap();
    for factor in [1e4, 1e5, 1e6] {
        let game = scaled(&base, factor);
        let expected = exact_nucleolus(&game, Domain::Imputation);
        let tolerance = 1e-9 * game.max_abs_value();
        for method in [Method::Full, Method::ConstraintGeneration] {
            let x = with_method(&game, Domain::Imputation, method);
            assert_close(&x, &expected, tolerance, &format!("{method:?} x{factor:e}"));
        }
        let x = oracle::nucleolus(&game).unwrap().into_allocation();
        assert_close(&x, &expected, tolerance, &format!("oracle x{factor:e}"));
    }
    // 1e4 倍の厳密な仁 (検算用に文献値の形で書く)。
    let expected = [
        230000.0 / 3.0,
        0.0,
        230000.0 / 3.0,
        1280000.0 / 3.0,
        10000.0,
    ];
    let x = with_method(&scaled(&base, 1e4), Domain::Imputation, Method::Full);
    assert_close(&x, &expected, 1e-4, "x1e4");
}

#[test]
fn nucleolus_of_tiny_game_is_exact() {
    let values = [
        0.0, 1.0, -1.0, 0.0, -1.0, 0.0, 2.0, 0.0, 2.0, 1.0, 1.0, 2.0, -1.0, 1.0, 3.0, 0.0, 2.0,
        -1.0, 0.0, 0.0, 0.0, 1.0, -1.0, 1.0, 1.0, 3.0, -1.0, 0.0, 2.0, 0.0, 2.0,
    ];
    let base = ExplicitGame::from_binary(&values).unwrap();
    let game = scaled(&base, 1e-7);
    let expected = exact_nucleolus(&game, Domain::Imputation);
    assert_close(
        &expected,
        &[2.5e-8, 1e-7, 0.0, 5e-8, 2.5e-8],
        1e-20,
        "exact",
    );
    let tolerance = 1e-9 * game.max_abs_value();
    for method in [Method::Full, Method::ConstraintGeneration] {
        let x = with_method(&game, Domain::Imputation, method);
        assert_close(&x, &expected, tolerance, &format!("{method:?}"));
    }
    let x = oracle::nucleolus(&game).unwrap().into_allocation();
    assert_close(&x, &expected, tolerance, "oracle");
}

/// 2 のべき乗の倍率は浮動小数点数で丸めが起きないので、解もちょうど同じ倍率になる。
#[test]
fn power_of_two_scaling_gives_identical_results() {
    for seed in 0..4 {
        for kind in [1, 2, 4] {
            let base = generators::bnf(kind, 5, seed).unwrap();
            for exponent in [-40, -20, 20, 40] {
                let factor = 2f64.powi(exponent);
                let game = scaled(&base, factor);
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    let Ok(expected) =
                        nucleolus::nucleolus_with(&base, Options::new(&base, domain))
                    else {
                        continue;
                    };
                    let expected: Vec<f64> =
                        expected.allocation().iter().map(|v| v * factor).collect();
                    let actual = nucleolus::nucleolus_with(&game, Options::new(&game, domain))
                        .unwrap()
                        .into_allocation();
                    assert_eq!(actual, expected, "bnf{kind} seed {seed} 2^{exponent}");
                    let oracle_actual =
                        oracle::nucleolus_with(&game, domain, oracle::default_tolerance(&game))
                            .unwrap()
                            .into_allocation();
                    let oracle_expected: Vec<f64> =
                        oracle::nucleolus_with(&base, domain, oracle::default_tolerance(&base))
                            .unwrap()
                            .allocation()
                            .iter()
                            .map(|v| v * factor)
                            .collect();
                    assert_eq!(
                        oracle_actual, oracle_expected,
                        "oracle bnf{kind} seed {seed}"
                    );
                }
            }
        }
    }
}

#[test]
fn nucleolus_is_scale_invariant_against_exact() {
    for seed in 0..6 {
        for kind in [1, 2, 3, 4] {
            let base = generators::bnf(kind, 5, seed).unwrap();
            for factor in [1e-7, 1e-3, 1e3, 1e7] {
                let game = scaled(&base, factor);
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    let exact = ExactGame::from_explicit(&game).unwrap();
                    let Ok(expected) = exact::nucleolus(&exact, domain) else {
                        continue;
                    };
                    let expected: Vec<f64> = expected
                        .allocation
                        .iter()
                        .map(|q| q.to_f64().unwrap())
                        .collect();
                    let tolerance = 1e-6 * game.max_abs_value();
                    for method in [Method::Full, Method::ConstraintGeneration] {
                        let x = with_method(&game, domain, method);
                        assert_close(
                            &x,
                            &expected,
                            tolerance,
                            &format!("bnf{kind} seed {seed} x{factor:e} {method:?} {domain:?}"),
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn proportional_nucleolus_is_scale_invariant() {
    // 4 人ゲーム (ビット順)。
    let base = ExplicitGame::from_binary(&[
        1.0, 1.0, 25.0, 2.0, 7.0, 0.0, 21.0, 2.0, 1.0, 0.0, 9.0, 0.0, 17.0, 30.0, 13.0,
    ])
    .unwrap();
    let unit = variants::proportional_nucleolus(&base, Domain::Imputation).unwrap();
    for factor in [1e-6, 1e3, 1e6] {
        let game = scaled(&base, factor);
        let result = variants::proportional_nucleolus(&game, Domain::Imputation).unwrap();
        let expected: Vec<f64> = unit.allocation().iter().map(|v| v * factor).collect();
        assert_close(
            result.allocation(),
            &expected,
            1e-7 * game.max_abs_value(),
            &format!("x{factor:e}"),
        );
        // 不満は比なので、段の値は倍率によらない。
        assert_close(result.levels(), unit.levels(), 1e-7, "levels");
    }
    let x = variants::proportional_nucleolus(&scaled(&base, 1e3), Domain::Imputation).unwrap();
    assert_close(
        x.allocation(),
        &[2148.936, 6851.064, 2000.0, 2000.0],
        1e-3,
        "x1e3",
    );

    // 3 人ゲーム (ビット順) の 1e6 倍。
    let base = ExplicitGame::from_binary(&[0.0, 0.0, 11.0, 1.0, 30.0, 0.0, 23.0]).unwrap();
    let unit = variants::proportional_nucleolus(&base, Domain::Imputation).unwrap();
    let game = scaled(&base, 1e6);
    let result = variants::proportional_nucleolus(&game, Domain::Imputation).unwrap();
    let expected: Vec<f64> = unit.allocation().iter().map(|v| v * 1e6).collect();
    assert_close(result.allocation(), &expected, 1e-1, "n=3 x1e6");
    assert_close(
        result.allocation(),
        &[21.08e6, 0.0, 1.92e6],
        1e4,
        "n=3 x1e6",
    );
}

#[test]
fn disruption_nucleolus_is_scale_invariant() {
    // CoopGame 0.2.2 の disruptionNucleolus のヘルプの例。
    let base = ExplicitGame::from_lex(&[
        0.0, 0.0, 0.0, 0.0, 2.0, 3.0, 4.0, 1.0, 3.0, 2.0, 8.0, 11.0, 6.5, 9.5, 14.0,
    ])
    .unwrap();
    let expected = [3.193548, 4.754839, 2.129032, 3.922581];
    for factor in [1e-6, 1e3, 1e6] {
        let game = scaled(&base, factor);
        let result = variants::disruption_nucleolus(&game).unwrap();
        let expected: Vec<f64> = expected.iter().map(|v| v * factor).collect();
        assert_close(
            result.allocation(),
            &expected,
            1e-6 * factor,
            &format!("x{factor:e}"),
        );
    }
}

#[test]
fn variants_are_scale_invariant_on_random_games() {
    type Variant = fn(&ExplicitGame) -> coopgame::Result<Vec<f64>>;
    let variants: [(&str, Variant); 4] = [
        ("per_capita", |g| {
            variants::per_capita_nucleolus(g, Domain::Imputation).map(|r| r.into_allocation())
        }),
        ("proportional", |g| {
            variants::proportional_nucleolus(g, Domain::Imputation).map(|r| r.into_allocation())
        }),
        ("disruption", |g| {
            variants::disruption_nucleolus(g).map(|r| r.into_allocation())
        }),
        ("modiclus", |g| {
            variants::modiclus(g).map(|r| r.into_allocation())
        }),
    ];
    for seed in 0..6 {
        // 非負で凸に近いゲーム (比例仁・disruption nucleolus の対象)。
        let base = generators::random_convex(4, seed).unwrap();
        for (name, solve) in variants {
            let unit = solve(&base).unwrap_or_else(|err| panic!("{name} seed {seed}: {err}"));
            for factor in [1e-6, 1e3, 1e6] {
                let game = scaled(&base, factor);
                let x = solve(&game)
                    .unwrap_or_else(|err| panic!("{name} seed {seed} x{factor:e}: {err}"));
                let expected: Vec<f64> = unit.iter().map(|v| v * factor).collect();
                assert_close(
                    &x,
                    &expected,
                    1e-6 * game.max_abs_value(),
                    &format!("{name} seed {seed} x{factor:e}"),
                );
            }
        }
    }
}

#[test]
fn convex_transfer_scheme_is_accurate_for_small_values() {
    for seed in [21, 22, 42] {
        let n = 2 + (seed % 10) as usize;
        for factor in [1e-6, 1e6] {
            let game = scaled(&generators::random_convex(n, seed).unwrap(), factor);
            let expected = exact_nucleolus(&game, Domain::Imputation);
            let checked = ConvexChecked::new(game.clone()).expect("凸ゲーム");
            let solution = convex::nucleolus(&checked).unwrap();
            assert!(
                max_abs_difference(solution.allocation(), &expected) <= 1e-7 * game.max_abs_value(),
                "seed {seed} x{factor:e}: {:?} != {expected:?}",
                solution.allocation()
            );
        }
    }
}

#[test]
fn single_player_games() {
    let game = ExplicitGame::from_binary(&[5.0]).unwrap();
    assert!(nucleolus::has_nonempty_core(&game).unwrap());
    for domain in [Domain::Imputation, Domain::Preimputation] {
        let least = nucleolus::least_core(&game, domain).unwrap();
        assert_eq!(least.allocation(), &[5.0]);
        assert_eq!(least.epsilon(), f64::NEG_INFINITY);
        let least = oracle::least_core(&game, domain).unwrap();
        assert_eq!(least.allocation(), &[5.0]);
        assert_eq!(least.epsilon(), f64::NEG_INFINITY);
    }
    let x = variants::disruption_nucleolus(&game).unwrap();
    assert_eq!(x.allocation(), &[5.0]);
}

#[test]
fn nearly_empty_imputation_set_is_handled_consistently() {
    // sum v({i}) - v(N) = 5e-8 は既定の許容誤差 (3e-7) 以下なので、配分集合は空とみなさない。
    // 下限を差の等分だけ下げた 1 点 (kernel_point の初期点と同じ) を返す。
    let game = ExplicitGame::from_binary(&[1.0, 2.0, 3.0 - 5e-8]).unwrap();
    let expected = [1.0 - 2.5e-8, 2.0 - 2.5e-8];
    let x = nucleolus::nucleolus(&game).unwrap().into_allocation();
    assert_close(&x, &expected, 1e-15, "nucleolus");
    let x = oracle::nucleolus(&game).unwrap().into_allocation();
    assert_close(&x, &expected, 1e-15, "oracle");
    let x = variants::per_capita_nucleolus(&game, Domain::Imputation)
        .unwrap()
        .into_allocation();
    assert_close(&x, &expected, 1e-15, "per_capita");
    let least = nucleolus::least_core(&game, Domain::Imputation).unwrap();
    assert_close(least.allocation(), &expected, 1e-15, "least_core");

    // 許容誤差を超えて空なら、どの手法も EmptyImputationSet を返す。
    let empty = ExplicitGame::from_binary(&[1.0, 2.0, 3.0 - 1e-5]).unwrap();
    assert_eq!(nucleolus::nucleolus(&empty), Err(Error::EmptyImputationSet));
    assert!(matches!(
        oracle::nucleolus(&empty),
        Err(Error::EmptyImputationSet)
    ));
    assert!(matches!(
        variants::per_capita_nucleolus(&empty, Domain::Imputation),
        Err(Error::EmptyImputationSet)
    ));
    assert!(matches!(
        kernel::kernel_set(&empty, Domain::Imputation, SetOptions::for_game(&empty)),
        Err(Error::EmptyImputationSet)
    ));

    // 3 人でも同じ。
    let game = ExplicitGame::from_lex(&[1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0 - 5e-8]).unwrap();
    let x = nucleolus::nucleolus(&game).unwrap().into_allocation();
    assert_close(&x, &[1.0 - 5e-8 / 3.0; 3], 1e-15, "n=3");
}

#[test]
fn kernel_set_agrees_with_kernel_point_on_nearly_empty_imputation_set() {
    // sum v({i}) - v(N) = 1e-8。同じ許容誤差なら、kernel_set と kernel_point は空かどうかの判断が一致し、
    // 空でなければ同じ 1 点を返す。
    let game = ExplicitGame::from_binary(&[1.0, 1.0, 2.0 - 1e-8]).unwrap();
    for tolerance in [1e-9, 1e-7] {
        let mut set_options = SetOptions::for_game(&game);
        set_options.tolerance = tolerance;
        let mut transfer = TransferOptions::for_game(&game);
        transfer.tolerance = tolerance;
        let set = kernel::kernel_set(&game, Domain::Imputation, set_options);
        let point = kernel::kernel_point(&game, Domain::Imputation, None, transfer);
        match (set, point) {
            (Err(Error::EmptyImputationSet), Err(Error::EmptyImputationSet)) => {
                assert!(tolerance < 1e-8);
            }
            (Ok(set), Ok(point)) => {
                assert!(tolerance >= 1e-8);
                assert_eq!(set.pieces.len(), 1, "{set:?}");
                assert!(set.contains(point.allocation(), 1e-12), "{set:?} {point:?}");
            }
            (set, point) => panic!("tolerance {tolerance}: {set:?} / {point:?}"),
        }
    }
}

#[test]
fn kernel_set_is_scale_invariant() {
    let base = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
    for factor in [1e-7, 1.0, 3e6, 1e8] {
        let game = scaled(&base, factor);
        let set =
            kernel::kernel_set(&game, Domain::Imputation, SetOptions::for_game(&game)).unwrap();
        assert_eq!(set.pieces.len(), 1, "x{factor:e}: {set:?}");
        let piece = &set.pieces[0];
        assert_eq!(piece.dimension, 0);
        assert_close(
            &piece.vertices.as_ref().unwrap()[0],
            &[2.0 * factor, 2.0 * factor, 0.0],
            1e-9 * factor,
            &format!("x{factor:e}"),
        );
    }
    // 多面体の形 (次元と頂点の数) も倍率によらない。
    for seed in 0..12 {
        let base = generators::bnf(1 + (seed % 3) as u8, 4, seed).unwrap();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let shape = |game: &ExplicitGame| {
                kernel::kernel_set(game, domain, SetOptions::for_game(game)).map(|set| {
                    set.pieces
                        .iter()
                        .map(|piece| (piece.dimension, piece.vertices.as_ref().map(Vec::len)))
                        .collect::<Vec<_>>()
                })
            };
            let unit = shape(&base);
            for factor in [1e-6, 1e8] {
                assert_eq!(
                    shape(&scaled(&base, factor)),
                    unit,
                    "seed {seed} x{factor:e}"
                );
            }
        }
    }
}

#[test]
fn certification_rejects_points_beyond_tolerance() {
    let game = ExplicitGame::from_binary(&[
        0.0, 0.0, 15.0, 0.0, 18.0, 2.0, 19.0, 0.0, 1.0, 7.0, 18.0, 6.0, 16.0, 18.0, 4.0, 0.0, 15.0,
        11.0, 6.0, 9.0, 8.0, 10.0, 7.0, 10.0, 1.0, 17.0, 3.0, 7.0, 18.0, 11.0, 3.0,
    ])
    .unwrap();
    let nucleolus = [1.0, 1.0, 1.0, 0.0, 0.0];
    // 仁から max|v| * 1e-6 (= 1.9e-5) より離れた点。
    let far = [
        0.9999841272027151,
        0.9999909802939532,
        0.9999898790344425,
        -3.482426073439582e-7,
        3.536171149653298e-5,
    ];
    let tolerance = 1e-6 * game.max_abs_value();
    assert!(max_abs_difference(&far, &nucleolus) > tolerance);
    let report = verify::certify(&game, &far, Domain::Imputation).unwrap();
    assert!(!report.satisfied, "{report:?}");
    let check = verify::check(&game, &far, Domain::Imputation, VerifyOptions::default()).unwrap();
    assert!(!matches!(check, Check::Certified { .. }), "{check:?}");

    // 許容誤差の内側の点は、厳密な仁に復元されて合格する。
    let near: Vec<f64> = nucleolus
        .iter()
        .zip(&far)
        .map(|(a, b)| a + 0.25 * (b - a))
        .collect();
    assert!(
        verify::certify(&game, &near, Domain::Imputation)
            .unwrap()
            .satisfied
    );
    let check = verify::check(&game, &near, Domain::Imputation, VerifyOptions::default()).unwrap();
    assert!(matches!(check, Check::Certified { .. }), "{check:?}");
}

#[test]
fn tau_value_and_gately_point_are_scale_invariant() {
    use coopgame::compromise::{gately_point, tau_value};
    // 2 のべき乗の倍率なら丸めが起きないので、許容誤差が値に比例していれば同じ判定になる。
    let factor = 2f64.powi(-40);
    for seed in 0..20 {
        let base = generators::random_convex(4, seed).unwrap();
        let game = scaled(&base, factor);
        let expected: Vec<f64> = tau_value(&base)
            .unwrap()
            .iter()
            .map(|v| v * factor)
            .collect();
        assert_close(
            &tau_value(&game).unwrap(),
            &expected,
            1e-9 * game.max_abs_value(),
            "tau",
        );
        let expected: Vec<f64> = gately_point(&base)
            .unwrap()
            .iter()
            .map(|v| v * factor)
            .collect();
        assert_close(
            &gately_point(&game).unwrap(),
            &expected,
            1e-9 * game.max_abs_value(),
            "gately",
        );
    }
}

#[cfg(feature = "io")]
#[test]
fn partial_game_nucleolus_of_tiny_game_is_exact() {
    use coopgame::io::parse_json;
    let values = [
        0.0, 1.0, -1.0, 0.0, -1.0, 0.0, 2.0, 0.0, 2.0, 1.0, 1.0, 2.0, -1.0, 1.0, 3.0, 0.0, 2.0,
        -1.0, 0.0, 0.0, 0.0, 1.0, -1.0, 1.0, 1.0, 3.0, -1.0, 0.0, 2.0, 0.0, 2.0,
    ];
    let game = scaled(&ExplicitGame::from_binary(&values).unwrap(), 1e-7);
    let entries: Vec<String> = (1..32u32)
        .map(|mask| {
            let members: Vec<String> = (0..5)
                .filter(|i| mask >> i & 1 == 1)
                .map(|i| i.to_string())
                .collect();
            format!(
                "\"{}\": {:e}",
                members.join(","),
                values[mask as usize - 1] * 1e-7
            )
        })
        .collect();
    let json = format!("{{\"players\": 5, \"values\": {{{}}}}}", entries.join(", "));
    let partial = parse_json(&json).unwrap().partial().unwrap();
    let x = partial
        .nucleolus(Domain::Imputation)
        .unwrap()
        .into_allocation();
    let expected = exact_nucleolus(&game, Domain::Imputation);
    assert_close(&x, &expected, 1e-9 * game.max_abs_value(), "partial");
}
