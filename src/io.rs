//! 提携ごとの値の読み書き (JSON・CSV) と、一部の提携だけ値が分かるゲーム。
//!
//! プレイヤーは 0 始まりの番号で書く。
//!
//! # JSON
//!
//! ```json
//! {
//!   "players": 3,
//!   "type": "value",
//!   "values": {"0": 0, "1": 0, "2": 0, "0,1": 4, "0,2": 3, "1,2": 2, "0,1,2": 6}
//! }
//! ```
//!
//! - `type` は `"value"` (既定、提携の価値) か `"cost"` (提携の費用)。
//! - `values` のキーは提携のメンバーの番号をカンマでつないだもの (順序と空白は問わない)。
//!   `[{"coalition": [0, 1], "value": 4}, ...]` の配列でもよい。
//! - `players` (人数) を省くと、現れた最大の番号 + 1 人とする。
//! - 他のトップレベルのキーは無視する (説明などを書き添えてよい)。ただし `"tpye"` や `"Players"` のように
//!   上の 3 つの書き間違いらしいキー (大文字小文字の違い、1 文字の違い・隣り合う 2 文字の入れ替え) はエラーにする。
//! - 値は数で書く。[`parse_json_exact`] は数の字面を 10 進数の値どおりの有理数として読む。
//! - 同じオブジェクトの中で同じキーを 2 回書くとエラーにする (後の値で黙って上書きしない)。
//!
//! # CSV
//!
//! 1 行目は見出し `coalition,value`。提携のメンバーの番号は `|` でつなぐ (例: `0|1,4`)。
//! 費用ゲームは見出しを `coalition,cost` にする。人数は現れた最大の番号 + 1。
//! 値は `8/3` の形の分数でもよい。先頭の UTF-8 の BOM は無視する。
//! 読めない行のエラー ([`Error::Parse`]) には 1 始まりの行番号が付く。
//!
//! # 重複と番号の範囲 (JSON・CSV 共通)
//!
//! - 同じ提携の値を 2 回書くとエラーにする (`"0,1"` と `"1,0"` のように順序だけが違う書き方も同じ提携)。
//! - 1 つの提携の中で同じ番号を 2 回書くとエラーにする (`0|0` は `0|1` の打ち間違いかもしれないので、`{0}` とは読まない)。
//! - 番号は `players` 未満 (`players` を省いたときは 63 未満。人数は 63 人まで)。
//!
//! # 値が分からない提携
//!
//! 全ての提携の値を見積もれないことは多い。[`Missing`] で扱いを選ぶ。
//!
//! - [`Missing::Error`]: 分からない提携があればエラーにして一覧を示す (既定)。
//! - [`Missing::Zero`]: 分からない提携の値を 0 とみなす。費用のゲームでは費用 `c(S) = 0` とみなすので、
//!   その提携の節約 `sum_{i in S} c({i}) - c(S)` は 1 人提携の費用の和になる。
//! - 値が分かる提携だけで解く: [`GameData::partial`] で [`PartialGame`] を作り、オラクル版の仁に渡す。
//!   結果は「値が分かる提携の超過だけを辞書式に最小化した配分」で、真のゲームの仁とは限らない。
//!   全提携の表を作れない 31 人以上 (63 人まで) のゲームも解ける。
//!
//! 分からない提携があるかは [`GameData::has_missing`] で、提携を列挙せずに確かめられる。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use num_traits::Zero;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::Value;
use serde_json::value::RawValue;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::exact::{self, ExactGame, Rational};
use crate::game::oracle::OracleGame;
use crate::game::scale::difference_or_zero;
use crate::game::{Coalition, ExplicitGame, MAX_PLAYERS, PlayerSet, SetFunction, strip_bom};
use crate::nucleolus::{NucleolusResult, Tolerance};
use crate::sorted::{LazySorted, by_excess_desc};

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
    pub players: usize,
    pub kind: Kind,
    /// 値が分かる提携 (空でない) と値。
    pub known: Vec<(Coalition, f64)>,
}

/// 値を 10 進数・分数の値どおりの有理数として読んだゲーム ([`parse_exact_by_extension`] など)。
///
/// JSON の数は字面 (`0.1` なら `1/10`) のまま有理数にし、浮動小数点数を経由しない。
/// CSV の値は `8/3` の形の分数も書ける。
#[derive(Clone, Debug, PartialEq)]
pub struct ExactGameData {
    pub players: usize,
    pub kind: Kind,
    /// 値が分かる提携 (空でない) と値。
    pub known: Vec<(Coalition, Rational)>,
}

impl ExactGameData {
    /// 値が分からない (空でない) 提携の一覧 (提携の番号順)。
    ///
    /// 一覧を作れるのは [`MAX_PLAYERS`] 人まで (超えれば [`Error::TooManyPlayers`])。
    /// 一覧は分からない提携の数に比例するメモリを使う (30 人で値がほとんどなければ約 8.6 GB)。
    /// 分からない提携があるかだけを知るには [`ExactGameData::has_missing`] を使う。
    pub fn missing(&self) -> Result<Vec<Coalition>> {
        missing_coalitions(self.players, self.known.iter().map(|(c, _)| *c))
    }

    /// 値が分からない (空でない) 提携があるか。提携を列挙せず、値が分かる提携の数で判定する。
    pub fn has_missing(&self) -> bool {
        has_missing_coalitions(self.players, self.known.iter().map(|(c, _)| *c))
    }

    /// 全提携の値を有理数で持つゲームにする。人数の上限と `missing` の扱いは [`GameData::to_explicit`] と同じ。
    pub fn to_exact(&self, missing: Missing) -> Result<ExactGame> {
        let n = self.players;
        check_table(n, self.known.iter().map(|(c, _)| *c), missing)?;
        let mut values = vec![Rational::zero(); (1usize << n) - 1];
        for (coalition, value) in &self.known {
            values[coalition.index() - 1] = value.clone();
        }
        ExactGame::from_binary(values)
    }
}

