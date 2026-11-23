//! 浮動小数点数の和の符号を、値どおりの有理数として厳密に求める。
//!
//! 有限の `f64` は 2 進の有理数をちょうど表すので、`v(S ∪ T) >= v(S) + v(T)` のような判定は、
//! 値を有理数として読めば丸めなしに決まる。浮動小数点数で和を取ると丸めで結論が変わる
//! (`[0, 0, 0, 1e8, 0, 0, 1e8 - 6]` を許容誤差つきで凸と判定していたなど) ので、性質の判定はここで行う。
//!
//! 速さのため、まず浮動小数点数で和を取り、丸め誤差の上限より和の絶対値が大きければその符号を返す
//! (誤差の上限は Higham, *Accuracy and Stability of Numerical Algorithms* 2nd ed., 式 (4.4) の
//! 逐次和の誤差 `|ŝ - s| <= γ_{k-1} Σ|t_i|` に余裕を持たせた `k ε Σ|t_i|`)。
//! 上限の内側に入ったときだけ有理数で和を取る。

use std::cmp::Ordering;

use num_traits::{Signed, Zero};

use crate::game::exact::Rational;

/// これより小さい `Σ|t_i|` では、誤差の上限の計算がアンダーフローしうるので、有理数で和を取る。
const TINY: f64 = 1e-280;

/// 整数の値の絶対値の和がこれ未満なら、浮動小数点数の和は丸めなしで厳密 (`2^53`)。
const INTEGER_LIMIT: f64 = 9_007_199_254_740_992.0;

/// 有限の値 `terms` の和の符号 (値どおりの有理数として厳密)。
///
/// # Panics
///
/// 有限でない値があるとパニックする (呼び出し側で有限であることを確かめてから使う)。
pub(crate) fn sum_sign(terms: &[f64]) -> Ordering {
    let mut sum = 0.0f64;
    let mut magnitude = 0.0f64;
    for &term in terms {
        sum += term;
        magnitude += term.abs();
    }
    if magnitude == 0.0 {
        return Ordering::Equal;
    }
    if let Some(sign) = filtered_sign(sum, magnitude, terms.len()) {
        return sign;
    }
    // 全ての値が整数で絶対値の和が 2^53 未満なら、途中の和も 2^53 未満の整数なので、浮動小数点数の和は丸めなしで厳密。
    if magnitude < INTEGER_LIMIT && terms.iter().all(|t| t.fract() == 0.0) {
        return sum.partial_cmp(&0.0).expect("有限の和");
    }
    // 符号が浮動小数点数の和で決まらない (等号に近い) とき、値の桁の幅が i128 に収まれば、共通の 2 のべきの
    // 整数倍として整数で和を取る (整数ゲームや格子の上の値の等号はここで決まる)。収まらなければ有理数で和を取る。
    let headroom = usize::BITS - terms.len().leading_zeros() + 1;
    if let Some(scale) = DyadicScale::new(terms.iter().copied(), headroom) {
        let total: i128 = terms.iter().map(|&t| scale.integer(t)).sum();
        return total.cmp(&0);
    }
    sign_of(&exact_sum(terms))
}

/// 浮動小数点数で計算した和 `sum` (`terms` 個の値の逐次和、絶対値の和は `magnitude` 以下) の符号が
/// 丸め誤差によらず確かなら、その符号を返す。
pub(crate) fn filtered_sign(sum: f64, magnitude: f64, terms: usize) -> Option<Ordering> {
    if !(sum.is_finite() && magnitude.is_finite()) || magnitude < TINY {
        return None;
    }
    let bound = (terms.max(1) as f64) * f64::EPSILON * magnitude;
    if sum > bound {
        Some(Ordering::Greater)
    } else if sum < -bound {
        Some(Ordering::Less)
    } else {
        None
    }
}

/// 有限の値 `terms` の和を有理数で厳密に求める。
///
/// # Panics
///
/// 有限でない値があるとパニックする。
pub(crate) fn exact_sum(terms: &[f64]) -> Rational {
    terms.iter().fold(Rational::zero(), |sum, &term| {
        sum + Rational::from_float(term).expect("和を取る値は有限")
    })
}

pub(crate) fn sign_of(value: &Rational) -> Ordering {
    if value.is_zero() {
        Ordering::Equal
    } else if value.is_positive() {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

/// 有限の値を、共通の `2^exponent` の整数倍 (`i128`) として丸めなしに表す尺度。
///
/// 全ての値の最下位のビットから最上位のビットまでの幅に `headroom` ビットを足して 127 ビットに収まるときだけ作れる
/// (`headroom` は、整数にした値どうしの和や差で増える桁の分)。収まれば `i128` の和や比較は丸めなしの有理数の計算と同じになる。
#[derive(Clone, Copy, Debug)]
pub(crate) struct DyadicScale {
    exponent: i32,
}

/// 0 でない有限の `value` を `(符号付きの奇数の仮数, 指数)` に分ける (`value = mantissa * 2^exponent`)。
fn decompose(value: f64) -> (i64, i32) {
    let bits = value.to_bits();
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mut mantissa, mut exponent) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), biased - 1075)
    };
    let zeros = mantissa.trailing_zeros();
    mantissa >>= zeros;
    exponent += zeros as i32;
    let signed = if value < 0.0 {
        -(mantissa as i64)
    } else {
        mantissa as i64
    };
    (signed, exponent)
}

