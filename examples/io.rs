//! プレイヤー名付きの JSON・CSV を読む例 (`io` モジュール、feature `io`)。
//!
//! 1. JSON と CSV で同じ 3 人ゲームを読み、仁と Shapley 値を名前付きで出す。
//! 2. 費用ゲーム (`"type": "cost"`) を読み、費用の分担を求める。
//! 3. 値が分からない提携のあるゲームを、0 とみなす場合と、分かる提携だけで解く場合 (`PartialGame`) で比べる。
//!
//! ```bash
//! cargo run --features io --example io
//! ```
//!
//! 入力は例の中に文字列で埋め込んでいる (ファイルは読まない)。形式は docs/input-format.md を参照。
//! 読み込んだプレイヤーは名前の出てきた順に 0 始まりの番号が付く。表示には `GameData::names` を使う。

use coopgame::game::{format_coalition, player_name};
use coopgame::games::cost::CostGame;
use coopgame::io::{self, Kind, Missing};
use coopgame::{Coalition, Domain, ExplicitGame, Guarantee, nucleolus, values};

/// 共通のサンプル入力の 3 人ゲーム (辞書式順 [0, 0, 0, 4, 6, 8, 12]) を JSON で書いたもの。
const VALUE_JSON: &str = r#"{
  "players": ["A", "B", "C"],
  "type": "value",
  "values": {"A": 0, "B": 0, "C": 0, "A,B": 4, "A,C": 6, "B,C": 8, "A,B,C": 12}
}"#;

/// 同じゲームの CSV。提携のメンバーは `|` で区切る。`#` で始まる行は読み飛ばす。
const VALUE_CSV: &str = "\
coalition,value
# 1 人の提携
A,0
B,0
C,0
# 2 人以上の提携
A|B,4
A|C,6
B|C,8
A|B|C,12
";

/// 3 人の共同配送の費用 (docs/examples.md の費用ゲームの節と同じ値)。値を配列の形で書く。
const COST_JSON: &str = r#"{
  "players": ["A", "B", "C"],
  "type": "cost",
  "values": [
    {"coalition": ["A"], "value": 6},
    {"coalition": ["B"], "value": 6},
    {"coalition": ["C"], "value": 8},
    {"coalition": ["A", "B"], "value": 9},
    {"coalition": ["A", "C"], "value": 11},
    {"coalition": ["B", "C"], "value": 12},
    {"coalition": ["A", "B", "C"], "value": 15}
  ]
}"#;

/// 4 人ゲームで、提携 {B, D} と {C, D} の値が分からない場合。
const MISSING_JSON: &str = r#"{
  "players": ["A", "B", "C", "D"],
  "values": {
    "A": 0, "B": 0, "C": 0, "D": 0,
    "A,B": 4, "A,C": 6, "B,C": 8, "A,D": 2,
    "A,B,C": 12, "A,B,D": 8, "A,C,D": 10, "B,C,D": 12,
    "A,B,C,D": 18
  }
}"#;

fn main() -> coopgame::Result<()> {
    value_game()?;
    cost_game()?;
    missing_values()?;
    Ok(())
}

/// JSON と CSV で同じゲームを読む。
fn value_game() -> coopgame::Result<()> {
    println!("== 価値のゲーム (JSON・CSV) ==");
    let from_json = io::parse_json(VALUE_JSON)?;
    let from_csv = io::parse_csv(VALUE_CSV)?;
    println!("プレイヤー: {:?}", from_json.names);
    assert_eq!(from_json.kind, Kind::Value);

    // 全ての提携の値が分かっているので、Missing::Error でも全提携の表を作れる。
    let game = from_json.to_explicit(Missing::Error)?;
    let same = from_csv.to_explicit(Missing::Error)?;
    assert_eq!(from_json.names, from_csv.names);
    assert_eq!(game.values(), same.values());
    let lex = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    assert_eq!(game.values(), lex.values());
    println!("JSON と CSV は同じゲームになる。");

    let names = &from_json.names;
    let nucleolus = nucleolus::nucleolus(&game)?;
    let shapley = values::shapley(&game);
    println!(
        "仁: {} (保証 {})",
        format_allocation(&nucleolus.allocation, Some(names)),
        nucleolus.guarantee
    );
    println!("Shapley 値: {}", format_allocation(&shapley, Some(names)));
    assert!(close(&nucleolus.allocation, &[2.0, 4.0, 6.0], 1e-6));
    assert!(close(&shapley, &[3.0, 4.0, 5.0], 1e-9));

    // 値の分かる提携は GameData::known にある。format_coalition に名前を渡すと {A, B} の形で書ける。
    println!("読んだ値:");
    for (coalition, value) in &from_json.known {
        println!("  {}\t{value}", format_coalition(*coalition, Some(names)));
    }
    Ok(())
}

