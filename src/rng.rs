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

    /// `[low, high]` の一様な整数。
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        debug_assert!(low <= high);
        low + self.next_u64() % (high - low + 1)
    }
}
