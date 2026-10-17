//! 先頭から読んだ分だけ並べ替える列。

use std::cmp::Ordering;

/// 超過の降順、同じ超過は提携の昇順。`(提携, 超過)` の組を超過の大きい順に並べる全順序。
pub(crate) fn by_excess_desc<K: Ord>(a: &(K, f64), b: &(K, f64)) -> Ordering {
    b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0))
}

/// `compare` の順に要素を返す列。読んだ分だけ並べ替える。
///
/// 未確定の要素から次の `chunk` 個を `select_nth_unstable_by` で切り出し、その部分だけを並べ替える。
/// 読み切ったら `chunk` を 4 倍にして続ける。先頭の `k` 個だけ読む費用は、要素数 `N` に対して
/// おおよそ `O(N + k log k)` (全体を並べ替えると `O(N log N)`)。
///
/// 不安定な選択と並べ替えを使うので、`compare` が全順序 (異なる要素を `Equal` にしない) なら
/// 結果は安定ソートと同じになる。
pub(crate) struct LazySorted<T, F> {
    /// 未確定の要素は `items[..items.len() - ready]`、確定した要素は末尾に逆順で並ぶ (`pop` で次の要素が出る)。
    items: Vec<T>,
    ready: usize,
    chunk: usize,
    compare: F,
}

impl<T, F> LazySorted<T, F>
where
    F: FnMut(&T, &T) -> Ordering,
{
    /// `first_chunk` は最初に確定させる個数 (1 未満は 1 として扱う)。
    pub(crate) fn new(items: Vec<T>, first_chunk: usize, compare: F) -> LazySorted<T, F> {
        LazySorted {
            items,
            ready: 0,
            chunk: first_chunk.max(1),
            compare,
        }
    }

    /// 未確定の要素から次の `chunk` 個を末尾に確定させる。
    fn fill(&mut self) {
        let len = self.items.len();
        // 残りが少なければ全部を確定させる (選択の費用が並べ替えに見合わないため)。
        let take = if 2 * self.chunk >= len {
            len
        } else {
            self.chunk
        };
        let compare = &mut self.compare;
        // 逆順で比べ、`compare` で先に来る要素ほど末尾に置く。
        let mut reversed = |a: &T, b: &T| compare(b, a);
        if take < len {
            self.items.select_nth_unstable_by(len - take, &mut reversed);
        }
        self.items[len - take..].sort_unstable_by(&mut reversed);
        self.ready = take;
        self.chunk = self.chunk.saturating_mul(4);
    }
}

impl<T, F> Iterator for LazySorted<T, F>
where
    F: FnMut(&T, &T) -> Ordering,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.ready == 0 {
            if self.items.is_empty() {
                return None;
            }
            self.fill();
        }
        self.ready -= 1;
        self.items.pop()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.items.len(), Some(self.items.len()))
    }
}

impl<T, F> ExactSizeIterator for LazySorted<T, F> where F: FnMut(&T, &T) -> Ordering {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    #[test]
    fn matches_full_sort_for_every_prefix_size() {
        let mut rng = SplitMix64::new(7);
        for len in [0, 1, 2, 3, 7, 63, 64, 65, 300, 1000] {
            // 同じ値を多く含める (値は 0..10 の整数)。
            let items: Vec<(u32, f64)> = (0..len)
                .map(|i| (i as u32, rng.range(0, 9) as f64))
                .collect();
            let mut expected = items.clone();
            expected.sort_by(by_excess_desc);
            for first_chunk in [0, 1, 2, 5, 64, 2000] {
                let lazy: Vec<_> =
                    LazySorted::new(items.clone(), first_chunk, by_excess_desc).collect();
                assert_eq!(lazy, expected, "len={len} first_chunk={first_chunk}");
                // 途中で止めても先頭は一致する。
                let prefix: Vec<_> = LazySorted::new(items.clone(), first_chunk, by_excess_desc)
                    .take(len / 3)
                    .collect();
                assert_eq!(
                    prefix,
                    expected[..len / 3],
                    "len={len} first_chunk={first_chunk}"
                );
            }
        }
    }

    #[test]
    fn reports_remaining_length() {
        let mut lazy = LazySorted::new(vec![3, 1, 2], 1, |a: &i32, b: &i32| a.cmp(b));
        assert_eq!(lazy.len(), 3);
        assert_eq!(lazy.next(), Some(1));
        assert_eq!(lazy.len(), 2);
        assert_eq!(lazy.collect::<Vec<_>>(), vec![2, 3]);
    }
}
