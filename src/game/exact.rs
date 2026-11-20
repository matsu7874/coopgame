//! 特性関数を有理数で持つゲームと、有理数の読み書き。

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

use super::check_players;
use super::coalition::{lexicographic_order, players_from_len};
use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame};

pub type Rational = BigRational;

/// 浮動小数点数を、その 2 進数の値どおりの有理数にする。
pub(crate) fn to_rational(value: f64) -> Result<Rational> {
    Rational::from_float(value)
        .ok_or_else(|| Error::InvalidArgument(format!("{value} は有限の数ではない")))
}

/// [`parse_rational`] が受け付ける指数表記の指数 (`1.5e3` の `3`) の絶対値の上限。
///
/// `1e2000000000` のような入力で `10^k` の計算が終わらなくなるのを防ぐ。
/// `f64` で表せる値の指数 (およそ `-324..=308`) を十分に含む。
pub const MAX_DECIMAL_EXPONENT: i32 = 4096;

/// `8/3`、`-4`、`2.5`、`1.5e3` の形の文字列を有理数にする (小数は 10 進数の値どおり)。
///
/// 前後の空白は無視する。受け付ける形は次の 2 つで、桁は ASCII の数字 `0-9` だけ (`_` などの区切りは不可)。
///
/// - 分数 `p/q`: `p` と `q` は、符号 (`+`・`-`) を先頭に 1 つまで付けた 1 桁以上の整数。
///   `/` の前後の空白は無視する。`q` が 0 ならエラー。
/// - 10 進数: 符号を先頭に 1 つまで付け、整数部と小数部の少なくとも一方に数字がある (`12`、`-1.5`、`5.`、`.5`)。
///   指数 `e`・`E` に、符号を 1 つまで付けた 1 桁以上の整数を続けてよい (`1.5e3`、`2E-2`)。
///
/// 10 進数の書き方は `str::parse::<f64>` の 10 進数と同じである (ただし `inf`・`nan` は受け付けない)。
/// 指数の絶対値が [`MAX_DECIMAL_EXPONENT`] を超えるとエラー。
pub fn parse_rational(text: &str) -> Result<Rational> {
    let text = text.trim();
    let bad = || Error::InvalidArgument(format!("有理数として読めない: {text:?}"));
    if let Some((numerator, denominator)) = text.split_once('/') {
        let numerator = parse_integer(numerator.trim()).ok_or_else(bad)?;
        let denominator = parse_integer(denominator.trim()).ok_or_else(bad)?;
        if denominator.is_zero() {
            return Err(bad());
        }
        return Ok(Rational::new(numerator, denominator));
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (text, None),
    };
    let (negative, unsigned) = split_sign(mantissa);
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if (integer.is_empty() && fraction.is_empty())
        || !is_ascii_digits(integer)
        || !is_ascii_digits(fraction)
    {
        return Err(bad());
    }
    let exponent = match exponent {
        None => 0,
        Some(exponent) => {
            let (negative, digits) = split_sign(exponent);
            if digits.is_empty() || !is_ascii_digits(digits) {
                return Err(bad());
            }
            // 桁数が多くて u32 に収まらない指数も、上限を超える指数として扱う。
            let magnitude = digits
                .parse::<u32>()
                .ok()
                .filter(|m| *m <= MAX_DECIMAL_EXPONENT.unsigned_abs())
                .ok_or_else(|| {
                    Error::InvalidArgument(format!(
                        "有理数として読めない: {text:?} (指数の絶対値が上限 {MAX_DECIMAL_EXPONENT} を超える)"
                    ))
                })?;
            if negative {
                -i64::from(magnitude)
            } else {
                i64::from(magnitude)
            }
        }
    };
    let digits =
        BigInt::parse_bytes(format!("{integer}{fraction}").as_bytes(), 10).ok_or_else(bad)?;
    let digits = if negative { -digits } else { digits };
    // 指数が上限内なので、桁の移動の大きさは入力の長さ程度に収まる。
    let shift = exponent - i64::try_from(fraction.len()).map_err(|_| bad())?;
    let magnitude = usize::try_from(shift.unsigned_abs()).map_err(|_| bad())?;
    let power = num_traits::pow(BigInt::from(10), magnitude);
    Ok(if shift >= 0 {
        Rational::from_integer(digits * power)
    } else {
        Rational::new(digits, power)
    })
}

