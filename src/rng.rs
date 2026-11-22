//! 再現性のある疑似乱数 (サンプリングと検証の乱択、ゲームの生成が共有する)。

/// 再現性のための小さな疑似乱数生成器 (SplitMix64)。
#[derive(Clone, Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `[0, 1)` の一様乱数。
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `[low, high]` の一様な整数 ([`SplitMix64::next_u64`] を幅で割った余りに `low` を足す)。
    ///
    /// `range(0, u64::MAX)` のように幅が `2^64` なら、`next_u64` の値をそのまま返す。
    ///
    /// # Panics
    ///
    /// `low > high` ならパニックする。
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        assert!(low <= high, "range の下限 {low} が上限 {high} を超える");
        match (high - low).checked_add(1) {
            Some(width) => low + self.next_u64() % width,
            None => self.next_u64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成器の再現性 (`generators` のゲーム、`data/` の値、文書の数値) が依存する出力列。
    #[test]
    fn range_keeps_output_sequence() {
        let mut rng = SplitMix64::new(2026);
        let small: Vec<u64> = (0..5).map(|_| rng.range(0, 9)).collect();
        let offset: Vec<u64> = (0..3).map(|_| rng.range(10, 1000)).collect();
        assert_eq!(small, [1, 1, 4, 6, 1]);
        assert_eq!(offset, [960, 503, 508]);
        assert_eq!(rng.range(0, u64::MAX - 1), 6176811619522188020);
        assert_eq!(rng.range(7, 7), 7);
        assert_eq!(rng.range(1, u64::MAX), 6182220553586467694);
    }

    #[test]
    fn full_range_returns_raw_output() {
        let mut rng = SplitMix64::new(7);
        let mut raw = rng.clone();
        for _ in 0..4 {
            assert_eq!(rng.range(0, u64::MAX), raw.next_u64());
        }
        assert_eq!(SplitMix64::new(1).range(u64::MAX, u64::MAX), u64::MAX);
    }

    #[test]
    #[should_panic(expected = "下限")]
    fn reversed_range_panics() {
        SplitMix64::new(1).range(5, 3);
    }
}
