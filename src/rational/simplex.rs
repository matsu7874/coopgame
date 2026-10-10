//! 有理数の単体法: 実行可能性の判定 ([`is_feasible`]) と、2 段階法による最小化 ([`minimize`])。
//!
//! `A z = b`, `z >= 0` に実行可能解があるかを、人為変数の和を最小化して判定する。
//! 入る変数・出る変数は Bland の規則 (添字の最小のもの) で選ぶので、退化しても循環せずに停止する。
//! 全ての演算は多倍長の有理数で行い、丸め誤差はない。

use num_rational::BigRational;
use num_traits::{Signed, Zero};

/// `rows[i] . z = rhs[i]` (i = 0..m), `z >= 0` に解があるか。
pub fn is_feasible(rows: &[Vec<BigRational>], rhs: &[BigRational]) -> bool {
    rows.is_empty() || phase_one(rows, rhs, rows[0].len()).is_some()
}

/// 単体表。列 `variables..` は人為変数。
struct Tableau {
    lines: Vec<Vec<BigRational>>,
    values: Vec<BigRational>,
    basis: Vec<usize>,
}

/// 第 1 段階: 人為変数の和を最小化する。実行可能なら、元の変数の基底解の単体表を返す。
fn phase_one(rows: &[Vec<BigRational>], rhs: &[BigRational], variables: usize) -> Option<Tableau> {
    let m = rows.len();
    // 右辺を非負にそろえ、人為変数 (列 variables..variables + m) を加えた表を作る。
    let mut lines: Vec<Vec<BigRational>> = Vec::with_capacity(m);
    let mut values: Vec<BigRational> = Vec::with_capacity(m);
    for (i, (row, b)) in rows.iter().zip(rhs).enumerate() {
        let flip = b.is_negative();
        let mut line: Vec<BigRational> = row
            .iter()
            .map(|a| if flip { -a.clone() } else { a.clone() })
            .collect();
        line.extend((0..m).map(|k| if k == i { one() } else { BigRational::zero() }));
        lines.push(line);
        values.push(if flip { -b.clone() } else { b.clone() });
    }
    let columns = variables + m;
    let mut basis: Vec<usize> = (variables..columns).collect();
    // 被約費用 d_j = c_j - c_B B^-1 A_j。人為変数の費用は 1、元の変数は 0。
    let mut reduced: Vec<BigRational> = (0..columns)
        .map(|j| {
            if j >= variables {
                BigRational::zero()
            } else {
                -lines
                    .iter()
                    .map(|line| line[j].clone())
                    .fold(BigRational::zero(), |acc, a| acc + a)
            }
        })
        .collect();
    let mut infeasibility: BigRational = values.iter().fold(BigRational::zero(), |acc, b| acc + b);
    // Bland の規則: 被約費用が負の最小の添字の列を入れる。
    while let Some(entering) = (0..columns).find(|&j| reduced[j].is_negative()) {
        let row = ratio_test(&lines, &values, &basis, entering).expect("第 1 段階の LP は有界");
        pivot(&mut lines, &mut values, row, entering);
        let factor = reduced[entering].clone();
        for (d, a) in reduced.iter_mut().zip(&lines[row]) {
            *d -= &factor * a;
        }
        // 入る変数を θ = values[row] だけ増やすと、目的関数は d_e θ だけ変わる (d_e < 0)。
        infeasibility += &factor * &values[row];
        basis[row] = entering;
    }
    if !infeasibility.is_zero() {
        return None;
    }
    // 値 0 で基底に残った人為変数を、元の変数と入れ替える (入れ替えられない行は冗長なので残す)。
    for row in 0..m {
        if basis[row] >= variables
            && let Some(column) = (0..variables).find(|&j| !lines[row][j].is_zero())
        {
            pivot(&mut lines, &mut values, row, column);
            basis[row] = column;
        }
    }
    Some(Tableau {
        lines,
        values,
        basis,
    })
}

