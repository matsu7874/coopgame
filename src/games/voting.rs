//! 重み付き投票ゲームのオラクル。
//!
//! `v(S) = 1` (重みの和が基準 `q` 以上) / `0` (未満) なので、超過は勝利提携で `1 - x(S)`、
//! 敗北提携で `-x(S)` になる。重みの和を `q` で打ち切った値を状態とする動的計画法で、
//! 各状態について `x(S)` の小さい提携を上位 `k` 個求め、超過の大きい順に並べる
//! (計算量は `O(n s k log k)`、`s` は打ち切った重みの和のうち到達できる値の数 (`q + 1` 以下))。
//! `k` 個を使い切ったら `k` を倍にして計算し直す。同じ超過の提携は提携の全順序で並べるので、
//! 計算し直しても既に返した列の順序は変わらない。
//!
//! 重み付き投票ゲームの仁を重みについて擬多項式時間で計算するアルゴリズムは Pashkovich (2022) が示した。
//! それ以前の Elkind & Pasechnik (2009) のアルゴリズムは、仁を計算する保証がないと後続研究で指摘されている。
//! ここでは仁の制約生成 ([`crate::nucleolus::oracle`]) の LP は単体法で解き、違反する提携の探索
//! ([`crate::game::oracle::Separation::violated_outside_span`]) に Pashkovich の分離の方法を使う。段の数や LP の大きさについての
//! 計算量の保証はない。
//!
//! # 確定済みの空間に入らない提携の探索
//!
//! 仁が 2 段以上で決まるゲーム (拒否権を持つプレイヤーがいるゲームなど) では、後の段で、超過の大きい提携の
//! ほとんどが確定済みの空間 (確定した提携の特性ベクトル `z_1, ..., z_j` が張る空間) に入る。超過の順に読んで
//! 読み飛ばすと、その数 (拒否権 3 人のゲームで約 `3 * 2^(n - 4)` 個) だけ読むことになる。
//! [`OracleGame::search_outside_span`] は、次の順に探す。
//!
//! 1. 超過の順に `ENUMERATION_BUDGET` (256) 個まで読む。その範囲で違反の有無が決まれば (違反する提携が見つかるか、
//!    超過が `bound` 以下の提携まで読めば) その結果を返す。確定済みの提携が少ないゲームは、ここで終わる。
//! 2. 素数 `p` と、`F_p` 上で `z_1, ..., z_j` に直交するベクトルの乱数の線形結合 `r` を 1 つ選び、
//!    内積 `<r, χ(S)> mod p` を状態に加えた動的計画法で、剰余が 0 でない提携を探す。剰余が 0 でなければ
//!    `χ(S)` は `F_p` 上で `z` の張る空間に入らず、したがって実数上でも入らない (下の補題 4 の易しい向き)。
//!    見つかった提携は、実数の計算 (浮動小数点数の消去) でも空間に入らないことを確かめてから返す。
//! 3. 2 で見つからなければ、各素数 `p` と、`F_p` 上の直交補空間の基底の各ベクトル `v` について、
//!    `<v, χ(S)> mod p != 0` の提携を同じ動的計画法で探す (Pashkovich の補題 7 の分離)。全ての組で見つからない
//!    ときだけ「違反なし」と答える。
//!
//! 3 が厳密であることは Pashkovich (arXiv:1810.02670 v1) の補題 4 による: 異なる素数の集合 `P` の積が
//! 0/1 行列の小行列式の絶対値の上界を超えれば、0/1 ベクトルが実数上で一次独立であることと、ある `p ∈ P` で
//! `F_p` 上一次独立であることは同値である。証明は、実数上で独立なら 0 でない小行列式 `α` があり、全ての `p` で
//! 従属なら `α` は全ての `p` で割り切れるので `|α| >= ∏p` となり、上界と矛盾する、というものである。
//! 原典は上界に `|α| <= k! <= n!` を使い `|P| >= log2(n!)` 個の素数を取るが、ここでは Hadamard の不等式
//! `|det A| <= ∏ ||a_i||` (`a_i` は列) から、成分が 0/1 で大きさ `m <= n` の小行列式について
//! `|α| <= m^(m/2) <= n^(n/2)` を使い、積が `n^(n/2)` を超える最小の素数の組を使う (`hadamard_primes`)。
//! 証明の残りは同じである。30 人で、素数は 108 個 (和 28,697) から 18 個 (和 501) に減る。
//! `z` が `F_p` 上で従属な素数は使わない (原典は直交補空間の基底を 0 にする。同じ扱い)。
//!
//! 原典の補題 7 の証明は `γ_{k,g,U} := min{w(S) : ...}` と書いているが、比べる相手 (`1 + ε`) から
//! `min{x(S) : ...}` の誤植である。また原典は配分を `x >= 0` とし、勝利提携 (`U >= W`) だけを確かめる。
//! プレ仁では `x` が負になりうるので、ここでは敗北提携の状態 (超過 `-x(S)`) も確かめる。

use num_bigint::BigUint;

use crate::error::{Error, Result};
use crate::game::oracle::{OracleGame, violated_in_order};
use crate::game::{PlayerSet, SetFunction};
use crate::linalg::Span;
use crate::rng::SplitMix64;

