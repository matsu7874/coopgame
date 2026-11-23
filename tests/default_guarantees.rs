//! 許容誤差を指定しない既定の経路 (提携構造つきの仁・オラクル版の仁・仁の変種・費用分担の仁) の保証を確かめる。
//!
//! 規則は既定の仁 (`nucleolus::nucleolus`、`Tolerance::Auto`) と同じ:
//!
//! - 確かめられるなら確かめ、確かめた結果だけを `exact` にする。確かめていない結果は `approximate`。
//! - 許容誤差を明示したら、その許容誤差で 1 回だけ解いて `exact` (許容誤差の範囲で成り立つ)。
//!
//! 値が大きい整数のゲームでは、比 1e-7 の許容誤差で 1 回だけ解くと、超過の差が許容誤差より小さい段をまとめて
//! 仁からずれる (`tests/tolerance.rs`)。既定の経路が、そのずれた配分を `exact` と名乗らないことを確かめる。

mod common;

use common::max_abs_difference;
use coopgame::auto::AutoNucleolus;
use coopgame::game::exact::ExactGame;
use coopgame::games::cost::CostGame;
use coopgame::games::spanning_tree::SpanningTreeGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::nucleolus::{self, Options, Tolerance, exact, oracle, variants};
use coopgame::num_traits::ToPrimitive;
use coopgame::partition::{self, CoalitionStructure};
use coopgame::{Domain, ExplicitGame, Guarantee, generators};

/// 値が `scale * |S| + 0..6` (1 人提携は 0) の整数ゲーム (`tests/tolerance.rs` と同じ形)。
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

/// 監査で使った 96 個 (scale 1e6..1e9、3-6 人を 24 個ずつ)。
fn audit_games() -> Vec<ExplicitGame> {
    [1e6, 1e7, 1e8, 1e9]
        .iter()
        .flat_map(|&scale| integer_games(scale, &[3, 4, 5, 6], 24, 3))
        .collect()
}

fn rational_nucleolus(game: &ExplicitGame, domain: Domain) -> Vec<f64> {
    let exact = ExactGame::from_explicit(game).unwrap();
    exact::nucleolus(&exact, domain)
        .unwrap()
        .allocation
        .iter()
        .map(|q| q.to_f64().unwrap())
        .collect()
}

/// 全体 1 ブロックの提携構造つきの仁は仁そのものなので、既定は仁の既定と同じく確かめて `exact` にする。
/// 1 回だけ解いてずれた配分を `exact` と名乗らない。
#[test]
fn structured_nucleolus_with_grand_block_is_checked() {
    let games = audit_games();
    let mut once_off = 0;
    for (case, game) in games.iter().enumerate() {
        let grand = CoalitionStructure::grand(game.players()).unwrap();
        let truth = rational_nucleolus(game, Domain::Imputation);
        let precision = 1e-13 * game.max_abs_value();
        let result = partition::nucleolus(game, &grand, Domain::Imputation).unwrap();
        assert_eq!(result.guarantee(), Guarantee::Exact, "case {case}");
        assert!(
            max_abs_difference(result.allocation(), &truth) <= precision,
            "case {case}: {result:?} / {truth:?}"
        );
        assert_eq!(result, nucleolus::nucleolus(game).unwrap(), "case {case}");
        // 許容誤差を明示すると 1 回だけ解いて exact (許容誤差の範囲で成り立つ)。
        let mut options = Options::new(Domain::Imputation);
        options.tolerance = Tolerance::Relative(1e-7);
        let fixed = partition::nucleolus_with(game, &grand, options).unwrap();
        assert_eq!(fixed.guarantee(), Guarantee::Exact);
        if max_abs_difference(fixed.allocation(), &truth) > precision {
            once_off += 1;
        }
    }
    // 1 回だけ解くとずれるゲームの数 (docs/guarantees.md の「既定の経路ごとの保証」が引用する)。
    assert_eq!(once_off, 53);
}

