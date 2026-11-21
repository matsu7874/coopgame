//! オラクル版の仁・最小コアの保証が、オラクルが全提携を覆うかで決まることを確かめる。
//!
//! - 全提携について答えるオラクル (明示ゲーム・破産・投票・利用者の分離オラクル) は `exact`。
//! - 一部の提携しか知らないオラクル (`SampledGame`・`PartialGame`) は、`nucleolus::oracle` に直接渡しても
//!   `approximate` になる。

use coopgame::game::oracle::{OracleGame, Separation};
use coopgame::game::{PlayerSet, SetFunction};
use coopgame::games::bankruptcy::BankruptcyGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::generators;
use coopgame::nucleolus::oracle;
use coopgame::nucleolus::sampled::SampledGame;
use coopgame::{Domain, ExplicitGame, Guarantee};

/// `oracle::{nucleolus, prenucleolus, least_core}` の保証をまとめて返す。
fn oracle_guarantees<G: Separation + ?Sized>(game: &G) -> [Guarantee; 3] {
    [
        oracle::nucleolus(game).unwrap().guarantee(),
        oracle::prenucleolus(game).unwrap().guarantee(),
        oracle::least_core(game, Domain::Imputation)
            .unwrap()
            .guarantee(),
    ]
}

#[test]
fn complete_oracles_give_exact_results() {
    let explicit = generators::bnf(1, 6, 0).unwrap();
    assert!(Separation::covers_all_coalitions(&explicit));
    assert_eq!(oracle_guarantees(&explicit), [Guarantee::Exact; 3]);

    let bankruptcy = BankruptcyGame::new(60.0, vec![10.0, 20.0, 30.0, 40.0]).unwrap();
    assert_eq!(oracle_guarantees(&bankruptcy), [Guarantee::Exact; 3]);

    let voting = WeightedVotingGame::new(vec![3, 2, 2, 1], 5).unwrap();
    assert_eq!(oracle_guarantees(&voting), [Guarantee::Exact; 3]);
}

/// 5 組だけサンプルした 8 人ゲームは、真の仁から大きく離れうる。`oracle` に直接渡しても `exact` にならない。
#[test]
fn sampled_game_gives_approximate_results_through_oracle() {
    let game = generators::bnf(1, 8, 1).unwrap();
    let sampled = SampledGame::sample(&game, 5, 0).unwrap();
    assert!(!OracleGame::lists_all_coalitions(&sampled));
    assert!(!Separation::covers_all_coalitions(&sampled));
    assert_eq!(oracle_guarantees(&sampled), [Guarantee::Approximate; 3]);
}

/// 全提携をサンプルしても、`SampledGame` はサンプルしたゲームとして近似を名乗る
/// (`sampled::nucleolus` の結果と同じ扱い)。
#[test]
fn sampled_game_with_all_coalitions_is_still_approximate() {
    let game = generators::bnf(2, 4, 3).unwrap();
    let sampled = SampledGame::sample(&game, 2000, 1).unwrap();
    assert_eq!(sampled.evaluations(), (1 << 4) - 1);
    assert_eq!(oracle_guarantees(&sampled), [Guarantee::Approximate; 3]);
}

/// 提携の一部 (1 人提携とその補集合) だけについて答える、利用者の分離オラクル。
struct SingletonsOnly(ExplicitGame);

impl SetFunction for SingletonsOnly {
    fn players(&self) -> usize {
        self.0.players()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        SetFunction::value(&self.0, coalition)
    }
}

impl Separation for SingletonsOnly {
    fn violated(
        &self,
        x: &[f64],
        bound: f64,
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        let n = self.0.players();
        (0..n)
            .flat_map(|i| {
                let single = PlayerSet::singleton(n, i);
                [single.complement(), single]
            })
            .map(|set| {
                let excess = self.value(&set) - set.sum(x);
                (set, excess)
            })
            .filter(|(set, excess)| *excess > bound && !skip(set))
            .take(limit)
            .collect()
    }

    fn value_scale(&self) -> f64 {
        self.0.max_abs_value()
    }

    fn covers_all_coalitions(&self) -> bool {
        false
    }
}

#[test]
fn user_oracle_declaring_partial_coverage_is_approximate() {
    let game = generators::bnf(4, 5, 2).unwrap();
    assert_eq!(
        oracle_guarantees(&SingletonsOnly(game)),
        [Guarantee::Approximate; 3]
    );
}

/// 値が分かる提携だけを持つゲーム (`io::PartialGame`) は、値のない提携が 1 つでもあれば、`oracle` に直接渡しても
/// 近似になる。
#[cfg(feature = "io")]
#[test]
fn partial_game_with_missing_coalitions_is_approximate() {
    // 4 人。2 人提携のうち {2, 3} の値がない。
    let text = "coalition,value\n0,0\n1,0\n2,0\n3,0\n0|1,2\n0|2,2\n0|3,2\n1|2,2\n1|3,2\n\
                0|1|2,5\n0|1|3,5\n0|2|3,5\n1|2|3,5\n0|1|2|3,8\n";
    let partial = coopgame::io::parse_csv(text).unwrap().partial().unwrap();
    assert!(!Separation::covers_all_coalitions(&partial));
    assert_eq!(
        oracle::nucleolus(&partial).unwrap().guarantee(),
        Guarantee::Approximate
    );
    assert_eq!(
        oracle::least_core(&partial, Domain::Imputation)
            .unwrap()
            .guarantee(),
        Guarantee::Approximate
    );
    // PartialGame::nucleolus も同じ仕組みで近似になる。
    assert_eq!(
        partial.nucleolus(Domain::Imputation).unwrap().guarantee(),
        Guarantee::Approximate
    );
}

/// 全ての提携の値が分かっていれば、`PartialGame` は全提携について答えるオラクルで、結果は全提携の表から求めた仁と
/// 同じ保証になる。
#[cfg(feature = "io")]
#[test]
fn partial_game_with_all_coalitions_is_exact() {
    let text = "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|2,3\n1|2,2\n0|1|2,6\n";
    let data = coopgame::io::parse_csv(text).unwrap();
    let partial = data.partial().unwrap();
    assert!(Separation::covers_all_coalitions(&partial));
    let solved = partial.nucleolus(Domain::Imputation).unwrap();
    assert_eq!(solved.guarantee(), Guarantee::Exact);
    assert_eq!(
        oracle::least_core(&partial, Domain::Imputation)
            .unwrap()
            .guarantee(),
        Guarantee::Exact
    );
    let full = data.to_explicit(coopgame::io::Missing::Error).unwrap();
    let expected = coopgame::nucleolus::nucleolus(&full).unwrap();
    for (a, e) in solved.allocation().iter().zip(expected.allocation()) {
        assert!((a - e).abs() < 1e-7, "{solved:?} / {expected:?}");
    }
}