/// 確定済みの空間を使った探索 ([`OracleGame::search_outside_span`]) の前に、超過の順に読む提携の数の上限。
///
/// 確定済みの空間に入る提携のうち超過の大きいものが少なければ、この範囲で違反の有無が決まり、素数を法とする
/// 探索 (時間は素数の和と直交補空間の次元と重みの和の状態の数に比例する) をしなくて済む。上限を大きくすると、拒否権のある
/// ゲームのように上限を超えるときに、超過の順の列を作る時間とメモリが無駄になる。
///
/// 上限を 64・256・1024・4096 にしたビルドで測った (`data/bench/voting-budget.csv`)。
/// - 仁が 1 段で決まるゲーム (多数決・apex・重みが 1-10 と 1-100 のランダムなゲーム、30 人前後) では、
///   確定済みの提携のある探索で読んだ提携は全て 0 個で、時間は上限によらない。
/// - 重みが 1-100 で基準が 9/10 の 10 人のゲームは、1 回の探索で最大 73 個読む。上限 64 では素数の探索に進み、
///   時間が 0.003 秒から 0.004 秒になる。
/// - 拒否権のある 40 人のゲームは上限を超え、上限 256 で 0.42 秒・24 MB、1024 で 0.85 秒・69 MB、
///   4096 で 2.4 秒・377 MB (拒否権 3 人)。
///
/// 2 のべき乗にしているのは、超過の順の列が上位 `k` 個を `k = 64, 128, ...` と倍にしながら求めるためで、
/// この数で止めれば最後の計算がちょうどこの大きさになる。
pub(crate) const ENUMERATION_BUDGET: usize = 256;

/// 乱数の線形結合 (探索の 2) の係数を作る乱数の種。結果を再現できるよう固定する。
const COMBINATION_SEED: u64 = 0x5eed_0f5a_4e00;

#[derive(Clone, Debug, PartialEq)]
pub struct WeightedVotingGame {
    weights: Vec<u64>,
    quota: u64,
}

impl WeightedVotingGame {
    /// 動的計画法の状態数 `q + 1` が大きすぎないよう、基準の上限を設ける。
    pub const MAX_QUOTA: u64 = 1_000_000;

    /// 重み `weights` と基準 `quota` から作る。重みは 1 つ以上、基準は 1 以上で重みの和以下、かつ
    /// [`Self::MAX_QUOTA`] 以下。検査は [`crate::generators::weighted_voting`] と同じ。
    ///
    /// 基準が 0 だと空提携も勝利提携になり、空提携の値が 1 になる ([`SetFunction`] の前提に反する)。
    /// 基準が重みの和を超えると全体提携も勝てず、全ての提携の値が 0 になる (基準か重みの書き間違いとみなす)。
    /// 重みの和は基準で打ち切って数えるので、和が `u64` を超える重みでも正しく扱う。
    pub fn new(weights: Vec<u64>, quota: u64) -> Result<WeightedVotingGame> {
        if weights.is_empty() {
            return Err(Error::InvalidArgument("重みが 1 つもない".into()));
        }
        if quota == 0 {
            return Err(Error::InvalidArgument(
                "基準は 1 以上 (0 だと空提携が勝利提携になる)".into(),
            ));
        }
        let total = weights.iter().fold(0u64, |acc, &w| acc.saturating_add(w));
        if quota > total {
            return Err(Error::InvalidArgument(format!(
                "基準 {quota} が重みの和 {total} を超える (全体提携も勝てず、全ての提携の値が 0 になる)"
            )));
        }
        if quota > Self::MAX_QUOTA {
            return Err(Error::InvalidArgument(format!(
                "基準 {quota} は上限 {} を超える",
                Self::MAX_QUOTA
            )));
        }
        Ok(WeightedVotingGame { weights, quota })
    }

    pub fn weights(&self) -> &[u64] {
        &self.weights
    }

    pub fn quota(&self) -> u64 {
        self.quota
    }

    /// 提携の重みの和 (`u64` であふれないよう飽和させる。勝敗は基準との比較だけで決まる)。
    fn weight(&self, coalition: &PlayerSet) -> u64 {
        coalition
            .members()
            .fold(0u64, |acc, i| acc.saturating_add(self.weights[i]))
    }

    /// 超過の大きい順 (同じなら提携の昇順) に、真部分提携を最大 `k` 個返す。
    fn top(&self, x: &[f64], k: usize) -> Vec<(PlayerSet, f64)> {
        self.top_with(x, k, None)
    }