/// 2 つ以上のブロックの提携構造つきの仁は、既定では確かめていないので `approximate`。
/// 許容誤差を明示すれば、1 回だけ解いて `exact`。全員単独は LP を解かずに決まるので `exact`。
#[test]
fn structured_nucleolus_without_check_is_approximate() {
    for (case, game) in integer_games(1e6, &[4, 5, 6], 12, 7).iter().enumerate() {
        let n = game.players();
        let structure = CoalitionStructure::new(n, &[vec![0, 1], (2..n).collect()]).unwrap();
        let default = partition::nucleolus(game, &structure, Domain::Imputation).unwrap();
        assert_eq!(default.guarantee(), Guarantee::Approximate, "case {case}");
        let mut options = Options::new(Domain::Imputation);
        options.tolerance = Tolerance::Relative(1e-7);
        let fixed = partition::nucleolus_with(game, &structure, options).unwrap();
        assert_eq!(fixed.guarantee(), Guarantee::Exact, "case {case}");
        assert_eq!(fixed.allocation(), default.allocation(), "case {case}");
        assert_eq!(fixed.tolerance(), 1e-7 * game.max_abs_value());

        let alone = CoalitionStructure::singletons(n).unwrap();
        let result = partition::nucleolus(game, &alone, Domain::Imputation).unwrap();
        assert_eq!(result.guarantee(), Guarantee::Exact, "case {case}");
    }
}

/// オラクル版の仁の既定は、20 人以下なら全提携の表を作って Kohlberg 基準で確かめ、満たさなければ
/// 既定の仁の手順で求め直す。1 回だけ解いてずれた配分を `exact` と名乗らない。
#[test]
fn oracle_default_is_checked_up_to_twenty_players() {
    let games = audit_games();
    let mut once_off = 0;
    for (case, game) in games.iter().enumerate() {
        let truth = rational_nucleolus(game, Domain::Imputation);
        let precision = 1e-13 * game.max_abs_value();
        let result = oracle::nucleolus(game).unwrap();
        assert_eq!(result.guarantee(), Guarantee::Exact, "case {case}");
        assert!(
            max_abs_difference(result.allocation(), &truth) <= precision,
            "case {case}: {result:?} / {truth:?}"
        );
        // 許容誤差を明示すると、1 回だけ解いて exact (許容誤差の範囲で成り立つ)。
        let fixed =
            oracle::nucleolus_with(game, Domain::Imputation, 1e-7 * game.max_abs_value()).unwrap();
        assert_eq!(fixed.guarantee(), Guarantee::Exact);
        if max_abs_difference(fixed.allocation(), &truth) > precision {
            once_off += 1;
        }
    }
    // 1 回だけ解くとずれるゲームの数 (docs/guarantees.md の「既定の経路ごとの保証」が引用する)。
    assert_eq!(once_off, 55);
}

/// オラクル版のプレ仁も同じ規則。
#[test]
fn oracle_default_prenucleolus_is_checked() {
    for (case, game) in integer_games(1e8, &[4, 5], 12, 11).iter().enumerate() {
        let truth = rational_nucleolus(game, Domain::Preimputation);
        let result = oracle::prenucleolus(game).unwrap();
        assert_eq!(result.guarantee(), Guarantee::Exact, "case {case}");
        assert!(
            max_abs_difference(result.allocation(), &truth) <= 1e-13 * game.max_abs_value(),
            "case {case}: {result:?} / {truth:?}"
        );
    }
}

/// 21 人以上のオラクル版の仁は確かめないので `approximate`。許容誤差を明示すれば `exact`。
/// 重み付き投票ゲームの自動選択も同じ。
#[test]
fn oracle_default_beyond_check_limit_is_approximate() {
    let n = nucleolus::MAX_AUTO_CHECK_PLAYERS + 1;
    let weights: Vec<u64> = (0..n as u64).map(|i| 1 + i % 4).collect();
    let quota = weights.iter().sum::<u64>() / 2 + 1;
    let voting = WeightedVotingGame::new(weights, quota).unwrap();
    let default = oracle::nucleolus(&voting).unwrap();
    assert_eq!(default.guarantee(), Guarantee::Approximate);
    let fixed = oracle::nucleolus_with(&voting, Domain::Imputation, 1e-7).unwrap();
    assert_eq!(fixed.guarantee(), Guarantee::Exact);
    assert_eq!(fixed.allocation(), default.allocation());
    assert_eq!(
        voting.nucleolus_auto().unwrap().guarantee(),
        Guarantee::Approximate
    );

    // 20 人の投票ゲームは確かめる。
    let weights: Vec<u64> = (0..20u64).map(|i| 1 + i % 4).collect();
    let quota = weights.iter().sum::<u64>() / 2 + 1;
    let voting = WeightedVotingGame::new(weights, quota).unwrap();
    assert_eq!(
        voting.nucleolus_auto().unwrap().guarantee(),
        Guarantee::Exact
    );
}