/// 行に結び付かない (入力全体についての) 形式のエラー。行番号は 0 にする。
fn parse_error(message: impl Into<String>) -> Error {
    error_at(None, message)
}

/// 行番号 (CSV の 1 始まりの行) が分かれば付けた形式のエラー。
fn error_at(line: Option<usize>, message: impl Into<String>) -> Error {
    Error::Parse {
        line: line.unwrap_or(0),
        message: message.into(),
    }
}

/// プレイヤーの番号を読む。
fn player(text: &str) -> std::result::Result<usize, String> {
    let text = text.trim();
    text.parse()
        .map_err(|_| format!("プレイヤーの番号 (0 以上の整数) でない: {text:?}"))
}

/// 数として解釈する前の 1 つの提携の値。値は字面のまま持ち、浮動小数点数と有理数のどちらにも読めるようにする。
struct RawEntry {
    members: Vec<usize>,
    value: String,
    /// CSV の行番号 (JSON は `None`)。
    line: Option<usize>,
}

/// 数として解釈する前のゲーム。
struct RawGame {
    players: Option<usize>,
    kind: Kind,
    entries: Vec<RawEntry>,
}

/// 値の字面を浮動小数点数にする (`8/3` の形の分数は最も近い浮動小数点数にする)。
fn real(text: &str) -> std::result::Result<f64, String> {
    use num_traits::ToPrimitive;
    let value = if text.contains('/') {
        exact::parse_rational(text).ok().and_then(|q| q.to_f64())
    } else {
        text.parse::<f64>().ok()
    };
    match value {
        None => Err(format!("値として読めない: {text:?}")),
        Some(v) if !v.is_finite() => Err(format!("値が有限の数でない: {text:?}")),
        Some(v) => Ok(v),
    }
}

/// 値の字面を 10 進数・分数の値どおりの有理数にする。
fn rational(text: &str) -> std::result::Result<Rational, String> {
    exact::parse_rational(text).map_err(|_| format!("値として読めない: {text:?}"))
}

/// 値が分かる提携と値 (提携の番号順)。
type Known<T> = Vec<(Coalition, T)>;

/// 読める人数の上限。提携を `u64` のビットで表し、全体提携 `2^n - 1` を作れる範囲。
const MAX_FILE_PLAYERS: usize = 63;

/// 提携のメンバーの番号を検査する (`limit` 未満で、重複がない)。`players` は入力に書かれた人数。
fn check_members(
    members: &[usize],
    limit: usize,
    players: Option<usize>,
) -> std::result::Result<(), String> {
    let mut seen: u64 = 0;
    for &i in members {
        if i >= limit {
            return Err(match players {
                Some(n) => format!("提携 {members:?} のプレイヤー番号 {i} が人数 {n} 以上"),
                None => format!(
                    "提携 {members:?} のプレイヤー番号 {i} が範囲外 (番号は 0 から {} まで)",
                    MAX_FILE_PLAYERS - 1
                ),
            });
        }
        if seen >> i & 1 == 1 {
            return Err(format!(
                "提携 {members:?} にプレイヤー番号 {i} が 2 回以上ある"
            ));
        }
        seen |= 1 << i;
    }
    Ok(())
}

/// 人数を決め、提携と値を検査して提携の番号順に並べる。`number` で値の字面を数にする。
fn build<T>(
    raw: RawGame,
    number: impl Fn(&str) -> std::result::Result<T, String>,
) -> Result<(usize, Kind, Known<T>)> {
    if let Some(n) = raw.players
        && n > MAX_FILE_PLAYERS
    {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_FILE_PLAYERS,
        });
    }
    // 番号を先に検査する (人数を省いたときに、最大の番号 + 1 があふれないように)。
    let limit = raw.players.unwrap_or(MAX_FILE_PLAYERS);
    for entry in &raw.entries {
        check_members(&entry.members, limit, raw.players)
            .map_err(|message| error_at(entry.line, message))?;
    }
    let highest = raw.entries.iter().flat_map(|e| &e.members).max();
    let n = raw.players.unwrap_or(highest.map_or(0, |&i| i + 1));
    if n == 0 {
        return Err(parse_error("プレイヤーがいない"));
    }
    let mut known: HashMap<u64, T> = HashMap::new();
    for entry in raw.entries {
        if entry.members.is_empty() {
            return Err(error_at(entry.line, "空の提携の値は指定しない (常に 0)"));
        }
        let value = number(&entry.value).map_err(|message| error_at(entry.line, message))?;
        let coalition = Coalition::from_players(&entry.members);
        if known.insert(coalition.0, value).is_some() {
            return Err(error_at(
                entry.line,
                format!("提携 {coalition} の値が重複している"),
            ));
        }
    }
    let mut known: Known<T> = known
        .into_iter()
        .map(|(mask, v)| (Coalition(mask), v))
        .collect();
    known.sort_by_key(|(c, _)| c.0);
    let grand = Coalition::grand(n).0;
    if !known.iter().any(|(c, _)| c.0 == grand) {
        return Err(parse_error("全体提携の値がない"));
    }
    Ok((n, raw.kind, known))
}

/// JSON のトップレベルのキー。これ以外のキーは無視する (説明などを書き添えられるように)。
const JSON_KEYS: [&str; 3] = ["players", "type", "values"];

/// `key` が `known` の書き間違いらしいか (大文字小文字の違い、または 1 文字の挿入・削除・置換・隣り合う 2 文字の入れ替え)。
fn looks_like(key: &str, known: &str) -> bool {
    let a: Vec<char> = key.to_lowercase().chars().collect();
    let b: Vec<char> = known.chars().collect();
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    match long.len() - short.len() {
        0 => {
            let diff: Vec<usize> = (0..a.len()).filter(|&k| a[k] != b[k]).collect();
            match diff[..] {
                [_] => true,
                [i, j] => j == i + 1 && a[i] == b[j] && a[j] == b[i],
                _ => false,
            }
        }
        1 => {
            let k = (0..short.len())
                .find(|&k| short[k] != long[k])
                .unwrap_or(short.len());
            short[k..] == long[k + 1..]
        }
        _ => false,
    }
}

