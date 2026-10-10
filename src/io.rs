//! プレイヤー名付きのゲームの読み書き (JSON・CSV) と、一部の提携だけ値が分かるゲーム。
//!
//! # JSON
//!
//! ```json
//! {
//!   "players": ["A", "B", "C"],
//!   "type": "value",
//!   "values": {"A": 0, "B": 0, "C": 0, "A,B": 4, "A,C": 3, "B,C": 2, "A,B,C": 6}
//! }
//! ```
//!
//! - `type` は `"value"` (既定、提携の価値) か `"cost"` (提携の費用)。
//! - `values` のキーは提携のメンバーの名前をカンマでつないだもの (順序と空白は問わない)。
//!   `[{"coalition": ["A", "B"], "value": 4}, ...]` の配列でもよい。
//! - `players` を省くと、`values` に現れた名前を現れた順に使う。
//!
//! # CSV
//!
//! 1 行目は見出し `coalition,value`。提携のメンバーは `|` でつなぐ (例: `A|B,4`)。
//! 費用ゲームは見出しを `coalition,cost` にする。プレイヤーは現れた順。
//!
//! # 値が分からない提携
//!
//! 全ての提携の値を見積もれないことは多い。[`Missing`] で扱いを選ぶ。
//!
//! - [`Missing::Error`]: 分からない提携があればエラーにして一覧を示す (既定)。
//! - [`Missing::Zero`]: 分からない提携の値を 0 とみなす。
//! - 値が分かる提携だけで解く: [`GameData::partial`] で [`PartialGame`] を作り、オラクル版の仁に渡す。
//!   結果は「値が分かる提携の超過だけを辞書式に最小化した配分」で、真のゲームの仁とは限らない。

use std::collections::HashMap;

use serde_json::Value;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::coalition::Coalition;
use crate::game::oracle::OracleGame;
use crate::game::{ExplicitGame, MAX_PLAYERS};
use crate::game::{PlayerSet, SetFunction};
use crate::nucleolus::NucleolusResult;
use crate::solution::Guarantee;

/// 値の種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// 提携の価値 (大きいほどよい)。
    Value,
    /// 提携の費用 (小さいほどよい)。[`crate::games::cost::CostGame`] で扱う。
    Cost,
}

impl Kind {
    /// JSON の `type` と CSV の見出しでの名前 (`"value"`・`"cost"`)。
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Value => "value",
            Kind::Cost => "cost",
        }
    }

    pub fn parse(text: &str) -> Option<Kind> {
        match text {
            "value" => Some(Kind::Value),
            "cost" => Some(Kind::Cost),
            _ => None,
        }
    }
}

/// 値が分からない提携の扱い。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Missing {
    Error,
    Zero,
}

/// 読み込んだゲーム。
#[derive(Clone, Debug, PartialEq)]
pub struct GameData {
    pub names: Vec<String>,
    pub kind: Kind,
    /// 値が分かる提携 (空でない) と値。
    pub known: Vec<(Coalition, f64)>,
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::Parse {
        line: 0,
        message: message.into(),
    }
}

struct Names {
    names: Vec<String>,
    index: HashMap<String, usize>,
    fixed: bool,
}

impl Names {
    fn new(fixed: Option<Vec<String>>) -> Result<Names> {
        let mut names = Names {
            names: Vec::new(),
            index: HashMap::new(),
            fixed: fixed.is_some(),
        };
        for name in fixed.unwrap_or_default() {
            if names
                .index
                .insert(name.clone(), names.names.len())
                .is_some()
            {
                return Err(parse_error(format!("プレイヤー名 {name:?} が重複している")));
            }
            names.names.push(name);
        }
        Ok(names)
    }

    fn player(&mut self, name: &str) -> Result<usize> {
        let name = name.trim();
        if name.is_empty() {
            return Err(parse_error("空のプレイヤー名"));
        }
        if let Some(&i) = self.index.get(name) {
            return Ok(i);
        }
        if self.fixed {
            return Err(parse_error(format!("players にない名前 {name:?}")));
        }
        let i = self.names.len();
        self.index.insert(name.to_string(), i);
        self.names.push(name.to_string());
        Ok(i)
    }

    fn coalition<'a>(&mut self, members: impl Iterator<Item = &'a str>) -> Result<Vec<usize>> {
        members.map(|name| self.player(name)).collect()
    }
}

