//! 提携の特性ベクトルが張る部分空間を管理する。

use crate::coalition::Coalition;

/// 追加したベクトルの張る部分空間を、行階段形で保持する。
#[derive(Clone, Debug)]
pub struct Span {
    dimension: usize,
    tolerance: f64,
    /// (ピボット列, ピボット成分を 1 に正規化した行)
    rows: Vec<(usize, Vec<f64>)>,
}

impl Span {
    pub fn new(dimension: usize) -> Span {
        Span {
            dimension,
            tolerance: 1e-9,
            rows: Vec::new(),
        }
    }

    pub fn rank(&self) -> usize {
        self.rows.len()
    }

    pub fn is_full(&self) -> bool {
        self.rows.len() == self.dimension
    }

    fn reduce(&self, mut vector: Vec<f64>) -> Vec<f64> {
        for (pivot, row) in &self.rows {
            let factor = vector[*pivot];
            if factor != 0.0 {
                for (v, r) in vector.iter_mut().zip(row) {
                    *v -= factor * r;
                }
            }
        }
        vector
    }

    pub fn contains(&self, vector: &[f64]) -> bool {
        self.reduce(vector.to_vec())
            .iter()
            .all(|v| v.abs() <= self.tolerance)
    }

    pub fn contains_coalition(&self, coalition: Coalition) -> bool {
        self.contains(&indicator(coalition, self.dimension))
    }

    /// ベクトルを追加する。部分空間が広がった場合に `true` を返す。
    pub fn insert(&mut self, vector: &[f64]) -> bool {
        let reduced = self.reduce(vector.to_vec());
        let Some((pivot, &value)) = reduced
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        else {
            return false;
        };
        if value.abs() <= self.tolerance {
            return false;
        }
        let row: Vec<f64> = reduced.iter().map(|v| v / value).collect();
        // 既存の行からも新しいピボット列を消して、完全な行階段形を保つ。
        for (_, existing) in &mut self.rows {
            let factor = existing[pivot];
            if factor != 0.0 {
                for (e, r) in existing.iter_mut().zip(&row) {
                    *e -= factor * r;
                }
            }
        }
        self.rows.push((pivot, row));
        true
    }

    /// 追加したベクトル全てと直交するベクトルの空間 `{ z : row . z = 0 }` の基底。
    pub fn null_space(&self) -> Vec<Vec<f64>> {
        let mut is_pivot = vec![false; self.dimension];
        for (pivot, _) in &self.rows {
            is_pivot[*pivot] = true;
        }
        (0..self.dimension)
            .filter(|&free| !is_pivot[free])
            .map(|free| {
                let mut basis = vec![0.0; self.dimension];
                basis[free] = 1.0;
                // 各行はピボット列が 1、他の行のピボット列が 0 の既約な形をしている。
                for (pivot, row) in &self.rows {
                    basis[*pivot] = -row[free];
                }
                basis
            })
            .collect()
    }

    pub fn insert_coalition(&mut self, coalition: Coalition) -> bool {
        self.insert(&indicator(coalition, self.dimension))
    }
}

/// 正方行列の連立一次方程式 `a z = b` を部分ピボット選択付きの消去法で解く。
/// 特異(ピボットの絶対値が `tolerance` 以下)なら `None`。
pub fn solve_square(mut a: Vec<Vec<f64>>, mut b: Vec<f64>, tolerance: f64) -> Option<Vec<f64>> {
    let size = b.len();
    for column in 0..size {
        let pivot =
            (column..size).max_by(|&p, &q| a[p][column].abs().total_cmp(&a[q][column].abs()))?;
        if a[pivot][column].abs() <= tolerance {
            return None;
        }
        a.swap(column, pivot);
        b.swap(column, pivot);
        for row in column + 1..size {
            let factor = a[row][column] / a[column][column];
            if factor != 0.0 {
                let (head, tail) = a.split_at_mut(row);
                for (target, source) in tail[0][column..].iter_mut().zip(&head[column][column..]) {
                    *target -= factor * source;
                }
                b[row] -= factor * b[column];
            }
        }
    }
    let mut z = vec![0.0; size];
    for row in (0..size).rev() {
        let rest: f64 = (row + 1..size).map(|k| a[row][k] * z[k]).sum();
        z[row] = (b[row] - rest) / a[row][row];
    }
    Some(z)
}

/// 2 つのベクトルの差の最大絶対値 (無限大ノルム)。
pub fn max_abs_difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q).abs())
        .fold(0.0, f64::max)
}

pub fn indicator(coalition: Coalition, dimension: usize) -> Vec<f64> {
    (0..dimension)
        .map(|i| if coalition.contains(i) { 1.0 } else { 0.0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_span_of_coalitions() {
        let mut span = Span::new(3);
        assert!(span.insert_coalition(Coalition::grand(3)));
        assert!(span.insert_coalition(Coalition::from_players(&[0, 1])));
        // {2} = N - {0,1}
        assert!(span.contains_coalition(Coalition::singleton(2)));
        assert!(!span.contains_coalition(Coalition::singleton(0)));
        assert!(!span.insert_coalition(Coalition::singleton(2)));
        assert!(span.insert_coalition(Coalition::singleton(0)));
        assert!(span.is_full());
    }

    #[test]
    fn null_space_is_orthogonal_to_rows() {
        let mut span = Span::new(4);
        span.insert(&[1.0, 1.0, 1.0, 1.0]);
        span.insert(&[1.0, -1.0, 0.0, 2.0]);
        let basis = span.null_space();
        assert_eq!(basis.len(), 2);
        for z in &basis {
            for row in [[1.0, 1.0, 1.0, 1.0], [1.0, -1.0, 0.0, 2.0]] {
                let dot: f64 = row.iter().zip(z).map(|(a, b)| a * b).sum();
                assert!(dot.abs() < 1e-12);
            }
        }
    }

    #[test]
    fn solves_square_system() {
        let a = vec![vec![0.0, 2.0], vec![1.0, 1.0]];
        assert_eq!(solve_square(a, vec![4.0, 3.0], 1e-12), Some(vec![1.0, 2.0]));
        let singular = vec![vec![1.0, 1.0], vec![2.0, 2.0]];
        assert_eq!(solve_square(singular, vec![1.0, 2.0], 1e-12), None);
    }
}