type RawObject = BTreeMap<String, Box<RawValue>>;

/// JSON のオブジェクトのメンバーを、書かれた順に重複も含めて読んだもの (値は字面のまま)。
///
/// `BTreeMap` などに直接読むと、同じキーを 2 回書いたときに後の値が黙って残る。
/// 重複を検出するために、メンバーを全て保ったまま読む。
struct RawMembers(Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for RawMembers {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct MembersVisitor;

        impl<'de> Visitor<'de> for MembersVisitor {
            type Value = RawMembers;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON のオブジェクト")
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<RawMembers, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry::<String, Box<RawValue>>()? {
                    members.push(member);
                }
                Ok(RawMembers(members))
            }
        }

        deserializer.deserialize_map(MembersVisitor)
    }
}

impl RawMembers {
    /// キーが重複していなければ、キーで引ける形にする。`object` はエラーに書くオブジェクトの説明。
    fn unique(self, object: &str) -> Result<RawObject> {
        let mut map = RawObject::new();
        for (key, value) in self.0 {
            if map.contains_key(&key) {
                return Err(parse_error(format!(
                    "{object}のキー {key:?} が重複している"
                )));
            }
            map.insert(key, value);
        }
        Ok(map)
    }
}

/// JSON を、値を字面のまま読む。
fn raw_json(text: &str) -> Result<RawGame> {
    let root = serde_json::from_str::<RawMembers>(strip_bom(text))
        .map_err(|e| parse_error(format!("JSON のオブジェクトとして読めない: {e}")))?
        .unique("トップレベル")?;
    for key in root.keys() {
        if JSON_KEYS.contains(&key.as_str()) {
            continue;
        }
        if let Some(known) = JSON_KEYS.iter().find(|known| looks_like(key, known)) {
            return Err(parse_error(format!(
                "未知のキー {key:?} ({known:?} の書き間違いではないか)"
            )));
        }
    }
    let kind = match root.get("type") {
        None => Kind::Value,
        Some(raw) => {
            let bad = || {
                parse_error(format!(
                    "type は文字列 \"value\" か \"cost\": {}",
                    raw.get()
                ))
            };
            let text: String = serde_json::from_str(raw.get()).map_err(|_| bad())?;
            Kind::parse(&text).ok_or_else(bad)?
        }
    };
    let players = match root.get("players") {
        None => None,
        Some(raw) => Some(
            serde_json::from_str::<usize>(raw.get())
                .map_err(|_| parse_error("players は人数 (0 以上の整数)"))?,
        ),
    };
    // 数の字面 (JSON の文法で検査済み) だけを値として受け付ける。
    let number = |raw: &RawValue| {
        let text = raw.get().trim();
        if text.starts_with(|c: char| c == '-' || c.is_ascii_digit()) {
            Ok(text.to_string())
        } else {
            Err(parse_error(format!("値は数: {text}")))
        }
    };
    let values = root
        .get("values")
        .map(|raw| raw.get().trim())
        .unwrap_or_default();
    let mut entries = Vec::new();
    if values.starts_with('{') {
        // 同じキーを 2 回書いた場合も、同じ提携の値の重複として build がエラーにする。
        let members: RawMembers =
            serde_json::from_str(values).map_err(|e| parse_error(format!("values: {e}")))?;
        for (key, value) in &members.0 {
            let members = key
                .split(',')
                .map(player)
                .collect::<std::result::Result<_, _>>()
                .map_err(parse_error)?;
            entries.push(RawEntry {
                members,
                value: number(value)?,
                line: None,
            });
        }
    } else if values.starts_with('[') {
        let list: Vec<RawMembers> = serde_json::from_str(values).map_err(|_| {
            parse_error("values の各要素は {\"coalition\": [番号, ...], \"value\": 値} の形")
        })?;
        for (k, item) in list.into_iter().enumerate() {
            let item = item.unique(&format!("values の {k} 番目の要素"))?;
            let members: Vec<usize> = item
                .get("coalition")
                .and_then(|raw| serde_json::from_str(raw.get()).ok())
                .ok_or_else(|| {
                    parse_error("各要素に coalition (番号 (0 以上の整数) の配列) が必要")
                })?;
            let value = item
                .get("value")
                .ok_or_else(|| parse_error("各要素に value が必要"))?;
            entries.push(RawEntry {
                members,
                value: number(value)?,
                line: None,
            });
        }
    } else {
        return Err(parse_error("values (オブジェクトか配列) が必要"));
    }
    Ok(RawGame {
        players,
        kind,
        entries,
    })
}

/// CSV を、値を字面のまま読む。
fn raw_csv(text: &str) -> Result<RawGame> {
    let mut lines = strip_bom(text)
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    let (header_line, header) = lines.next().ok_or_else(|| parse_error("空の CSV"))?;
    let header = header.replace(' ', "");
    let kind = header
        .strip_prefix("coalition,")
        .and_then(Kind::parse)
        .ok_or_else(|| {
            error_at(
                Some(header_line),
                format!("見出しは coalition,value か coalition,cost: {header:?}"),
            )
        })?;
    let mut entries = Vec::new();
    for (line, row) in lines {
        let (coalition, value) = row
            .rsplit_once(',')
            .ok_or_else(|| error_at(Some(line), "提携,値 の形でない"))?;
        let members = coalition
            .split('|')
            .map(player)
            .collect::<std::result::Result<_, _>>()
            .map_err(|message| error_at(Some(line), message))?;
        entries.push(RawEntry {
            members,
            value: value.trim().to_string(),
            line: Some(line),
        });
    }
    Ok(RawGame {
        players: None,
        kind,
        entries,
    })
}

