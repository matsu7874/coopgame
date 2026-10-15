//! 破産ゲーム・重み付き投票ゲームの入力検証の回帰テスト。
//!
//! NaN の請求、遺産が請求の総和を超える破産ゲーム、基準 0 の投票ゲーム (空提携の値が 0 でない)、
//! 重みの和の `u64` のあふれを扱う。

use coopgame::Error;
use coopgame::PlayerSet;
use coopgame::SetFunction;
use coopgame::game::exact::Rational;
use coopgame::game::oracle::OracleGame;
use coopgame::games::bankruptcy::{
    BankruptcyGame, constrained_equal_awards, constrained_equal_losses, talmud_rule,
};
use coopgame::games::voting::WeightedVotingGame;

#[test]
fn bankruptcy_rules_reject_non_finite_amounts() {
    assert!(talmud_rule(1.0, &[f64::NAN, 1.0]).is_err());
    assert!(talmud_rule(f64::NAN, &[1.0, 1.0]).is_err());
    assert!(constrained_equal_awards(f64::NAN, &[1.0, 1.0]).is_err());
    assert!(constrained_equal_losses(1.0, &[f64::INFINITY, 1.0]).is_err());
    assert!(talmud_rule(f64::INFINITY, &[f64::INFINITY]).is_err());
    // 有理数は常に有限なので、従来どおり計算できる。
    let claims = [
        Rational::from_integer(2.into()),
        Rational::from_integer(4.into()),
    ];
    assert!(talmud_rule(Rational::from_integer(3.into()), &claims).is_ok());
}

#[test]
fn weighted_voting_weights_do_not_overflow() {
    let game = WeightedVotingGame::new(vec![u64::MAX, u64::MAX, 1], 2).unwrap();
    let n = 3;
    let value = |members: &[usize]| game.value(&PlayerSet::from_players(n, members));
    // 単調: 勝利提携に人を加えても勝利のまま。
    assert_eq!(value(&[0]), 1.0);
    assert_eq!(value(&[0, 1]), 1.0);
    assert_eq!(value(&[0, 1, 2]), 1.0);
    assert_eq!(value(&[2]), 0.0);
    // オラクルも全ての真部分提携を返す。
    let order: Vec<_> = game.excess_order(&[0.5, 0.5, 0.0]).collect();
    assert_eq!(order.len(), 6);
    for (set, e) in &order {
        assert!((game.value(set) - set.sum(&[0.5, 0.5, 0.0]) - e).abs() < 1e-12);
    }
}

#[test]
fn bankruptcy_game_rejects_estate_above_total_claims() {
    let err = BankruptcyGame::new(10.0, vec![1.0, 2.0]).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    let game = BankruptcyGame::new(3.0, vec![1.0, 2.0]).unwrap();
    assert_eq!(game.value(&PlayerSet::empty(2)), 0.0);
}

#[test]
fn weighted_voting_rejects_zero_quota() {
    let err = WeightedVotingGame::new(vec![1, 2], 0).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    let game = WeightedVotingGame::new(vec![1, 2], 1).unwrap();
    assert_eq!(game.value(&PlayerSet::empty(2)), 0.0);
}

#[test]
fn bankruptcy_rules_accept_negative_zero() {
    // `-0.0` は 0 と同じ (JSON の `-0` や `0.0 * -1.0` で現れる)。
    assert_eq!(talmud_rule(-0.0, &[1.0, 2.0]).unwrap(), vec![0.0, 0.0]);
    assert!(constrained_equal_awards(1.0, &[-0.0, 2.0]).is_ok());
    assert!(constrained_equal_losses(-0.0, &[-0.0, 1.0]).is_ok());
}

#[test]
fn bankruptcy_generator_validates_like_bankruptcy_game() {
    use coopgame::generators;
    assert!(generators::bankruptcy(f64::NAN, &[1.0, 2.0]).is_err());
    assert!(generators::bankruptcy(1.0, &[f64::NAN, 2.0]).is_err());
    assert!(generators::bankruptcy(1.0, &[f64::INFINITY, 2.0]).is_err());
    assert!(generators::bankruptcy(10.0, &[1.0, 2.0]).is_err());
    assert!(BankruptcyGame::new(10.0, vec![1.0, 2.0]).is_err());
    assert!(generators::bankruptcy(3.0, &[1.0, 2.0]).is_ok());
    assert!(generators::bankruptcy(-0.0, &[1.0, 2.0]).is_ok());
}

#[test]
fn weighted_voting_generator_rejects_non_finite_inputs() {
    use coopgame::generators;
    assert!(generators::weighted_voting(&[1.0, f64::NAN], 1.0).is_err());
    assert!(generators::weighted_voting(&[1.0, 1.0], f64::NAN).is_err());
    assert!(generators::weighted_voting(&[1.0, f64::INFINITY], 1.0).is_err());
    assert!(generators::weighted_voting(&[1.0, 1.0], 2.0).is_ok());
}
