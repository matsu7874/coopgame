//! オラクル型ゲームの仁を、独立した基準と突き合わせる。
//!
//! - 破産ゲーム: 仁はタルムード則に一致する (Aumann & Maschler 1985)。全提携を列挙できない n = 100 まで確かめる。
//! - 重み付き投票ゲーム: 小さい n では明示ベクトル版の仁と一致する。大きい n では対称性などで確かめる。
//!   拒否権を持つプレイヤーがいるゲーム (仁が 2 段で決まる) は、確定済みの提携が超過の上位を占めるので、
//!   確定済みの空間を使った探索 (`Separation::violated_outside_span`) で解く。

mod common;

use common::assert_close;
use std::time::Instant;

use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::generators::{self, SplitMix64};
use coopgame::nucleolus::oracle as oracle_nucleolus;
use coopgame::{Domain, nucleolus};

#[test]
fn bankruptcy_oracle_matches_talmud_rule_for_large_n() {
    let mut rng = SplitMix64::new(2026);
    // n = 100 は 1 問あたり 5-15 秒かかるので 1 問だけ解く。
    for (n, cases) in [(5, 2), (10, 2), (20, 2), (40, 2), (70, 2), (100, 1)] {
        for case in 0..cases {
            let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 100) as f64).collect();
            let total: f64 = claims.iter().sum();
            let estate = (rng.next_f64() * total).round();
            let game = BankruptcyGame::new(estate, claims.clone()).unwrap();
            let started = Instant::now();
            let result = oracle_nucleolus::nucleolus(&game).unwrap();
            let expected = common::talmud_rule(estate, &claims);
            assert_close(
                result.allocation(),
                &expected,
                1e-6 * total,
                &format!("n={n} case={case} E={estate} ({:.2?})", started.elapsed()),
            );
        }
    }
}

/// 101 人以上が同じ額を受け取る段がある破産ゲーム。最後の段では 1 人提携 (遺産が請求の和の半分より多いときは
/// その補集合) が 101 個以上タイトになり、どれも確定する。確定判定の LP で各行を一律に緩めていたときは、
/// 緩めの効きが行の数だけ積み重なって許容誤差を超え、どの提携も確定と判定できずに `Error::Numerical` になった。
#[test]
fn bankruptcy_oracle_fixes_many_equal_awards() {
    let mut claims = vec![2.0, 4.0, 6.0];
    claims.extend([10.0; 101]);
    let total: f64 = claims.iter().sum();
    // 3 人は請求の半分 (1, 2, 3)、101 人は 4 を受け取る。逆の遺産では 101 人の損失が 4 になる。
    for estate in [410.0, total - 410.0] {
        let game = BankruptcyGame::new(estate, claims.clone()).unwrap();
        let expected = common::talmud_rule(estate, &claims);
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let result = oracle_nucleolus::nucleolus_with(
                &game,
                domain,
                oracle_nucleolus::default_tolerance(&game),
            )
            .unwrap();
            assert_close(
                result.allocation(),
                &expected,
                1e-9 * total,
                &format!("E={estate} {domain:?}"),
            );
        }
    }
}

#[test]
fn bankruptcy_oracle_matches_explicit_solver() {
    let mut rng = SplitMix64::new(7);
    for n in 3..=10 {
        let claims: Vec<f64> = (0..n).map(|_| rng.range(1, 50) as f64).collect();
        let total: f64 = claims.iter().sum();
        let estate = (0.6 * total).round();
        let oracle = BankruptcyGame::new(estate, claims.clone()).unwrap();
        let explicit = generators::bankruptcy(estate, &claims).unwrap();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let expected = nucleolus::nucleolus_with(&explicit, nucleolus::Options::new(domain))
                .unwrap()
                .into_allocation();
            let actual = oracle_nucleolus::nucleolus_with(
                &oracle,
                domain,
                oracle_nucleolus::default_tolerance(&oracle),
            )
            .unwrap()
            .into_allocation();
            assert_close(
                &actual,
                &expected,
                1e-6 * total,
                &format!("n={n} {domain:?}"),
            );
        }
    }
}