/// 仁の変種は、既定では確かめていないので `approximate`。許容誤差を明示すれば、1 回だけ解いて `exact`。
#[test]
fn variants_default_is_approximate_and_explicit_tolerance_is_exact() {
    let fixed = Tolerance::Relative(1e-7);
    for (case, game) in integer_games(1e6, &[3, 4, 5], 9, 13).iter().enumerate() {
        let tolerance = 1e-7 * game.max_abs_value();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let default = variants::per_capita_nucleolus(game, domain).unwrap();
            assert_eq!(default.guarantee(), Guarantee::Approximate, "case {case}");
            let explicit = variants::per_capita_nucleolus_with(game, domain, fixed).unwrap();
            assert_eq!(explicit.guarantee(), Guarantee::Exact, "case {case}");
            assert_eq!(explicit.allocation(), default.allocation());
            assert_eq!(explicit.tolerance(), tolerance);

            let default = variants::proportional_nucleolus(game, domain).unwrap();
            assert_eq!(default.guarantee(), Guarantee::Approximate, "case {case}");
            let explicit = variants::proportional_nucleolus_with(game, domain, fixed).unwrap();
            assert_eq!(explicit.guarantee(), Guarantee::Exact, "case {case}");
            assert_eq!(explicit.allocation(), default.allocation());
        }
        let default = variants::modiclus(game).unwrap();
        assert_eq!(default.guarantee(), Guarantee::Approximate, "case {case}");
        let explicit = variants::modiclus_with(game, fixed).unwrap();
        assert_eq!(explicit.guarantee(), Guarantee::Exact, "case {case}");
        assert_eq!(explicit.allocation(), default.allocation());
    }
    // disruption nucleolus はコアが空でないゲームに限る (凸ゲーム)。
    for seed in 0..6 {
        let game = generators::random_convex(4 + (seed % 3) as usize, seed).unwrap();
        let default = variants::disruption_nucleolus(&game).unwrap();
        assert_eq!(default.guarantee(), Guarantee::Approximate, "seed {seed}");
        let explicit = variants::disruption_nucleolus_with(&game, fixed).unwrap();
        assert_eq!(explicit.guarantee(), Guarantee::Exact, "seed {seed}");
        assert_eq!(explicit.allocation(), default.allocation());
    }
}

/// per capita 仁を比 1e-7 で 1 回だけ解いた配分は、値が大きい整数のゲームで、許容誤差を下げて解いた配分より
/// 辞書式に劣ることがある (並べた不満が、最初に異なる位置で大きい)。既定がこれを `exact` と名乗らない理由。
#[test]
fn per_capita_once_can_be_lexicographically_worse() {
    let mut worse = 0;
    for game in &audit_games() {
        let n = game.players();
        let once = variants::per_capita_nucleolus(game, Domain::Imputation).unwrap();
        assert_eq!(once.guarantee(), Guarantee::Approximate);
        // 許容誤差を下げて解けた最初の配分 (下げすぎると LP が数値的に進まないことがある)。
        let Some(finer) = [1e-9, 1e-10, 1e-11, 1e-12].iter().find_map(|&ratio| {
            variants::per_capita_nucleolus_with(
                game,
                Domain::Imputation,
                Tolerance::Relative(ratio),
            )
            .ok()
        }) else {
            continue;
        };
        let complaints = |x: &[f64]| {
            let mut sorted: Vec<f64> = (1..(1u64 << n) - 1)
                .map(|mask| {
                    let s = coopgame::Coalition(mask);
                    (game.value(s) - s.members().map(|i| x[i]).sum::<f64>()) / s.len() as f64
                })
                .collect();
            sorted.sort_by(|a, b| b.total_cmp(a));
            sorted
        };
        let (a, b) = (
            complaints(once.allocation()),
            complaints(finer.allocation()),
        );
        let separation = 1e-12 * game.max_abs_value();
        if let Some(k) = a
            .iter()
            .zip(&b)
            .position(|(p, q)| (p - q).abs() > separation)
        {
            // 許容誤差を下げた配分の方が辞書式に劣ることはなかった。
            assert!(a[k] > b[k], "{a:?} / {b:?}");
            worse += 1;
        }
    }
    // docs/guarantees.md の「既定の経路ごとの保証」が引用する。
    assert_eq!(worse, 30);
}

