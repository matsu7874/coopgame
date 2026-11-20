//! 逐次 LP の仁 (`nucleolus::nucleolus_with` など) の入力と結果の検証。
//!
//! - 許容誤差は有限で 0 以上でなければ `InvalidArgument` にする (NaN では比較が常に偽になり、
//!   空の配分集合でも配分を返してしまうため)。
//! - 元の大きさに戻した配分・最大超過・epsilon が f64 で表せなければ `Numerical` にする。

use coopgame::nucleolus::{self, Options};
use coopgame::{Domain, Error, ExplicitGame};

const DOMAINS: [Domain; 2] = [Domain::Imputation, Domain::Preimputation];

#[test]
fn non_finite_or_negative_tolerance_is_rejected() {
    // 配分集合が空のゲーム (sum v({i}) = 9 > v(N) = 8) と、仁が (24, 24, 24) のゲーム。
    let empty = ExplicitGame::from_lex(&[3.0, 3.0, 3.0, 6.0, 6.0, 6.0, 8.0]).unwrap();
    let symmetric = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 60.0, 60.0, 60.0, 72.0]).unwrap();
    for game in [&empty, &symmetric] {
        for tolerance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -1e-12] {
            for domain in DOMAINS {
                let mut options = Options::new(game, domain);
                options.tolerance = tolerance;
                let result = nucleolus::nucleolus_with(game, options);
                assert!(
                    matches!(result, Err(Error::InvalidArgument(_))),
                    "tolerance {tolerance} {domain:?}: {result:?}"
                );
            }
        }
    }
}

#[test]
fn zero_tolerance_is_accepted() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 60.0, 60.0, 60.0, 72.0]).unwrap();
    for domain in DOMAINS {
        let mut options = Options::new(&game, domain);
        options.tolerance = 0.0;
        let result = nucleolus::nucleolus_with(&game, options).unwrap();
        assert_eq!(result.allocation(), &[24.0, 24.0, 24.0]);
    }
}

#[test]
fn tolerance_too_large_for_the_values_is_rejected() {
    // 値が 1e-300 程度のゲームでは、許容誤差 1e10 を値の大きさで割ると f64 で表せない。
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1e-300, 1e-300, 1e-300, 1.2e-300]).unwrap();
    let mut options = Options::new(&game, Domain::Preimputation);
    options.tolerance = 1e10;
    assert!(matches!(
        nucleolus::nucleolus_with(&game, options),
        Err(Error::InvalidArgument(_))
    ));
}

#[test]
fn results_beyond_f64_range_are_numerical_errors() {
    // M = 1.5e308: プレ仁は (-5M/3, M/3, M/3) で、-5M/3 は f64 の上限を超える。
    let m = 1.5e308;
    let game = ExplicitGame::from_lex(&[-m, m, m, -m, -m, -m, -m]).unwrap();
    assert!(matches!(
        nucleolus::prenucleolus(&game),
        Err(Error::Numerical(_))
    ));
    assert!(matches!(
        nucleolus::least_core(&game, Domain::Preimputation),
        Err(Error::Numerical(_))
    ));
    // コアが空か (x_1, x_2 >= M なら x_0 <= -3M < v({0})) は、値の大きさによらず判定できる。
    assert!(!nucleolus::has_nonempty_core(&game).unwrap());

    // 配分は表せるが、最大超過 (と最小コアの epsilon) が f64 の上限を超える。
    let big = f64::MAX;
    let game = ExplicitGame::from_lex(&[0.0, 0.0, big, big, big, big, -big]).unwrap();
    assert!(matches!(
        nucleolus::prenucleolus(&game),
        Err(Error::Numerical(_))
    ));
    assert!(matches!(
        nucleolus::least_core(&game, Domain::Preimputation),
        Err(Error::Numerical(_))
    ));

    // M = 1e308 なら全ての結果が有限で表せる。
    let m = 1e308;
    let game = ExplicitGame::from_lex(&[-m, m, m, -m, -m, -m, -m]).unwrap();
    let result = nucleolus::prenucleolus(&game).unwrap();
    assert!(
        result
            .allocation()
            .iter()
            .chain(result.levels())
            .all(|v| v.is_finite())
    );
    let least = nucleolus::least_core(&game, Domain::Preimputation).unwrap();
    assert!(least.epsilon().is_finite());
}