/// 有理数の LP の結果。
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Optimal {
        value: BigRational,
        solution: Vec<BigRational>,
    },
    Infeasible,
    Unbounded,
}

/// `cost . z` を `rows[i] . z = rhs[i]`, `z >= 0` のもとで最小化する (2 段階の単体法、Bland の規則)。
pub fn minimize(rows: &[Vec<BigRational>], rhs: &[BigRational], cost: &[BigRational]) -> Outcome {
    let variables = cost.len();
    let m = rows.len();
    let Some(Tableau {
        lines: mut tableau,
        mut values,
        mut basis,
    }) = phase_one(rows, rhs, variables)
    else {
        return Outcome::Infeasible;
    };
    let columns = variables + m;

    // 第 2 段階: 元の目的関数。人為変数は入れない。
    let basic_cost = |i: usize| -> BigRational {
        if basis[i] < variables {
            cost[basis[i]].clone()
        } else {
            BigRational::zero()
        }
    };
    let mut reduced: Vec<BigRational> = (0..columns)
        .map(|j| {
            let c = if j < variables {
                cost[j].clone()
            } else {
                BigRational::zero()
            };
            (0..m).fold(c, |acc, i| acc - basic_cost(i) * &tableau[i][j])
        })
        .collect();
    while let Some(entering) = (0..variables).find(|&j| reduced[j].is_negative()) {
        let Some(row) = ratio_test(&tableau, &values, &basis, entering) else {
            return Outcome::Unbounded;
        };
        pivot(&mut tableau, &mut values, row, entering);
        let factor = reduced[entering].clone();
        for (d, a) in reduced.iter_mut().zip(&tableau[row]) {
            *d -= &factor * a;
        }
        basis[row] = entering;
    }
    let mut solution = vec![BigRational::zero(); variables];
    for (i, &b) in basis.iter().enumerate() {
        if b < variables {
            solution[b] = values[i].clone();
        }
    }
    let value = solution
        .iter()
        .zip(cost)
        .fold(BigRational::zero(), |acc, (z, c)| acc + z * c);
    Outcome::Optimal { value, solution }
}

/// 比の最小の行 (同じ比なら基底変数の添字が最小の行)。入る列に正の成分がなければ `None`。
fn ratio_test(
    tableau: &[Vec<BigRational>],
    values: &[BigRational],
    basis: &[usize],
    entering: usize,
) -> Option<usize> {
    let mut leaving: Option<(usize, BigRational)> = None;
    for (i, line) in tableau.iter().enumerate() {
        if line[entering].is_positive() {
            let ratio = &values[i] / &line[entering];
            let better = match &leaving {
                None => true,
                Some((r, best)) => ratio < *best || (ratio == *best && basis[i] < basis[*r]),
            };
            if better {
                leaving = Some((i, ratio));
            }
        }
    }
    leaving.map(|(row, _)| row)
}

fn one() -> BigRational {
    BigRational::from_integer(1.into())
}