fn build(names: Names, kind: Kind, entries: Vec<(Vec<usize>, f64)>) -> Result<GameData> {
    let n = names.names.len();
    if n == 0 {
        return Err(parse_error("プレイヤーがいない"));
    }
    if n > 63 {
        return Err(Error::TooManyPlayers {
            players: n,
            max: 63,
        });
    }
    let mut known: HashMap<u64, f64> = HashMap::new();
    for (members, value) in entries {
        if members.is_empty() {
            return Err(parse_error("空の提携の値は指定しない (常に 0)"));
        }
        if !value.is_finite() {
            return Err(parse_error("値が有限の数でない"));
        }
        let mask = Coalition::from_players(&members).0;
        if known.insert(mask, value).is_some() {
            return Err(parse_error(format!(
                "提携 {:?} の値が重複している",
                members
                    .iter()
                    .map(|&i| names.names[i].as_str())
                    .collect::<Vec<_>>()
            )));
        }
    }
    let mut known: Vec<(Coalition, f64)> = known
        .into_iter()
        .map(|(mask, v)| (Coalition(mask), v))
        .collect();
    known.sort_by_key(|(c, _)| c.0);
    let grand = Coalition::grand(n).0;
    if !known.iter().any(|(c, _)| c.0 == grand) {
        return Err(parse_error("全体提携の値がない"));
    }
    Ok(GameData {
        names: names.names,
        kind,
        known,
    })
}

/// JSON のテキストを読む。
pub fn parse_json(text: &str) -> Result<GameData> {
    let root: Value =
        serde_json::from_str(text).map_err(|e| parse_error(format!("JSON として読めない: {e}")))?;
    let kind = match root.get("type").and_then(Value::as_str) {
        None => Kind::Value,
        Some(text) => Kind::parse(text)
            .ok_or_else(|| parse_error(format!("type は \"value\" か \"cost\": {text:?}")))?,
    };
    let players = match root.get("players") {
        None => None,
        Some(Value::Array(list)) => Some(
            list.iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.trim().to_string())
                        .ok_or_else(|| parse_error("players は文字列の配列"))
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        Some(_) => return Err(parse_error("players は文字列の配列")),
    };
    let mut names = Names::new(players)?;
    let number = |v: &Value| v.as_f64().ok_or_else(|| parse_error("値は数"));
    let mut entries = Vec::new();
    match root.get("values") {
        Some(Value::Object(map)) => {
            for (key, value) in map {
                let members = names.coalition(key.split(','))?;
                entries.push((members, number(value)?));
            }
        }
        Some(Value::Array(list)) => {
            for item in list {
                let members = item
                    .get("coalition")
                    .and_then(Value::as_array)
                    .ok_or_else(|| parse_error("各要素に coalition (名前の配列) が必要"))?;
                let members: Vec<&str> = members
                    .iter()
                    .map(|m| {
                        m.as_str()
                            .ok_or_else(|| parse_error("coalition は名前の配列"))
                    })
                    .collect::<Result<_>>()?;
                let value = item
                    .get("value")
                    .ok_or_else(|| parse_error("各要素に value が必要"))?;
                entries.push((names.coalition(members.into_iter())?, number(value)?));
            }
        }
        _ => return Err(parse_error("values (オブジェクトか配列) が必要")),
    }
    build(names, kind, entries)
}

/// CSV のテキストを読む (見出し `coalition,value` か `coalition,cost`)。
pub fn parse_csv(text: &str) -> Result<GameData> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    let (_, header) = lines.next().ok_or_else(|| parse_error("空の CSV"))?;
    let header = header.replace(' ', "");
    let kind = header
        .strip_prefix("coalition,")
        .and_then(Kind::parse)
        .ok_or_else(|| {
            parse_error(format!(
                "見出しは coalition,value か coalition,cost: {header:?}"
            ))
        })?;
    let mut names = Names::new(None)?;
    let mut entries = Vec::new();
    for (line, row) in lines {
        let (coalition, value) = row.rsplit_once(',').ok_or(Error::Parse {
            line,
            message: "提携,値 の形でない".into(),
        })?;
        let value: f64 = value.trim().parse().map_err(|_| Error::Parse {
            line,
            message: format!("値として読めない: {value:?}"),
        })?;
        let members = names
            .coalition(coalition.split('|'))
            .map_err(|e| Error::Parse {
                line,
                message: e.to_string(),
            })?;
        entries.push((members, value));
    }
    build(names, kind, entries)
}

/// 拡張子 (`.json` か `.csv`) で形式を選んで読む。
pub fn parse_by_extension(path: &str, text: &str) -> Result<GameData> {
    if path.ends_with(".json") {
        parse_json(text)
    } else if path.ends_with(".csv") {
        parse_csv(text)
    } else {
        Err(Error::InvalidArgument(format!(
            "{path}: 拡張子が .json でも .csv でもない"
        )))
    }
}