/// 拡張子 (`.json` か `.csv`) で読み方を選ぶ。
fn raw_by_extension(path: &str, text: &str) -> Result<RawGame> {
    if path.ends_with(".json") {
        raw_json(text)
    } else if path.ends_with(".csv") {
        raw_csv(text)
    } else {
        Err(Error::InvalidArgument(format!(
            "{path}: 拡張子が .json でも .csv でもない"
        )))
    }
}

impl GameData {
    fn from_raw(raw: RawGame) -> Result<GameData> {
        let (players, kind, known) = build(raw, real)?;
        Ok(GameData {
            players,
            kind,
            known,
        })
    }
}

impl ExactGameData {
    fn from_raw(raw: RawGame) -> Result<ExactGameData> {
        let (players, kind, known) = build(raw, rational)?;
        Ok(ExactGameData {
            players,
            kind,
            known,
        })
    }
}

/// JSON のテキストを読む。
pub fn parse_json(text: &str) -> Result<GameData> {
    GameData::from_raw(raw_json(text)?)
}

/// CSV のテキストを読む (見出し `coalition,value` か `coalition,cost`)。
pub fn parse_csv(text: &str) -> Result<GameData> {
    GameData::from_raw(raw_csv(text)?)
}

/// 拡張子 (`.json` か `.csv`) で形式を選んで読む。
pub fn parse_by_extension(path: &str, text: &str) -> Result<GameData> {
    GameData::from_raw(raw_by_extension(path, text)?)
}

/// JSON のテキストを、値を 10 進数・分数の値どおりの有理数として読む。
pub fn parse_json_exact(text: &str) -> Result<ExactGameData> {
    ExactGameData::from_raw(raw_json(text)?)
}

/// CSV のテキストを、値を 10 進数・分数の値どおりの有理数として読む。
pub fn parse_csv_exact(text: &str) -> Result<ExactGameData> {
    ExactGameData::from_raw(raw_csv(text)?)
}

/// 拡張子 (`.json` か `.csv`) で形式を選び、値を 10 進数・分数の値どおりの有理数として読む。
pub fn parse_exact_by_extension(path: &str, text: &str) -> Result<ExactGameData> {
    ExactGameData::from_raw(raw_by_extension(path, text)?)
}

/// 人数 `n` のゲームで、`known` に含まれない (空でない) 提携 (提携の番号順)。
///
/// 一覧は最大 `2^n - 1` 個になるので、全提携の表と同じく [`MAX_PLAYERS`] 人までに限る。
fn missing_coalitions(n: usize, known: impl Iterator<Item = Coalition>) -> Result<Vec<Coalition>> {
    if n > MAX_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_PLAYERS,
        });
    }
    let known: HashSet<u64> = known.map(|c| c.0).collect();
    Ok((1u64..(1 << n))
        .filter(|mask| !known.contains(mask))
        .map(Coalition)
        .collect())
}

/// 人数 `n` のゲームで、`known` に含まれない (空でない) 提携があるか。
///
/// 提携を列挙せず、`known` の異なる提携 (空でなく、`n` 人の全体提携に含まれるもの) の数が
/// `2^n - 1` 未満かで判定する。値が分かる提携の数だけの時間で済む。
fn has_missing_coalitions(n: usize, known: impl Iterator<Item = Coalition>) -> bool {
    let inside = |c: &Coalition| !c.is_empty() && (n >= 64 || c.0 >> n == 0);
    let distinct: HashSet<u64> = known.filter(inside).map(|c| c.0).collect();
    let total = u32::try_from(n)
        .ok()
        .and_then(|n| 1u128.checked_shl(n))
        .map_or(u128::MAX, |t| t - 1);
    (distinct.len() as u128) < total
}

/// 値が分からない (空でない) 提携の数と、その最初の `limit` 個 (ビット表現の小さい順)。
///
/// 数は `2^n - 1` から値が分かる提携の数を引いて求め、提携は `limit` 個見つけた時点で列挙をやめる
/// (30 人で値がほとんどないゲームでも、`2^30` 個の提携を並べない)。`n` は [`MAX_PLAYERS`] 以下であること。
fn missing_summary(
    n: usize,
    known: impl Iterator<Item = Coalition>,
    limit: usize,
) -> (u64, Vec<Coalition>) {
    let total = (1u64 << n) - 1;
    let known: HashSet<u64> = known
        .map(|c| c.0)
        .filter(|mask| (1..=total).contains(mask))
        .collect();
    let first = (1..=total)
        .filter(|mask| !known.contains(mask))
        .take(limit)
        .map(Coalition)
        .collect();
    (total - known.len() as u64, first)
}

/// 全提携の値の表を作れるか確かめる (人数の上限と、値が分からない提携の扱い)。
fn check_table(n: usize, known: impl Iterator<Item = Coalition>, missing: Missing) -> Result<()> {
    if n > MAX_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_PLAYERS,
        });
    }
    if missing == Missing::Error {
        let (count, first) = missing_summary(n, known, 10);
        if count > 0 {
            let listed: Vec<String> = first.iter().map(Coalition::to_string).collect();
            return Err(Error::InvalidArgument(format!(
                "値が分からない提携が {count} 個ある: {}{}",
                listed.join(" "),
                if count > first.len() as u64 {
                    " ..."
                } else {
                    ""
                }
            )));
        }
    }
    Ok(())
}

impl GameData {
    /// 値が分からない (空でない) 提携の一覧 (提携の番号順)。
    ///
    /// 一覧を作れるのは [`MAX_PLAYERS`] 人まで (超えれば [`Error::TooManyPlayers`])。
    /// 一覧は分からない提携の数に比例するメモリを使う (30 人で値がほとんどなければ約 8.6 GB)。
    /// 分からない提携があるかだけを知るには [`GameData::has_missing`] を使う。
    pub fn missing(&self) -> Result<Vec<Coalition>> {
        missing_coalitions(self.players, self.known.iter().map(|(c, _)| *c))
    }

