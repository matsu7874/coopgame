//! 特性関数の値が不確かなときの配分の分析。
//!
//! 実務の `v(S)` は見積もりなので誤差を含む。次の 3 つで、誤差が配分に与える影響を調べる。
//!
//! - [`monte_carlo`]: `v` の分布からゲームを繰り返し引いて解き、配分の平均・標準偏差・分位点を求める。
//! - [`IntervalGame`]: 各提携の値を区間 `[下限, 上限]` で与え、区間内の一様分布として [`monte_carlo`] に渡す。
//! - [`influence`]: 提携 `S` の値を少し動かしたときの配分の変化率 `dx / dv(S)`
//!   (中心差分。片側のゲームが解けなければ片側差分)。
//!   仁では、超過の大きい段の提携 (配分を決めている提携) の値だけが効く。[`key_coalitions`] でその提携を選べる。
//!
//! 解き方 (仁・Shapley 値など) は関数として渡す。関数は配分 (`Vec<f64>`) か、保証つきの結果
//! ([`NucleolusResult`]・[`Solution`]) を返す。保証つきの結果を返すと、標本・差分で解いた解の保証の内訳
//! ([`GuaranteeCounts`]) を結果に残し、[`influence`] は差分の幅が解の許容誤差に埋もれていないかを確かめる。
//! 名前で選ぶ [`solver`] は、仁・プレ仁を仁の既定の手順 ([`Tolerance::Auto`]) で解く。

use std::fmt;

use crate::Domain;
use crate::analysis::explain::{self, ExplainOptions};
use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame};
use crate::nucleolus::{NucleolusResult, Options, Tolerance};
use crate::rng::SplitMix64;
use crate::solution::{Guarantee, Solution};

/// 解き方が返した 1 つの解: 配分と、解き方が報告した保証・許容誤差。
///
/// 配分 (`Vec<f64>`)・[`NucleolusResult`]・[`Solution`] から `From` で作る。保証はライブラリが計算した結果からだけ付く。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Solved {
    pub(crate) allocation: Vec<f64>,
    pub(crate) guarantee: Option<Guarantee>,
    pub(crate) tolerance: Option<f64>,
    /// LP を解かずに決まった仁の結果か ([`NucleolusResult::lp_solves`] が 0)。
    pub(crate) without_lp: bool,
}

impl Solved {
    pub fn allocation(&self) -> &[f64] {
        &self.allocation
    }

    pub fn into_allocation(self) -> Vec<f64> {
        self.allocation
    }

    /// 解き方が報告した保証。配分だけを返す解き方 (`Vec<f64>`) では `None`。
    pub fn guarantee(&self) -> Option<Guarantee> {
        self.guarantee
    }

    /// 解を求めた逐次 LP の許容誤差 (ゲームの値と同じ単位。[`NucleolusResult::tolerance`])。
    /// 許容誤差を使わない解き方と、報告しない解き方では `None`。
    pub fn tolerance(&self) -> Option<f64> {
        self.tolerance
    }

    /// 解いたゲーム `game` で、配分が許容誤差によらず決まった解の許容誤差を 0 にする。
    ///
    /// LP を解かずに決まった仁 (1 人ゲームと、`sum v({i}) = v(N)` で配分集合が 1 点のゲーム) は、
    /// 許容誤差を使わずに `x = v(N)`・`x_i = v({i})` と決まるので厳密である。
    /// `sum v({i})` が `v(N)` を許容誤差以内で超えて配分集合を 1 点とみなしたゲームは、超えた分を等分した配分で、
    /// 許容誤差の扱いで決まるので、許容誤差をそのまま残す。
    fn settled(mut self, game: &ExplicitGame) -> Solved {
        let n = game.players();
        let exact = n == 1
            || (0..n)
                .map(|i| game.value(Coalition::singleton(i)))
                .sum::<f64>()
                <= game.value(game.grand());
        if self.without_lp && self.guarantee == Some(Guarantee::Exact) && exact {
            self.tolerance = self.tolerance.map(|_| 0.0);
        }
        self
    }
}

impl From<Vec<f64>> for Solved {
    fn from(allocation: Vec<f64>) -> Solved {
        Solved {
            allocation,
            guarantee: None,
            tolerance: None,
            without_lp: false,
        }
    }
}

impl From<NucleolusResult> for Solved {
    fn from(result: NucleolusResult) -> Solved {
        Solved {
            guarantee: Some(result.guarantee),
            tolerance: Some(result.tolerance),
            without_lp: result.lp_solves == 0,
            allocation: result.allocation,
        }
    }
}

impl From<Solution> for Solved {
    fn from(solution: Solution) -> Solved {
        Solved {
            guarantee: Some(solution.guarantee),
            tolerance: None,
            without_lp: false,
            allocation: solution.allocation,
        }
    }
}

/// 標本・差分で解いた解の保証の内訳 (保証ごとの解の数)。
///
/// 表示は `exact:198,approximate:2` の形 (最初に現れた順。保証を報告しない解は `unreported:3`)。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct GuaranteeCounts {
    pub(crate) counts: Vec<(Guarantee, usize)>,
    pub(crate) unreported: usize,
}

impl GuaranteeCounts {
    fn add(&mut self, guarantee: Option<Guarantee>) {
        let Some(guarantee) = guarantee else {
            self.unreported += 1;
            return;
        };
        match self.counts.iter_mut().find(|(g, _)| *g == guarantee) {
            Some((_, count)) => *count += 1,
            None => self.counts.push((guarantee, 1)),
        }
    }