/// 費用ゲームを読み、費用の分担を求める。
fn cost_game() -> coopgame::Result<()> {
    println!("\n== 費用ゲーム (\"type\": \"cost\") ==");
    let data = io::parse_json(COST_JSON)?;
    assert_eq!(data.kind, Kind::Cost);
    // 費用 c(S) の表を CostGame に渡す。仁は節約ゲームで求めて費用に戻す。
    let game = CostGame::new(data.to_explicit(Missing::Error)?);
    let nucleolus = game.nucleolus()?;
    let shapley = game.shapley();
    println!(
        "単独の費用: {}",
        format_allocation(&game.standalone(), Some(&data.names))
    );
    println!(
        "仁による分担: {}",
        format_allocation(&nucleolus, Some(&data.names))
    );
    println!(
        "Shapley 値による分担: {}",
        format_allocation(&shapley, Some(&data.names))
    );
    assert!(close(
        &nucleolus,
        &[11.0 / 3.0, 14.0 / 3.0, 20.0 / 3.0],
        1e-6
    ));
    assert!(close(&shapley, &[4.0, 4.5, 6.5], 1e-6));
    println!("どちらも合計は全員の費用 15 で、各人は単独の費用より少なく払う。");
    Ok(())
}

/// 値が分からない提携のあるゲーム。
fn missing_values() -> coopgame::Result<()> {
    println!("\n== 値が分からない提携 ==");
    let data = io::parse_json(MISSING_JSON)?;
    let names = &data.names;
    let missing: Vec<String> = data
        .missing()
        .into_iter()
        .map(|c| format_coalition(c, Some(names)))
        .collect();
    println!("値が分からない提携: {}", missing.join(" "));
    assert_eq!(missing, vec!["{B, D}", "{C, D}"]);

    // 既定 (Missing::Error) では全提携の表を作れず、エラーで分からない提携を示す。
    match data.to_explicit(Missing::Error) {
        Ok(_) => panic!("値が分からない提携があるのに表を作れた"),
        Err(e) => println!("Missing::Error: {e}"),
    }

    // Missing::Zero: 分からない値を 0 とみなして全提携の表を作る。
    let zero = nucleolus::nucleolus(&data.to_explicit(Missing::Zero)?)?;
    println!(
        "0 とみなした仁: {} (保証 {})",
        format_allocation(&zero.allocation, Some(names)),
        zero.guarantee
    );

    // PartialGame: 値が分かる提携だけで超過を辞書式に最小化する。
    // 1 人提携とその補集合 (3 人の提携) の値が全て必要。真のゲームの仁とは限らないので、保証は approximate になる。
    let partial = data.partial()?;
    let approximate = partial.nucleolus(Domain::Imputation)?;
    println!(
        "分かる {} 個の提携だけで解いた仁: {} (保証 {})",
        partial.known(),
        format_allocation(&approximate.allocation, Some(names)),
        approximate.guarantee
    );
    assert_eq!(partial.known(), 13);
    assert_eq!(approximate.guarantee, Guarantee::Approximate);
    assert!(!approximate.guarantee.is_reliable());
    assert!(close(&zero.allocation, &[3.0, 5.0, 7.0, 3.0], 1e-6));
    assert!(close(&approximate.allocation, &[3.0, 5.0, 7.0, 3.0], 1e-6));

    // 分からなかった値が後で分かった場合と比べる。GameData::known に値を足すと全提携の表を作れる。
    let mut complete = data.clone();
    // 名前から番号を引く公開関数はないので、提携は 0 始まりの番号で書く。
    complete.known.push((Coalition::from_players(&[1, 3]), 7.0)); // {B, D}
    complete.known.push((Coalition::from_players(&[2, 3]), 9.0)); // {C, D}
    assert!(complete.missing().is_empty());
    let exact = nucleolus::nucleolus(&complete.to_explicit(Missing::Error)?)?;
    println!(
        "{{B, D}} = 7、{{C, D}} = 9 と分かった場合の仁: {} (保証 {})",
        format_allocation(&exact.allocation, Some(names)),
        exact.guarantee
    );
    // 分からなかった値によっては、分かる提携だけで解いた配分は真のゲームの仁と異なる。
    assert!(close(&exact.allocation, &[2.0, 5.0, 7.0, 4.0], 1e-6));
    Ok(())
}

/// 配分を `名前: 値` の形で 1 行に書く。名前がなければ 1 始まりの番号を使う。
fn format_allocation(x: &[f64], names: Option<&[String]>) -> String {
    let parts: Vec<String> = x
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{}: {v:.4}", player_name(names, i)))
        .collect();
    parts.join(", ")
}

/// 2 つの列が要素ごとに `tolerance` 以内で等しいか。
fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
