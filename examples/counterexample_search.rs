//! 反例の探索と縮小 (`search`) の実例。見つけた反例を辞書式順の値で出す。
//!
//! ```bash
//! cargo run --release --example counterexample_search > data/analysis/counterexamples.txt
//! ```
//!
//! 1. 優加法的なのに、カーネルが 1 点でないゲーム (整数値、4-5 人)。
//! 2. 凸と仮定した凸ゲームの手法 (`convex::nucleolus(&Assume::convex(..))`) が、
//!    止まらずに誤った配分を返すゲーム (整数値、3-5 人)。

use coopgame::coalition::binary_to_lex;
use coopgame::convex;
use coopgame::generators::SplitMix64;
use coopgame::kernel_set::{SetOptions, kernel_set};
use coopgame::search::{SearchOptions, search};
use coopgame::structure::Assume;
use coopgame::verify::{Verified, VerifyOptions};
use coopgame::{Domain, ExplicitGame, properties};

/// 値 `0..=10 |S|` の整数から、優加法的な被覆を取った整数値のゲーム。
fn integer_superadditive(rng: &mut SplitMix64, n: usize) -> ExplicitGame {
    let size = 1usize << n;
    let mut values = vec![0.0; size];
    for (mask, value) in values.iter_mut().enumerate().skip(1) {
        *value = rng.range(0, 10 * mask.count_ones() as u64) as f64;
    }
    for mask in 1..size {
        let mut part = (mask - 1) & mask;
        while part > 0 {
            let rest = mask & !part;
            if part < rest {
                values[mask] = values[mask].max(values[part] + values[rest]);
            }
            part = (part - 1) & mask;
        }
    }
    ExplicitGame::from_binary(&values[1..]).unwrap()
}

fn kernel_is_not_a_point(game: &ExplicitGame) -> bool {
    match kernel_set(game, Domain::Imputation, SetOptions::for_game(game)) {
        Ok(set) => set.pieces.len() > 1 || set.pieces.iter().any(|p| p.dimension > 0),
        Err(_) => false,
    }
}

fn assumed_convex_is_wrong(game: &ExplicitGame) -> bool {
    if properties::is_convex(game) {
        return false;
    }
    let Ok(unverified) = convex::nucleolus(&Assume::convex(game)) else {
        return false;
    };
    matches!(
        unverified.verify(game, VerifyOptions::default()),
        Ok(Verified::Refuted { .. })
    )
}

fn show(title: &str, found: Option<coopgame::search::Counterexample>) {
    println!("# {title}");
    match found {
        None => println!("見つからなかった"),
        Some(c) => {
            let lex = binary_to_lex(&c.game.values()[1..]).unwrap();
            let original = binary_to_lex(&c.original.values()[1..]).unwrap();
            println!(
                "{} 番目の生成で発見 ({} 人)。縮小で {} 回変形し {} 人に。",
                c.attempt + 1,
                c.original.players(),
                c.shrink_steps,
                c.game.players()
            );
            println!("元の値 (辞書式順): {original:?}");
            println!("縮小した値 (辞書式順): {lex:?}");
        }
    }
    println!();
}

fn main() {
    let mut options = SearchOptions::default();
    options.attempts = 300;
    options.seed = 1;
    options.shrink_budget = 400;
    let found = search(
        |rng| {
            let n = 4 + (rng.next_u64() % 2) as usize;
            Ok(integer_superadditive(rng, n))
        },
        |g| properties::is_superadditive(g) && kernel_is_not_a_point(g),
        options,
    )
    .unwrap();
    show("優加法的で、カーネルが 1 点でないゲーム", found);

    let found = search(
        |rng| {
            let n = 3 + (rng.next_u64() % 3) as usize;
            Ok(integer_superadditive(rng, n))
        },
        assumed_convex_is_wrong,
        options,
    )
    .unwrap();
    show("凸と仮定した手法が誤った配分を返すゲーム", found);
}