/// 変種の `*_with` に `Tolerance::Auto` を渡すと、既定と同じ (確かめずに `approximate`)。
/// 配分集合が 1 点に決まる (LP を解かない) ゲームは `exact`。
#[test]
fn variants_auto_matches_default_and_single_point_is_exact() {
    let game = generators::bnf(1, 5, 3).unwrap();
    assert_eq!(
        variants::per_capita_nucleolus_with(&game, Domain::Imputation, Tolerance::Auto).unwrap(),
        variants::per_capita_nucleolus(&game, Domain::Imputation).unwrap()
    );
    // v({i}) の和が v(N) に等しいゲーム: 配分集合は 1 点。
    let single = ExplicitGame::from_binary(&[1.0, 2.0, 5.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let result = variants::per_capita_nucleolus(&single, Domain::Imputation).unwrap();
    assert_eq!(result.lp_solves(), 0);
    assert_eq!(result.guarantee(), Guarantee::Exact);
    assert_eq!(result.allocation(), [1.0, 2.0, 3.0]);
}

/// 費用分担の仁は保証つきの結果を返す。配分は費用の分担で、保証・許容誤差・段は節約ゲームの既定の仁と同じ
/// (費用ゲームの超過 `y(S) - c(S)` は節約ゲームの超過 `s(S) - x(S)` に等しい)。
#[test]
fn cost_nucleolus_keeps_the_guarantee() {
    let costs = ExplicitGame::from_lex(&[6.0, 6.0, 8.0, 9.0, 11.0, 12.0, 15.0]).unwrap();
    let game = CostGame::new(costs);
    let result = game.nucleolus().unwrap();
    let savings = nucleolus::nucleolus(&game.savings_game().unwrap()).unwrap();
    assert_eq!(result.guarantee(), savings.guarantee());
    assert_eq!(result.tolerance(), savings.tolerance());
    assert_eq!(result.levels(), savings.levels());
    assert_eq!(result.allocation(), game.to_costs(savings.allocation()));
    assert_eq!(result.guarantee(), Guarantee::Exact);

    let mut options = Options::new(Domain::Imputation);
    options.tolerance = Tolerance::Absolute(1e-6);
    let fixed = game.nucleolus_with(options).unwrap();
    assert_eq!(fixed.tolerance(), 1e-6);
    assert_eq!(fixed.guarantee(), Guarantee::Exact);
}

/// 最小全域木ゲームの仁も保証つき。21 人では確かめないので `approximate`。
#[test]
fn spanning_tree_nucleolus_keeps_the_guarantee() {
    let tree = SpanningTreeGame::new(vec![
        vec![0.0, 10.0, 10.0, 10.0],
        vec![10.0, 0.0, 3.0, 8.0],
        vec![10.0, 3.0, 0.0, 4.0],
        vec![10.0, 8.0, 4.0, 0.0],
    ])
    .unwrap();
    let result = tree.nucleolus_costs().unwrap();
    assert_eq!(result.guarantee(), Guarantee::Exact);
    let total: f64 = result.allocation().iter().sum();
    assert!((total - 17.0).abs() < 1e-9, "{result:?}");

    // 21 人: 供給元から 10、隣どうし 1 の費用 (頂点 1..=21 を直線に並べる)。
    let n = nucleolus::MAX_AUTO_CHECK_PLAYERS + 1;
    let costs: Vec<Vec<f64>> = (0..=n)
        .map(|a| {
            (0..=n)
                .map(|b| match (a, b) {
                    _ if a == b => 0.0,
                    (0, _) | (_, 0) => 10.0,
                    _ => (a as f64 - b as f64).abs(),
                })
                .collect()
        })
        .collect();
    let large = SpanningTreeGame::new(costs).unwrap();
    let mut options = Options::new(Domain::Imputation);
    options.tolerance = Tolerance::Relative(1e-7);
    let fixed = large.nucleolus_costs_with(options).unwrap();
    assert_eq!(fixed.guarantee(), Guarantee::Exact);
    let default = large.nucleolus_costs().unwrap();
    assert_eq!(default.guarantee(), Guarantee::Approximate);
    assert_eq!(default.allocation(), fixed.allocation());
}

/// 空港ゲームの仁も保証つきの結果を返す。節約ゲームが凸であることを使う凸ゲームの手法なので `proven(convex)`。
/// 配分は費用の分担で、明示の費用ゲームの仁と一致する。
#[test]
fn airport_nucleolus_keeps_the_guarantee() {
    use coopgame::Property;
    use coopgame::games::airport::AirportGame;
    use coopgame::nucleolus::convex::ConvexOptions;

    let game = AirportGame::new(vec![3.0, 6.0, 12.0]).unwrap();
    let result = game.nucleolus_costs().unwrap();
    assert_eq!(result.guarantee(), Guarantee::Proven(Property::Convex));
    assert_eq!(result.domain(), Domain::Imputation);
    assert!(max_abs_difference(result.allocation(), &[1.5, 2.25, 8.25]) < 1e-9);

    let mut options = ConvexOptions::default();
    options.tolerance = Some(1e-12);
    let fixed = game.nucleolus_costs_with(options).unwrap();
    assert_eq!(fixed.guarantee(), Guarantee::Proven(Property::Convex));
    assert!(max_abs_difference(fixed.allocation(), result.allocation()) < 1e-9);
}

/// 値の大きさの目安 (`value_scale`) を実際より桁違いに小さく答えるオラクル。値と違反する提携は正しく答える。
/// 許容誤差がその目安から決まるので、逐次 LP が数値的に失敗する (LP が実行不可能、確定する提携が見つからない)。
struct UnderstatedScale {
    inner: ExplicitGame,
    scale: f64,
}

impl coopgame::SetFunction for UnderstatedScale {
    fn players(&self) -> usize {
        self.inner.players()
    }

    fn value(&self, coalition: &coopgame::PlayerSet) -> f64 {
        coopgame::SetFunction::value(&self.inner, coalition)
    }
}

impl coopgame::game::oracle::Separation for UnderstatedScale {
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&coopgame::PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(coopgame::PlayerSet, f64)> {
        self.inner.violated(x, bound, skip, limit)
    }

    fn value_scale(&self) -> f64 {
        self.scale
    }
}

/// オラクル版の既定は、1 回目の逐次 LP が数値的に失敗しても、20 人以下で全提携に答えるオラクルなら
/// 全提携の表を作り、既定の仁の手順で求め直す (明示ゲームの既定と同じ回復の経路)。
#[test]
fn oracle_default_recovers_from_numerical_failure() {
    use coopgame::Error;
    let mut recovered = 0;
    for seed in 0..10u64 {
        let game = generators::bnf(1 + (seed % 4) as u8, 6, seed).unwrap();
        let oracle_game = UnderstatedScale {
            scale: 1e-12 * game.max_abs_value(),
            inner: game.clone(),
        };
        let once = oracle::nucleolus_with(
            &oracle_game,
            Domain::Imputation,
            oracle::default_tolerance(&oracle_game),
        );
        let result = oracle::nucleolus(&oracle_game).unwrap();
        let expected = nucleolus::nucleolus(&game).unwrap();
        if matches!(once, Err(Error::Numerical(_) | Error::Lp(_))) {
            recovered += 1;
            assert_eq!(result, expected, "seed {seed}");
        }
        assert_eq!(result.guarantee(), Guarantee::Exact, "seed {seed}");
    }
    // 1 回目が失敗するゲームがある (なければ、回復の経路を通していない)。
    assert!(recovered >= 5, "{recovered}");
}
