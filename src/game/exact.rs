//! 特性関数を有理数で持つゲームと、有理数の読み書き。

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame};

pub type Rational = BigRational;

/// 浮動小数点数を、その 2 進数の値どおりの有理数にする。
pub(crate) fn to_rational(value: f64) -> Result<Rational> {
    Rational::from_float(value)
        .ok_or_else(|| Error::InvalidArgument(format!("{value} は有限の数ではない")))
}

/// `8/3`、`-4`、`2.5` の形の文字列を有理数にする (小数は 10 進数の値どおり)。
pub fn parse_rational(text: &str) -> Result<Rational> {
    let text = text.trim();
    let bad = || Error::InvalidArgument(format!("有理数として読めない: {text:?}"));
    if let Some((numerator, denominator)) = text.split_once('/') {
        let numerator: BigInt = numerator.trim().parse().map_err(|_| bad())?;
        let denominator: BigInt = denominator.trim().parse().map_err(|_| bad())?;
        if denominator.is_zero() {
            return Err(bad());
        }
        return Ok(Rational::new(numerator, denominator));
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().map_err(|_| bad())?),
        None => (text, 0),
    };
    let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: BigInt = format!("{integer}{fraction}").parse().map_err(|_| bad())?;
    let shift = exponent - fraction.len() as i32;
    let ten = BigInt::from(10);
    Ok(if shift >= 0 {
        Rational::from_integer(digits * num_traits::pow(ten, shift as usize))
    } else {
        Rational::new(digits, num_traits::pow(ten, (-shift) as usize))
    })
}

/// 1 行 1 値のテキスト (空行と `#` で始まる行は飛ばす) を有理数の列として読む。
/// 小数は 10 進数の値どおり (`0.1` は `1/10`) に読む。
pub fn parse_rational_values(text: &str) -> Result<Vec<Rational>> {
    text.lines()
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
        let players = crate::game::coalition::players_from_len(values.len())?;
        if players > crate::game::MAX_PLAYERS {
            return Err(Error::TooManyPlayers {
                players,
                max: crate::game::MAX_PLAYERS,
            });
        }
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
        let n = crate::game::coalition::players_from_len(values.len())?;
        let mut binary = vec![Rational::zero(); values.len()];
        for (value, coalition) in values
            .into_iter()
            .zip(crate::game::coalition::lexicographic_order(n))
        {
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
}