    /// `(保証, 解の数)` (最初に現れた順。数は 1 以上)。
    pub fn iter(&self) -> impl Iterator<Item = (Guarantee, usize)> + '_ {
        self.counts.iter().copied()
    }

    /// 保証が `guarantee` の解の数。
    pub fn count(&self, guarantee: Guarantee) -> usize {
        self.iter()
            .find(|&(g, _)| g == guarantee)
            .map_or(0, |(_, count)| count)
    }

    /// 保証を報告しない解き方 (配分だけを返す関数) で解いた数。
    pub fn unreported(&self) -> usize {
        self.unreported
    }

    /// 正しいと言えない保証 ([`Guarantee::is_reliable`] が偽: [`Guarantee::Approximate`]・[`Guarantee::Assumed`]) の解の数。
    pub fn unreliable(&self) -> usize {
        self.iter()
            .filter(|(g, _)| !g.is_reliable())
            .map(|(_, count)| count)
            .sum()
    }

    /// 解の数の合計 (保証を報告しない解を含む)。
    pub fn total(&self) -> usize {
        self.iter().map(|(_, count)| count).sum::<usize>() + self.unreported
    }
}

impl fmt::Display for GuaranteeCounts {
    /// `exact:198,approximate:2` (保証を報告しない解は `unreported:3`)。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = self.iter().map(|(g, c)| format!("{g}:{c}")).collect();
        if self.unreported > 0 {
            parts.push(format!("unreported:{}", self.unreported));
        }
        f.write_str(&parts.join(","))
    }
}

/// 配分の分布の要約。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Distribution {
    pub(crate) mean: Vec<f64>,
    pub(crate) std: Vec<f64>,
    pub(crate) quantiles: Vec<(f64, Vec<f64>)>,
    pub(crate) samples: usize,
    pub(crate) failures: usize,
    pub(crate) guarantees: GuaranteeCounts,
}

impl Distribution {
    /// プレイヤーごとの平均。
    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    /// プレイヤーごとの標本標準偏差 (解けた標本が 1 つなら 0)。
    pub fn std(&self) -> &[f64] {
        &self.std
    }

    /// `(分位, プレイヤーごとの値)`。5%, 50%, 95% の順。
    pub fn quantiles(&self) -> &[(f64, Vec<f64>)] {
        &self.quantiles
    }

    /// 解けた標本の数。
    pub fn samples(&self) -> usize {
        self.samples
    }

    /// 解けなかった標本の数 (配分集合が空など)。
    pub fn failures(&self) -> usize {
        self.failures
    }

    /// 解けた標本の保証の内訳。平均などには、保証によらず解けた標本を全て使う。
    /// [`GuaranteeCounts::unreliable`] が 0 でなければ、正しいと言えない解 ([`Guarantee::Approximate`] など) が混ざっている。
    pub fn guarantees(&self) -> &GuaranteeCounts {
        &self.guarantees
    }
}

const QUANTILES: [f64; 3] = [0.05, 0.5, 0.95];

/// `sample` でゲームを `samples` 回引き、`solve` で解いた配分の分布を求める。
///
/// `solve` は配分 (`Vec<f64>`) か、保証つきの結果 ([`NucleolusResult`]・[`Solution`]・[`Solved`]) を返す。
/// 保証つきの結果なら、標本ごとの保証の内訳を [`Distribution::guarantees`] に残す。
///
/// `solve` がエラーを返した標本は失敗として数える。`samples` が 0 の場合、`solve` が返した配分の長さが
/// 引いたゲームの人数と異なる場合、標本ごとに人数が異なる場合は [`Error::InvalidArgument`] を返す。
pub fn monte_carlo<R: Into<Solved>>(
    samples: usize,
    seed: u64,
    sample: impl FnMut(&mut SplitMix64) -> Result<ExplicitGame>,
    solve: impl FnMut(&ExplicitGame) -> Result<R>,
) -> Result<Distribution> {
    sample_and_solve(samples, seed, sample, solve, None)
}

/// [`monte_carlo`] の本体。`narrowest` (区間の幅の半分が最も小さい提携とその値) を与えると、
/// 標本の解の許容誤差がその値以上のときにエラーにする ([`interval_monte_carlo`])。
fn sample_and_solve<R: Into<Solved>>(
    samples: usize,
    seed: u64,
    mut sample: impl FnMut(&mut SplitMix64) -> Result<ExplicitGame>,
    mut solve: impl FnMut(&ExplicitGame) -> Result<R>,
    narrowest: Option<(Coalition, f64)>,
) -> Result<Distribution> {
    if samples == 0 {
        return Err(Error::InvalidArgument("標本の数は 1 以上".into()));
    }
    let mut rng = SplitMix64::new(seed);
    let mut allocations: Vec<Vec<f64>> = Vec::with_capacity(samples);
    let mut guarantees = GuaranteeCounts::default();
    let mut failures = 0;
    let mut first_failure: Option<Error> = None;
    let mut players: Option<usize> = None;
    for _ in 0..samples {
        let game = sample(&mut rng)?;
        let n = *players.get_or_insert(game.players());
        if game.players() != n {
            return Err(Error::InvalidArgument(format!(
                "標本のゲームの人数 {} が最初の標本の人数 {n} と異なる",
                game.players()
            )));
        }
        match solve(&game).map(|r| r.into().settled(&game)) {
            Ok(solved) if solved.allocation.len() != n => {
                return Err(Error::InvalidArgument(format!(
                    "解いた配分の長さ {} がプレイヤー数 {n} と異なる",
                    solved.allocation.len()
                )));
            }
            Ok(solved) => {
                if let (Some((coalition, half_width)), Some(tolerance)) =
                    (narrowest, solved.tolerance)
                    && tolerance >= half_width
                {
                    return Err(Error::InvalidArgument(format!(
                        "標本のゲームを許容誤差 {tolerance:e} で解いたので、提携 {coalition} の区間の幅の半分 \
                         {half_width:e} の変化が許容誤差に埋もれる \
                         (区間を広げるか、より小さい許容誤差で解く)"
                    )));
                }
                guarantees.add(solved.guarantee);
                allocations.push(solved.allocation);
            }
            Err(error) => {
                failures += 1;
                first_failure.get_or_insert(error);
            }
        }
    }
    summarize(&allocations, failures, first_failure, guarantees)
}