#[test]
fn voting_oracle_matches_explicit_solver() {
    let mut rng = SplitMix64::new(11);
    for n in 3..=11 {
        for case in 0..3 {
            let weights: Vec<u64> = (0..n).map(|_| rng.range(1, 10)).collect();
            let quota = weights.iter().sum::<u64>() / 2 + 1;
            let oracle = WeightedVotingGame::new(weights.clone(), quota).unwrap();
            let explicit = generators::weighted_voting(
                &weights.iter().map(|w| *w as f64).collect::<Vec<_>>(),
                quota as f64,
            )
            .unwrap();
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let expected =
                    nucleolus::nucleolus_with(&explicit, nucleolus::Options::new(domain))
                        .unwrap()
                        .into_allocation();
                let actual = oracle_nucleolus::nucleolus_with(&oracle, domain, 1e-7)
                    .unwrap()
                    .into_allocation();
                assert_close(
                    &actual,
                    &expected,
                    1e-6,
                    &format!("n={n} case={case} w={weights:?} {domain:?}"),
                );
            }
        }
    }
}

/// 全員の重みが等しい多数決ゲームの仁は、対称性から等分になる (仁は一意で、プレイヤーの入れ替えで不変)。
#[test]
fn symmetric_majority_game_splits_equally() {
    // n = 19 で 380 行を追加し、0.05 秒 (`data/bench/oracle-scaling.csv`)。31 人でも 620 行・0.2 秒で解ける。
    for n in [9, 15, 19, 31] {
        let game = WeightedVotingGame::new(vec![1; n], (n / 2 + 1) as u64).unwrap();
        let result = oracle_nucleolus::nucleolus(&game).unwrap();
        assert_close(
            result.allocation(),
            &vec![1.0 / n as f64; n],
            1e-6,
            &format!("n={n}"),
        );
    }
}

/// 1 人の重みだけで基準に達する独裁者ゲームでは、独裁者以外は誰とも組む価値がない。
/// コアは独裁者が全てを得る 1 点なので、仁もその点になる (コアが空でなければ仁はコアに属する)。
#[test]
fn dictator_game_gives_everything_to_dictator() {
    let n = 40;
    let mut weights = vec![1; n];
    weights[7] = 100;
    let game = WeightedVotingGame::new(weights, 100).unwrap();
    let result = oracle_nucleolus::nucleolus(&game).unwrap();
    let mut expected = vec![0.0; n];
    expected[7] = 1.0;
    assert_close(result.allocation(), &expected, 1e-6, "独裁者ゲーム");
}

/// 重み 1000 の `big` 人 (拒否権を持つ) と重み 1 の `n - big` 人、基準は `1000 big + floor((n - big) / 2) + 1`。
/// 勝利提携は拒否権の全員を含むので、コアは拒否権の人たちで分ける配分の集合で、仁は (1/big, ..., 1/big, 0, ..., 0)
/// (拒否権の人たちは対称で、他の人はコアで 0)。
fn veto_game(big: usize, n: usize) -> (WeightedVotingGame, Vec<f64>) {
    let mut weights = vec![1u64; n];
    weights[..big].fill(1000);
    let quota = 1000 * big as u64 + ((n - big) / 2 + 1) as u64;
    let mut expected = vec![0.0; n];
    expected[..big].fill(1.0 / big as f64);
    (WeightedVotingGame::new(weights, quota).unwrap(), expected)
}

/// 拒否権のあるゲームのオラクル版の仁・プレ仁が、明示ベクトル版の仁と一致する。
#[test]
fn veto_games_match_explicit_solver() {
    for big in [2, 3] {
        for n in big + 2..=12 {
            let (oracle, _) = veto_game(big, n);
            let explicit = generators::weighted_voting(
                &oracle
                    .weights()
                    .iter()
                    .map(|w| *w as f64)
                    .collect::<Vec<_>>(),
                oracle.quota() as f64,
            )
            .unwrap();
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let expected =
                    nucleolus::nucleolus_with(&explicit, nucleolus::Options::new(domain))
                        .unwrap()
                        .into_allocation();
                let result = oracle_nucleolus::nucleolus_with(&oracle, domain, 1e-7).unwrap();
                assert_eq!(result.guarantee(), coopgame::Guarantee::Exact);
                assert_close(
                    result.allocation(),
                    &expected,
                    1e-6,
                    &format!("big={big} n={n} {domain:?}"),
                );
            }
        }
    }
}