fn pivot(tableau: &mut [Vec<BigRational>], values: &mut [BigRational], row: usize, column: usize) {
    let scale = tableau[row][column].clone();
    for a in tableau[row].iter_mut() {
        *a /= &scale;
    }
    values[row] /= &scale;
    let pivot_row = tableau[row].clone();
    let pivot_value = values[row].clone();
    for (i, line) in tableau.iter_mut().enumerate() {
        if i == row || line[column].is_zero() {
            continue;
        }
        let factor = line[column].clone();
        for (a, p) in line.iter_mut().zip(&pivot_row) {
            *a -= &factor * p;
        }
        values[i] -= &factor * &pivot_value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;
    use microlp::{ComparisonOp, OptimizationDirection, Problem};

    fn r(n: i64) -> BigRational {
        BigRational::from_integer(n.into())
    }

    #[test]
    fn minimizes_small_programs() {
        // min -z1 - z2  s.t. z1 + 2 z2 + s1 = 4, 3 z1 + z2 + s2 = 6 -> z = (8/5, 6/5)、値 -14/5
        let rows = vec![vec![r(1), r(2), r(1), r(0)], vec![r(3), r(1), r(0), r(1)]];
        match minimize(&rows, &[r(4), r(6)], &[r(-1), r(-1), r(0), r(0)]) {
            Outcome::Optimal { value, solution } => {
                let q = |a: i64, b: i64| BigRational::new(a.into(), b.into());
                assert_eq!(value, q(-14, 5));
                assert_eq!(solution[..2], [q(8, 5), q(6, 5)]);
            }
            other => panic!("{other:?}"),
        }
        // min -z1  s.t. z1 - z2 = 1 -> 非有界
        assert_eq!(
            minimize(&[vec![r(1), r(-1)]], &[r(1)], &[r(-1), r(0)]),
            Outcome::Unbounded
        );
        // z1 + z2 = -1 -> 実行不可能
        assert_eq!(
            minimize(&[vec![r(1), r(1)]], &[r(-1)], &[r(1), r(1)]),
            Outcome::Infeasible
        );
        // 冗長な等式 (2 行目は 1 行目の 2 倍) があっても解ける。min z1 s.t. z1 + z2 = 2, 2 z1 + 2 z2 = 4
        match minimize(
            &[vec![r(1), r(1)], vec![r(2), r(2)]],
            &[r(2), r(4)],
            &[r(1), r(0)],
        ) {
            Outcome::Optimal { value, .. } => assert_eq!(value, r(0)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn feasible_and_infeasible_systems() {
        // z1 + z2 = 1, z1 - z2 = 0 -> z = (1/2, 1/2)
        assert!(is_feasible(
            &[vec![r(1), r(1)], vec![r(1), r(-1)]],
            &[r(1), r(0)]
        ));
        // z1 + z2 = -1 は非負解なし
        assert!(!is_feasible(&[vec![r(1), r(1)]], &[r(-1)]));
        // z1 - z2 = 3, z1 + z2 = 1 -> z2 = -1 で非負解なし
        assert!(!is_feasible(
            &[vec![r(1), r(-1)], vec![r(1), r(1)]],
            &[r(3), r(1)]
        ));
    }

    /// 実行可能性の判定が、浮動小数点の LP (microlp) と一致する。
    #[test]
    fn agrees_with_floating_lp() {
        let mut rng = SplitMix64::new(42);
        let mut feasible = 0;
        for case in 0..300 {
            let rows = 2 + (rng.next_u64() % 4) as usize;
            let columns = 3 + (rng.next_u64() % 6) as usize;
            let a: Vec<Vec<i64>> = (0..rows)
                .map(|_| (0..columns).map(|_| rng.range(0, 6) as i64 - 3).collect())
                .collect();
            let b: Vec<i64> = (0..rows).map(|_| rng.range(0, 10) as i64 - 5).collect();
            let exact_answer = is_feasible(
                &a.iter()
                    .map(|row| row.iter().map(|v| r(*v)).collect())
                    .collect::<Vec<_>>(),
                &b.iter().map(|v| r(*v)).collect::<Vec<_>>(),
            );
            let mut problem = Problem::new(OptimizationDirection::Minimize);
            let z: Vec<_> = (0..columns)
                .map(|_| problem.add_var(0.0, (0.0, f64::INFINITY)))
                .collect();
            for (row, rhs) in a.iter().zip(&b) {
                let terms: Vec<_> = z
                    .iter()
                    .zip(row)
                    .map(|(&var, &coef)| (var, coef as f64))
                    .collect();
                problem.add_constraint(terms, ComparisonOp::Eq, *rhs as f64);
            }
            let float_answer = problem.solve().is_ok();
            assert_eq!(
                exact_answer, float_answer,
                "case {case}: A = {a:?}, b = {b:?}"
            );
            feasible += usize::from(exact_answer);
        }
        // 実行可能・不可能の両方を十分に含むこと。
        assert!(feasible > 30 && feasible < 270, "feasible = {feasible}");
    }
}
