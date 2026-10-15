//! 人数の境界 (0 人・64 人以上・ゲームの人数の外の番号) と、入力の大きさによるあふれの回帰テスト。
//!
//! いずれも debug ではパニックし、release では黙って誤った値 (ビットの折り返しなど) を返していた。

use coopgame::game::exact::{Rational, parse_rational};
use coopgame::partition::CoalitionStructure;
use coopgame::{Coalition, Error, ExplicitGame, PlayerSet, SetFunction, generators};

/// 2 人の組だけが 4 を稼ぐ 3 人ゲーム (仁は (2, 2, 0))。
fn pair_game() -> ExplicitGame {
    ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap()
}

#[test]
fn coalition_grand_and_complement_for_64_players() {
    assert_eq!(Coalition::grand(64), Coalition(u64::MAX));
    assert_eq!(Coalition::grand(0), Coalition(0));
    assert_eq!(Coalition::grand(63), Coalition(u64::MAX >> 1));
    assert_eq!(Coalition::empty().complement(64), Coalition(u64::MAX));
    assert_eq!(
        Coalition::try_from(&PlayerSet::grand(64)).unwrap(),
        Coalition::grand(64)
    );
}

#[test]
#[should_panic(expected = "64 人まで")]
fn coalition_grand_panics_above_64() {
    let _ = Coalition::grand(65);
}

#[test]
fn coalition_rejects_players_beyond_64() {
    assert!(Coalition::try_from_players(100, &[70]).is_err());
    assert!(Coalition::try_from_players(100, &[63]).is_ok());
    assert!(Coalition::try_from_players(5, &[5]).is_err());
    assert!(!Coalition(1).contains(64));
    assert!(!Coalition(u64::MAX).contains(200));
    let mut c = Coalition(1);
    c.remove(64);
    assert_eq!(c, Coalition(1));
}

#[test]
#[should_panic(expected = "64 未満")]
fn coalition_singleton_panics_beyond_64() {
    let _ = Coalition::singleton(64);
}

#[test]
#[should_panic(expected = "64 未満")]
fn coalition_insert_panics_beyond_64() {
    let mut c = Coalition::empty();
    c.insert(70);
}

#[test]
#[should_panic(expected = "64 未満")]
fn coalition_from_players_panics_beyond_64() {
    let _ = Coalition::from_players(&[1, 64]);
}

#[test]
#[should_panic(expected = "人数 3 以上")]
fn player_set_from_coalition_rejects_extra_bits() {
    let _ = PlayerSet::from_coalition(3, Coalition(0b11111));
}

#[test]
#[should_panic(expected = "人数 3 以上")]
fn player_set_insert_rejects_out_of_range() {
    let mut set = PlayerSet::empty(3);
    set.insert(5);
}

#[test]
fn player_set_out_of_range_queries() {
    assert_eq!(PlayerSet::from_coalition(3, Coalition(0b101)).len(), 2);
    assert_eq!(PlayerSet::from_coalition(70, Coalition(u64::MAX)).len(), 64);
    let mut set = PlayerSet::grand(3);
    assert!(!set.contains(3) && !set.contains(64));
    set.remove(7);
    assert_eq!(set.len(), 3);
}

#[test]
fn subgame_rejects_members_outside_game() {
    let game = pair_game();
    let err = game.subgame(Coalition::from_players(&[0, 5])).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
    assert_eq!(
        game.subgame(Coalition::from_players(&[0, 1]))
            .unwrap()
            .players(),
        2
    );
}

struct Empty;

impl SetFunction for Empty {
    fn players(&self) -> usize {
        0
    }
    fn value(&self, _: &PlayerSet) -> f64 {
        0.0
    }
}

#[test]
fn tabulate_reports_zero_players_as_invalid_argument() {
    let err = ExplicitGame::tabulate(&Empty).unwrap_err();
    assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
}

#[test]
fn generators_check_player_count_first() {
    for n in [31, 64, 100] {
        for result in [
            generators::bnf(1, n, 0),
            generators::random_convex(n, 0),
            generators::random_superadditive(n, 0),
        ] {
            assert!(
                matches!(result, Err(Error::TooManyPlayers { players, .. }) if players == n),
                "n={n}"
            );
        }
    }
    assert!(matches!(
        generators::random_convex(0, 0),
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(
        generators::random_superadditive(0, 0),
        Err(Error::InvalidArgument(_))
    ));
    assert!(matches!(
        generators::random_superadditive_exact(0, 0),
        Err(Error::InvalidArgument(_))
    ));
}

#[test]
fn coalition_structure_beyond_64_players_is_an_error() {
    let blocks = vec![(0..70).collect::<Vec<usize>>()];
    assert!(matches!(
        CoalitionStructure::new(70, &blocks),
        Err(Error::TooManyPlayers {
            players: 70,
            max: 64
        })
    ));
    let blocks = vec![(0..64).collect::<Vec<usize>>()];
    assert!(CoalitionStructure::new(64, &blocks).is_ok());
}

#[test]
fn parse_rational_bounds_the_exponent() {
    assert!(parse_rational("1e2000000000").is_err());
    assert!(parse_rational("1e-2000000000").is_err());
    assert!(parse_rational("1e5000").is_err());
    // i32 の範囲外の指数も読めない値として扱う。
    assert!(parse_rational("1e99999999999").is_err());
    assert_eq!(
        parse_rational("1.5e3").unwrap(),
        Rational::from_integer(1500.into())
    );
    assert_eq!(
        parse_rational("25e-2").unwrap(),
        Rational::new(1.into(), 4.into())
    );
    assert!(parse_rational("1e308").is_ok());
}