/// 先頭の符号 (`+`・`-` のどちらか 1 つ) を外す。`-` なら `true` を返す。
fn split_sign(text: &str) -> (bool, &str) {
    match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    }
}

/// ASCII の数字 `0-9` だけからなるか (空文字列は真)。
fn is_ascii_digits(text: &str) -> bool {
    text.bytes().all(|b| b.is_ascii_digit())
}

/// 符号を先頭に 1 つまで付けた、1 桁以上の ASCII の数字の列を整数にする。
fn parse_integer(text: &str) -> Option<BigInt> {
    let (negative, digits) = split_sign(text);
    if digits.is_empty() || !is_ascii_digits(digits) {
        return None;
    }
    let magnitude = BigInt::parse_bytes(digits.as_bytes(), 10)?;
    Some(if negative { -magnitude } else { magnitude })
}

/// 1 行 1 値のテキスト (空行と `#` で始まる行は飛ばす) を有理数の列として読む。
/// 各行は [`parse_rational`] の形で書き、小数は 10 進数の値どおり (`0.1` は `1/10`) に読む。
/// 先頭の UTF-8 の BOM は無視する。
pub fn parse_rational_values(text: &str) -> Result<Vec<Rational>> {
    super::strip_bom(text)
        .lines()
        .enumerate()
        .map(|(index, line)| (index, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(index, line)| {
            parse_rational(line).map_err(|err| Error::Parse {
                line: index + 1,
                message: err.to_string(),
            })
        })
        .collect()
}

/// 有理数を `8/3` や `-4` の形で書く。
pub fn format_rational(value: &Rational) -> String {
    if value.is_integer() {
        value.numer().to_string()
    } else {
        format!("{}/{}", value.numer(), value.denom())
    }
}

/// 特性関数を有理数で持つゲーム。
#[derive(Clone, Debug)]
pub struct ExactGame {
    players: usize,
    values: Vec<Rational>,
}

impl ExactGame {
    /// 明示ゲームの値を 2 進数の値どおりに有理数にする (整数や 2 進で表せる値は誤差なし)。
    pub fn from_explicit(game: &ExplicitGame) -> Result<ExactGame> {
        let values = game
            .values()
            .iter()
            .map(|v| to_rational(*v))
            .collect::<Result<_>>()?;
        Ok(ExactGame {
            players: game.players(),
            values,
        })
    }

    /// ビット順に並んだ長さ `2^n - 1` の有理数の値 (空提携を除く) から作る。
    ///
    /// 値を浮動小数点数の和などで作ると、有理数では等しいはずの値がずれて厳密な検証が成り立たないことがある。
    /// その場合は値を有理数のまま構築し、[`crate::verify::certify_exact`] で検証する。
    pub fn from_binary(values: Vec<Rational>) -> Result<ExactGame> {
        let players = players_from_len(values.len())?;
        check_players(players)?;
        let mut all = Vec::with_capacity(values.len() + 1);
        all.push(Rational::zero());
        all.extend(values);
        Ok(ExactGame {
            players,
            values: all,
        })
    }

    /// 辞書式順 (CoopGame・TUGLab と同じ) に並んだ有理数の値から作る。
    pub fn from_lex(values: Vec<Rational>) -> Result<ExactGame> {
        let n = players_from_len(values.len())?;
        let mut binary = vec![Rational::zero(); values.len()];
        for (value, coalition) in values.into_iter().zip(lexicographic_order(n)) {
            binary[coalition.index() - 1] = value;
        }
        ExactGame::from_binary(binary)
    }

    pub fn players(&self) -> usize {
        self.players
    }

    pub fn value(&self, coalition: Coalition) -> &Rational {
        &self.values[coalition.index()]
    }

    /// 長さ `2^n` (空提携を含む) の値。
    pub fn values(&self) -> &[Rational] {
        &self.values
    }

    /// 各値を最も近い浮動小数点数にした明示ゲーム (LP などの浮動小数点の手法に渡す)。
    pub fn to_explicit(&self) -> Result<ExplicitGame> {
        ExplicitGame::from_binary(&to_f64(&self.values[1..]))
    }
}

/// 有理数の配分を浮動小数点数にする (表示や比較用)。
pub(crate) fn to_f64(values: &[Rational]) -> Vec<f64> {
    use num_traits::ToPrimitive;
    values
        .iter()
        .map(|v| v.to_f64().unwrap_or(f64::NAN))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(numerator: i64, denominator: i64) -> Rational {
        Rational::new(numerator.into(), denominator.into())
    }

    #[test]
    fn parses_and_formats_rationals() {
        assert_eq!(parse_rational("8/3").unwrap(), r(8, 3));
        assert_eq!(parse_rational("-4").unwrap(), r(-4, 1));
        assert_eq!(parse_rational("2.5").unwrap(), r(5, 2));
        assert_eq!(parse_rational("1.25e1").unwrap(), r(25, 2));
        assert_eq!(format_rational(&r(16, 6)), "8/3");
        assert_eq!(format_rational(&r(-4, 1)), "-4");
    }

    #[test]
    fn accepts_signs_points_and_spaces_where_allowed() {
        let cases = [
            ("5.", r(5, 1)),
            (".5", r(1, 2)),
            ("-.5", r(-1, 2)),
            ("+.5", r(1, 2)),
            ("1.e2", r(100, 1)),
            ("1e+2", r(100, 1)),
            ("2E-2", r(1, 50)),
            ("1.5e-1", r(3, 20)),
            ("1e-0001", r(1, 10)),
            ("007", r(7, 1)),
            ("-0", r(0, 1)),
            (" 2.5\t", r(5, 2)),
            (" 1/2 ", r(1, 2)),
            ("1 / 2", r(1, 2)),
            ("+1/+2", r(1, 2)),
            ("1/-2", r(-1, 2)),
            ("-3/-6", r(1, 2)),
        ];
        for (text, expected) in cases {
            assert_eq!(parse_rational(text).unwrap(), expected, "{text:?}");
        }
    }

    #[test]
    fn rejects_misplaced_signs_and_non_digits() {
        for text in [
            ".+5", ".-5", "1.+5", "-1.-5", "+-1", "-+1", "--1", "++1", "1-", "1e+-2", "1e--2",
            "1_0", "1.5_0", "1_", "_1", "1e1_0", "1/2_0", "1_/2", "1/_2", "+/2", "1/-", "1/2/3",
            "1/0", "1.5/2", "1e2/3", "1 .5", "1. 5", "1 e2", "- 1", "1/- 2", "", " ", "+", "-",
            ".", "-.", "e1", ".e1", "1e", "1e+", "1e2e3", "1.2.3", "inf", "-inf", "nan",
            "infinity", "0x10", "1f", "٣", "１",
        ] {
            assert!(parse_rational(text).is_err(), "{text:?} は読めないはず");
        }
    }

    #[test]
    fn exponent_limit_applies_to_any_digit_count() {
        assert!(parse_rational("1e4096").is_ok());
        assert!(parse_rational("1e-4096").is_ok());
        assert!(parse_rational("1e00004096").is_ok());
        for text in [
            "1e4097",
            "1e-4097",
            "1e99999999999",
            "1e-99999999999999999999",
        ] {
            let err = parse_rational(text).unwrap_err().to_string();
            assert!(err.contains("上限"), "{text:?}: {err}");
        }
    }

    /// 10 進数の字面を部品から組み立て、値も同じ部品から (`parse_rational` を使わずに) 求める。
    fn random_decimal(rng: &mut crate::rng::SplitMix64) -> (String, Rational) {
        /// `min..=max` 桁のランダムな数字の列。
        fn digits(rng: &mut crate::rng::SplitMix64, min: u64, max: u64) -> String {
            let len = rng.range(min, max);
            (0..len)
                .map(|_| char::from(b'0' + rng.range(0, 9) as u8))
                .collect()
        }
        let negative = rng.range(0, 2);
        let integer = digits(rng, 0, 8);
        let point = integer.is_empty() || rng.range(0, 1) == 1;
        let fraction = if point {
            digits(rng, u64::from(integer.is_empty()), 8)
        } else {
            String::new()
        };
        let exponent = if rng.range(0, 1) == 1 {
            Some(rng.range(0, 60) as i64 - 30)
        } else {
            None
        };

        let mut text = " ".repeat(rng.range(0, 1) as usize);
        text.push_str(["", "+", "-"][negative as usize]);
        text.push_str(&integer);
        if point {
            text.push('.');
            text.push_str(&fraction);
        }
        if let Some(e) = exponent {
            text.push(['e', 'E'][rng.range(0, 1) as usize]);
            if e < 0 {
                text.push('-');
            } else if rng.range(0, 1) == 1 {
                text.push('+');
            }
            let width = rng.range(1, 3) as usize;
            text.push_str(&format!("{:0width$}", e.unsigned_abs()));
        }
        text.push_str(&" ".repeat(rng.range(0, 1) as usize));

        let mantissa = integer
            .bytes()
            .chain(fraction.bytes())
            .fold(BigInt::zero(), |acc, d| acc * 10 + BigInt::from(d - b'0'));
        let shift = exponent.unwrap_or(0) - fraction.len() as i64;
        let ten = Rational::from_integer(BigInt::from(10));
        let mut value = Rational::from_integer(mantissa);
        for _ in 0..shift.unsigned_abs() {
            if shift > 0 {
                value *= ten.clone();
            } else {
                value /= ten.clone();
            }
        }
        if negative == 2 {
            value = -value;
        }
        (text, value)
    }

    /// 字面を 1 文字だけ壊す (置き換え・挿入・削除)。`/` は分数の形になるので使わない。
    fn corrupt(rng: &mut crate::rng::SplitMix64, text: &str) -> String {
        const ALPHABET: &[u8] = b"0123456789+-.eE_ x";
        let mut bytes = text.as_bytes().to_vec();
        let at = rng.range(0, bytes.len() as u64) as usize;
        let c = ALPHABET[rng.range(0, ALPHABET.len() as u64 - 1) as usize];
        match rng.range(0, 2) {
            0 if at < bytes.len() => bytes[at] = c,
            1 => bytes.insert(at, c),
            _ if at < bytes.len() => {
                bytes.remove(at);
            }
            _ => bytes.push(c),
        }
        String::from_utf8(bytes).unwrap()
    }

    /// 指数の絶対値が [`MAX_DECIMAL_EXPONENT`] を超えるか (`f64` として読める字面について)。
    fn exponent_exceeds_limit(text: &str) -> bool {
        text.trim()
            .split_once(['e', 'E'])
            .is_some_and(|(_, exponent)| {
                exponent
                    .trim_start_matches(['+', '-'])
                    .parse::<u32>()
                    .map_or(true, |m| m > MAX_DECIMAL_EXPONENT.unsigned_abs())
            })
    }

    /// 10 進数の字面は、`f64` の読み取り (`parse_values` の経路) と同じものを受け付け、同じ値を表す。
    #[test]
    fn decimal_syntax_matches_f64_parsing() {
        use num_traits::ToPrimitive;
        let mut rng = crate::rng::SplitMix64::new(20261121);
        let mut corrupted_accepted = 0;
        let mut corrupted_rejected = 0;
        for _ in 0..20_000 {
            let (text, value) = random_decimal(&mut rng);
            assert_eq!(parse_rational(&text).unwrap(), value, "{text:?}");
            let float: f64 = text.trim().parse().unwrap();
            assert_eq!(value.to_f64().unwrap(), float, "{text:?}");

            let broken = corrupt(&mut rng, &text);
            let parsed = parse_rational(&broken);
            match broken.trim().parse::<f64>() {
                Ok(_) if exponent_exceeds_limit(&broken) => {
                    assert!(parsed.is_err(), "{broken:?} は指数の上限を超える");
                }
                Ok(float) => {
                    let parsed = parsed.unwrap_or_else(|e| panic!("{broken:?}: {e}"));
                    assert_eq!(parsed.to_f64().unwrap(), float, "{broken:?}");
                    corrupted_accepted += 1;
                }
                Err(_) => {
                    assert!(parsed.is_err(), "{broken:?} は f64 では読めない");
                    corrupted_rejected += 1;
                }
            }
        }
        // 壊した字面に、読めるものと読めないものの両方が十分に含まれる。
        assert!(corrupted_accepted > 1000 && corrupted_rejected > 1000);
    }

    #[test]
    fn rational_lines_reject_what_parse_rational_rejects() {
        let err = parse_rational_values("1\n.+5\n2").unwrap_err();
        assert!(matches!(err, Error::Parse { line: 2, .. }), "{err:?}");
        assert!(crate::game::parse_values("1/2_0").is_err());
    }
}
