//! 逐次 LP (Kopelowitz 方式) による仁・プレ仁・最小コアの計算。
//!
//! 第 k 段では、超過がまだ確定していない提携の最大超過 `t` を最小化する:
//!
//! ```text
//! min t  s.t.  v(S) - x(S) <= t   (未確定の S)
//!              x(S) = c_S         (確定済みの S。N を含む)
//!              x_i >= v({i})      (仁の場合のみ)
//! ```
//!
//! 最適値 `t*` を達成する提携のうち、最適解集合全体で超過が `t*` に固定されるものを
//! 「確定」させる。確定判定は、最適解集合の上で `x(S)` を最大化する LP で行う。
//! 確定した提携の特性ベクトルが張る空間が `R^n` 全体になった時点で配分は一意に決まる。
//! 各段で次元が 1 以上増えるため、段数は高々 `n - 1`。
//!
//! 超過の比較には許容誤差を使うので、超過が `t*` と許容誤差以内で等しいだけの提携も確定しうる。
//! 確定値 `c_S` は、その段の LP の解 `x` での `x(S)` にする (`v(S) - t*` にはしない)。
//! `x` が全ての確定の等式を同時に満たすので、以後の LP は常に実行可能点 `x` を持つ。
//!
//! LP は絶対誤差で判定するので、値の大きさによらず同じ精度で解けるよう、ゲームの値を 2 のべき乗で割って
//! 最大絶対値を `[1, 2)` にそろえてから解き、結果を元の大きさに戻す (許容誤差も同じ倍率で割る)。
//! 倍率が 2 のべき乗なら、`c` 倍したゲームの結果はちょうど `c` 倍になる。
//!
//! [`Method::ConstraintGeneration`] (既定) では、`2^n` 個の不等式を最初から全て入れず、
//! 解が違反する提携だけを行として追加し、前回の解から解き直す(制約生成)。
//! 一度追加した提携は以降の LP でも初期の行として使う。
//!
//! 同じ仁を別の手法で求めるサブモジュール:
//!
//! | モジュール | 対象 | 手法 |
//! |---|---|---|
//! | (このモジュール) | 全提携の表 ([`ExplicitGame`]) | 逐次 LP + 制約生成 |
//! | [`exact`] | 有理数のゲーム (10 人まで) | 有理数の逐次 LP。許容誤差を使わない |
//! | [`oracle`] | 違反する提携を返せるゲーム | 逐次 LP + オラクルによる制約生成 |
//! | [`convex`] | 凸ゲーム | カーネル = 仁 を使う transfer scheme |
//! | [`sampled`] | 値を返すだけのゲーム | サンプリングした提携だけの逐次 LP (近似) |
//! | [`variants`] | 全提携の表 | per capita 仁・比例仁・modiclus (不満の定義を変えた仁) |
//!
//! ゲームの型からこれらの手法を自動で選ぶには [`crate::auto`] を使う。

pub mod convex;
pub mod exact;
pub mod oracle;
pub mod sampled;
pub mod variants;

use microlp::{Problem, Solution, Variable};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::{block_lower_bounds, coalition_sums, excess_table};
use crate::game::exact::{ExactGame, to_rational};
use crate::game::scale::check_tolerance;
use crate::game::{Coalition, ExplicitGame, default_tolerance};
use crate::linalg::Span;
use crate::lp::{self, Cmp, Counter};
use crate::solution::Guarantee;

/// LP に不等式を入れる方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Method {
    /// 未確定の提携を全て最初から LP の行に入れる。
    Full,
    /// 解が違反する提携だけを行として追加する。
    #[default]
    ConstraintGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Options {
    pub domain: Domain,
    /// 超過の比較に使う許容誤差 (ゲームの値と同じ単位)。既定は [`default_tolerance`]。
    ///
    /// 有限で 0 以上の値にする。NaN・無限大・負の値を渡すと、解く前に [`Error::InvalidArgument`] を返す。
    ///
    /// 配分集合は `sum v({i}) <= v(N) + tolerance` なら空とみなさない。和が `v(N)` を許容誤差以内で
    /// 超えるときは、超えた分を人数で等分して下限 `v({i})` を下げた 1 点を配分集合とする。
    pub tolerance: f64,
    pub method: Method,
}

impl Options {
    pub fn new(game: &ExplicitGame, domain: Domain) -> Options {
        Options {
            domain,
            tolerance: default_tolerance(game),
            method: Method::default(),
        }
    }