fn summarize(
    allocations: &[Vec<f64>],
    failures: usize,
    first_failure: Option<Error>,
    guarantees: GuaranteeCounts,
) -> Result<Distribution> {
    let Some(first) = allocations.first() else {
        // 1 つも解けないのは、解き方の指定の誤り (許容誤差など) のことが多いので、最初の失敗の理由を添える。
        let reason = first_failure
            .map(|error| format!("。最初の失敗: {error}"))
            .unwrap_or_default();
        return Err(Error::InvalidArgument(format!(
            "解けた標本がない ({failures} 個とも失敗{reason})"
        )));
    };
    let n = first.len();
    let count = allocations.len() as f64;
    let mean: Vec<f64> = (0..n)
        .map(|i| allocations.iter().map(|x| x[i]).sum::<f64>() / count)
        .collect();
    let std: Vec<f64> = (0..n)
        .map(|i| {
            if allocations.len() < 2 {
                return 0.0;
            }
            let squares: f64 = allocations.iter().map(|x| (x[i] - mean[i]).powi(2)).sum();
            (squares / (count - 1.0)).sqrt()
        })
        .collect();
    let quantiles = QUANTILES
        .iter()
        .map(|&q| {
            let values = (0..n)
                .map(|i| {
                    let mut column: Vec<f64> = allocations.iter().map(|x| x[i]).collect();
                    column.sort_by(f64::total_cmp);
                    // 線形補間なしの最近傍の順位。
                    let rank =
                        ((q * (column.len() - 1) as f64).round() as usize).min(column.len() - 1);
                    column[rank]
                })
                .collect();
            (q, values)
        })
        .collect();
    Ok(Distribution {
        mean,
        std,
        quantiles,
        samples: allocations.len(),
        failures,
        guarantees,
    })
}

/// 各提携の値を区間で与えたゲーム。
#[derive(Clone, Debug, PartialEq)]
pub struct IntervalGame {
    lower: ExplicitGame,
    upper: ExplicitGame,
}

impl IntervalGame {
    /// 下限と上限のゲーム (同じ人数で、全ての提携で下限 <= 上限)。
    pub fn new(lower: ExplicitGame, upper: ExplicitGame) -> Result<IntervalGame> {
        if lower.players() != upper.players() {
            return Err(Error::InvalidArgument("下限と上限の人数が異なる".into()));
        }
        if lower
            .values()
            .iter()
            .zip(upper.values())
            .any(|(l, u)| l > u)
        {
            return Err(Error::InvalidArgument(
                "下限が上限を超える提携がある".into(),
            ));
        }
        Ok(IntervalGame { lower, upper })
    }

    /// 各値の前後 `relative` (例: 0.1 なら ±10%) の区間。
    pub fn around(game: &ExplicitGame, relative: f64) -> Result<IntervalGame> {
        if !(relative.is_finite() && relative >= 0.0) {
            return Err(Error::InvalidArgument("相対誤差は非負の有限値".into()));
        }
        let shift = |sign: f64| game.map_values(|_, v| v + sign * relative * v.abs());
        IntervalGame::new(shift(-1.0)?, shift(1.0)?)
    }

    pub fn lower(&self) -> &ExplicitGame {
        &self.lower
    }

    pub fn upper(&self) -> &ExplicitGame {
        &self.upper
    }

