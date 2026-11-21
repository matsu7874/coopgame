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
//! 超過の差が許容誤差より小さい別の段があるゲームでは、結果が仁から許容誤差の程度ずれる。
//! 既定の設定 ([`Tolerance::Auto`]) は、解いた配分が Kohlberg 基準を細かい許容誤差でも満たすかでこのずれを確かめ、
//! 必要なら許容誤差を下げて解き直すか、有理数の逐次 LP ([`exact`]) で求める ([`nucleolus_with`])。
//! 許容誤差を指定すれば ([`Tolerance::Absolute`]・[`Tolerance::Relative`])、その値で 1 回だけ解く。
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
use crate::game::exact::{ExactGame, to_f64, to_rational};
use crate::game::scale::{
    normalized_tolerance, positive_scale, rounding_magnitude, tolerance_from_ratio,
};
use crate::game::{Coalition, ExplicitGame, default_tolerance};
use crate::levels::kohlberg_at;
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

/// 逐次 LP の許容誤差の決め方。
///
/// 許容誤差は、超過の比較 (どの提携を同じ段として確定するか) と、配分集合が空かどうかの判定に使う。
/// 配分集合は `sum v({i}) <= v(N) + 許容誤差` なら空とみなさない。和が `v(N)` を許容誤差以内で超えるときは、
/// 超えた分を人数で等分して下限 `v({i})` を下げた 1 点を配分集合とする ([`Tolerance::Auto`] では既定の許容誤差で判定する)。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Tolerance {
    /// 仁を正しく求めることを優先し、必要なら解き直す (既定)。手順は [`nucleolus_with`] に書く。
    /// 解く回数と時間は入力によって変わる。
    #[default]
    Auto,
    /// ゲームの値と同じ単位の許容誤差で、1 回だけ解く。有限で 0 以上の値にする。
    Absolute(f64),
    /// 値の最大絶対値 `max|v(S)|` に対する比の許容誤差で、1 回だけ解く ([`crate::game::relative_tolerance`])。
    /// 有限で 0 以上の値にする。`Auto` が最初に使う比は `1e-7`。
    Relative(f64),
}