    /// [`Self::top`] と同じだが、`residue = Some((v, p))` なら `<v, χ(S)> mod p != 0` の提携に限る。
    ///
    /// 状態は (打ち切った重みの和, 剰余)。重みの和は到達できる値だけを持つ ([`Reach`])。
    fn top_with(
        &self,
        x: &[f64],
        k: usize,
        residue: Option<(&[u64], u64)>,
    ) -> Vec<(PlayerSet, f64)> {
        let n = self.weights.len();
        let (coefficients, p) = residue.map_or((None, 1), |(v, p)| (Some(v), p as usize));
        let keep = k + 2; // 空提携と全体提携を除いても k 個残るように
        let by_sum = |a: &(f64, PlayerSet), b: &(f64, PlayerSet)| {
            a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1))
        };
        let mut reach = Reach::new(self.quota);
        // states[slot * p + g]: 打ち切った重みの和が sums[slot]、剰余が g の部分集合のうち、x(S) の小さい上位 keep 個
        let mut states: Vec<Vec<(f64, PlayerSet)>> = vec![Vec::new(); p];
        states[0].push((0.0, PlayerSet::empty(n)));
        for (i, (&weight, &payoff)) in self.weights.iter().zip(x).enumerate() {
            let step = coefficients.map_or(0, |v| v[i] as usize);
            let moves = reach.advance(weight);
            let mut next: Vec<Vec<(f64, PlayerSet)>> = vec![Vec::new(); reach.len() * p];
            for (slot, &(stay, take)) in moves.iter().enumerate() {
                for g in 0..p {
                    for (sum, set) in &states[slot * p + g] {
                        next[stay * p + g].push((*sum, set.clone()));
                        let mut with = set.clone();
                        with.insert(i);
                        next[take * p + (g + step) % p].push((sum + payoff, with));
                    }
                }
            }
            for list in &mut next {
                list.sort_by(by_sum);
                list.truncate(keep);
            }
            states = next;
        }
        let mut result: Vec<(PlayerSet, f64)> = Vec::new();
        for (index, list) in states.into_iter().enumerate() {
            let (slot, g) = (index / p, index % p);
            if residue.is_some() && g == 0 {
                continue;
            }
            let value = if reach.sum(slot) == self.quota {
                1.0
            } else {
                0.0
            };
            for (sum, set) in list {
                if set.is_proper() {
                    result.push((set, value - sum));
                }
            }
        }
        result.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        result.truncate(k);
        result
    }

    /// `<v, χ(S)> mod p != 0` の真部分提携の超過の最大値 (なければ `-∞`)。提携は求めず、各状態の `x(S)` の
    /// 最小値だけを持つ。
    ///
    /// `<v, χ(N)> ≡ 0` (`v` が全体提携の特性ベクトルに直交する) なら、剰余が 0 でない提携は空でも全体でもない。
    fn max_excess_with(&self, x: &[f64], v: &[u64], p: u64) -> f64 {
        let p = p as usize;
        let mut reach = Reach::new(self.quota);
        let mut states = vec![f64::INFINITY; p];
        states[0] = 0.0;
        for (i, (&weight, &payoff)) in self.weights.iter().zip(x).enumerate() {
            let step = v[i] as usize;
            let moves = reach.advance(weight);
            let mut next = vec![f64::INFINITY; reach.len() * p];
            for (slot, &(stay, take)) in moves.iter().enumerate() {
                for g in 0..p {
                    let sum = states[slot * p + g];
                    if sum == f64::INFINITY {
                        continue;
                    }
                    let a = &mut next[stay * p + g];
                    *a = a.min(sum);
                    let b = &mut next[take * p + (g + step) % p];
                    *b = b.min(sum + payoff);
                }
            }
            states = next;
        }
        let mut best = f64::NEG_INFINITY;
        for (index, &sum) in states.iter().enumerate() {
            let (slot, g) = (index / p, index % p);
            if g != 0 && sum < f64::INFINITY {
                let value = if reach.sum(slot) == self.quota {
                    1.0
                } else {
                    0.0
                };
                best = best.max(value - sum);
            }
        }
        best
    }

    /// `<v, χ(S)> mod p != 0` で、超過が `bound` より大きく `accept(S)` が真の真部分提携を、最大 `limit` 個返す。
    /// 該当する提携があれば空でない列を返す (`accept` が偽の提携が上位を占めるときは `k` を倍にして探し直す)。
    fn search_residue(
        &self,
        x: &[f64],
        bound: f64,
        (v, p): (&[u64], u64),
        accept: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        if self.max_excess_with(x, v, p) <= bound {
            return Vec::new();
        }
        let mut k = limit.max(32);
        loop {
            let list = self.top_with(x, k, Some((v, p)));
            let mut found = Vec::new();
            for (coalition, excess) in &list {
                if *excess <= bound {
                    return found;
                }
                if accept(coalition) {
                    found.push((coalition.clone(), *excess));
                    if found.len() >= limit {
                        return found;
                    }
                }
            }
            if list.len() < k || !found.is_empty() {
                return found;
            }
            k *= 2;
        }
    }

    /// 確定済みの空間 (`basis` の張る空間、`basis` は実数上で一次独立) に入らない提携の中から、超過が `bound`
    /// より大きく `accept(S)` が真のものを探す (モジュールの説明の 2 と 3)。
    ///
    /// `accept` は `S` が確定済みの空間に入らないことも確かめる。違反する提携があれば空でない列を返す。
    ///
    /// どの素数でも `basis` が `F_p` 上で一次従属なら `None` を返す (素数の探索では判定できない)。
    /// `basis` が実数上で一次独立なら補題 4 からこれは起こらないが、`basis` の独立性は浮動小数点数の消去
    /// ([`Span`]) で判定しているので、判定が食い違ったときに「違反なし」と誤って答えないよう、呼び出し側は
    /// 超過の順の探索に戻る。
    fn search_by_primes(
        &self,
        x: &[f64],
        bound: f64,
        basis: &[PlayerSet],
        accept: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
        randomized: bool,
    ) -> Option<Vec<(PlayerSet, f64)>> {
        let n = self.weights.len();
        let complements: Vec<(u64, Vec<Vec<u64>>)> = hadamard_primes(n)
            .into_iter()
            .filter_map(|p| orthogonal_complement(basis, n, p).map(|vectors| (p, vectors)))
            .collect();
        if complements.is_empty() {
            return None;
        }
        // 2. 最も大きい素数で、直交補空間の基底の乱数の線形結合を 1 つ試す。
        if randomized && let Some((p, vectors)) = complements.last() {
            let mut rng =
                SplitMix64::new(COMBINATION_SEED ^ ((n as u64) << 32) ^ basis.len() as u64);
            let mut combination = vec![0u64; n];
            for vector in vectors {
                let r = rng.range(1, p - 1);
                for (c, &e) in combination.iter_mut().zip(vector) {
                    *c = (*c + r * e) % p;
                }
            }
            let found = self.search_residue(x, bound, (&combination, *p), accept, limit);
            if !found.is_empty() {
                return Some(found);
            }
        }
        // 3. 全ての素数と、直交補空間の基底の全てのベクトルで確かめる。
        for (p, vectors) in &complements {
            for vector in vectors {
                let found = self.search_residue(x, bound, (vector, *p), accept, limit);
                if !found.is_empty() {
                    return Some(found);
                }
            }
        }
        Some(Vec::new())
    }

    /// [`OracleGame::search_outside_span`] の本体。超過の順に読む数の上限 `budget` を受け取る
    /// (テストでは上限を小さくして、素数の探索に進む経路を小さいゲームで通す)。
    fn search_outside_span_with_budget(
        &self,
        x: &[f64],
        bound: f64,
        fixed: &[PlayerSet],
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
        budget: usize,
    ) -> Vec<(PlayerSet, f64)> {
        let n = self.players();
        let (span, basis) = independent_basis(fixed, n);
        // 全体提携 (と空) だけが張る空間には、真部分提携は入らない。超過の順に探せばよい。
        if basis.iter().all(PlayerSet::is_full) {
            return violated_in_order(self.excess_order(x), bound, skip, limit);
        }
        // 1. 超過の順に budget 個まで読む。
        let mut found = Vec::new();
        let mut read = 0;
        for (coalition, excess) in self.excess_order(x).take(budget) {
            if excess <= bound || found.len() >= limit {
                return found;
            }
            read += 1;
            if !skip(&coalition) {
                found.push((coalition, excess));
            }
        }
        if read < budget || !found.is_empty() {
            return found;
        }
        // 2・3. 素数を法とする動的計画法で、確定済みの空間に入らない提携を探す。
        let accept =
            |coalition: &PlayerSet| !skip(coalition) && !span.contains(&coalition.indicator());
        match self.search_by_primes(x, bound, &basis, &accept, limit, true) {
            Some(found) => found,
            None => {
                debug_assert!(
                    false,
                    "実数上で一次独立と判定した基底 {basis:?} が、どの素数でも F_p 上で従属"
                );
                // 正しさを優先し、超過の順に全て読む探索に戻る (skip は確定済みの提携を除く)。
                violated_in_order(self.excess_order(x), bound, skip, limit)
            }
        }
    }
}