/// 拒否権のあるゲームの仁を、有理数の逐次 LP で求めた仁と比べる。
#[test]
fn veto_games_match_rational_nucleolus() {
    use coopgame::game::exact::ExactGame;
    use coopgame::num_traits::ToPrimitive;
    for (big, n) in [(2, 6), (3, 7)] {
        let (oracle, expected) = veto_game(big, n);
        let explicit = generators::weighted_voting(
            &oracle
                .weights()
                .iter()
                .map(|w| *w as f64)
                .collect::<Vec<_>>(),
            oracle.quota() as f64,
        )
        .unwrap();
        let exact = ExactGame::from_explicit(&explicit).unwrap();
        let rational: Vec<f64> = nucleolus::exact::nucleolus(&exact, Domain::Imputation)
            .unwrap()
            .allocation
            .iter()
            .map(|r| r.to_f64().unwrap())
            .collect();
        assert_close(
            &rational,
            &expected,
            1e-15,
            &format!("big={big} n={n} 有理数"),
        );
        let actual = oracle_nucleolus::nucleolus(&oracle).unwrap();
        assert_close(
            actual.allocation(),
            &rational,
            1e-6,
            &format!("big={big} n={n}"),
        );
    }
}

/// 拒否権のあるゲームを、確定済みの提携を読み飛ばすと指数個 (約 3 * 2^(n-4)) 読む人数で解く。
/// 修正前は 24 人で 27 秒・3.7 GB、26 人で 6 GB を超えた (`data/bench/voting-oracle-before.csv`)。
/// 確定済みの空間を使った探索では 40 人まで 1 秒程度で解ける (release ビルド。`data/bench/voting-oracle-after.csv`)。
#[test]
fn veto_games_with_many_fixed_coalitions_are_solved_quickly() {
    for (big, n) in [(3, 24), (2, 24), (3, 40), (2, 40)] {
        let (game, expected) = veto_game(big, n);
        let started = Instant::now();
        let result = oracle_nucleolus::nucleolus(&game).unwrap();
        let seconds = started.elapsed().as_secs_f64();
        // 既定の手順は、Kohlberg 基準を確かめない 21 人以上の結果を approximate にする。
        assert_eq!(result.guarantee(), coopgame::Guarantee::Approximate);
        assert_eq!(result.levels().len(), 2, "big={big} n={n}");
        assert_close(
            result.allocation(),
            &expected,
            1e-6,
            &format!("big={big} n={n}"),
        );
        // 計算量の回帰の目安 (debug ビルドでも余裕のある上限)。
        assert!(seconds < 60.0, "big={big} n={n}: {seconds} 秒");
    }
}