    /// 値を `divisor` で割ったゲームに合わせた設定 (許容誤差も割る)。
    ///
    /// 許容誤差が有限で 0 以上でないとき、または割った許容誤差が有限でない
    /// (ゲームの値に比べて大きすぎる) ときは [`Error::InvalidArgument`]。
    fn divided_by(self, divisor: f64) -> Result<Options> {
        let tolerance = check_tolerance(self.tolerance)? / divisor;
        if !tolerance.is_finite() {
            return Err(Error::InvalidArgument(format!(
                "許容誤差 {} がゲームの値の大きさに比べて大きすぎる",
                self.tolerance
            )));
        }
        Ok(Options { tolerance, ..self })
    }
}

/// 元の大きさに戻した値が全て有限かを確かめる。f64 で表せない値があれば [`Error::Numerical`]。
pub(crate) fn check_representable(values: &[f64], what: &str) -> Result<()> {
    match values.iter().find(|v| !v.is_finite()) {
        None => Ok(()),
        Some(value) => Err(Error::Numerical(format!(
            "{what}を元の大きさに戻すと f64 で表せない ({value})"
        ))),
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct NucleolusResult {
    pub(crate) allocation: Vec<f64>,
    pub(crate) levels: Vec<f64>,
    pub(crate) lp_solves: usize,
    pub(crate) rows_added: usize,
    pub(crate) guarantee: Guarantee,
}

impl NucleolusResult {
    pub fn allocation(&self) -> &[f64] {
        &self.allocation
    }

    pub fn into_allocation(self) -> Vec<f64> {
        self.allocation
    }

    /// 各段の最適な最大超過 `t*`(非増加列)。
    ///
    /// LP を解かずに配分が決まった場合 (1 人ゲーム、配分集合が 1 点) は空。
    pub fn levels(&self) -> &[f64] {
        &self.levels
    }

    /// 最初から組み立てて解いた LP の数。
    pub fn lp_solves(&self) -> usize {
        self.lp_solves
    }

    /// 制約生成で後から追加した行の数(追加のたびに前回の解から解き直す)。
    pub fn rows_added(&self) -> usize {
        self.rows_added
    }

    /// 逐次 LP では [`Guarantee::Exact`]。
    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }

    /// 正規化したゲームで求めた結果を、元の大きさ (`factor` 倍) に戻す。
    ///
    /// 戻した配分か最大超過が f64 で表せない (有限でない) ときは [`Error::Numerical`]。
    pub(crate) fn scaled_by(mut self, factor: f64) -> Result<NucleolusResult> {
        for value in self.allocation.iter_mut().chain(self.levels.iter_mut()) {
            *value *= factor;
        }
        check_representable(&self.allocation, "配分")?;
        check_representable(&self.levels, "最大超過")?;
        Ok(self)
    }

    /// 手法によらない共通の結果にする (保証はこの結果のものを引き継ぐ)。
    pub(crate) fn into_solution(
        self,
        domain: Domain,
        method: &'static str,
    ) -> crate::solution::Solution {
        crate::solution::Solution {
            domain,
            allocation: self.allocation,
            guarantee: self.guarantee,
            method,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct LeastCore {
    pub(crate) epsilon: f64,
    pub(crate) allocation: Vec<f64>,
    pub(crate) guarantee: Guarantee,
}

impl LeastCore {
    /// 最小コアの `epsilon`(空でない真部分提携の最大超過の最小値)。負ならコアは内部を持つ。
    ///
    /// 1 人ゲームには真部分提携がないので、最大超過は空集合の最大値として `f64::NEG_INFINITY` とする
    /// (どの `epsilon` でも `epsilon`-コアは `{v(N)}` で、コアは空でない)。
    pub fn epsilon(&self) -> f64 {
        self.epsilon
    }

    /// 最小コアに属する配分の 1 つ。
    pub fn allocation(&self) -> &[f64] {
        &self.allocation
    }

    pub fn into_allocation(self) -> Vec<f64> {
        self.allocation
    }

    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }
}

/// 仁(配分集合上で超過ベクトルを辞書式に最小化する点)。
pub fn nucleolus(game: &ExplicitGame) -> Result<NucleolusResult> {
    nucleolus_with(game, Options::new(game, Domain::Imputation))
}

/// プレ仁(原配分集合上で超過ベクトルを辞書式に最小化する点)。
pub fn prenucleolus(game: &ExplicitGame) -> Result<NucleolusResult> {
    nucleolus_with(game, Options::new(game, Domain::Preimputation))
}

/// 設定 `options` で仁 (`Domain::Imputation`) またはプレ仁を求める。
///
/// # Errors
///
/// - `options.tolerance` が有限で 0 以上でない: [`Error::InvalidArgument`]
/// - 配分集合が空 (`Domain::Imputation`): [`Error::EmptyImputationSet`]
/// - LP が数値的に進まない、または結果 (配分・各段の最大超過) が f64 で表せない: [`Error::Numerical`]
pub fn nucleolus_with(game: &ExplicitGame, options: Options) -> Result<NucleolusResult> {
    let (normalized, divisor) = game.normalized();
    let options = options.divided_by(divisor)?;
    let result = Sequential::new(&normalized, options)?.run()?;
    result.scaled_by(divisor)
}

/// 提携構造 `blocks` (N の分割) のもとでの仁 (`Domain::Imputation`) またはプレ仁。
///
/// 各ブロック `B` で `x(B) = v(B)` を課し、その中で超過ベクトルを辞書式に最小化する
/// (Aumann & Drèze 1974 の提携構造つきの仁)。ブロックが N だけなら [`nucleolus_with`] と同じ。
/// 分割になっているかは呼び出し側 ([`crate::partition::CoalitionStructure`]) で確かめる。
pub(crate) fn nucleolus_with_structure(
    game: &ExplicitGame,
    blocks: &[Coalition],
    options: Options,
) -> Result<NucleolusResult> {
    let (normalized, divisor) = game.normalized();
    let options = options.divided_by(divisor)?;
    let lower = block_lower_bounds(&normalized, blocks, options.domain, options.tolerance)?;
    if blocks.iter().all(|block| block.len() == 1) {
        // 全員が単独なら x_i = v({i}) に決まる (逐次 LP は 1 段も解かない)。
        return Ok(NucleolusResult {
            allocation: game.singleton_values(),
            levels: Vec::new(),
            lp_solves: 0,
            rows_added: 0,
            guarantee: Guarantee::Exact,
        });
    }
    let result = Sequential::with_blocks(&normalized, options, blocks, lower).run()?;
    result.scaled_by(divisor)
}

/// 最小コア: 最大超過を最小化する LP を 1 回解く。
///
/// 1 人ゲームでは配分は `v(N)` だけで、`epsilon` は `f64::NEG_INFINITY` ([`LeastCore::epsilon`])。
/// `epsilon` か配分が f64 で表せない (値が f64 の上限に近く、有限でなくなる) ときは [`Error::Numerical`]。
pub fn least_core(game: &ExplicitGame, domain: Domain) -> Result<LeastCore> {
    let (normalized, divisor) = game.normalized();
    let options = Options::new(game, domain).divided_by(divisor)?;
    let Some((epsilon, allocation)) = normalized_least_core(&normalized, options)? else {
        return Ok(LeastCore {
            epsilon: f64::NEG_INFINITY,
            allocation: vec![game.value(game.grand())],
            guarantee: Guarantee::Exact,
        });
    };
    let epsilon = epsilon * divisor;
    let allocation: Vec<f64> = allocation.iter().map(|v| v * divisor).collect();
    check_representable(&[epsilon], "最小コアの epsilon ")?;
    check_representable(&allocation, "配分")?;
    Ok(LeastCore {
        epsilon,
        allocation,
        guarantee: Guarantee::Exact,
    })
}

/// 正規化したゲームで最大超過を最小化し、`(epsilon, 配分)` を返す。1 人ゲーム (真部分提携がない) は `None`。
fn normalized_least_core(
    normalized: &ExplicitGame,
    options: Options,
) -> Result<Option<(f64, Vec<f64>)>> {
    let mut solver = Sequential::new(normalized, options)?;
    if solver.n == 1 {
        return Ok(None);
    }
    solver.minimize_max_excess().map(Some)
}

/// [`has_nonempty_core`] が、最小コアの `epsilon` の符号を有理数の LP で厳密に判定する人数の上限。
pub const MAX_EXACT_CORE_PLAYERS: usize = 9;

/// コアが空でないか (原配分集合の上の最小コアの `epsilon` ([`least_core`]) が 0 以下か)。
///
/// 判定の方法:
///
/// 1. 浮動小数点の LP で `epsilon` を求める。許容誤差 `tol` ([`default_tolerance`]) の帯の外
///    (`epsilon < -tol` または `epsilon > tol`) なら、その符号で決める。
/// 2. 帯の中 (`|epsilon| <= tol`) では、人数が [`MAX_EXACT_CORE_PLAYERS`] 以下なら、ゲームの値を
///    f64 の値どおりの有理数として `epsilon` を有理数の LP ([`exact`]) で厳密に求め、
///    入力の丸めの大きさ `4 n ε max|v|` (`ε` は [`f64::EPSILON`]) 以下かで決める。
///    `epsilon` がちょうど 0 のゲーム (手袋ゲーム、コアが 1 点のゲームなど) はこの場合に当たる。
///    0 ではなく丸めの大きさと比べるのは、値を 10 進数の和などで作ると、有理数では 0 のはずの
///    `epsilon` が丸めで正になるからである (`v = (0.1, 0.2, 0.3)` の 2 人ゲームなど)。
/// 3. [`MAX_EXACT_CORE_PLAYERS`] より多い人数では、帯の中を全て「空でない」とする
///    (`epsilon` が `(0, tol]` に入る、コアが空のゲームでも true を返す)。
///
/// したがって [`MAX_EXACT_CORE_PLAYERS`] 人以下では、`epsilon` が丸めの大きさを超えて正のゲーム
/// (値が大きい整数で `epsilon = 1/3` のゲームなど) を、値の大きさによらず空と判定する。
pub fn has_nonempty_core(game: &ExplicitGame) -> Result<bool> {
    let (normalized, divisor) = game.normalized();
    // コアが空かどうかは値の大きさによらないので、正規化したゲームの epsilon の符号で判定する
    // (元の大きさに戻すと f64 で表せない値のゲームでも判定できる)。
    let options = Options::new(game, Domain::Preimputation).divided_by(divisor)?;
    let Some((epsilon, x)) = normalized_least_core(&normalized, options)? else {
        // 1 人ゲームのコアは {v(N)}。
        return Ok(true);
    };
    let tolerance = options.tolerance;
    if epsilon < -tolerance {
        return Ok(true);
    }
    if epsilon > tolerance {
        return Ok(false);
    }
    let n = game.players();
    if n > MAX_EXACT_CORE_PLAYERS {
        return Ok(true);
    }
    // 浮動小数点の LP で最大超過を達成した提携のうち、特性ベクトルが 1 次独立なものを最初から行に入れ、
    // 有理数の LP の解き直しを減らす (退化したゲームでは達成する提携が多く、全てを入れると LP が重くなる)。
    let excess = excess_table(&normalized, &x);
    let grand = game.grand();
    let mut span = Span::new(n);
    span.insert_coalition(grand);
    let mut seed = Vec::new();
    for (mask, value) in excess.iter().enumerate().take(grand.index()).skip(1) {
        let coalition = Coalition(mask as u64);
        if (value - epsilon).abs() <= tolerance && !span.contains_coalition(coalition) {
            span.insert_coalition(coalition);
            seed.push(coalition);
        }
    }
    let exact_game = ExactGame::from_explicit(game)?;
    let epsilon = exact::least_core_epsilon(&exact_game, &seed)?;
    // 値は浮動小数点数なので、入力の丸めの大きさに収まる正の epsilon は 0 とみなす。
    let rounding = 4.0 * n as f64 * f64::EPSILON * game.max_abs_value();
    Ok(epsilon <= to_rational(rounding)?)
}

/// 提携 `S` の行の形。
#[derive(Clone, Copy)]
enum RowForm {
    /// `x(S) + t >= v(S)`(最大超過の最小化)。
    WithLevel(Variable),
    /// `x(S) >= v(S) - bound`(最大超過が `bound` 以下の配分に限る)。
    AtMost(f64),
}

struct Sequential<'a> {
    game: &'a ExplicitGame,
    options: Options,
    n: usize,
    /// 各プレイヤーの下限 (`Domain::Imputation` のみ。[`block_lower_bounds`])。
    lower: Option<Vec<f64>>,
    /// 提携構造のブロック (提携構造がなければ全体提携だけ)。
    blocks: Vec<Coalition>,
    /// 超過が確定し、LP の不等式から外した提携(ビット順、長さ `2^n`)。
    determined: Vec<bool>,
    /// 確定した提携と `x(S)` の値。
    fixed: Vec<(Coalition, f64)>,
    span: Span,
    counter: Counter,
    rows_added: usize,
    /// 制約生成で使う行の候補。一度追加した提携を覚えておく。
    pool: Vec<Coalition>,
    in_pool: Vec<bool>,
}

impl<'a> Sequential<'a> {
    fn new(game: &'a ExplicitGame, options: Options) -> Result<Sequential<'a>> {
        let blocks = [game.grand()];
        let lower = block_lower_bounds(game, &blocks, options.domain, options.tolerance)?;
        Ok(Sequential::with_blocks(game, options, &blocks, lower))
    }

    /// 提携構造 `blocks` (N の分割) の各ブロック `B` で `x(B) = v(B)` を課す。
    fn with_blocks(
        game: &'a ExplicitGame,
        options: Options,
        blocks: &[Coalition],
        lower: Option<Vec<f64>>,
    ) -> Sequential<'a> {
        let n = game.players();
        let grand = game.grand();
        let mut determined = vec![false; 1 << n];
        determined[0] = true;
        let mut span = Span::new(n);
        let mut fixed = Vec::with_capacity(blocks.len());
        for &block in blocks {
            determined[block.index()] = true;
            span.insert_coalition(block);
            fixed.push((block, game.value(block)));
        }
        let mut solver = Sequential {
            game,
            options,
            n,
            lower,
            blocks: blocks.to_vec(),
            determined,
            fixed,
            span,
            counter: Counter::default(),
            rows_added: 0,
            pool: Vec::new(),
            in_pool: vec![false; 1 << n],
        };
        // 1 人提携とその補集合があれば、最初の LP は有界になる。
        for i in 0..n {
            let single = Coalition::singleton(i);
            solver.remember(single);
            solver.remember(Coalition(grand.0 & !single.0));
        }
        solver.mark_span_determined();
        solver
    }

    fn run(&mut self) -> Result<NucleolusResult> {
        if self.n == 1 {
            return Ok(NucleolusResult {
                allocation: vec![self.game.value(self.game.grand())],
                levels: Vec::new(),
                lp_solves: 0,
                rows_added: 0,
                guarantee: Guarantee::Exact,
            });
        }
        self.pin_single_point_blocks();
        if self.span.is_full() {
            // 全てのブロックで配分が 1 点に決まった。LP は解かない。
            return Ok(NucleolusResult {
                allocation: self
                    .lower
                    .clone()
                    .expect("1 点に決まるのは配分集合の場合だけ"),
                levels: Vec::new(),
                lp_solves: 0,
                rows_added: 0,
                guarantee: Guarantee::Exact,
            });
        }
        let mut levels = Vec::new();
        let mut allocation = Vec::new();
        while !self.span.is_full() {
            let (level, x) = self.minimize_max_excess()?;
            levels.push(level);
            self.fix_tight_coalitions(level, &x)?;
            self.mark_span_determined();
            allocation = x;
        }
        Ok(NucleolusResult {
            allocation,
            levels,
            lp_solves: self.counter.solves,
            rows_added: self.rows_added,
            guarantee: Guarantee::Exact,
        })
    }

    /// 下限の和が `v(B)` 以上のブロック `B` (許容誤差以内で空の配分集合を含む) では、配分は 1 点
    /// `x_i = lower_i` に決まる。その 1 人提携を確定させる。
    ///
    /// 超過の差が許容誤差より小さくなるので、LP の段で確定させると誤った値で確定しうるため、先に確定させる。
    /// 和の等式と矛盾しないよう、ブロックの最後の 1 人は和の等式に任せる。
    fn pin_single_point_blocks(&mut self) {
        let Some(lower) = self.lower.clone() else {
            return;
        };
        for block in self.blocks.clone() {
            let singles: f64 = block
                .members()
                .map(|i| self.game.value(Coalition::singleton(i)))
                .sum();
            if block.len() < 2 || singles < self.game.value(block) {
                continue;
            }
            for i in block.members().take(block.len() - 1) {
                let single = Coalition::singleton(i);
                self.fixed.push((single, lower[i]));
                self.span.insert_coalition(single);
            }
        }
        self.mark_span_determined();
    }

    fn tolerance(&self) -> f64 {
        self.options.tolerance
    }

    fn remember(&mut self, coalition: Coalition) {
        if !coalition.is_empty() && !self.in_pool[coalition.index()] {
            self.in_pool[coalition.index()] = true;
            self.pool.push(coalition);
        }
    }

    fn is_open(&self, coalition: Coalition) -> bool {
        !self.determined[coalition.index()]
    }

    fn open_coalitions(&self) -> impl Iterator<Item = Coalition> + '_ {
        self.determined
            .iter()
            .enumerate()
            .filter(|(_, determined)| !**determined)
            .map(|(mask, _)| Coalition(mask as u64))
    }

    fn mark_span_determined(&mut self) {
        for mask in 1..self.determined.len() {
            if !self.determined[mask] && self.span.contains_coalition(Coalition(mask as u64)) {
                self.determined[mask] = true;
            }
        }
    }

    fn add_players(&self, problem: &mut Problem, objective: Option<Coalition>) -> Vec<Variable> {
        (0..self.n)
            .map(|i| {
                let lower = self
                    .lower
                    .as_ref()
                    .map_or(f64::NEG_INFINITY, |lower| lower[i]);
                let coefficient = match objective {
                    Some(s) if s.contains(i) => 1.0,
                    _ => 0.0,
                };
                problem.add_var(coefficient, (lower, f64::INFINITY))
            })
            .collect()
    }

    fn add_fixed(&self, problem: &mut Problem, x: &[Variable]) {
        for &(coalition, value) in &self.fixed {
            lp::add(problem, lp::coalition_expr(x, coalition), Cmp::Eq, value);
        }
    }

    fn row(
        &self,
        x: &[Variable],
        form: RowForm,
        coalition: Coalition,
    ) -> (Vec<(Variable, f64)>, f64) {
        let mut terms = lp::coalition_expr(x, coalition);
        let value = self.game.value(coalition);
        match form {
            RowForm::WithLevel(t) => {
                terms.push((t, 1.0));
                (terms, value)
            }
            RowForm::AtMost(bound) => (terms, value - bound),
        }
    }

    /// 行を入れて LP を解く。制約生成では、違反する提携を追加しながら解き直す。
    ///
    /// `build` は変数・目的関数・等式制約だけを入れた LP を作る。
    /// `seed` は最初から入れておく提携(制約生成のときのみ使う)。
    fn solve_rows(
        &mut self,
        build: impl Fn(&Self) -> (Problem, Vec<Variable>, RowForm),
        seed: &[Coalition],
    ) -> Result<(Solution, Vec<Variable>, RowForm)> {
        if self.options.method == Method::ConstraintGeneration
            && let Some(solved) = self.solve_lazily(&build, seed)?
        {
            return Ok(solved);
        }
        // 全ての行を入れて解く(制約生成で有界にならなかった場合もここに来る)。
        let (mut problem, x, form) = build(self);
        for coalition in self.open_coalitions() {
            let (terms, rhs) = self.row(&x, form, coalition);
            lp::add(&mut problem, terms, Cmp::Ge, rhs);
        }
        let solution = self
            .counter
            .solve(&problem)?
            .ok_or_else(|| Error::Numerical("LP が実行不可能".into()))?;
        Ok((solution, x, form))
    }

    /// 制約生成で解く。最初の LP が非有界なら `None` を返す。
    fn solve_lazily(
        &mut self,
        build: &impl Fn(&Self) -> (Problem, Vec<Variable>, RowForm),
        seed: &[Coalition],
    ) -> Result<Option<(Solution, Vec<Variable>, RowForm)>> {
        for &coalition in seed {
            self.remember(coalition);
        }
        let (mut problem, x, form) = build(self);
        let mut in_lp = vec![false; 1 << self.n];
        for &coalition in &self.pool {
            if self.is_open(coalition) {
                let (terms, rhs) = self.row(&x, form, coalition);
                lp::add(&mut problem, terms, Cmp::Ge, rhs);
                in_lp[coalition.index()] = true;
            }
        }
        let mut solution = match self.counter.solve_bounded(&problem)? {
            Some(solution) => solution,
            None => return Ok(None),
        };
        // 違反量がこれを超える提携を追加する。
        let slack = 1e-3 * self.tolerance();
        let batch = 4 * self.n;
        loop {
            let values: Vec<f64> = x.iter().map(|&var| solution[var]).collect();
            let sums = coalition_sums(&values);
            let bound = match form {
                RowForm::WithLevel(t) => solution[t],
                RowForm::AtMost(bound) => bound,
            };
            let mut violated: Vec<(f64, Coalition)> = self
                .open_coalitions()
                .filter(|s| !in_lp[s.index()])
                .filter_map(|s| {
                    let violation = self.game.value(s) - sums[s.index()] - bound;
                    (violation > slack).then_some((violation, s))
                })
                .collect();
            if violated.is_empty() {
                return Ok(Some((solution, x, form)));
            }
            violated.sort_by(|a, b| b.0.total_cmp(&a.0));
            for &(_, coalition) in violated.iter().take(batch) {
                let (terms, rhs) = self.row(&x, form, coalition);
                solution = lp::add_to_solution(solution, terms, Cmp::Ge, rhs)?;
                in_lp[coalition.index()] = true;
                self.remember(coalition);
                self.rows_added += 1;
            }
        }
    }

    /// 未確定の提携の最大超過を最小化し、`(t*, x*)` を返す。
    fn minimize_max_excess(&mut self) -> Result<(f64, Vec<f64>)> {
        let build = |this: &Self| {
            let mut problem = lp::minimize();
            let x = this.add_players(&mut problem, None);
            let t = problem.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
            this.add_fixed(&mut problem, &x);
            (problem, x, RowForm::WithLevel(t))
        };
        let (solution, x, form) = self.solve_rows(build, &[])?;
        let RowForm::WithLevel(t) = form else {
            unreachable!("最大超過の最小化は WithLevel の行を使う")
        };
        let allocation = x.iter().map(|&var| solution[var]).collect();
        Ok((solution[t], allocation))
    }

    /// 最大超過が `level` 以下の配分の中で `x(target)` を最大化する。
    fn maximize_coalition(&mut self, target: Coalition, level: f64) -> Result<(f64, Vec<f64>)> {
        // 最適値の丸め誤差で実行不可能にならないよう、許容誤差の 1% だけ緩める。
        let bound = level + 0.01 * self.tolerance();
        let build = |this: &Self| {
            let mut problem = lp::maximize();
            let x = this.add_players(&mut problem, Some(target));
            this.add_fixed(&mut problem, &x);
            (problem, x, RowForm::AtMost(bound))
        };
        // 補集合の行があれば x(target) は上から抑えられる。
        let complement = Coalition(self.game.grand().0 & !target.0);
        let (solution, x, _) = self.solve_rows(build, &[target, complement])?;
        let allocation: Vec<f64> = x.iter().map(|&var| solution[var]).collect();
        Ok((solution.objective(), allocation))
    }

    /// 段の最適解 `x` (最大超過 `level`) で超過が `level` と許容誤差以内で等しい提携のうち、
    /// 最大超過が `level` 以下の配分の上で `x(S)` がそれ以上大きくなれないものを確定させる。
    ///
    /// 確定値は `v(S) - level` ではなく `x(S)` にする。許容誤差で候補にした提携は、最適解集合の上で
    /// `x(S)` が `v(S) - level` に届かないことがある。`x(S)` で確定させれば、`x` が全ての確定の等式を
    /// 同時に満たすので、`x` は同じ段の以後の LP ([`Self::maximize_coalition`]) と次の段の LP の
    /// 実行可能点であり続け、確定の等式どうしも最大超過の制約とも矛盾しない。
    /// 本当に超過が `level` に固定される提携では、`x(S)` は `v(S) - level` と丸めの範囲で等しい。
    fn fix_tight_coalitions(&mut self, level: f64, x: &[f64]) -> Result<()> {
        let sums = coalition_sums(x);
        let tight: Vec<Coalition> = self
            .open_coalitions()
            .filter(|s| {
                let excess = self.game.value(*s) - sums[s.index()];
                (excess - level).abs() <= self.tolerance()
            })
            .collect();
        if tight.is_empty() {
            return Err(Error::Numerical(format!(
                "最大超過 {level} を達成する提携が見つからない"
            )));
        }
        let mut not_fixed = vec![false; tight.len()];
        let mut progressed = false;
        for k in 0..tight.len() {
            if not_fixed[k] || self.span.contains_coalition(tight[k]) {
                continue;
            }
            let target = tight[k];
            let (max_sum, witness) = self.maximize_coalition(target, level)?;
            if max_sum <= self.game.value(target) - level + self.tolerance() {
                self.fixed.push((target, sums[target.index()]));
                self.span.insert_coalition(target);
                progressed = true;
                if self.span.is_full() {
                    break;
                }
            } else {
                // witness では超過が level 未満になる提携も確定していない。
                let witness_sums = coalition_sums(&witness);
                for (later, flag) in tight.iter().zip(not_fixed.iter_mut()).skip(k + 1) {
                    let excess = self.game.value(*later) - witness_sums[later.index()];
                    if excess < level - self.tolerance() {
                        *flag = true;
                    }
                }
            }
        }
        if !progressed {
            return Err(Error::Numerical(format!(
                "最大超過 {level} の段で確定する提携が見つからない(許容誤差 {} を見直す)",
                self.tolerance()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators;

    fn assert_close(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-6, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn detects_empty_core() {
        assert!(has_nonempty_core(&generators::random_convex(4, 1).unwrap()).unwrap());
        // 3 人多数決: 優加法的だが、コアは空。
        let majority = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        assert!(!has_nonempty_core(&majority).unwrap());
    }

    #[test]
    fn symmetric_game_splits_equally() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 60.0, 60.0, 60.0, 72.0]).unwrap();
        assert_close(&nucleolus(&game).unwrap().allocation, &[24.0, 24.0, 24.0]);
        assert_close(
            &prenucleolus(&game).unwrap().allocation,
            &[24.0, 24.0, 24.0],
        );
    }

    #[test]
    fn pair_game() {
        // v(12)=4, v(N)=4、他は 0。仁は (2, 2, 0)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let result = nucleolus(&game).unwrap();
        assert_close(&result.allocation, &[2.0, 2.0, 0.0]);
        assert!((result.levels[0] - 0.0).abs() < 1e-9);
    }

    #[test]
    fn bankruptcy_nucleolus_is_talmud_rule() {
        // Aumann-Maschler (1985): 請求 (100, 200, 300) に対するタルムードの配分。
        let claims = [100.0, 200.0, 300.0];
        for (estate, expected) in [
            (100.0, [100.0 / 3.0, 100.0 / 3.0, 100.0 / 3.0]),
            (200.0, [50.0, 75.0, 75.0]),
            (300.0, [50.0, 100.0, 150.0]),
        ] {
            let game = generators::bankruptcy(estate, &claims).unwrap();
            assert_close(&nucleolus(&game).unwrap().allocation, &expected);
        }
    }

    #[test]
    fn prenucleolus_can_leave_imputation_set() {
        // v(3)=5, v(12)=10, v(13)=v(23)=5, v(N)=12。
        // プレ仁は e(12) = x_3 - 2 と e(3) = 5 - x_3 を釣り合わせて x_3 = 3.5 < v(3) となる。
        // 仁は x_3 >= 5 の制約で x_3 = 5 に止まる。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 5.0, 10.0, 5.0, 5.0, 12.0]).unwrap();
        assert_close(&prenucleolus(&game).unwrap().allocation, &[4.25, 4.25, 3.5]);
        assert_close(&nucleolus(&game).unwrap().allocation, &[3.5, 3.5, 5.0]);
    }

    #[test]
    fn empty_imputation_set_is_error() {
        let game = ExplicitGame::from_lex(&[3.0, 3.0, 3.0, 6.0, 6.0, 6.0, 8.0]).unwrap();
        assert_eq!(nucleolus(&game), Err(Error::EmptyImputationSet));
        assert!(prenucleolus(&game).is_ok());
    }

    #[test]
    fn structure_rejects_invalid_tolerance() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 60.0, 60.0, 60.0, 72.0]).unwrap();
        let blocks = [Coalition(0b011), Coalition(0b100)];
        for tolerance in [f64::NAN, f64::INFINITY, -1.0] {
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let mut options = Options::new(&game, domain);
                options.tolerance = tolerance;
                let result = nucleolus_with_structure(&game, &blocks, options);
                assert!(
                    matches!(result, Err(Error::InvalidArgument(_))),
                    "{tolerance} {domain:?}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn least_core_of_game_with_empty_core() {
        // 3 人多数決ゲーム: コアは空で、最小コアは epsilon = 1/3、配分は (1/3, 1/3, 1/3)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let least = least_core(&game, Domain::Preimputation).unwrap();
        assert!((least.epsilon - 1.0 / 3.0).abs() < 1e-9);
        assert_close(&least.allocation, &[1.0 / 3.0; 3]);
    }
}