    /// 幅が正の区間のうち、幅の半分が最も小さい提携とその値 (全ての幅が 0 なら `None`)。
    fn narrowest_half_width(&self) -> Option<(Coalition, f64)> {
        (1..self.lower.values().len())
            .map(|mask| {
                let coalition = Coalition(mask as u64);
                let width = self.upper.value(coalition) - self.lower.value(coalition);
                (coalition, width / 2.0)
            })
            .filter(|&(_, half)| half > 0.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// 各提携の値を区間から独立に一様に引いたゲーム。
    pub(crate) fn sample(&self, rng: &mut SplitMix64) -> Result<ExplicitGame> {
        self.lower
            .map_values(|s, l| l + (self.upper.value(s) - l) * rng.next_f64())
    }
}

/// 名前で選べる解き方 ([`solver`])。
pub type Solver = Box<dyn FnMut(&ExplicitGame) -> Result<Solved>>;

/// [`solver`] が受け付ける名前。
pub const SOLVERS: [&str; 3] = ["nucleolus", "prenucleolus", "shapley"];

/// 名前 (`"nucleolus"`・`"prenucleolus"`・`"shapley"`) から解き方を選ぶ。
///
/// 仁・プレ仁は、仁の既定の手順 ([`Tolerance::Auto`]。[`crate::nucleolus::nucleolus_with`]) で解く。
/// 標本や差分の数だけ解くので、解き直しや有理数の逐次 LP に切り替わるゲームでは時間が延びる
/// (`docs/performance.md` の「値の不確かさの分析の時間」)。確かめずに 1 回だけ解いて速くするには、
/// [`solver_with`] で許容誤差を指定する ([`Tolerance::Relative`] など)。
/// Shapley 値は定義どおりに計算するので、保証は [`Guarantee::Exact`]。
pub fn solver(name: &str) -> Option<Solver> {
    solver_with(name, Tolerance::Auto)
}

/// 仁・プレ仁の許容誤差の決め方を指定した [`solver`] (`"shapley"` は `tolerance` を使わない)。
pub fn solver_with(name: &str, tolerance: Tolerance) -> Option<Solver> {
    let nucleolus = move |domain: Domain| -> Solver {
        Box::new(move |game: &ExplicitGame| {
            let mut options = Options::new(domain);
            options.tolerance = tolerance;
            Ok(crate::nucleolus::nucleolus_with(game, options)?.into())
        })
    };
    match name {
        "nucleolus" => Some(nucleolus(Domain::Imputation)),
        "prenucleolus" => Some(nucleolus(Domain::Preimputation)),
        "shapley" => Some(Box::new(|g: &ExplicitGame| {
            Ok(Solved {
                allocation: crate::values::shapley(g),
                guarantee: Some(Guarantee::Exact),
                tolerance: None,
                without_lp: false,
            })
        })),
        _ => None,
    }
}

/// 区間ゲームの値を一様に引いて解いた配分の分布。
///
/// [`monte_carlo`] のエラーに加えて、解いた標本の解の許容誤差 ([`Solved::tolerance`]) が、幅が正の区間のうち
/// 最も狭いものの幅の半分以上なら [`Error::InvalidArgument`] を返す (エラーの文章にその提携を書く)。
/// 幅の半分は、[`influence`] の差分の幅 `delta` (値を `±delta` 動かす) に当たる。[`influence`] は動かす提携のどれか
/// 1 つでも `delta` が許容誤差に埋もれればエラーにするので、ここでも 1 つの提携でも埋もれればエラーにする。
/// 埋もれた提携の値の揺れは、配分の分布に許容誤差の扱いとしてしか現れず、分布がその揺れの影響を表さないためである。
/// 幅が 0 の提携 (値を固定した提携。`IntervalGame::around` では値が 0 の提携) は揺れないので比べない。
/// [`Tolerance::Auto`] の解は、[`influence`] と同じく返した配分を求めた逐次 LP の許容誤差で比べる
/// (LP を解かずに決まった厳密な解は 0)。
///
/// 埋もれるかは最も狭い区間で決まるので、次のゲームは許容誤差を小さくしないとエラーになりうる。
/// 値が丸め誤差程度の非 0 の提携 (`0.1 + 0.2 - 0.3` など) を持つゲームは、[`IntervalGame::around`] の幅がその値に
/// 比例して極めて狭くなる。値の桁が `1e6` 倍以上離れた提携を持つゲームは、小さい値の提携の幅が、最大の値に比例する
/// 許容誤差 (既定は `max|v(S)|` の `1e-7` 倍) より狭くなりやすい。どちらも、その提携の揺れは許容誤差に埋もれて
/// 分布に現れないので、エラーにする。値を 0 に丸めるか、[`IntervalGame::new`] で幅を与えるか、より小さい許容誤差で解く。
pub fn interval_monte_carlo<R: Into<Solved>>(
    game: &IntervalGame,
    samples: usize,
    seed: u64,
    solve: impl FnMut(&ExplicitGame) -> Result<R>,
) -> Result<Distribution> {
    sample_and_solve(
        samples,
        seed,
        |rng| game.sample(rng),
        solve,
        game.narrowest_half_width(),
    )
}

/// 感度 `dx / dv(S)` を求めた差分の取り方。`x(v)` は値 `v` のゲームを解いた配分、`δ` は動かす幅。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Difference {
    /// 中心差分 `(x(v(S) + δ) - x(v(S) - δ)) / 2δ`。両側のゲームが解けたとき。
    Central,
    /// 前進差分 `(x(v(S) + δ) - x(v(S))) / δ`。`v(S) - δ` のゲームが解けなかったとき
    /// (配分集合が 1 点のゲームで `v(N)` を下げた場合など)。
    Forward,
    /// 後退差分 `(x(v(S)) - x(v(S) - δ)) / δ`。`v(S) + δ` のゲームが解けなかったとき
    /// (配分集合が 1 点のゲームで 1 人提携の値を上げた場合など)。
    Backward,
}

impl fmt::Display for Difference {
    /// `central`・`forward`・`backward`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Difference::Central => "central",
            Difference::Forward => "forward",
            Difference::Backward => "backward",
        })
    }
}

/// 提携の値に対する配分の感度。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Influence {
    pub(crate) coalition: Coalition,
    pub(crate) sensitivity: Vec<f64>,
    pub(crate) difference: Difference,
    pub(crate) guarantees: GuaranteeCounts,
}

impl Influence {
    /// 値を動かした提携。
    pub fn coalition(&self) -> Coalition {
        self.coalition
    }

    /// 各プレイヤーの `dx_i / dv(S)`。
    pub fn sensitivity(&self) -> &[f64] {
        &self.sensitivity
    }

    /// [`Influence::sensitivity`] を求めた差分の取り方。
    pub fn difference(&self) -> Difference {
        self.difference
    }

    /// 差分に使った 2 つの解の保証の内訳。[`GuaranteeCounts::unreliable`] が 0 でなければ、
    /// 正しいと言えない解 ([`Guarantee::Approximate`] など) から求めた感度である。
    pub fn guarantees(&self) -> &GuaranteeCounts {
        &self.guarantees
    }

    /// 感度の絶対値の最大。
    pub fn magnitude(&self) -> f64 {
        self.sensitivity.iter().fold(0.0, |acc, s| acc.max(s.abs()))
    }
}