/// 分離オラクルを包み、`violated_outside_span` が返す提携が確定済みの空間に入らないことを、仁の計算の
/// 呼び出しごとに確かめる (空間は整数の消去による階数で判定する)。確定済みの提携を直接与えた探索も、総当たりと比べる。
#[test]
fn voting_separation_never_returns_fixed_coalitions() {
    use coopgame::game::oracle::Separation;
    use coopgame::{PlayerSet, SetFunction};
    use std::cell::Cell;

    struct Checked {
        game: WeightedVotingGame,
        calls: Cell<usize>,
        returned: Cell<usize>,
    }
    impl SetFunction for Checked {
        fn players(&self) -> usize {
            self.game.players()
        }
        fn value(&self, coalition: &PlayerSet) -> f64 {
            self.game.value(coalition)
        }
    }
    /// 整数ベクトルの階数。
    fn rank(vectors: &[Vec<i64>]) -> usize {
        let mut rows = vectors.to_vec();
        let columns = rows.first().map_or(0, Vec::len);
        let mut rank = 0;
        for column in 0..columns {
            let Some(found) = (rank..rows.len()).find(|&r| rows[r][column] != 0) else {
                continue;
            };
            rows.swap(rank, found);
            let pivot_row = rows[rank].clone();
            for row in rows.iter_mut().skip(rank + 1) {
                let (a, b) = (pivot_row[column], row[column]);
                if b != 0 {
                    for (e, &q) in row.iter_mut().zip(&pivot_row) {
                        *e = *e * a - q * b;
                    }
                    let g = row.iter().fold(0i64, |g, &e| num_gcd(g, e.abs()));
                    if g > 1 {
                        row.iter_mut().for_each(|e| *e /= g);
                    }
                }
            }
            rank += 1;
        }
        rank
    }
    fn num_gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a } else { num_gcd(b, a % b) }
    }
    fn vector(s: &PlayerSet, n: usize) -> Vec<i64> {
        (0..n).map(|i| i64::from(s.contains(i))).collect()
    }
    fn in_span_of(fixed: &[PlayerSet], s: &PlayerSet, n: usize) -> bool {
        let mut vectors: Vec<Vec<i64>> = fixed.iter().map(|f| vector(f, n)).collect();
        let base = rank(&vectors);
        vectors.push(vector(s, n));
        rank(&vectors) == base
    }
    impl Separation for Checked {
        fn violated(
            &self,
            _x: &[f64],
            _bound: f64,
            _skip: &dyn Fn(&PlayerSet) -> bool,
            _limit: usize,
        ) -> Vec<(PlayerSet, f64)> {
            unreachable!("仁の制約生成は violated_outside_span を呼ぶ")
        }
        fn violated_outside_span(
            &self,
            x: &[f64],
            bound: f64,
            fixed: &[PlayerSet],
            skip: &dyn Fn(&PlayerSet) -> bool,
            limit: usize,
        ) -> Vec<(PlayerSet, f64)> {
            self.calls.set(self.calls.get() + 1);
            let found = self
                .game
                .violated_outside_span(x, bound, fixed, skip, limit);
            let n = self.game.players();
            for (s, _) in &found {
                assert!(!in_span_of(fixed, s, n), "確定済みの提携 {s} を返した");
            }
            self.returned.set(self.returned.get() + found.len());
            found
        }
        fn value_scale(&self) -> f64 {
            1.0
        }
    }
    for (big, n) in [(3, 14), (2, 14)] {
        let (game, expected) = veto_game(big, n);
        let checked = Checked {
            game,
            calls: Cell::new(0),
            returned: Cell::new(0),
        };
        let result = oracle_nucleolus::nucleolus(&checked).unwrap();
        assert_close(
            result.allocation(),
            &expected,
            1e-6,
            &format!("big={big} n={n}"),
        );
        // 拒否権のゲームは 1 人提携とその補集合の行だけで違反がなくなる (返す提携は 0 個) ので、
        // 返す提携の検査は下の直接の呼び出しで行う。
        assert!(checked.calls.get() > 0, "big={big} n={n}");
    }
    // 返す提携が確定済みの空間に入らないことを、確定済みの提携を直接与えて確かめる。
    let n = 14;
    let (game, _) = veto_game(3, n);
    let x: Vec<f64> = (0..n)
        .map(|i| if i < 3 { 1.0 / 3.0 } else { 0.0 })
        .collect();
    let mut fixed = vec![PlayerSet::grand(n)];
    fixed.extend((3..n).map(|i| PlayerSet::singleton(n, i)));
    let in_span = |s: &PlayerSet| in_span_of(&fixed, s, n);
    for bound in [-0.9, -0.5, -0.2] {
        let found = game.violated_outside_span(&x, bound, &fixed, &in_span, 64);
        for (s, e) in &found {
            assert!(!in_span(s), "bound={bound}: {s}");
            assert!(*e > bound);
        }
        // 総当たり: 違反があれば空でない
        let any = (1..(1usize << n) - 1).any(|bits| {
            let members: Vec<usize> = (0..n).filter(|i| bits >> i & 1 == 1).collect();
            let s = PlayerSet::from_players(n, &members);
            game.value(&s) - s.sum(&x) > bound && !in_span(&s)
        });
        assert_eq!(any, !found.is_empty(), "bound={bound}");
    }
}
