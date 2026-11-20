//! 超過の差が許容誤差より小さい提携を含むゲームで、浮動小数点の逐次 LP が解けることを確かめる。
//!
//! 段の LP の最適解での超過が最大超過と許容誤差以内で等しい提携は、確定の候補になる。
//! 本当は確定しない提携が候補になっても、以後の LP が実行不可能にならず、
//! 有理数の逐次 LP (`nucleolus::exact`) の仁と許容誤差の数倍以内で一致することを確かめる。

mod common;

use common::max_abs_difference;
use coopgame::game::default_tolerance;
use coopgame::game::exact::ExactGame;
use coopgame::nucleolus::{self, Options, exact};
use coopgame::num_traits::ToPrimitive;
use coopgame::{Domain, ExplicitGame};

/// 有理数の逐次 LP の仁と比べる。浮動小数点の結果の誤差は許容誤差の 2 倍以内とする。
fn assert_matches_exact(game: &ExplicitGame, domain: Domain, label: &str) {
    let expected = exact::nucleolus(&ExactGame::from_explicit(game).unwrap(), domain)
        .unwrap()
        .allocation
        .iter()
        .map(|q| q.to_f64().unwrap())
        .collect::<Vec<_>>();
    let actual = nucleolus::nucleolus_with(game, Options::new(game, domain))
        .unwrap_or_else(|err| panic!("{label} {domain:?}: {err}"));
    let tolerance = default_tolerance(game);
    let difference = max_abs_difference(actual.allocation(), &expected);
    assert!(
        difference <= 2.0 * tolerance,
        "{label} {domain:?}: {:?} != {expected:?} (差 {difference}, 許容誤差 {tolerance})",
        actual.allocation()
    );
}

#[test]
fn coalition_whose_excess_is_within_tolerance_of_the_level() {
    // 値が 300 万程度の整数の 5 人ゲーム (許容誤差 0.3)。第 2 段 (最大超過 500001 + 7/12) では、
    // 提携 {0, 1, 2} と {0, 2, 3} の超過が最適解集合の上で最大超過より 1/6 小さい値に固定される。
    // 差 1/6 が許容誤差より小さいので確定の候補になるが、x(S) = v(S) - (最大超過) とは両立しない。
    // 仁 = プレ仁 = (-13/12, 3999995/4, 6000023/12, 3/4, 2999999/3)。
    let values = [
        -999996.0, -499999.0, -1500000.0, -499997.0, -999999.0, -499996.0, 2000001.0, -500000.0,
        -1499997.0, 1500003.0, 1500000.0, -499997.0, 1000003.0, 2000003.0, 0.0, -500000.0,
        1000003.0, 2500000.0, -1499999.0, 2000001.0, 2000004.0, -999997.0, 1000001.0, 1500002.0,
        1000002.0, -999997.0, -1500000.0, 1000000.0, 1000000.0, 3000002.0, 2500000.0,
    ];
    let game = ExplicitGame::from_binary(&values).unwrap();
    let expected = [
        -13.0 / 12.0,
        3999995.0 / 4.0,
        6000023.0 / 12.0,
        3.0 / 4.0,
        2999999.0 / 3.0,
    ];
    for domain in [Domain::Imputation, Domain::Preimputation] {
        let actual = nucleolus::nucleolus_with(&game, Options::new(&game, domain))
            .unwrap_or_else(|err| panic!("{domain:?}: {err}"));
        let difference = max_abs_difference(actual.allocation(), &expected);
        assert!(
            difference <= default_tolerance(&game),
            "{domain:?}: {:?} != {expected:?}",
            actual.allocation()
        );
        assert_matches_exact(&game, domain, "5 人の整数ゲーム");
    }
}

/// xorshift64 (テストのゲームを seed で固定するため)。
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() % ((high - low + 1) as u64)) as i64
    }
}

/// 提携の値を `k * big + (小さい整数)` にしたゲーム。値の端数の刻み (1) が許容誤差 (`1e-7 max|v|`) に近いので、
/// 超過の差が許容誤差より小さい提携の組が多く現れる。
fn near_tie_game(seed: u64, big: f64) -> ExplicitGame {
    let mut rng = Xorshift(0x1234567 ^ (seed * 104729 + 3));
    let n = 3 + (seed % 3) as usize;
    let size = 1usize << n;
    let mut values = vec![0.0; size];
    for (mask, value) in values.iter_mut().enumerate().skip(1) {
        let k = mask.count_ones() as usize;
        *value = if k >= n - 1 && rng.next().is_multiple_of(2) {
            rng.range(-3, 3) as f64
        } else {
            rng.range(-3, 6) as f64 + rng.range(0, 4) as f64 * big
        };
    }
    values[size - 1] = rng.range(0, 8) as f64;
    ExplicitGame::from_binary(&values[1..]).unwrap()
}

#[test]
fn games_with_excess_gaps_below_tolerance_match_exact_nucleolus() {
    for big in [3e6, 1e7, 3e7] {
        for seed in 0..120 {
            let game = near_tie_game(seed, big);
            let exact_game = ExactGame::from_explicit(&game).unwrap();
            for domain in [Domain::Imputation, Domain::Preimputation] {
                if exact::nucleolus(&exact_game, domain).is_err() {
                    // 配分集合が空 (浮動小数点でも許容誤差を超えて空なら同じく失敗する)。
                    continue;
                }
                assert_matches_exact(&game, domain, &format!("big {big:e} seed {seed}"));
            }
        }
    }
}