/// `solve` でゲームを解く。解けなければ内側の `Err` (片側差分に切り替える判断に使う)、
/// 解いた配分の長さが人数と異なれば外側の `Err` (`solve` の誤りなので計算を止める)。
fn try_solve<R: Into<Solved>>(
    solve: &mut impl FnMut(&ExplicitGame) -> Result<R>,
    game: &ExplicitGame,
) -> Result<std::result::Result<Solved, Error>> {
    match solve(game).map(|r| r.into().settled(game)) {
        Ok(solved) if solved.allocation.len() != game.players() => {
            Err(Error::InvalidArgument(format!(
                "解いた配分の長さ {} がプレイヤー数 {} と異なる",
                solved.allocation.len(),
                game.players()
            )))
        }
        Ok(solved) => Ok(Ok(solved)),
        Err(e) => Ok(Err(e)),
    }
}

/// 片側差分に使う元のゲームの解。`base` に 1 度だけ求めて保持する。
///
/// `shift` は解けなかった側の動かし幅 (`+δ` か `-δ`)、`failed` はそのときのエラー (解けなければ文章に書く)。
fn base_solution<'a, R: Into<Solved>>(
    base: &'a mut Option<Solved>,
    solve: &mut impl FnMut(&ExplicitGame) -> Result<R>,
    game: &ExplicitGame,
    coalition: Coalition,
    shift: f64,
    failed: &Error,
) -> Result<&'a Solved> {
    if base.is_none() {
        match try_solve(solve, game)? {
            Ok(x) => *base = Some(x),
            Err(e) => {
                return Err(Error::InvalidArgument(format!(
                    "提携 {coalition} の値を {shift:+} 動かしたゲームが解けず ({failed})、\
                     片側差分に使う元のゲームも解けない ({e})"
                )));
            }
        }
    }
    match base {
        Some(solved) => Ok(solved),
        None => unreachable!("直前で求めた"),
    }
}

/// 差分に使う解の許容誤差が差分の幅 `delta` 以上なら、変化が許容誤差に埋もれるのでエラーにする。
fn check_resolution(solved: &Solved, coalition: Coalition, delta: f64) -> Result<()> {
    match solved.tolerance {
        Some(tolerance) if tolerance >= delta => Err(Error::InvalidArgument(format!(
            "提携 {coalition} の値を動かしたゲームを許容誤差 {tolerance:e} で解いたので、\
             差分の幅 delta = {delta:e} の変化が許容誤差に埋もれる \
             (delta を許容誤差より大きくするか、より小さい許容誤差で解く)"
        ))),
        _ => Ok(()),
    }
}

/// 提携 `coalitions` の値を `±delta` 動かしたときの配分の変化率 `dx / dv(S)`。
///
/// 各提携 `S` について `v(S) + delta` と `v(S) - delta` のゲームを `solve` で解き、
/// 中心差分 `(x(v(S) + δ) - x(v(S) - δ)) / 2δ` を求める ([`Difference::Central`])。
/// 片側のゲームだけが解けないとき (配分集合が 1 点のゲームで `v(N)` を下げる・1 人提携の値を上げると、
/// 配分集合が空になる) は、元のゲームの解 `x(v)` との片側差分を使い、[`Influence::difference`] に
/// [`Difference::Forward`] か [`Difference::Backward`] を記録する。解が値の向きで形を変える点では、
/// 中心差分は左右の変化率の平均、片側差分は解ける側の変化率になる。
///
/// 提携は空でなく、プレイヤー数の範囲の番号だけからなる必要がある (空提携の値は 0 に固定のため動かせない)。
/// 次の場合は [`Error::InvalidArgument`] を返す。
///
/// - `solve` が返した配分の長さがプレイヤー数と異なる。
/// - ある提携で両側のゲームがどちらも解けない、または片側だけ解けて元のゲームが解けない
///   (エラーの文章にその提携と、`solve` が返したエラーを書く)。
/// - 差分に使う解の許容誤差 ([`Solved::tolerance`]) が `delta` 以上 (配分の変化が許容誤差に埋もれ、
///   感度が許容誤差の扱いで決まってしまう)。`delta` を大きくするか、より小さい許容誤差で解く。
///   [`Tolerance::Auto`] で求めた結果の許容誤差は、返した配分を求めた逐次 LP の許容誤差
///   (有理数の逐次 LP で求めた結果と、LP を解かずに決まった厳密な結果 (1 人ゲーム、`sum v({i}) = v(N)` の
///   ゲーム) では 0) で比べる。
///
/// `solve` が保証つきの結果を返せば、差分に使った解の保証の内訳を [`Influence::guarantees`] に残す。
pub fn influence<R: Into<Solved>>(
    game: &ExplicitGame,
    coalitions: &[Coalition],
    delta: f64,
    mut solve: impl FnMut(&ExplicitGame) -> Result<R>,
) -> Result<Vec<Influence>> {
    if !(delta.is_finite() && delta > 0.0) {
        return Err(Error::InvalidArgument("delta は正の有限値".into()));
    }
    let n = game.players();
    for coalition in coalitions {
        if coalition.is_empty() {
            return Err(Error::InvalidArgument(
                "空の提携の値は 0 に固定なので動かせない".into(),
            ));
        }
        if !coalition.is_subset(game.grand()) {
            return Err(Error::InvalidArgument(format!(
                "提携 {coalition} に人数 {n} 以上の番号がある"
            )));
        }
    }
    // 元のゲームの解は、片側差分が必要になったときに 1 度だけ求める。
    let mut base: Option<Solved> = None;
    let mut rows = Vec::with_capacity(coalitions.len());
    for &coalition in coalitions {
        let shifted = |sign: f64| game.with_value(coalition, game.value(coalition) + sign * delta);
        let plus = try_solve(&mut solve, &shifted(1.0)?)?;
        let minus = try_solve(&mut solve, &shifted(-1.0)?)?;
        let (upper, lower, width, difference) = match (plus, minus) {
            (Ok(plus), Ok(minus)) => (plus, minus, 2.0 * delta, Difference::Central),
            (Ok(plus), Err(failed)) => {
                let x = base_solution(&mut base, &mut solve, game, coalition, -delta, &failed)?;
                (plus, x.clone(), delta, Difference::Forward)
            }
            (Err(failed), Ok(minus)) => {
                let x = base_solution(&mut base, &mut solve, game, coalition, delta, &failed)?;
                (x.clone(), minus, delta, Difference::Backward)
            }
            (Err(plus), Err(minus)) => {
                return Err(Error::InvalidArgument(format!(
                    "提携 {coalition} の値を ±{delta} 動かしたゲームがどちらも解けない \
                     (+{delta}: {plus}、-{delta}: {minus})"
                )));
            }
        };
        let mut guarantees = GuaranteeCounts::default();
        for solved in [&upper, &lower] {
            check_resolution(solved, coalition, delta)?;
            guarantees.add(solved.guarantee);
        }
        let sensitivity = upper
            .allocation
            .iter()
            .zip(&lower.allocation)
            .map(|(a, b)| (a - b) / width)
            .collect();
        rows.push(Influence {
            coalition,
            sensitivity,
            difference,
            guarantees,
        });
    }
    Ok(rows)
}