/// 打ち切った重みの和のうち、先頭から何人かを見た時点で到達できる値 (昇順)。
///
/// 重みが大きく基準が大きいゲーム (重み 1000 の 3 人と重み 1 の人たち) では、`q + 1` 個の値のうち
/// 到達できるものはわずかなので、状態を到達できる値に限る。
struct Reach {
    quota: u64,
    sums: Vec<u64>,
}

impl Reach {
    fn new(quota: u64) -> Reach {
        Reach {
            quota,
            sums: vec![0],
        }
    }

    fn len(&self) -> usize {
        self.sums.len()
    }

    fn sum(&self, slot: usize) -> u64 {
        self.sums[slot]
    }

    /// 重み `weight` のプレイヤーを加え、元の各値について (加えない場合, 加えた場合) の新しい位置を返す。
    fn advance(&mut self, weight: u64) -> Vec<(usize, usize)> {
        let added: Vec<u64> = self
            .sums
            .iter()
            .map(|&c| c.saturating_add(weight).min(self.quota))
            .collect();
        let mut merged: Vec<u64> = self.sums.iter().chain(&added).copied().collect();
        merged.sort_unstable();
        merged.dedup();
        let position = |c: u64| merged.binary_search(&c).expect("到達できる値");
        let moves = self
            .sums
            .iter()
            .zip(&added)
            .map(|(&c, &a)| (position(c), position(a)))
            .collect();
        self.sums = merged;
        moves
    }
}

/// 積が `n^(n/2)` を超える最小の素数の組 (小さい順の素数の先頭)。
///
/// 大きさ `m <= n` で成分が 0/1 の行列の行列式は、Hadamard の不等式 `|det A| <= ∏ ||a_i||` と
/// `||a_i|| <= sqrt(m)` から `m^(m/2) <= n^(n/2)` 以下である。積がこれを超えれば、Pashkovich の補題 4 の証明が
/// そのまま成り立つ (モジュールの説明)。積の 2 乗と `n^n` を整数で比べる。
pub(crate) fn hadamard_primes(n: usize) -> Vec<u64> {
    let bound = BigUint::from(n).pow(n as u32);
    let mut product = BigUint::from(1u32);
    let mut primes = Vec::new();
    let mut candidate = 2u64;
    while &product * &product <= bound {
        if (2..candidate)
            .take_while(|d| d * d <= candidate)
            .all(|d| !candidate.is_multiple_of(d))
        {
            primes.push(candidate);
            product *= candidate;
        }
        candidate += 1;
    }
    primes
}

