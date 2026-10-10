//! 破産問題: 配分規則 (閉じた形の解) と破産ゲーム ([`BankruptcyGame`])。
//!
//! 遺産 `E` と請求 `d_1..d_n` (`0 <= E <= D = sum d_i`) に対する規則を、請求を並べ替えて `O(n log n)` で計算する。
//!
//! - [`constrained_equal_awards`] (CEA): 全員に同じ額を配り、請求に達した人はそこで止める。
//! - [`constrained_equal_losses`] (CEL): 全員に同じ損失を負わせ、受取が 0 になった人はそこで止める。
//! - [`talmud_rule`]: 請求の半分 `h_i = d_i / 2` について、`E <= D / 2` なら半分に CEA を、
//!   `E > D / 2` なら損失 `D - E` を半分に CEA で割り当てる。
//!
//! 破産ゲーム `v(S) = max(0, E - d(N \ S))` の仁はタルムード則に一致する (Aumann & Maschler 1985)。
//! したがって [`talmud_rule`] は、破産ゲームの仁を LP を使わずに求める閉じた形の解である。
//!
//! 数の型は浮動小数点数 (`f64`) と有理数 ([`crate::game::exact::Rational`]) のどちらでもよい。
//! 有理数で計算すると誤差のない仁が得られる。

mod game;

pub use game::BankruptcyGame;

use num_traits::{FromPrimitive, Num, Signed};

use crate::error::{Error, Result};

/// 規則に使える数の型。
pub trait Amount: Clone + PartialOrd + Num + Signed + FromPrimitive {}

impl<T: Clone + PartialOrd + Num + Signed + FromPrimitive> Amount for T {}

fn validate<T: Amount>(estate: &T, claims: &[T]) -> Result<T> {
    if claims.iter().any(Signed::is_negative) {
        return Err(Error::InvalidArgument("請求は非負".into()));
    }
    let total = claims.iter().cloned().fold(T::zero(), |acc, d| acc + d);
    if estate.is_negative() || *estate > total {
        return Err(Error::InvalidArgument(
            "遺産は 0 以上、請求の総和以下 (破産問題の前提)".into(),
        ));
    }
    Ok(total)
}

/// 上限 `caps` を超えないように `amount` を均等に配る (`0 <= amount <= sum caps` を仮定)。
fn equal_awards<T: Amount>(amount: T, caps: &[T]) -> Vec<T> {
    let n = caps.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| caps[a].partial_cmp(&caps[b]).expect("比較できる値"));
    let mut shares = vec![T::zero(); n];
    let mut remaining = amount;
    for (position, &i) in order.iter().enumerate() {
        let left = T::from_usize(n - position).expect("人数を数として表せる");
        let even = remaining.clone() / left;
        if caps[i] <= even {
            // 均等額が上限を超えるので、上限で止めて残りを他の人で分ける。
            shares[i] = caps[i].clone();
            remaining = remaining - caps[i].clone();
        } else {
            // 以降の人は全員、上限が均等額以上なので均等額を受け取る。
            for &j in &order[position..] {
                shares[j] = even.clone();
            }
            break;
        }
    }
    shares
}

/// 制約付き均等配分 (CEA): `x_i = min(d_i, λ)`、`sum x_i = E`。
pub fn constrained_equal_awards<T: Amount>(estate: T, claims: &[T]) -> Result<Vec<T>> {
    validate(&estate, claims)?;
    Ok(equal_awards(estate, claims))
}

/// 制約付き均等損失 (CEL): `x_i = max(0, d_i - μ)`、`sum x_i = E`。
pub fn constrained_equal_losses<T: Amount>(estate: T, claims: &[T]) -> Result<Vec<T>> {
    let total = validate(&estate, claims)?;
    let losses = equal_awards(total - estate, claims);
    Ok(claims
        .iter()
        .cloned()
        .zip(losses)
        .map(|(d, l)| d - l)
        .collect())
}

/// タルムード則 (Aumann & Maschler 1985)。破産ゲームの仁に一致する。
pub fn talmud_rule<T: Amount>(estate: T, claims: &[T]) -> Result<Vec<T>> {
    let total = validate(&estate, claims)?;
    let two = T::from_u8(2).expect("2 を表せる");
    let halves: Vec<T> = claims.iter().map(|d| d.clone() / two.clone()).collect();
    if estate.clone() * two.clone() <= total {
        Ok(equal_awards(estate, &halves))
    } else {
        let losses = equal_awards(total - estate, &halves);
        Ok(claims
            .iter()
            .cloned()
            .zip(losses)
            .map(|(d, l)| d - l)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::exact::Rational;

    fn q(numerator: i64, denominator: i64) -> Rational {
        Rational::new(numerator.into(), denominator.into())
    }

    #[test]
    fn talmud_cases() {
        let claims = [100.0, 200.0, 300.0];
        assert_eq!(talmud_rule(200.0, &claims).unwrap(), vec![50.0, 75.0, 75.0]);
        assert_eq!(
            talmud_rule(300.0, &claims).unwrap(),
            vec![50.0, 100.0, 150.0]
        );
        let exact: Vec<Rational> = [100, 200, 300].iter().map(|d| q(*d, 1)).collect();
        assert_eq!(talmud_rule(q(100, 1), &exact).unwrap(), vec![q(100, 3); 3]);
    }

    #[test]
    fn equal_awards_and_losses() {
        let claims = [10.0, 30.0, 60.0];
        // CEA(50): 10 は請求で止まり、残り 40 を 2 人で 20 ずつ。
        assert_eq!(
            constrained_equal_awards(50.0, &claims).unwrap(),
            vec![10.0, 20.0, 20.0]
        );
        // CEL(50): 損失 50 を均等に負わせ、10 の人は 0 で止まり、残り 40 を 2 人で 20 ずつ。
        assert_eq!(
            constrained_equal_losses(50.0, &claims).unwrap(),
            vec![0.0, 10.0, 40.0]
        );
    }

    #[test]
    fn boundary_estates() {
        let claims = [4.0, 1.0, 7.0];
        assert_eq!(talmud_rule(0.0, &claims).unwrap(), vec![0.0; 3]);
        assert_eq!(talmud_rule(12.0, &claims).unwrap(), claims.to_vec());
        // E = D / 2 なら請求の半分ずつ。
        assert_eq!(talmud_rule(6.0, &claims).unwrap(), vec![2.0, 0.5, 3.5]);
        assert!(talmud_rule(13.0, &claims).is_err());
        assert!(talmud_rule(-1.0, &claims).is_err());
        assert!(talmud_rule(1.0, &[2.0, -1.0]).is_err());
    }
}