/// 配分 `x` で超過が大きい上位 `levels` 段の提携 (仁ではこれが配分を決めている) と全体提携。
pub fn key_coalitions(game: &ExplicitGame, x: &[f64], levels: usize) -> Result<Vec<Coalition>> {
    let report = explain::report(
        game,
        x,
        ExplainOptions {
            levels,
            max_listed: usize::MAX,
            tolerance: None,
        },
    )?;
    let mut coalitions: Vec<Coalition> = report
        .levels
        .into_iter()
        .flat_map(|level| level.coalitions)
        .collect();
    coalitions.push(game.grand());
    Ok(coalitions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{nucleolus, values};

    fn four_player() -> ExplicitGame {
        ExplicitGame::from_lex(&[
            0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
        ])
        .unwrap()
    }

    fn solve_nucleolus(game: &ExplicitGame) -> Result<Vec<f64>> {
        Ok(nucleolus::nucleolus(game)?.allocation)
    }

    #[test]
    fn zero_width_interval_has_no_spread() {
        let game = four_player();
        let interval = IntervalGame::around(&game, 0.0).unwrap();
        let distribution = interval_monte_carlo(&interval, 5, 1, solve_nucleolus).unwrap();
        let expected = solve_nucleolus(&game).unwrap();
        for ((mean, std), e) in distribution
            .mean
            .iter()
            .zip(&distribution.std)
            .zip(&expected)
        {
            assert!((mean - e).abs() < 1e-9);
            assert!(*std < 1e-9);
        }
        assert_eq!(distribution.samples, 5);
    }

    #[test]
    fn spread_grows_with_uncertainty() {
        let game = four_player();
        let narrow = interval_monte_carlo(
            &IntervalGame::around(&game, 0.01).unwrap(),
            100,
            2,
            solve_nucleolus,
        )
        .unwrap();
        let wide = interval_monte_carlo(
            &IntervalGame::around(&game, 0.1).unwrap(),
            100,
            2,
            solve_nucleolus,
        )
        .unwrap();
        for i in 0..4 {
            assert!(wide.std[i] > narrow.std[i]);
            let (low, high) = (&wide.quantiles[0].1, &wide.quantiles[2].1);
            assert!(low[i] <= wide.mean[i] && wide.mean[i] <= high[i]);
        }
    }

    /// 仁は超過の大きい段の提携の値だけで決まる。超過がずっと小さい提携の値を動かしても変わらない。
    #[test]
    fn nucleolus_depends_only_on_binding_coalitions() {
        let game = four_player();
        let x = solve_nucleolus(&game).unwrap();
        let key = key_coalitions(&game, &x, 1).unwrap();
        let binding = influence(&game, &key, 1e-4, solve_nucleolus).unwrap();
        assert!(binding.iter().any(|i| i.magnitude() > 0.1));
        // {1} の超過は -3.5 で、最大の超過 -0.5 よりずっと小さい。
        let slack = influence(&game, &[Coalition::singleton(0)], 1e-4, solve_nucleolus).unwrap();
        assert!(slack[0].magnitude() < 1e-6);
    }

    /// Shapley 値は線形で、`dφ_i / dv(S)` は i ∈ S なら (|S|-1)!(n-|S|)!/n!、i ∉ S なら -|S|!(n-|S|-1)!/n!。
    #[test]
    fn shapley_sensitivity_matches_weights() {
        let game = four_player();
        let s = Coalition::from_players(&[0, 1]);
        let result = influence(&game, &[s], 0.5, |g| Ok(values::shapley(g))).unwrap();
        let inside = 1.0 * 2.0 / 24.0; // (2-1)!(4-2)!/4!
        let outside = -2.0 * 1.0 / 24.0; // -2!(4-2-1)!/4!
        let expected = [inside, inside, outside, outside];
        for (a, e) in result[0].sensitivity.iter().zip(expected) {
            assert!((a - e).abs() < 1e-9, "{:?}", result[0].sensitivity);
        }
        assert_eq!(result[0].difference, Difference::Central);
    }

    /// 配分集合が 1 点の加法的なゲーム: v(N) を下げる、1 人提携の値を上げると配分集合が空になる。
    /// 解ける側だけで片側差分をとり、どちらの差分かを結果に残す。
    #[test]
    fn one_sided_difference_when_one_side_cannot_be_solved() {
        let game = ExplicitGame::from_binary(&[1.0, 1.0, 2.0, 1.0, 2.0, 2.0, 3.0]).unwrap();
        let x = solve_nucleolus(&game).unwrap();
        let key = key_coalitions(&game, &x, 2).unwrap();
        let rows = influence(&game, &key, 1e-4, solve_nucleolus).unwrap();
        assert_eq!(rows.len(), 7);
        for row in &rows {
            let (difference, expected) = if row.coalition == game.grand() {
                // v(N) を上げた分は 3 人で等分される。
                (Difference::Forward, 1.0 / 3.0)
            } else if row.coalition.len() == 1 {
                (Difference::Backward, 0.0)
            } else {
                (Difference::Central, 0.0)
            };
            assert_eq!(row.difference, difference, "{}", row.coalition);
            for s in &row.sensitivity {
                assert!((s - expected).abs() < 1e-6, "{}: {row:?}", row.coalition);
            }
        }
    }

    /// 両側とも解けない提携は、その提携を示したエラーにする (入力のゲームが解けないとは書かない)。
    #[test]
    fn both_sides_unsolvable_names_the_coalition() {
        let game = four_player();
        let s = Coalition::from_players(&[1, 2]);
        let err = influence(&game, &[s], 1e-4, |_| {
            Err::<Vec<f64>, _>(Error::EmptyImputationSet)
        })
        .unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
        let text = err.to_string();
        assert!(
            text.contains("{1, 2}") && text.contains("どちらも解けない"),
            "{text}"
        );
        // 片側だけ解けて、片側差分に使う元のゲームが解けない場合も、提携を示す。
        let original = game.value(s);
        let err = influence(&game, &[s], 1e-4, |g| {
            if g.value(s) > original {
                solve_nucleolus(g)
            } else {
                Err(Error::Numerical("解けない".into()))
            }
        })
        .unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("{1, 2}") && text.contains("元のゲーム"),
            "{text}"
        );
    }

    #[test]
    fn key_coalitions_of_one_player_game() {
        // 真部分提携がないので、全体提携だけ。
        let game = ExplicitGame::from_lex(&[1.0]).unwrap();
        let key = key_coalitions(&game, &[1.0], 2).unwrap();
        assert_eq!(key, vec![game.grand()]);
    }

    /// 値が 1e12 程度の 4 人の整数ゲーム。比 1e-7 の許容誤差 (約 4e5) で 1 回だけ解いた配分は仁と異なり、
    /// 既定の手順では有理数の逐次 LP で仁を求める。
    fn large() -> ExplicitGame {
        ExplicitGame::from_binary(&[
            0.0,
            0.0,
            2000000000002.0,
            0.0,
            2000000000001.0,
            2000000000006.0,
            3000000000002.0,
            0.0,
            2000000000006.0,
            2000000000006.0,
            3000000000005.0,
            2000000000005.0,
            3000000000002.0,
            3000000000002.0,
            4000000000002.0,
        ])
        .unwrap()
    }

    /// 名前で選ぶ解き方は、仁の既定の手順で解く (1 回だけ解いて速さを優先するのは、許容誤差を指定したときだけ)。
    #[test]
    fn named_solver_uses_default_nucleolus_procedure() {
        let game = large();
        let mut solve = solver("nucleolus").unwrap();
        let solved = solve(&game).unwrap();
        let expected = nucleolus::nucleolus(&game).unwrap();
        assert!(expected.is_rational());
        assert_eq!(solved.allocation(), expected.allocation());
        assert_eq!(solved.guarantee(), Some(Guarantee::Exact));
        assert_eq!(solved.tolerance(), Some(0.0));
        let mut fast = solver_with("nucleolus", Tolerance::Relative(1e-7)).unwrap();
        assert_ne!(fast(&game).unwrap().allocation(), expected.allocation());
        let mut shapley = solver("shapley").unwrap();
        let solved = shapley(&game).unwrap();
        assert_eq!(
            (solved.guarantee(), solved.tolerance()),
            (Some(Guarantee::Exact), None)
        );
    }

    /// 分布と感度は、解いた解の保証の内訳を持つ。正しいと言えない解が混ざれば区別できる。
    #[test]
    fn results_count_guarantees_of_solutions() {
        let game = four_player();
        let interval = IntervalGame::around(&game, 0.1).unwrap();
        let distribution =
            interval_monte_carlo(&interval, 20, 3, solver("nucleolus").unwrap()).unwrap();
        assert_eq!(distribution.guarantees().count(Guarantee::Exact), 20);
        assert_eq!(distribution.guarantees().unreliable(), 0);
        assert_eq!(distribution.guarantees().to_string(), "exact:20");
        // 標本の一部が近似の解なら、内訳に残る。
        let mut calls = 0;
        let distribution = interval_monte_carlo(&interval, 10, 3, |g| {
            calls += 1;
            let mut solved = Solved::from(nucleolus::nucleolus(g)?);
            if calls % 2 == 0 {
                solved.guarantee = Some(Guarantee::Approximate);
            }
            Ok(solved)
        })
        .unwrap();
        let counts = distribution.guarantees();
        assert_eq!(
            (
                counts.count(Guarantee::Exact),
                counts.count(Guarantee::Approximate)
            ),
            (5, 5)
        );
        assert_eq!(counts.unreliable(), 5);
        assert_eq!(counts.to_string(), "exact:5,approximate:5");
        // 配分だけを返す解き方は、保証を報告しない解として数える。
        let distribution = interval_monte_carlo(&interval, 4, 3, solve_nucleolus).unwrap();
        assert_eq!(distribution.guarantees().unreported(), 4);
        assert_eq!(distribution.guarantees().to_string(), "unreported:4");
        // 感度は、差分に使った 2 つの解の内訳を持つ。
        let rows = influence(&game, &[game.grand()], 1e-4, solver("nucleolus").unwrap()).unwrap();
        assert_eq!(rows[0].guarantees().count(Guarantee::Exact), 2);
    }

    /// 差分の幅が解の許容誤差以下なら、変化が許容誤差に埋もれるのでエラーにする。
    #[test]
    fn influence_rejects_delta_buried_in_tolerance() {
        let game = large();
        let s = Coalition::from_players(&[2, 3]);
        // 比 1e-7 の許容誤差は約 4e5。delta = 10 の変化は埋もれる。
        let fast = solver_with("nucleolus", Tolerance::Relative(1e-7)).unwrap();
        let err = influence(&game, &[s], 10.0, fast).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
        let text = err.to_string();
        assert!(
            text.contains("{2, 3}") && text.contains("埋もれる"),
            "{text}"
        );
        // 既定の手順 (有理数の逐次 LP で許容誤差 0) なら求められる。
        let rows = influence(&game, &[s], 10.0, solver("nucleolus").unwrap()).unwrap();
        assert_eq!(rows[0].guarantees().count(Guarantee::Exact), 2);
        // 許容誤差より大きい delta なら、1 回だけ解く解き方でも求める。
        let fast = solver_with("nucleolus", Tolerance::Relative(1e-7)).unwrap();
        assert!(influence(&game, &[s], 1e8, fast).is_ok());
    }

    #[test]
    fn monte_carlo_requires_at_least_one_sample() {
        let game = four_player();
        let interval = IntervalGame::around(&game, 0.1).unwrap();
        let err = interval_monte_carlo(&interval, 0, 1, solve_nucleolus).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
        assert!(err.to_string().contains("1 以上"), "{err}");
    }

    /// 区間の幅の半分が解の許容誤差以下の提携があれば、揺れが許容誤差に埋もれるのでエラーにする。
    #[test]
    fn interval_rejects_width_buried_in_tolerance() {
        let game = large();
        // 各値 ±1e-13 倍 (最も狭い幅の半分は 0.2 程度)。比 1e-7 の許容誤差は約 4e5。
        let interval = IntervalGame::around(&game, 1e-13).unwrap();
        let fast = solver_with("nucleolus", Tolerance::Relative(1e-7)).unwrap();
        let err = interval_monte_carlo(&interval, 3, 0, fast).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err:?}");
        assert!(err.to_string().contains("埋もれる"), "{err}");
        // 1 つの提携だけが狭くてもエラーにする (値が 0 の提携は幅 0 なので比べない)。
        let mut upper = game.clone();
        for mask in 1..16u64 {
            let s = Coalition(mask);
            let width = if mask == 3 { 1.0 } else { 1e9 };
            upper = upper.with_value(s, game.value(s) + width).unwrap();
        }
        let interval = IntervalGame::new(game.clone(), upper).unwrap();
        let fast = solver_with("nucleolus", Tolerance::Relative(1e-7)).unwrap();
        let err = interval_monte_carlo(&interval, 3, 0, fast).unwrap_err();
        assert!(err.to_string().contains("{0, 1}"), "{err}");
        // 幅が許容誤差より広ければ求める。
        let interval = IntervalGame::around(&game, 0.1).unwrap();
        let fast = solver_with("nucleolus", Tolerance::Relative(1e-7)).unwrap();
        assert!(interval_monte_carlo(&interval, 3, 0, fast).is_ok());
        // 許容誤差を報告しない解き方では比べない。
        let interval = IntervalGame::around(&game, 1e-13).unwrap();
        assert!(interval_monte_carlo(&interval, 3, 0, |g| Ok(values::shapley(g))).is_ok());
    }

    /// LP を解かずに決まった厳密な解 (配分集合が 1 点、1 人ゲーム) は許容誤差 0 として比べ、小さい delta でも求める。
    #[test]
    fn exact_solutions_without_lp_have_no_tolerance() {
        // 加法的なゲーム: 2 人提携の値を動かしても sum v({i}) = v(N) のままで、配分は (1, 1, 1) に厳密に決まる。
        let additive = ExplicitGame::from_binary(&[1.0, 1.0, 2.0, 1.0, 2.0, 2.0, 3.0]).unwrap();
        let pair = Coalition::from_players(&[0, 1]);
        let rows = influence(&additive, &[pair], 1e-8, solver("nucleolus").unwrap()).unwrap();
        assert_eq!(rows[0].difference, Difference::Central);
        assert_eq!(rows[0].sensitivity, vec![0.0; 3]);
        let one = ExplicitGame::from_binary(&[2.0]).unwrap();
        let rows = influence(&one, &[one.grand()], 1e-12, solver("nucleolus").unwrap()).unwrap();
        assert!((rows[0].sensitivity[0] - 1.0).abs() < 1e-3, "{rows:?}");
        // 1 人提携の値を上げると sum v({i}) が v(N) を許容誤差以内で超え、超えた分を等分した点を配分集合とみなす。
        // 下げると LP で解く。どちらも許容誤差の扱いで決まるので、許容誤差で比べてエラーにする。
        let err = influence(
            &additive,
            &[Coalition::singleton(0)],
            1e-8,
            solver("nucleolus").unwrap(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("埋もれる"), "{err}");
    }
}