impl DyadicScale {
    pub(crate) fn new(values: impl IntoIterator<Item = f64>, headroom: u32) -> Option<DyadicScale> {
        let mut low = i32::MAX;
        let mut high = i32::MIN;
        for value in values {
            if !value.is_finite() {
                return None;
            }
            if value == 0.0 {
                continue;
            }
            let (mantissa, exponent) = decompose(value);
            let width = 64 - mantissa.unsigned_abs().leading_zeros() as i32;
            low = low.min(exponent);
            high = high.max(exponent + width);
        }
        if low == i32::MAX {
            return Some(DyadicScale { exponent: 0 });
        }
        (high - low + headroom as i32 <= 126).then_some(DyadicScale { exponent: low })
    }

    /// `value` を `2^exponent` の整数倍にした整数。`value` は [`DyadicScale::new`] に渡した値のどれかであること。
    pub(crate) fn integer(&self, value: f64) -> i128 {
        if value == 0.0 {
            return 0;
        }
        let (mantissa, exponent) = decompose(value);
        i128::from(mantissa) << (exponent - self.exponent)
    }

    /// 整数 `value` (`2^exponent` の倍数として表した値) の有理数。
    pub(crate) fn rational(&self, value: i128) -> Rational {
        let integer = Rational::from_integer(value.into());
        let power =
            Rational::from_integer(num_bigint::BigInt::from(1) << self.exponent.unsigned_abs());
        if self.exponent >= 0 {
            integer * power
        } else {
            integer / power
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decides_sums_that_floating_point_rounds_away() {
        // 0.1 + 0.2 は 2 進の値どおりでは 0.3 (= 0.2999...) より大きい。
        assert_eq!(sum_sign(&[0.1, 0.2, -0.3]), Ordering::Greater);
        // 浮動小数点数の和では 1e16 + 1 - 1e16 = 0 だが、値どおりでは 1。
        assert_eq!(sum_sign(&[1e16, 1.0, -1e16]), Ordering::Greater);
        assert_eq!(sum_sign(&[1e16, -1.0, -1e16]), Ordering::Less);
        assert_eq!(sum_sign(&[1e8, -1e8, 0.0]), Ordering::Equal);
        assert_eq!(sum_sign(&[]), Ordering::Equal);
        // 和が f64 の上限を超えても符号は決まる。
        assert_eq!(
            sum_sign(&[f64::MAX, f64::MAX, -f64::MAX, -f64::MAX]),
            Ordering::Equal
        );
        assert_eq!(
            sum_sign(&[f64::MAX, f64::MAX, -f64::MAX]),
            Ordering::Greater
        );
        // 非正規化数。
        let tiny = f64::from_bits(1);
        assert_eq!(sum_sign(&[tiny, -tiny, tiny]), Ordering::Greater);
    }

    #[test]
    fn dyadic_scale_represents_values_exactly() {
        let values = [0.1, -3.0, 1e8, 0.0, 2f64.powi(-60)];
        let scale = DyadicScale::new(values, 8).unwrap();
        for value in values {
            assert_eq!(
                scale.rational(scale.integer(value)),
                Rational::from_float(value).unwrap()
            );
        }
        // 0.1 + 0.2 - 0.3 を整数で計算すると有理数の和と一致する。
        let scale = DyadicScale::new([0.1, 0.2, 0.3], 4).unwrap();
        let sum = scale.integer(0.1) + scale.integer(0.2) - scale.integer(0.3);
        assert_eq!(scale.rational(sum), exact_sum(&[0.1, 0.2, -0.3]));
        // 桁の幅が 127 ビットを超える組は作らない。
        assert!(DyadicScale::new([1e30, 1e-30], 0).is_none());
        assert!(DyadicScale::new([f64::NAN], 0).is_none());
        let tiny = f64::from_bits(1);
        let scale = DyadicScale::new([tiny, 3.0 * tiny], 4).unwrap();
        assert_eq!(scale.integer(3.0 * tiny), 3);
    }

    #[test]
    fn filter_agrees_with_exact_sums() {
        let mut rng = crate::rng::SplitMix64::new(3);
        for _ in 0..20_000 {
            let k = 2 + (rng.next_u64() % 6) as usize;
            let terms: Vec<f64> = (0..k)
                .map(|_| {
                    let scale = [1.0, 1e-8, 1e8, 3.0][(rng.next_u64() % 4) as usize];
                    (rng.next_f64() - 0.5) * scale
                })
                .collect();
            // 和がほぼ 0 になる組も作る。
            let mut terms = terms;
            if rng.next_u64().is_multiple_of(2) {
                let total: f64 = terms.iter().sum();
                terms.push(-total);
            }
            assert_eq!(sum_sign(&terms), sign_of(&exact_sum(&terms)), "{terms:?}");
        }
    }

    #[test]
    fn equalities_are_decided_on_integers_or_rationals() {
        // 等号 (和がちょうど 0) は浮動小数点数の和の誤差の上限で決まらないので、桁の幅が i128 に収まれば整数で、
        // 収まらなければ有理数で決める。どちらも有理数の和と一致する。
        let cases: [&[f64]; 6] = [
            &[3.0, 5.0, -8.0],
            &[1e9 + 7.0, 2e9 + 1.0, -(3e9 + 8.0), 0.0],
            &[0.1, 0.2, -(0.1 + 0.2)],
            &[2f64.powi(-60), 1.0, -1.0, -(2f64.powi(-60))],
            // 桁の幅が 127 ビットを超える (有理数の和に落ちる)。
            &[1e30, 1e-30, -1e30, -1e-30],
            &[1e30, 1e-30, -1e30],
        ];
        for terms in cases {
            assert_eq!(sum_sign(terms), sign_of(&exact_sum(terms)), "{terms:?}");
        }
        assert_eq!(sum_sign(&[1e30, 1e-30, -1e30, -1e-30]), Ordering::Equal);
        assert_eq!(sum_sign(&[1e30, 1e-30, -1e30]), Ordering::Greater);
    }
}