    /// 値が分からない (空でない) 提携があるか。
    ///
    /// 提携を列挙せず、値が分かる提携の数が `2^n - 1` 未満かで判定するので、
    /// 全提携の表を作れない人数 ([`MAX_PLAYERS`] 人を超える) でも使える。
    pub fn has_missing(&self) -> bool {
        has_missing_coalitions(self.players, self.known.iter().map(|(c, _)| *c))
    }

    /// 全提携の値の表にする。
    ///
    /// [`MAX_PLAYERS`] 人を超えると [`Error::TooManyPlayers`]。値が分からない提携があり、`missing` が
    /// [`Missing::Error`] なら、分からない提携 (最初の 10 個) を示した [`Error::InvalidArgument`] を返す。
    /// 分からない値を 0 とみなすなら [`Missing::Zero`]、分かる提携だけで解くなら [`GameData::partial`] を使う。
    pub fn to_explicit(&self, missing: Missing) -> Result<ExplicitGame> {
        let n = self.players;
        check_table(n, self.known.iter().map(|(c, _)| *c), missing)?;
        let mut values = vec![0.0; (1usize << n) - 1];
        for (coalition, value) in &self.known {
            values[coalition.index() - 1] = *value;
        }
        ExplicitGame::from_binary(&values)
    }

    /// 1 人提携の値 `v({i})` (分からない 1 人提携があればエラー)。
    pub fn singleton_values(&self) -> Result<Vec<f64>> {
        (0..self.players)
            .map(|i| {
                let single = Coalition::singleton(i);
                self.known
                    .iter()
                    .find(|(c, _)| *c == single)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| {
                        Error::InvalidArgument(format!("1 人提携 {single} の値が分からない"))
                    })
            })
            .collect()
    }

    /// 費用のゲーム (type = cost) を、値が分かる提携だけの節約ゲーム `s(S) = sum_{i in S} c({i}) - c(S)` に直す。
    ///
    /// 1 人提携の費用が全て必要。節約ゲームの配分 `x` は、[`crate::games::cost::CostGame`] と同じく
    /// `y_i = c({i}) - x_i` で費用の分担に戻す (`c({i})` は [`GameData::singleton_values`])。
    pub fn savings(&self) -> Result<GameData> {
        if self.kind != Kind::Cost {
            return Err(Error::InvalidArgument(
                "節約ゲームに直すのは費用のゲーム (type = cost) に限る".into(),
            ));
        }
        let standalone = self.singleton_values()?;
        let known = self
            .known
            .iter()
            .map(|(c, cost)| {
                let alone: f64 = c.members().map(|i| standalone[i]).sum();
                (*c, difference_or_zero(alone, *cost, c.len()))
            })
            .collect();
        Ok(GameData {
            players: self.players,
            kind: Kind::Value,
            known,
        })
    }

    /// 値が分かる提携だけを持つゲーム (価値のゲームに限る。費用なら [`GameData::savings`] で節約ゲームに直してから使う)。
    pub fn partial(&self) -> Result<PartialGame> {
        if self.kind != Kind::Value {
            return Err(Error::InvalidArgument(
                "値が分かる提携だけで解くのは価値のゲーム (type = value) に限る".into(),
            ));
        }
        let n = self.players;
        let known: HashMap<PlayerSet, f64> = self
            .known
            .iter()
            .map(|(c, v)| (PlayerSet::from_coalition(n, *c), *v))
            .collect();
        Ok(PartialGame { players: n, known })
    }

    /// 値が分かる提携だけで仁 (`Domain::Imputation`) またはプレ仁を求め、入力の値の種類で配分を返す。
    ///
    /// 価値のゲームは [`GameData::partial`] のゲームを解く。費用のゲームは節約ゲーム ([`GameData::savings`])
    /// を解き、費用の分担 `c({i}) - x_i` に直す。必要な値は [`PartialGame::nucleolus`] と同じで、
    /// 費用のゲームでは 1 人提携の費用も全て必要。
    pub fn partial_nucleolus(&self, domain: Domain) -> Result<PartialNucleolus> {
        self.partial_nucleolus_with(domain, Tolerance::Auto)
    }

    /// 許容誤差の決め方を指定した [`GameData::partial_nucleolus`] ([`PartialGame::nucleolus_with`])。
    /// 費用のゲームでは、許容誤差は節約ゲームの値の単位で指定する (比で指定するときも、節約ゲームの値の大きさに対する比)。
    pub fn partial_nucleolus_with(
        &self,
        domain: Domain,
        tolerance: Tolerance,
    ) -> Result<PartialNucleolus> {
        let (game, standalone) = match self.kind {
            Kind::Value => (self.partial()?, None),
            Kind::Cost => (self.savings()?.partial()?, Some(self.singleton_values()?)),
        };
        let solved = game.nucleolus_with(domain, tolerance)?;
        let allocation = match standalone {
            Some(costs) => costs
                .iter()
                .zip(solved.allocation())
                .map(|(cost, saving)| cost - saving)
                .collect(),
            None => solved.allocation().to_vec(),
        };
        Ok(PartialNucleolus { allocation, solved })
    }

    /// JSON に書く (値の種類と、値が分かる全ての提携)。
    pub fn to_json(&self) -> String {
        let values: serde_json::Map<String, Value> = self
            .known
            .iter()
            .map(|(c, v)| {
                let key = c
                    .members()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                (key, Value::from(*v))
            })
            .collect();
        let root = serde_json::json!({
            "players": self.players,
            "type": self.kind.as_str(),
            "values": values,
        });
        serde_json::to_string_pretty(&root).expect("JSON に書ける")
    }
}