impl Tolerance {
    /// 1 回だけ解くときの許容誤差 (ゲームの値と同じ単位)。`Auto` は既定の許容誤差。
    ///
    /// `Relative` の比が有限で 0 以上でなければ [`Error::InvalidArgument`]
    /// (`Absolute` の値は、解く前に [`Settings::new`] が確かめる)。
    fn fixed(self, game: &ExplicitGame) -> Result<f64> {
        match self {
            Tolerance::Auto => Ok(default_tolerance(game)),
            Tolerance::Absolute(tolerance) => Ok(tolerance),
            Tolerance::Relative(ratio) => tolerance_from_ratio(ratio, game.max_abs_value()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Options {
    pub domain: Domain,
    /// 許容誤差の決め方。既定は [`Tolerance::Auto`]。
    ///
    /// 許容誤差を指定して、その値が有限で 0 以上でなければ、解く前に [`Error::InvalidArgument`] を返す。
    pub tolerance: Tolerance,
    pub method: Method,
}

impl Options {
    /// 既定の設定: 許容誤差は [`Tolerance::Auto`]、行の入れ方は [`Method::ConstraintGeneration`]。
    pub fn new(domain: Domain) -> Options {
        Options {
            domain,
            tolerance: Tolerance::Auto,
            method: Method::default(),
        }
    }
}

/// 値を 2 のべき乗で割ったゲームを解くときの設定。
#[derive(Clone, Copy)]
struct Settings {
    domain: Domain,
    /// 割ったゲームの値の単位での許容誤差。
    tolerance: f64,
    method: Method,
    /// 制約生成で後から追加する行の数の上限。超えたら [`Error::LimitExceeded`] で打ち切る。
    row_budget: usize,
}

impl Settings {
    /// `tolerance` (元のゲームの値の単位) を確かめ、値を `divisor` で割ったゲームに合わせる。
    ///
    /// 許容誤差が有限で 0 以上でないとき、または割った許容誤差が有限でない
    /// (ゲームの値に比べて大きすぎる) ときは [`Error::InvalidArgument`]。
    fn new(domain: Domain, method: Method, tolerance: f64, divisor: f64) -> Result<Settings> {
        Ok(Settings {
            domain,
            tolerance: normalized_tolerance(tolerance, divisor)?,
            method,
            row_budget: usize::MAX,
        })
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
    pub(crate) tolerance: f64,
    pub(crate) refinements: usize,
    pub(crate) rational: bool,
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

    /// この結果を求めた逐次 LP の許容誤差 (ゲームの値と同じ単位)。有理数の逐次 LP で求めた結果では 0。
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// [`Tolerance::Auto`] で、許容誤差を下げて解き直しを試みた回数 (LP が失敗した試行を含む)。
    /// 返した配分を求めた許容誤差は [`NucleolusResult::tolerance`]。
    pub fn refinements(&self) -> usize {
        self.refinements
    }

    /// [`Tolerance::Auto`] で、有理数の逐次 LP ([`exact`]) に切り替えて求めた結果か。
    pub fn is_rational(&self) -> bool {
        self.rational
    }

    /// 全提携の値を持つゲームの逐次 LP ([`nucleolus`] など) では [`Guarantee::Exact`]。
    /// ただし [`Tolerance::Auto`] で、許容誤差を下げても Kohlberg 基準を満たさず、有理数の逐次 LP にも
    /// 切り替えなかった結果は [`Guarantee::Approximate`] ([`nucleolus_with`])。
    /// オラクル版 ([`oracle`]) では、オラクルが全ての真部分提携について答えれば `Exact`、
    /// 一部の提携だけなら [`Guarantee::Approximate`]。
    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }

    /// LP を解かずに決まった配分の結果 (1 人ゲーム、配分集合が 1 点のゲームなど)。
    pub(crate) fn without_lp(
        allocation: Vec<f64>,
        guarantee: Guarantee,
        tolerance: f64,
    ) -> NucleolusResult {
        NucleolusResult {
            allocation,
            levels: Vec::new(),
            lp_solves: 0,
            rows_added: 0,
            guarantee,
            tolerance,
            refinements: 0,
            rational: false,
        }
    }

    /// 正規化したゲームで求めた結果を、元の大きさ (`factor` 倍) に戻す。
    ///
    /// 戻した配分か最大超過が f64 で表せない (有限でない) ときは [`Error::Numerical`]。
    pub(crate) fn scaled_by(mut self, factor: f64) -> Result<NucleolusResult> {
        for value in self.allocation.iter_mut().chain(self.levels.iter_mut()) {
            *value *= factor;
        }
        self.tolerance *= factor;
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

    /// [`NucleolusResult::guarantee`] と同じ規則で決まる。
    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }
}

/// 仁(配分集合上で超過ベクトルを辞書式に最小化する点)。既定の設定 ([`Options::new`]) で求める。
pub fn nucleolus(game: &ExplicitGame) -> Result<NucleolusResult> {
    nucleolus_with(game, Options::new(Domain::Imputation))
}

/// プレ仁(原配分集合上で超過ベクトルを辞書式に最小化する点)。既定の設定 ([`Options::new`]) で求める。
pub fn prenucleolus(game: &ExplicitGame) -> Result<NucleolusResult> {
    nucleolus_with(game, Options::new(Domain::Preimputation))
}

/// [`Tolerance::Auto`] が順に試す許容誤差 (`max|v(S)|` に対する比)。
const AUTO_TOLERANCES: [f64; 3] = [1e-7, 1e-9, 1e-11];

/// [`Tolerance::Auto`] の解き直しで、制約生成が追加してよい行の数の、最初に解いたときに追加した行の数に対する倍率
/// (少なくとも `4 n^2` 行は追加できる)。
///
/// 許容誤差を下げると、LP の解の誤差を違反とみなして行を足し続け、ほぼ全ての提携を行にしても終わらないことがある。
/// 許容誤差を下げて正しい仁が求まった解では、追加した行は最初の 5.3 倍以下だった
/// (11・14・16 人の整数ゲーム。`docs/performance.md` の「上限と既定値の根拠」)。上限を超えた解き直しは打ち切り、
/// その許容誤差では解けなかったものとして扱う。
const REFINEMENT_ROW_FACTOR: usize = 8;

/// [`Tolerance::Auto`] が、解いた配分の Kohlberg 基準を確かめるときに超過の段を読む許容誤差 (`max|v(S)|` に対する比)。
///
/// 細かいほど、間隔の小さい段をまとめた配分を見分けられるが、逐次 LP の解の誤差を別の段と読み違えやすくなる。
/// 同点が本物のゲーム (BNF・凸・優加法的・重み付き投票の表、20 人まで) では、`1e-12` で読み違いがなく、
/// `1e-13` で 1 件あった。読み違えると、有理数の逐次 LP に切り替えられる人数では時間がかかるだけだが、
/// それより多い人数では正しい配分の保証を近似にしてしまうので、余裕を 1 桁多く取る。
const AUTO_READING: f64 = 1e-11;

/// 有理数の逐次 LP に切り替えられる人数 ([`MAX_AUTO_RATIONAL_PLAYERS`] 人以下) での [`AUTO_READING`]。
const AUTO_READING_WITH_RATIONAL: f64 = 1e-12;

/// [`Tolerance::Auto`] が、有理数の逐次 LP ([`exact::nucleolus`]) に自動で切り替える人数の上限。
///
/// 有理数の逐次 LP の時間は人数とともに急に延びる。切り替えが起きる種類のゲーム (値が大きい整数のゲーム) では、
/// 8 人で 2-10 秒、9 人で 47-133 秒だった (`docs/performance.md`)。それより多い人数で有理数の仁が要るときは、
/// [`exact::nucleolus`] を直接使う ([`exact::MAX_EXACT_PLAYERS`] 人まで)。
pub const MAX_AUTO_RATIONAL_PLAYERS: usize = 8;

/// [`Tolerance::Auto`] が、解いた配分の Kohlberg 基準を確かめる人数の上限
/// (全ての提携の超過を並べ、段の提携族の平衡性を LP で判定する)。
pub const MAX_AUTO_CHECK_PLAYERS: usize = 20;

/// 設定 `options` で仁 (`Domain::Imputation`) またはプレ仁を求める。
///
/// 逐次 LP は、超過が段の最大超過と許容誤差以内で等しい提携を同じ段として確定しうる。そのため、
/// 超過の差が許容誤差より小さい別の段があるゲーム (値が `1e6` 以上の整数のゲームなど) では、
/// 結果が仁から許容誤差の程度ずれることがある。`options.tolerance` でこの扱いを選ぶ。
///
/// - [`Tolerance::Absolute`]・[`Tolerance::Relative`]: 指定した許容誤差で 1 回だけ解く。
///   結果は許容誤差の範囲で成り立つ。
/// - [`Tolerance::Auto`] (既定): 仁を正しく求めることを優先し、次の順に進める。
///   1. `max|v(S)|` の `1e-7` 倍の許容誤差で解く。
///   2. [`MAX_AUTO_CHECK_PLAYERS`] 人以下なら、解いた配分が Kohlberg 基準を満たすかを、`max|v(S)|` の `1e-11` 倍
///      ([`MAX_AUTO_RATIONAL_PLAYERS`] 人以下では `1e-12` 倍) の許容誤差で確かめる: 和と個人合理性を満たし、
///      超過の段を上から順に合わせた提携族がどれも平衡であること。満たせばその配分を返す。
///      別の段を同じ段として確定した配分は、細かく読むと段が分かれて平衡でなくなる。
///      超過の段の間隔がこの許容誤差より小さいゲームでは、見分けられないことがある。
///   3. 満たさなければ、許容誤差を `1e-9` 倍、`1e-11` 倍に下げて解き直し、同じ確認をする。
///      解き直しで制約生成が追加する行は、最初に解いたときの 8 倍 (少なくとも `4 n^2` 行) までとし、
///      超えたらその許容誤差では解けなかったものとして扱う。
///   4. どの許容誤差でも満たさない (または LP が数値的に失敗した) ときは、[`MAX_AUTO_RATIONAL_PLAYERS`]
///      人以下なら有理数の逐次 LP ([`exact::nucleolus`]) で求める。値は f64 の 2 進数の値どおりの有理数として扱う。
///      それより多い人数と、有理数の逐次 LP で求められなかったときは、最後に解けた配分を保証
///      [`Guarantee::Approximate`] で返す。
///
///   解き直しと有理数の逐次 LP の分だけ時間がかかる (有理数の逐次 LP は 8 人で 10 秒ほどかかることがある)。
///   [`MAX_AUTO_CHECK_PLAYERS`] 人を超えるゲームは 1. だけを行う。
///   結果の [`NucleolusResult::tolerance`] (返した配分を求めた許容誤差)・[`NucleolusResult::refinements`]
///   (許容誤差を下げて解き直しを試みた回数。失敗した試行を含む)・[`NucleolusResult::is_rational`]
///   で、どう求めたかが分かる。
///
/// # Errors
///
/// - 指定した許容誤差が有限で 0 以上でない、またはゲームの値の大きさで割ると有限でない
///   (値に比べて大きすぎる): [`Error::InvalidArgument`]
/// - 配分集合が空 (`Domain::Imputation`): [`Error::EmptyImputationSet`]
/// - LP が数値的に進まない、または結果 (配分・各段の最大超過) が f64 で表せない: [`Error::Numerical`]
///   (`Auto` では、有理数の逐次 LP でも求められなかった場合)
pub fn nucleolus_with(game: &ExplicitGame, options: Options) -> Result<NucleolusResult> {
    match options.tolerance {
        Tolerance::Auto => solve_auto(game, options.domain, options.method, true),
        fixed => solve_once(game, options.domain, options.method, fixed.fixed(game)?),
    }
}

/// 逐次 LP だけで求めた仁 (プレ仁)。[`Tolerance::Auto`] の手順のうち、有理数の逐次 LP への切り替えをしない。
///
/// 検証 ([`crate::verify`]) が基準の仁として使う (有理数の逐次 LP を使うかは検証の設定で決める)。
pub(crate) fn nucleolus_by_lp(game: &ExplicitGame, domain: Domain) -> Result<NucleolusResult> {
    solve_auto(game, domain, Method::default(), false)
}

/// 許容誤差 `tolerance` (ゲームの値と同じ単位) で 1 回だけ解く。
fn solve_once(
    game: &ExplicitGame,
    domain: Domain,
    method: Method,
    tolerance: f64,
) -> Result<NucleolusResult> {
    let (normalized, divisor) = game.normalized();
    let settings = Settings::new(domain, method, tolerance, divisor)?;
    let result = Sequential::new(&normalized, settings)?.run()?;
    result.scaled_by(divisor)
}

/// [`Tolerance::Auto`] の手順 ([`nucleolus_with`])。`rational_fallback` が偽なら、有理数の逐次 LP に切り替えない。
///
/// 逐次 LP と確認は、値を 2 のべき乗で割ったゲームの上で行う (値が f64 の上限に近いゲームや非正規化数のゲームでも、
/// 超過が溢れたり潰れたりしない)。
fn solve_auto(
    game: &ExplicitGame,
    domain: Domain,
    method: Method,
    rational_fallback: bool,
) -> Result<NucleolusResult> {
    let n = game.players();
    let (normalized, divisor) = game.normalized();
    // 割ったゲームの値の最大絶対値 (`[1, 2)`。全ての値が 0 なら 1)。
    let scale = positive_scale(normalized.max_abs_value());
    let rational_fallback = rational_fallback && n <= MAX_AUTO_RATIONAL_PLAYERS;
    let reading = scale
        * if rational_fallback {
            AUTO_READING_WITH_RATIONAL
        } else {
            AUTO_READING
        };
    // 元の大きさの結果にする。`refinements` は、許容誤差を下げて解き直しを試みた回数。
    let finish = |mut result: NucleolusResult, refinements: usize| {
        result.refinements = refinements;
        result.scaled_by(divisor)
    };
    let mut solved: Option<NucleolusResult> = None;
    let mut failure: Option<Error> = None;
    let mut attempts = 0;
    // 解き直しで追加してよい行の数。最初に解いたときに追加した行の数から決める。
    let mut row_budget = usize::MAX;
    for ratio in AUTO_TOLERANCES {
        attempts += 1;
        let settings = Settings {
            domain,
            tolerance: ratio * scale,
            method,
            row_budget,
        };
        let result =
            match Sequential::new(&normalized, settings).and_then(|mut solver| solver.run()) {
                Ok(result) => result,
                // LP の数値的な失敗と、解き直しの行の上限は、有理数の逐次 LP で解けることがある。
                // それ以外 (入力の誤り、配分集合が空) は解き直しても変わらない。
                Err(error @ (Error::Numerical(_) | Error::Lp(_) | Error::LimitExceeded(_))) => {
                    failure = Some(error);
                    break;
                }
                Err(error) => return Err(error),
            };
        if attempts == 1 {
            row_budget = (REFINEMENT_ROW_FACTOR * result.rows_added).max(4 * n * n);
        }
        // LP を解かずに決まった配分 (1 人ゲーム、配分集合が 1 点) には、確かめる段がない。
        if result.lp_solves == 0
            || n > MAX_AUTO_CHECK_PLAYERS
            || kohlberg_at(&normalized, &result.allocation, domain, reading)
                .is_ok_and(|check| check.satisfied)
        {
            return finish(result, attempts - 1);
        }
        solved = Some(result);
    }
    let refinements = attempts - 1;
    if rational_fallback && let Ok(result) = solve_rationally(game, domain, refinements) {
        return Ok(result);
    }
    match (solved, failure) {
        (Some(mut result), _) => {
            result.guarantee = Guarantee::Approximate;
            finish(result, refinements)
        }
        (None, Some(error)) => Err(error),
        (None, None) => unreachable!("解けたか、失敗を記録したかのどちらか"),
    }
}

/// 有理数の逐次 LP で求め、f64 の結果にする。値は f64 の 2 進数の値どおりの有理数として扱う。
fn solve_rationally(
    game: &ExplicitGame,
    domain: Domain,
    refinements: usize,
) -> Result<NucleolusResult> {
    let solved = exact::nucleolus(&ExactGame::from_explicit(game)?, domain)?;
    let allocation = to_f64(&solved.allocation);
    let levels = to_f64(&solved.levels);
    check_representable(&allocation, "配分")?;
    check_representable(&levels, "最大超過")?;
    Ok(NucleolusResult {
        allocation,
        levels,
        lp_solves: solved.lp_solves,
        rows_added: 0,
        guarantee: Guarantee::Exact,
        tolerance: 0.0,
        refinements,
        rational: true,
    })
}

/// 提携構造 `blocks` (N の分割) のもとでの仁 (`Domain::Imputation`) またはプレ仁。
///
/// 各ブロック `B` で `x(B) = v(B)` を課し、その中で超過ベクトルを辞書式に最小化する
/// (Aumann & Drèze 1974 の提携構造つきの仁)。ブロックが N だけなら、同じ許容誤差で 1 回だけ解いた [`nucleolus_with`] と同じ。
/// 分割になっているかは呼び出し側 ([`crate::partition::CoalitionStructure`]) で確かめる。
///
/// 1 回だけ解く。[`Tolerance::Auto`] は既定の許容誤差 ([`default_tolerance`]) で解く
/// (Kohlberg 基準の確認は、提携構造のない仁だけが行う)。
pub(crate) fn nucleolus_with_structure(
    game: &ExplicitGame,
    blocks: &[Coalition],
    options: Options,
) -> Result<NucleolusResult> {
    let (normalized, divisor) = game.normalized();
    let tolerance = options.tolerance.fixed(game)?;
    let settings = Settings::new(options.domain, options.method, tolerance, divisor)?;
    let lower = block_lower_bounds(&normalized, blocks, settings.domain, settings.tolerance)?;
    if blocks.iter().all(|block| block.len() == 1) {
        // 全員が単独なら x_i = v({i}) に決まる (逐次 LP は 1 段も解かない)。
        return Ok(NucleolusResult::without_lp(
            game.singleton_values(),
            Guarantee::Exact,
            tolerance,
        ));
    }
    let result = Sequential::with_blocks(&normalized, settings, blocks, lower).run()?;
    result.scaled_by(divisor)
}

/// 最小コア: 最大超過を最小化する LP を 1 回解く。
///
/// 1 人ゲームでは配分は `v(N)` だけで、`epsilon` は `f64::NEG_INFINITY` ([`LeastCore::epsilon`])。
/// `epsilon` か配分が f64 で表せない (値が f64 の上限に近く、有限でなくなる) ときは [`Error::Numerical`]。
pub fn least_core(game: &ExplicitGame, domain: Domain) -> Result<LeastCore> {
    let (normalized, divisor) = game.normalized();
    let settings = Settings::new(domain, Method::default(), default_tolerance(game), divisor)?;
    let Some((epsilon, allocation)) = normalized_least_core(&normalized, settings)? else {
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
    settings: Settings,
) -> Result<Option<(f64, Vec<f64>)>> {
    let mut solver = Sequential::new(normalized, settings)?;
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
    let settings = Settings::new(
        Domain::Preimputation,
        Method::default(),
        default_tolerance(game),
        divisor,
    )?;
    let Some((epsilon, x)) = normalized_least_core(&normalized, settings)? else {
        // 1 人ゲームのコアは {v(N)}。
        return Ok(true);
    };
    let tolerance = settings.tolerance;
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
    let rounding = rounding_magnitude(n, game.max_abs_value());
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
    options: Settings,
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
    fn new(game: &'a ExplicitGame, options: Settings) -> Result<Sequential<'a>> {
        let blocks = [game.grand()];
        let lower = block_lower_bounds(game, &blocks, options.domain, options.tolerance)?;
        Ok(Sequential::with_blocks(game, options, &blocks, lower))
    }

    /// 提携構造 `blocks` (N の分割) の各ブロック `B` で `x(B) = v(B)` を課す。
    fn with_blocks(
        game: &'a ExplicitGame,
        options: Settings,
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
            return Ok(NucleolusResult::without_lp(
                vec![self.game.value(self.game.grand())],
                Guarantee::Exact,
                self.tolerance(),
            ));
        }
        self.pin_single_point_blocks();
        if self.span.is_full() {
            // 全てのブロックで配分が 1 点に決まった。LP は解かない。
            return Ok(NucleolusResult::without_lp(
                self.lower
                    .clone()
                    .expect("1 点に決まるのは配分集合の場合だけ"),
                Guarantee::Exact,
                self.tolerance(),
            ));
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
            tolerance: self.tolerance(),
            refinements: 0,
            rational: false,
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
            if self.rows_added > self.options.row_budget {
                return Err(Error::LimitExceeded(format!(
                    "制約生成で追加した行が上限 {} を超えた (許容誤差 {})",
                    self.options.row_budget,
                    self.tolerance()
                )));
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
                let mut options = Options::new(domain);
                options.tolerance = Tolerance::Absolute(tolerance);
                let result = nucleolus_with_structure(&game, &blocks, options);
                assert!(
                    matches!(result, Err(Error::InvalidArgument(_))),
                    "{tolerance} {domain:?}: {result:?}"
                );
            }
        }
    }

    /// 制約生成が追加する行に上限を置くと、超えたところで `LimitExceeded` を返す
    /// (既定の手順の解き直しが、この失敗をその許容誤差では解けなかったものとして扱う)。
    #[test]
    fn row_budget_stops_constraint_generation() {
        let game = generators::bnf(1, 8, 0).unwrap();
        let (normalized, divisor) = game.normalized();
        let tolerance = default_tolerance(&game);
        let solve = |row_budget: usize| {
            let mut settings = Settings::new(
                Domain::Imputation,
                Method::ConstraintGeneration,
                tolerance,
                divisor,
            )
            .unwrap();
            settings.row_budget = row_budget;
            Sequential::new(&normalized, settings).and_then(|mut solver| solver.run())
        };
        let unlimited = solve(usize::MAX).unwrap();
        assert!(unlimited.rows_added > 1, "{}", unlimited.rows_added);
        // ちょうどの上限では解け、1 行少ない上限では打ち切る。
        let exact = solve(unlimited.rows_added).unwrap();
        assert_eq!(exact.allocation, unlimited.allocation);
        let result = solve(unlimited.rows_added - 1);
        assert!(matches!(result, Err(Error::LimitExceeded(_))), "{result:?}");
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