/// `F_p` 上で、`basis` の特性ベクトル全てに直交するベクトルの空間の基底 (`n - basis.len()` 本)。
///
/// `basis` が `F_p` 上で一次従属なら `None` (この素数は使わない)。
pub(crate) fn orthogonal_complement(
    basis: &[PlayerSet],
    n: usize,
    p: u64,
) -> Option<Vec<Vec<u64>>> {
    let mut rows: Vec<Vec<u64>> = basis
        .iter()
        .map(|set| (0..n).map(|i| u64::from(set.contains(i))).collect())
        .collect();
    let inverse = |a: u64| power(a, p - 2, p);
    // 行既約階段形にする。
    let mut pivots = Vec::new();
    for column in 0..n {
        let rank = pivots.len();
        // 成分は常に 0 以上 p 未満に保つ。
        let Some(found) = (rank..rows.len()).find(|&r| rows[r][column] != 0) else {
            continue;
        };
        rows.swap(rank, found);
        let scale = inverse(rows[rank][column]);
        for e in &mut rows[rank] {
            *e = *e * scale % p;
        }
        let pivot_row = rows[rank].clone();
        for (r, row) in rows.iter_mut().enumerate() {
            let factor = row[column];
            if r != rank && factor != 0 {
                for (e, &q) in row.iter_mut().zip(&pivot_row) {
                    *e = (*e + (p - factor) * q) % p;
                }
            }
        }
        pivots.push(column);
    }
    if pivots.len() < basis.len() {
        return None;
    }
    // 自由な列ごとに 1 本: 自由な列を 1、ピボット列を -(その行の自由な列の成分) にする。
    let mut vectors = Vec::new();
    for free in (0..n).filter(|c| !pivots.contains(c)) {
        let mut vector = vec![0u64; n];
        vector[free] = 1;
        for (r, &pivot) in pivots.iter().enumerate() {
            vector[pivot] = (p - rows[r][free]) % p;
        }
        vectors.push(vector);
    }
    Some(vectors)
}

/// `base^exponent mod modulus`。
fn power(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1 % modulus;
    base %= modulus;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = result * base % modulus;
        }
        base = base * base % modulus;
        exponent >>= 1;
    }
    result
}

impl SetFunction for WeightedVotingGame {
    fn players(&self) -> usize {
        self.weights.len()
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if self.weight(coalition) >= self.quota {
            1.0
        } else {
            0.0
        }
    }
}

struct VotingExcess<'a> {
    game: &'a WeightedVotingGame,
    x: &'a [f64],
    k: usize,
    list: Vec<(PlayerSet, f64)>,
    index: usize,
    exhausted: bool,
}

impl Iterator for VotingExcess<'_> {
    type Item = (PlayerSet, f64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.list.len() {
            if self.exhausted {
                return None;
            }
            self.k *= 2;
            self.list = self.game.top(self.x, self.k);
            self.exhausted = self.list.len() < self.k;
            if self.index >= self.list.len() {
                return None;
            }
        }
        self.index += 1;
        Some(self.list[self.index - 1].clone())
    }
}

impl OracleGame for WeightedVotingGame {
    fn excess_order<'a>(&'a self, x: &'a [f64]) -> Box<dyn Iterator<Item = (PlayerSet, f64)> + 'a> {
        let n = self.players();
        assert_eq!(
            x.len(),
            n,
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        );
        Box::new(VotingExcess {
            game: self,
            x,
            k: 32,
            list: Vec::new(),
            index: 0,
            exhausted: false,
        })
    }

    fn scale(&self) -> f64 {
        1.0
    }

    /// 確定済みの空間に入らない提携を、モジュールの説明の 1-3 の順に探す。
    fn search_outside_span(
        &self,
        x: &[f64],
        bound: f64,
        fixed: &[PlayerSet],
        skip: &dyn Fn(&PlayerSet) -> bool,
        limit: usize,
    ) -> Vec<(PlayerSet, f64)> {
        self.search_outside_span_with_budget(x, bound, fixed, skip, limit, ENUMERATION_BUDGET)
    }
}