/// 値が分かる提携だけで解いた仁 ([`GameData::partial_nucleolus`] の結果)。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PartialNucleolus {
    /// 入力の値の種類での配分。価値のゲームなら配分、費用のゲームなら費用の分担。
    pub allocation: Vec<f64>,
    /// 解いたゲーム (費用のゲームでは節約ゲーム) での逐次 LP の結果。段の値と保証はこのゲームでのもの。
    pub solved: NucleolusResult,
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
    /// 結果は真のゲームの仁とは限らないので、保証は [`crate::Guarantee::Approximate`] になる
    /// (このゲームは値が分かる提携だけを返すオラクルなので、[`crate::nucleolus::oracle`] が近似とする)。
    /// 全ての提携の値が分かっていれば配分は全提携の表から求めた仁と一致するが、保証は変えない。
    pub fn nucleolus(&self, domain: Domain) -> Result<NucleolusResult> {
        self.nucleolus_with(domain, Tolerance::Auto)
    }

    /// 許容誤差の決め方を指定した [`PartialGame::nucleolus`]。
    ///
    /// [`Tolerance::Relative`] は値が分かる提携の値の最大絶対値に対する比。[`Tolerance::Auto`] は既定の許容誤差
    /// (比 `1e-7`) で 1 回だけ解く (値が分かる提携だけでは Kohlberg 基準を確かめられないので、解き直しはしない)。
    pub fn nucleolus_with(&self, domain: Domain, tolerance: Tolerance) -> Result<NucleolusResult> {
        let n = self.players;
        let full = PlayerSet::grand(n);
        let mut absent = Vec::new();
        for i in 0..n {
            let single = PlayerSet::singleton(n, i);
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
        let tolerance = match tolerance {
            Tolerance::Absolute(tolerance) => tolerance,
            Tolerance::Relative(ratio) => {
                crate::game::scale::tolerance_from_ratio(ratio, OracleGame::scale(self))?
            }
            // 値が分かる提携だけでは Kohlberg 基準を確かめられないので、既定の許容誤差で 1 回だけ解く。
            _ => crate::nucleolus::oracle::default_tolerance(self),
        };
        crate::nucleolus::oracle::nucleolus_with(self, domain, tolerance)
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
        let order: Vec<(PlayerSet, f64)> = self
            .known
            .iter()
            .filter(|(set, _)| set.is_proper())
            .map(|(set, v)| (set.clone(), v - set.sum(x)))
            .collect();
        let first_chunk = 4 * self.players * self.players;
        Box::new(LazySorted::new(order, first_chunk, by_excess_desc))
    }

    fn scale(&self) -> f64 {
        self.known.values().fold(0.0, |acc, v| acc.max(v.abs()))
    }

    /// 値が分かる提携だけを返すので `false`。このゲームで求めた仁・最小コアは近似になる
    /// (全ての提携の値が分かっている場合も、値が分かる提携だけを持つゲームとして扱う。
    /// 保証は求め方で決まり、厳密さは [`crate::verify`] の検証で示す)。
    fn lists_all_coalitions(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nucleolus;
    use crate::solution::Guarantee;

    const JSON: &str = r#"{
        "players": 3,
        "values": {"0": 0, "1": 0, "2": 0, "0,1": 4, "0, 2": 3, "2,1": 2, "0,1,2": 6}
    }"#;

    #[test]
    fn json_and_csv_give_same_game() {
        let from_json = parse_json(JSON).unwrap();
        let csv = "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|2,3\n1|2,2\n0|1|2,6\n";
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
            {"coalition": [0], "value": 5}, {"coalition": [1], "value": 7},
            {"coalition": [0, 1], "value": 9}]}"#;
        let data = parse_json(text).unwrap();
        assert_eq!(data.kind, Kind::Cost);
        assert_eq!(data.players, 2);
        assert!(data.partial().is_err());
    }

    #[test]
    fn missing_values_are_reported_or_filled() {
        let csv = "coalition,value\n0,1\n1,1\n2,1\n0|1,3\n0|1|2,6\n";
        let data = parse_csv(csv).unwrap();
        assert_eq!(data.missing().unwrap().len(), 2);
        assert!(data.has_missing());
        let err = data.to_explicit(Missing::Error).unwrap_err().to_string();
        assert!(err.contains("{0, 2}") && err.contains("{1, 2}"), "{err}");
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
        // 全提携の値が分かっていても、値が分かる提携だけを扱う手法の結果なので近似とする。
        assert_eq!(partial.guarantee, Guarantee::Approximate);
        // 1 人提携の補集合 (2 人提携) が欠けていると解けない。
        let csv = "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|1|2,6\n";
        assert!(
            parse_csv(csv)
                .unwrap()
                .partial()
                .unwrap()
                .nucleolus(Domain::Imputation)
                .is_err()
        );
    }

    /// 値が分かる提携だけで解く仁は、価値のゲームでは配分、費用のゲームでは節約ゲームで解いた費用の分担を返す。
    #[test]
    fn partial_nucleolus_follows_the_kind_of_values() {
        // 2 人提携の値がない 4 人ゲーム。
        let rows = "0,5\n1,7\n2,6\n3,4\n0|1|2,15\n0|1|3,13\n0|2|3,12\n1|2|3,14\n0|1|2|3,18\n";
        // 費用として読むと、節約ゲームは 3 人提携が全て 3、全体が 4 で対称なので、節約は 1 ずつ。
        let cost = parse_csv(&format!("coalition,cost\n{rows}")).unwrap();
        let solved = cost.partial_nucleolus(Domain::Imputation).unwrap();
        for (share, expected) in solved.allocation.iter().zip([4.0, 6.0, 5.0, 3.0]) {
            assert!((share - expected).abs() < 1e-7, "{solved:?}");
        }
        for saving in solved.solved.allocation() {
            assert!((saving - 1.0).abs() < 1e-7, "{solved:?}");
        }
        assert_eq!(solved.solved.guarantee(), Guarantee::Approximate);
        // 価値として読むと、PartialGame::nucleolus と同じ結果を返す
        // (1 人提携の値の和が全体の値を超えて配分集合が空なので、プレ仁で比べる)。
        let value = parse_csv(&format!("coalition,value\n{rows}")).unwrap();
        let solved = value.partial_nucleolus(Domain::Preimputation).unwrap();
        let direct = value
            .partial()
            .unwrap()
            .nucleolus(Domain::Preimputation)
            .unwrap();
        assert_eq!(solved.allocation, direct.allocation);
        assert_eq!(solved.solved, direct);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_json(r#"{"players": 1, "values": {"1": 1}}"#).is_err()); // 人数の範囲外
        assert!(parse_json(r#"{"values": {"0": 1, "0": 2}}"#).is_err()); // JSON のキー重複
        assert!(parse_json(r#"{"values": {"A": 1}}"#).is_err()); // 番号でない
        assert!(parse_csv("coalition,value\n0,1\n0,2\n").is_err());
        assert!(parse_csv("coalition,value\n0,1\n1,1\n").is_err()); // 全体提携がない
        assert!(parse_csv("name,value\n0,1\n").is_err());
    }

    /// 同じ提携の値の重複は、JSON のキー・順序だけ違うキー・配列・CSV のどの書き方でもエラーにする。
    #[test]
    fn duplicate_coalitions_are_rejected_in_every_form() {
        let inputs = [
            parse_json(r#"{"values": {"0": 0, "1": 0, "0,1": 4, "0,1": 5}}"#),
            parse_json(r#"{"values": {"0": 0, "1": 0, "0,1": 4, "1,0": 5}}"#),
            parse_json(
                r#"{"values": [{"coalition": [0], "value": 0}, {"coalition": [1], "value": 0},
                    {"coalition": [0, 1], "value": 4}, {"coalition": [1, 0], "value": 5}]}"#,
            ),
            parse_csv("coalition,value\n0,0\n1,0\n0|1,4\n1|0,5\n"),
        ];
        for result in inputs {
            let err = result.unwrap_err().to_string();
            assert!(err.contains("提携 {0, 1} の値が重複している"), "{err}");
        }
        // 字面どおりに読む場合も同じ。
        let err = parse_json_exact(r#"{"values": {"0": 0, "1": 0, "0,1": 4, "0,1": 5}}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("重複"), "{err}");
    }

    /// トップレベルや配列の要素の中で同じキーを 2 回書くと、後の値で上書きせずにエラーにする。
    #[test]
    fn duplicate_json_keys_are_rejected() {
        let err = parse_json(r#"{"type": "cost", "type": "value", "values": {"0": 1}}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("トップレベルのキー \"type\" が重複"), "{err}");
        let err = parse_json(r#"{"values": [{"coalition": [0], "value": 1, "value": 2}]}"#)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("values の 0 番目の要素のキー \"value\" が重複"),
            "{err}"
        );
    }

    /// 1 つの提携の中の同じ番号は、`{0}` と読まずにエラーにする (`0|1` の打ち間違いを見逃さないため)。
    #[test]
    fn duplicate_members_are_rejected() {
        let err = parse_csv("coalition,value\n1,0\n0|0,1\n0|1,3\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 3, .. }), "{err:?}");
        assert!(
            err.to_string()
                .contains("提携 [0, 0] にプレイヤー番号 0 が 2 回以上ある"),
            "{err}"
        );
        for text in [
            r#"{"values": {"0,0": 1, "1": 0, "0,1": 3}}"#,
            r#"{"values": [{"coalition": [0, 0], "value": 1}, {"coalition": [0, 1], "value": 3}]}"#,
        ] {
            let err = parse_json(text).unwrap_err().to_string();
            assert!(err.contains("2 回以上ある"), "{text}: {err}");
        }
    }

    /// 人数を省いた入力の番号が大きすぎても、あふれずに範囲外の番号として行番号つきで報告する。
    #[test]
    fn huge_player_index_is_out_of_range() {
        let max = usize::MAX;
        let err = parse_csv(&format!("coalition,value\n0,1\n{max},1\n")).unwrap_err();
        assert!(matches!(err, Error::Parse { line: 3, .. }), "{err:?}");
        assert!(err.to_string().contains("範囲外"), "{err}");
        let err = parse_csv("coalition,value\n63,1\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 2, .. }), "{err:?}");
        let err = parse_json(&format!(
            r#"{{"values": [{{"coalition": [{max}], "value": 1}}]}}"#
        ))
        .unwrap_err();
        assert!(err.to_string().contains("範囲外"), "{err}");
        // 番号 62 (63 人) までは読める。
        let grand: Vec<String> = (0..63).map(|i| i.to_string()).collect();
        let data = parse_csv(&format!("coalition,value\n{},1\n", grand.join("|"))).unwrap();
        assert_eq!(data.players, 63);
        // 人数を書いた場合は人数と比べる (行番号つき)。
        let err = parse_json(r#"{"players": 2, "values": {"0": 1, "2": 1}}"#).unwrap_err();
        assert!(err.to_string().contains("人数 2 以上"), "{err}");
        assert!(matches!(
            parse_json(r#"{"players": 64, "values": {"0": 1}}"#).unwrap_err(),
            Error::TooManyPlayers { players: 64, .. }
        ));
    }

    /// 分からない提携があるかは、提携を列挙せずに分かる (全提携の表を作れない人数でも)。
    #[test]
    fn has_missing_does_not_enumerate_coalitions() {
        let n = 40;
        let grand: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        let data = parse_csv(&format!("coalition,value\n0,1\n{},1\n", grand.join("|"))).unwrap();
        assert_eq!(data.players, n);
        assert!(data.has_missing());
        assert!(matches!(
            data.missing(),
            Err(Error::TooManyPlayers { players: 40, .. })
        ));
        assert!(matches!(
            data.to_explicit(Missing::Zero),
            Err(Error::TooManyPlayers { .. })
        ));
        let complete = parse_json(JSON).unwrap();
        assert!(!complete.has_missing());
        assert!(complete.missing().unwrap().is_empty());
        // フィールドを直接書き換えた重複や範囲外の提携は数えない。
        let mut padded = parse_csv("coalition,value\n0,1\n0|1,2\n").unwrap();
        padded.known.push((Coalition::singleton(0), 1.0));
        padded.known.push((Coalition::singleton(5), 1.0));
        assert!(padded.has_missing());
        let exact = parse_csv_exact("coalition,value\n0,1\n1,1\n0|1,2\n").unwrap();
        assert!(!exact.has_missing());
    }

    /// 値が分からない提携のエラーは、提携を全て並べずに数と最初の 10 個を示す (30 人でもすぐに返る)。
    #[test]
    fn missing_error_counts_without_enumerating_coalitions() {
        let n = MAX_PLAYERS;
        let grand: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        let data = parse_csv(&format!("coalition,value\n1,1\n{},1\n", grand.join("|"))).unwrap();
        let error = data.to_explicit(Missing::Error).unwrap_err().to_string();
        let count = (1u64 << n) - 3;
        assert!(
            error.contains(&format!(
                "値が分からない提携が {count} 個ある: {{0}} {{0, 1}} {{2}}"
            )),
            "{error}"
        );
        assert!(error.ends_with(" ..."), "{error}");
        // 分からない提携が 10 個以下なら、全て示して省略の印を付けない。
        let data = parse_csv("coalition,value\n0,1\n1,1\n0|1|2,6\n").unwrap();
        let error = data.to_explicit(Missing::Error).unwrap_err().to_string();
        assert!(
            error.ends_with("値が分からない提携が 4 個ある: {0, 1} {2} {0, 2} {1, 2}"),
            "{error}"
        );
    }

    #[test]
    fn rejects_non_string_type_and_misspelled_keys() {
        let err = parse_json(r#"{"type": 1, "values": {"0": 1}}"#).unwrap_err();
        assert!(err.to_string().contains("type は文字列"), "{err}");
        for key in ["tpye", "Type", "typ", "player", "Values"] {
            let text = format!(r#"{{"{key}": "cost", "values": {{"0": 1}}}}"#);
            let text = if key.eq_ignore_ascii_case("values") {
                r#"{"Values": {"0": 1}, "values": {"0": 1}}"#.to_string()
            } else {
                text
            };
            let err = parse_json(&text).unwrap_err().to_string();
            assert!(err.contains("書き間違い"), "{key}: {err}");
        }
        // 書き間違いらしくないキーは無視する (説明などを書き添えられる)。
        let data = parse_json(r#"{"name": "例", "description": "x", "values": {"0": 1}}"#).unwrap();
        assert_eq!(data.kind, Kind::Value);
    }

    #[test]
    fn csv_with_bom_and_fractions() {
        let data = parse_csv("\u{feff}coalition,value\n0,1\n1,1\n0|1,8/3\n").unwrap();
        assert_eq!(data.players, 2);
        assert_eq!(data.known[2].1, 8.0 / 3.0);
        assert!(parse_json("\u{feff}{\"values\": {\"0\": 1}}").is_ok());
    }

    #[test]
    fn errors_carry_line_only_when_known() {
        // 全体提携がない: 行に結び付かないので行番号を出さない。
        let err = parse_csv("coalition,value\n0,1\n1,1\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 0, .. }));
        assert_eq!(err.to_string(), "入力が不正: 全体提携の値がない");
        // 重複・読めない値・読めない番号は、その行の番号を出す。
        let err = parse_csv("coalition,value\n0,1\n# c\n0,2\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 4, .. }), "{err}");
        let err = parse_csv("coalition,value\n0,abc\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 2, .. }), "{err}");
        let err = parse_csv("coalition,value\nA,1\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "2 行目: プレイヤーの番号 (0 以上の整数) でない: \"A\""
        );
        let err = parse_csv("coalition,value\n0,inf\n").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 2, .. }), "{err}");
        let err = parse_json(r#"{"values": {"0": 1}"#).unwrap_err();
        assert!(matches!(err, Error::Parse { line: 0, .. }));
    }

    #[test]
    fn exact_parse_keeps_decimal_literals() {
        use num_bigint::BigInt;
        let q = |n: i64, d: i64| Rational::new(BigInt::from(n), BigInt::from(d));
        // 0.1 や 0.30000000000000000001 は浮動小数点数を経由すると値が変わる。
        let text =
            r#"{"type": "cost", "values": {"0": 0.1, "1": 2e-1, "0,1": 0.30000000000000000001}}"#;
        let data = parse_json_exact(text).unwrap();
        assert_eq!(data.kind, Kind::Cost);
        assert_eq!(data.known[0].1, q(1, 10));
        assert_eq!(data.known[1].1, q(1, 5));
        assert_eq!(
            data.known[2].1,
            Rational::new(
                BigInt::from(30000000000000000001i128),
                num_traits::pow(BigInt::from(10), 20)
            )
        );
        let game = parse_exact_by_extension("g.csv", "coalition,value\n0,1/3\n1,0\n0|1,1\n")
            .unwrap()
            .to_exact(Missing::Error)
            .unwrap();
        assert_eq!(*game.value(Coalition::singleton(0)), q(1, 3));
        let partial = parse_csv_exact("coalition,value\n0,1\n0|1,1\n").unwrap();
        assert!(partial.to_exact(Missing::Error).is_err());
        assert_eq!(
            *partial
                .to_exact(Missing::Zero)
                .unwrap()
                .value(Coalition::singleton(1)),
            q(0, 1)
        );
        assert!(parse_json_exact(r#"{"values": {"0": "1"}}"#).is_err());
    }

    #[test]
    fn savings_of_partial_cost_game() {
        let csv = "coalition,cost\n0,5\n1,7\n2,6\n0|1|2,15\n";
        let data = parse_csv(csv).unwrap();
        let savings = data.savings().unwrap();
        assert_eq!(savings.kind, Kind::Value);
        assert_eq!(savings.known.last().unwrap().1, 3.0);
        assert_eq!(data.singleton_values().unwrap(), vec![5.0, 7.0, 6.0]);
        assert!(savings.savings().is_err()); // 価値のゲームは直さない
        let no_single = parse_csv("coalition,cost\n0,5\n0|1,9\n").unwrap();
        assert!(no_single.savings().is_err());
    }
}