impl GameData {
    pub fn players(&self) -> usize {
        self.names.len()
    }

    /// 値が分からない (空でない) 提携。
    pub fn missing(&self) -> Vec<Coalition> {
        let n = self.players();
        if n > MAX_PLAYERS {
            return Vec::new();
        }
        let known: std::collections::HashSet<u64> = self.known.iter().map(|(c, _)| c.0).collect();
        (1u64..(1 << n))
            .filter(|mask| !known.contains(mask))
            .map(Coalition)
            .collect()
    }

    /// 提携のメンバーの名前 (番号の昇順)。
    pub fn member_names(&self, coalition: Coalition) -> Vec<&str> {
        coalition
            .players()
            .map(|i| self.names[i].as_str())
            .collect()
    }

    fn describe(&self, coalition: Coalition) -> String {
        crate::game::coalition::format_coalition(coalition, Some(&self.names))
    }

    /// 全提携の値の表にする。
    pub fn to_explicit(&self, missing: Missing) -> Result<ExplicitGame> {
        let n = self.players();
        if n > MAX_PLAYERS {
            return Err(Error::TooManyPlayers {
                players: n,
                max: MAX_PLAYERS,
            });
        }
        let absent = self.missing();
        if missing == Missing::Error && !absent.is_empty() {
            let listed: Vec<String> = absent.iter().take(10).map(|c| self.describe(*c)).collect();
            return Err(Error::InvalidArgument(format!(
                "値が分からない提携が {} 個ある: {}{} (0 とみなすか、分かる提携だけで解く)",
                absent.len(),
                listed.join(" "),
                if absent.len() > 10 { " ..." } else { "" }
            )));
        }
        let mut values = vec![0.0; (1usize << n) - 1];
        for (coalition, value) in &self.known {
            values[coalition.index() - 1] = *value;
        }
        ExplicitGame::from_binary(&values)
    }

    /// 値が分かる提携だけを持つゲーム (価値のゲームに限る。費用なら節約ゲームに直してから使う)。
    pub fn partial(&self) -> Result<PartialGame> {
        if self.kind != Kind::Value {
            return Err(Error::InvalidArgument(
                "値が分かる提携だけで解くのは価値のゲーム (type = value) に限る".into(),
            ));
        }
        let n = self.players();
        let known: HashMap<PlayerSet, f64> = self
            .known
            .iter()
            .map(|(c, v)| (PlayerSet::from_coalition(n, *c), *v))
            .collect();
        Ok(PartialGame { players: n, known })
    }

    /// JSON に書く (値の種類と、値が分かる全ての提携)。
    pub fn to_json(&self) -> String {
        let values: serde_json::Map<String, Value> = self
            .known
            .iter()
            .map(|(c, v)| {
                let key = self.member_names(*c);
                (key.join(","), Value::from(*v))
            })
            .collect();
        let root = serde_json::json!({
            "players": self.names,
            "type": self.kind.as_str(),
            "values": values,
        });
        serde_json::to_string_pretty(&root).expect("JSON に書ける")
    }
}

/// 値が分かる提携だけを持つゲーム。分からない提携の値は NaN を返す。
#[derive(Clone, Debug, PartialEq)]
pub struct PartialGame {
    players: usize,
    known: HashMap<PlayerSet, f64>,
}

impl PartialGame {
    pub fn known(&self) -> usize {
        self.known.len()
    }

    /// 値が分かる提携の超過だけを辞書式に最小化した配分 (分かる提携に限った仁)。
    ///
    /// 逐次 LP を有界にするため、1 人提携とその補集合の値が全て分かっている必要がある。
    /// 結果は真のゲームの仁とは限らないので、保証は [`Guarantee::Approximate`] にする。
    pub fn nucleolus(&self, domain: Domain) -> Result<NucleolusResult> {
        let n = self.players;
        let full = PlayerSet::full(n);
        let mut absent = Vec::new();
        for i in 0..n {
            let single = PlayerSet::from_players(n, &[i]);
            for set in [single.complement(), single] {
                if set.is_proper() && !self.known.contains_key(&set) {
                    absent.push(set);
                }
            }
        }
        if !absent.is_empty() {
            return Err(Error::InvalidArgument(format!(
                "分かる提携だけで解くには、1 人提携とその補集合の値が全て必要 ({} 個が足りない)",
                absent.len()
            )));
        }
        if !self.known.contains_key(&full) {
            return Err(Error::InvalidArgument("全体提携の値がない".into()));
        }
        let mut result = crate::nucleolus::oracle::nucleolus_with(
            self,
            domain,
            crate::nucleolus::oracle::default_tolerance(self),
        )?;
        result.guarantee = Guarantee::Approximate;
        Ok(result)
    }
}