/// `fixed` の特性ベクトルが張る空間と、その基底になる `fixed` の部分列 (実数上で一次独立)。
fn independent_basis(fixed: &[PlayerSet], n: usize) -> (Span, Vec<PlayerSet>) {
    let mut span = Span::new(n);
    let basis = fixed
        .iter()
        .filter(|set| span.insert(&set.indicator()))
        .cloned()
        .collect();
    (span, basis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    /// 整数行列の階数 (分数を使わない Gauss の消去。成分が小さいので i128 であふれない)。
    fn rank_over_rationals(vectors: &[Vec<i128>]) -> usize {
        let mut rows = vectors.to_vec();
        let columns = rows.first().map_or(0, Vec::len);
        let mut rank = 0;
        for column in 0..columns {
            let Some(found) = (rank..rows.len()).find(|&r| rows[r][column] != 0) else {
                continue;
            };
            rows.swap(rank, found);
            let pivot_row = rows[rank].clone();
            for (r, row) in rows.iter_mut().enumerate() {
                if r != rank && row[column] != 0 {
                    let (a, b) = (pivot_row[column], row[column]);
                    for (e, &q) in row.iter_mut().zip(&pivot_row) {
                        *e = *e * a - q * b;
                    }
                    let g = row.iter().fold(0i128, |g, &e| gcd(g, e.abs()));
                    if g > 1 {
                        for e in row.iter_mut() {
                            *e /= g;
                        }
                    }
                }
            }
            rank += 1;
        }
        rank
    }

    fn gcd(a: i128, b: i128) -> i128 {
        if b == 0 { a } else { gcd(b, a % b) }
    }

    fn set_of(n: usize, bits: usize) -> PlayerSet {
        let members: Vec<usize> = (0..n).filter(|i| bits >> i & 1 == 1).collect();
        PlayerSet::from_players(n, &members)
    }

    fn determinant(matrix: &[Vec<i128>]) -> i128 {
        let m = matrix.len();
        if m == 1 {
            return matrix[0][0];
        }
        (0..m)
            .map(|c| {
                let minor: Vec<Vec<i128>> = matrix[1..]
                    .iter()
                    .map(|row| [&row[..c], &row[c + 1..]].concat())
                    .collect();
                let sign = if c % 2 == 0 { 1 } else { -1 };
                sign * matrix[0][c] * determinant(&minor)
            })
            .sum()
    }

    /// 補題 4 の上界の置き換え: 大きさ `m <= 4` の全ての 0/1 行列で `det^2 <= m^m` (Hadamard の不等式)。
    /// [`hadamard_primes`] の積は `n^(n/2)` を超え、最後の素数を除くと超えない (最小の組)。
    #[test]
    fn hadamard_bound_holds_for_all_small_binary_matrices() {
        for m in 1..=4usize {
            let mut largest = 0;
            for bits in 0..1u32 << (m * m) {
                let matrix: Vec<Vec<i128>> = (0..m)
                    .map(|r| {
                        (0..m)
                            .map(|c| i128::from(bits >> (r * m + c) & 1))
                            .collect()
                    })
                    .collect();
                let det = determinant(&matrix).abs();
                largest = largest.max(det);
                assert!(det * det <= (m as i128).pow(m as u32), "m={m} det={det}");
            }
            // 0/1 行列の行列式の最大値は 1, 1, 2, 3 (m = 1..4)。
            assert_eq!(largest, [1, 1, 2, 3][m - 1], "m={m}");
        }
        for n in 1..=60usize {
            let primes = hadamard_primes(n);
            let bound = BigUint::from(n).pow(n as u32);
            let product: BigUint = primes.iter().map(|&p| BigUint::from(p)).product();
            assert!(&product * &product > bound, "n={n}");
            let without_last = &product / primes.last().unwrap();
            assert!(
                &without_last * &without_last <= bound,
                "n={n} は最小の組でない"
            );
        }
        assert_eq!(hadamard_primes(30).len(), 18);
        assert_eq!(hadamard_primes(30).iter().sum::<u64>(), 501);
    }

    /// 補題 4 (素数を [`hadamard_primes`] にしたもの): n <= 5 の 0/1 ベクトルの全ての組
    /// (大きさ n 以下) で、実数上で一次独立であることと、ある素数で F_p 上一次独立であることが同値。
    #[test]
    fn lemma_4_holds_with_hadamard_primes() {
        for n in 2..=5usize {
            let primes = hadamard_primes(n);
            let vectors: Vec<usize> = (1..1usize << n).collect();
            let mut checked = 0;
            let mut subset: Vec<usize> = Vec::new();
            // 大きさ n 以下の部分集合を辞書式に列挙する。
            fn walk(
                start: usize,
                n: usize,
                vectors: &[usize],
                subset: &mut Vec<usize>,
                visit: &mut dyn FnMut(&[usize]),
            ) {
                if !subset.is_empty() {
                    visit(subset);
                }
                if subset.len() == n {
                    return;
                }
                for i in start..vectors.len() {
                    subset.push(vectors[i]);
                    walk(i + 1, n, vectors, subset, visit);
                    subset.pop();
                }
            }
            walk(0, n, &vectors, &mut subset, &mut |chosen| {
                let rows: Vec<Vec<i128>> = chosen
                    .iter()
                    .map(|&bits| (0..n).map(|i| (bits >> i & 1) as i128).collect())
                    .collect();
                let real = rank_over_rationals(&rows) == chosen.len();
                let sets: Vec<PlayerSet> = chosen.iter().map(|&b| set_of(n, b)).collect();
                let modular = primes
                    .iter()
                    .any(|&p| orthogonal_complement(&sets, n, p).is_some());
                assert_eq!(real, modular, "n={n} {chosen:?}");
                checked += 1;
            });
            assert!(checked > 0);
        }
    }

    /// 直交補空間の基底は `n - j` 本で、どれも `basis` に直交し、`χ(S)` が F_p 上で `basis` の張る空間に
    /// 入らないことと、ある基底ベクトルとの内積が 0 でないことが同値 (n = 5 の全ての提携で確かめる)。
    #[test]
    fn orthogonal_complement_detects_membership() {
        let n = 5;
        let mut rng = SplitMix64::new(3);
        for _ in 0..200 {
            let size = rng.range(1, 4) as usize;
            let basis: Vec<PlayerSet> = (0..size)
                .map(|_| set_of(n, rng.range(1, 31) as usize))
                .collect();
            for p in [2u64, 3, 5, 7, 11] {
                let Some(vectors) = orthogonal_complement(&basis, n, p) else {
                    continue;
                };
                assert_eq!(vectors.len(), n - size);
                let dot = |v: &[u64], s: &PlayerSet| s.members().map(|i| v[i]).sum::<u64>() % p;
                for v in &vectors {
                    assert!(basis.iter().all(|z| dot(v, z) == 0));
                }
                for bits in 1..1usize << n {
                    let s = set_of(n, bits);
                    // F_p 上で basis と s の階数 (s を加えて独立なら空間に入らない)
                    let mut with = basis.clone();
                    with.push(s.clone());
                    let outside = orthogonal_complement(&with, n, p).is_some();
                    assert_eq!(outside, vectors.iter().any(|v| dot(v, &s) != 0));
                }
            }
        }
    }

    /// 素数を法とする探索 (乱数の線形結合あり・なし) と、探索全体 ([`OracleGame::search_outside_span`]) が、
    /// 総当たりと一致する: 違反する提携 (超過が bound より大きく、skip でなく、確定済みの空間に入らない) が
    /// あれば空でない列を返し、返す提携はどれもその条件を満たす。
    #[test]
    fn searches_outside_span_match_brute_force() {
        let mut rng = SplitMix64::new(17);
        for case in 0..300 {
            let n = rng.range(3, 9) as usize;
            let weights: Vec<u64> = (0..n)
                .map(|_| {
                    if rng.range(0, 3) == 0 {
                        50
                    } else {
                        rng.range(1, 6)
                    }
                })
                .collect();
            let quota = rng.range(1, weights.iter().sum::<u64>());
            let game = WeightedVotingGame::new(weights.clone(), quota).unwrap();
            let x: Vec<f64> = (0..n).map(|_| rng.next_f64() * 0.5 - 0.1).collect();
            let mut fixed = vec![PlayerSet::grand(n)];
            for _ in 0..rng.range(0, n as u64 - 1) {
                fixed.push(set_of(n, rng.range(1, (1 << n) - 2) as usize));
            }
            let (span, basis) = independent_basis(&fixed, n);
            let skipped: Vec<PlayerSet> = (0..rng.range(0, 3))
                .map(|_| set_of(n, rng.range(1, (1 << n) - 2) as usize))
                .collect();
            let in_span = |s: &PlayerSet| span.contains(&s.indicator());
            let skip = |s: &PlayerSet| skipped.contains(s) || in_span(s);
            let accept = |s: &PlayerSet| !skip(s);
            let bound = rng.next_f64() * 0.6 - 0.3;
            let excess = |s: &PlayerSet| game.value(s) - s.sum(&x);
            let expected: Vec<PlayerSet> = (1..(1usize << n) - 1)
                .map(|bits| set_of(n, bits))
                .filter(|s| excess(s) > bound && !skip(s))
                .collect();
            let limit = rng.range(1, 6) as usize;
            let results = [
                game.search_by_primes(&x, bound, &basis, &accept, limit, true)
                    .expect("実数上で独立な基底は、ある素数で独立"),
                game.search_by_primes(&x, bound, &basis, &accept, limit, false)
                    .expect("実数上で独立な基底は、ある素数で独立"),
                game.search_outside_span(&x, bound, &fixed, &skip, limit),
            ];
            for (which, found) in results.iter().enumerate() {
                let label =
                    format!("case={case} which={which} w={weights:?} q={quota} fixed={fixed:?}");
                assert_eq!(found.is_empty(), expected.is_empty(), "{label}");
                assert!(found.len() <= limit, "{label}");
                for (s, e) in found {
                    assert!(expected.contains(s), "{label}: {s}");
                    assert!((e - excess(s)).abs() < 1e-12, "{label}: {s}");
                }
            }
        }
    }

    /// 列挙の上限を超えて素数の探索に進む経路を、拒否権のある 10-12 人のランダムな投票ゲームで総当たりと比べる。
    /// 上限は既定の 256 と、テスト用に小さくした 8 の両方で、探索全体 (`search_outside_span_with_budget`) を通す。
    /// 600 問のうち、素数の探索に進んで違反なしと確かめたものが上限 256 で 76 問・8 で 96 問、違反を見つけたものが
    /// 15 問・89 問ある (種 2027)。
    #[test]
    fn search_beyond_budget_matches_brute_force_on_veto_games() {
        let mut rng = SplitMix64::new(2027);
        // 上限ごとに、素数の探索に進んで (違反なしと確かめた数, 違反を見つけた数)
        let mut beyond = [(0usize, 0usize); 2];
        for case in 0..600 {
            let n = rng.range(10, 12) as usize;
            let vetoes = rng.range(1, 3) as usize;
            let mut weights: Vec<u64> = (0..n).map(|_| rng.range(1, 5)).collect();
            weights[..vetoes].fill(100);
            let small: u64 = weights[vetoes..].iter().sum();
            let quota = 100 * vetoes as u64 + rng.range(1, small);
            let game = WeightedVotingGame::new(weights.clone(), quota).unwrap();
            let x: Vec<f64> = (0..n)
                .map(|i| {
                    if i < vetoes {
                        1.0 / vetoes as f64 + rng.next_f64() * 0.1 - 0.05
                    } else {
                        rng.next_f64() * 0.1 - 0.03
                    }
                })
                .collect();
            // 確定済み: 全体提携、小さいプレイヤーの大半の 1 人提携、ときどき任意の提携。
            let mut fixed = vec![PlayerSet::grand(n)];
            for i in vetoes..n {
                if rng.range(0, 4) > 0 {
                    fixed.push(PlayerSet::singleton(n, i));
                }
            }
            for _ in 0..rng.range(0, 2) {
                fixed.push(set_of(n, rng.range(1, (1 << n) - 2) as usize));
            }
            let (span, _) = independent_basis(&fixed, n);
            let in_lp: Vec<PlayerSet> = (0..rng.range(0, 4))
                .map(|_| set_of(n, rng.range(1, (1 << n) - 2) as usize))
                .collect();
            let in_span = |s: &PlayerSet| span.contains(&s.indicator());
            let skip = |s: &PlayerSet| in_lp.contains(s) || in_span(s);
            let bound = rng.next_f64() * 0.8 - 0.7;
            let excess = |s: &PlayerSet| game.value(s) - s.sum(&x);
            let above: Vec<PlayerSet> = (1..(1usize << n) - 1)
                .map(|bits| set_of(n, bits))
                .filter(|s| excess(s) > bound)
                .collect();
            let expected: Vec<&PlayerSet> = above.iter().filter(|s| !skip(s)).collect();
            let skipped_above = above.len() - expected.len();
            // 違反する提携のうち最大の超過より大きい超過の、読み飛ばす提携の数 (超過の順でそれより前に並ぶ)
            let best = expected
                .iter()
                .map(|s| excess(s))
                .fold(f64::NEG_INFINITY, f64::max);
            let skipped_before_best = above.iter().filter(|s| skip(s) && excess(s) > best).count();
            let limit = rng.range(1, 8) as usize;
            for (slot, budget) in [ENUMERATION_BUDGET, 8].into_iter().enumerate() {
                // 超過の順で上位 budget 個が全て読み飛ばす提携なら、素数の探索に進む。
                if expected.is_empty() && skipped_above >= budget {
                    beyond[slot].0 += 1;
                } else if !expected.is_empty() && skipped_before_best >= budget {
                    beyond[slot].1 += 1;
                }
                let found =
                    game.search_outside_span_with_budget(&x, bound, &fixed, &skip, limit, budget);
                let label = format!("case={case} budget={budget} w={weights:?} q={quota}");
                assert_eq!(found.is_empty(), expected.is_empty(), "{label}");
                assert!(found.len() <= limit, "{label}");
                for (s, e) in &found {
                    assert!(expected.contains(&s), "{label}: {s}");
                    assert!((e - excess(s)).abs() < 1e-12, "{label}: {s}");
                }
            }
        }
        // 両方の上限で、素数の探索に進んで違反がないと確かめる場合と、違反を見つける場合を十分に含む。
        assert!(
            beyond.iter().all(|&(none, some)| none >= 10 && some >= 10),
            "{beyond:?}"
        );
    }

    /// 基底がどの素数でも F_p 上で従属なら、素数の探索は判定できないことを `None` で伝える
    /// (呼び出し側は超過の順の探索に戻る)。実数上で従属な基底 (同じ提携を 2 回) で確かめる。
    #[test]
    fn search_by_primes_reports_dependent_basis() {
        let n = 6;
        let game = WeightedVotingGame::new(vec![3, 1, 1, 1, 1, 1], 5).unwrap();
        let x = vec![0.3, 0.1, 0.1, 0.1, 0.1, 0.1];
        let pair = set_of(n, 0b11);
        let basis = [PlayerSet::grand(n), pair.clone(), pair];
        assert!(
            game.search_by_primes(&x, -1.0, &basis, &|_| true, 4, true)
                .is_none()
        );
    }

    /// 拒否権 3 人のゲームで、確定済みの提携が超過の上位を占める状況 (仁の 2 段目) を作り、探索が
    /// 列挙の上限を超えて素数の探索に進んでも、確定済みの空間に入る提携を返さないこと。
    #[test]
    fn veto_game_skips_fixed_coalitions_beyond_budget() {
        let n = 20;
        let mut weights = vec![1u64; n];
        weights[..3].fill(1000);
        let game = WeightedVotingGame::new(weights, 3000 + 9).unwrap();
        let x: Vec<f64> = (0..n)
            .map(|i| if i < 3 { 1.0 / 3.0 } else { 0.0 })
            .collect();
        // 小さいプレイヤーの 1 人提携と全体提携を確定済みにすると、超過 0 の勝利提携 (拒否権の 3 人を含み、
        // 小さいプレイヤーの任意の部分集合を加えたもの) は全て空間に入る。
        let mut fixed = vec![PlayerSet::grand(n)];
        fixed.extend((3..n).map(|i| PlayerSet::singleton(n, i)));
        let (span, _) = independent_basis(&fixed, n);
        let skip = |s: &PlayerSet| span.contains(&s.indicator());
        // 超過 0 の提携は上限より多い (2^17 個) が、どれも確定済みなので、超過が -0.3 を超える違反はない。
        assert!(
            game.search_outside_span(&x, -0.3, &fixed, &skip, 8)
                .is_empty()
        );
        // 拒否権のプレイヤー 1 人 (と小さいプレイヤー) の提携は超過 -1/3 で、空間に入らない。
        let found = game.search_outside_span(&x, -0.5, &fixed, &skip, 8);
        assert!(!found.is_empty());
        for (s, e) in &found {
            assert!(!skip(s), "{s}");
            assert!((e + 1.0 / 3.0).abs() < 1e-12, "{s} {e}");
        }
    }

    #[test]
    fn matches_explicit_voting_game() {
        let weights = vec![4, 1, 3, 2, 2, 5, 1];
        let quota = 10;
        let oracle = WeightedVotingGame::new(weights.clone(), quota).unwrap();
        let explicit = generators::weighted_voting(
            &weights.iter().map(|w| *w as f64).collect::<Vec<_>>(),
            quota as f64,
        )
        .unwrap();
        let x = [0.2, -0.1, 0.15, 0.1, 0.05, 0.3, 0.3];
        let from_oracle: Vec<(PlayerSet, f64)> = oracle.excess_order(&x).collect();
        let from_explicit: Vec<(PlayerSet, f64)> = explicit.excess_order(&x).collect();
        // 全 126 個の真部分提携を、k を倍にしながら全て返す。
        assert_eq!(from_oracle.len(), 126);
        for ((_, a), (_, b)) in from_oracle.iter().zip(&from_explicit) {
            assert!((a - b).abs() < 1e-12);
        }
        for (set, e) in &from_oracle {
            assert!((oracle.value(set) - set.sum(&x) - e).abs() < 1e-12);
        }
    }
}