impl SetFunction for PartialGame {
    fn players(&self) -> usize {
        self.players
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if coalition.is_empty() {
            return 0.0;
        }
        self.known.get(coalition).copied().unwrap_or(f64::NAN)
    }
}

impl OracleGame for PartialGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        let mut order: Vec<(PlayerSet, f64)> = self
            .known
            .iter()
            .filter(|(set, _)| set.is_proper())
            .map(|(set, v)| (set.clone(), v - set.sum(x)))
            .collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Box::new(order.into_iter())
    }

    fn scale(&self) -> f64 {
        self.known.values().fold(1.0, |acc, v| acc.max(v.abs()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nucleolus;

    const JSON: &str = r#"{
        "players": ["A", "B", "C"],
        "values": {"A": 0, "B": 0, "C": 0, "A,B": 4, "A, C": 3, "C,B": 2, "A,B,C": 6}
    }"#;

    #[test]
    fn json_and_csv_give_same_game() {
        let from_json = parse_json(JSON).unwrap();
        let csv = "coalition,value\nA,0\nB,0\nC,0\nA|B,4\nA|C,3\nB|C,2\nA|B|C,6\n";
        let from_csv = parse_csv(csv).unwrap();
        assert_eq!(from_json, from_csv);
        let game = from_json.to_explicit(Missing::Error).unwrap();
        let expected = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 3.0, 2.0, 6.0]).unwrap();
        assert_eq!(game, expected);
        // 書き出して読み直すと同じ。
        assert_eq!(parse_json(&from_json.to_json()).unwrap(), from_json);
    }

    #[test]
    fn array_form_and_cost_type() {
        let text = r#"{"type": "cost", "values": [
            {"coalition": ["x"], "value": 5}, {"coalition": ["y"], "value": 7},
            {"coalition": ["x", "y"], "value": 9}]}"#;
        let data = parse_json(text).unwrap();
        assert_eq!(data.kind, Kind::Cost);
        assert_eq!(data.names, vec!["x".to_string(), "y".to_string()]);
        assert!(data.partial().is_err());
    }

    #[test]
    fn missing_values_are_reported_or_filled() {
        let csv = "coalition,value\nA,1\nB,1\nC,1\nA|B,3\nA|B|C,6\n";
        let data = parse_csv(csv).unwrap();
        assert_eq!(data.missing().len(), 2);
        let err = data.to_explicit(Missing::Error).unwrap_err().to_string();
        assert!(err.contains("{A, C}") && err.contains("{B, C}"), "{err}");
        let filled = data.to_explicit(Missing::Zero).unwrap();
        assert_eq!(filled.value(Coalition::from_players(&[0, 2])), 0.0);
    }

    #[test]
    fn partial_game_uses_only_known_coalitions() {
        // 全提携が分かっていれば、仁と一致する。
        let data = parse_json(JSON).unwrap();
        let full = data.to_explicit(Missing::Error).unwrap();
        let expected = nucleolus::nucleolus(&full).unwrap().allocation;
        let partial = data
            .partial()
            .unwrap()
            .nucleolus(Domain::Imputation)
            .unwrap();
        for (a, e) in partial.allocation.iter().zip(&expected) {
            assert!((a - e).abs() < 1e-7);
        }
        assert_eq!(partial.guarantee, Guarantee::Approximate);
        // 1 人提携の補集合 (2 人提携) が欠けていると解けない。
        let csv = "coalition,value\nA,0\nB,0\nC,0\nA|B,4\nA|B|C,6\n";
        assert!(
            parse_csv(csv)
                .unwrap()
                .partial()
                .unwrap()
                .nucleolus(Domain::Imputation)
                .is_err()
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_json(r#"{"players": ["A"], "values": {"B": 1}}"#).is_err());
        assert!(parse_json(r#"{"values": {"A": 1, "A": 2}}"#).is_ok()); // JSON のキー重複は後勝ち
        assert!(parse_csv("coalition,value\nA,1\nA,2\n").is_err());
        assert!(parse_csv("coalition,value\nA,1\nB,1\n").is_err()); // 全体提携がない
        assert!(parse_csv("name,value\nA,1\n").is_err());
    }
}
