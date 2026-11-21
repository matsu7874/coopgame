//! 求めた解の事後検証: 厳密な検証による昇格と、反例による否定。
//!
//! 配分が仁 (プレ仁) かを、ゲームの値だけを使って確かめる。
//!
//! - 判定: [`check`]・[`check_exact`] は、任意の配分を判定する。保証とは関係しない。
//! - 昇格: [`Unverified::verify`]・[`Unverified::verify_exact`] は、性質を仮定して求めた結果を
//!   判定し、合格すれば [`Guarantee::Certified`] を付ける。保証が `Certified` になるのはここだけ。
//!
//! - 人数が [`MAX_CERTIFY_PLAYERS`] 以下: 全提携の値の表を作り、有理数の Kohlberg 判定
//!   ([`crate::verify::certify`]) で検証する。合格すれば [`Guarantee::Certified`] に昇格する。
//!   不合格なら、逐次 LP で求めて検証した仁と比べる。基準の仁が厳密な検証に合格し、候補との差が許容誤差以下なら
//!   その厳密な仁で合格とし、離れていれば否定 (誤り) と確定する。基準の仁を求められないなど、
//!   検証自体が成り立たないときは未決とする。
//! - それより多い人数 ([`MAX_PAIR_CHECK_PLAYERS`] まで): 仁はカーネルに、プレ仁はプレカーネルに属する
//!   (Davis & Maschler 1965、Maschler, Peleg & Shapley 1979) ので、ランダムに選んだ組 `(i, j)` について
//!   最大余剰 `s_ij`, `s_ji` を総当たりで (`2^(n-2)` 個の提携で) 求め、カーネル条件を確かめる。
//!   破れていれば否定と確定する。破れていなくても正しいとは言えないので、未決 (仮定付きのまま) とする。
//!
//! 個別の判定も公開する。
//!
//! - [`kohlberg`] は、浮動小数点の配分が仁 (プレ仁) であるかを Kohlberg 基準で判定する。
//! - [`certify`]・[`certify_exact`] は、浮動小数点の配分から厳密な配分を復元し、有理数で Kohlberg 基準を判定する。
//! - [`kohlberg_exact`] は、有理数の配分を有理数で判定する。
//!
//! カーネル条件の判定は [`crate::kernel::is_in_kernel`]、交渉集合は [`crate::bargaining`] にある。

mod exact;
mod kohlberg;

pub use exact::{
    ExactReport, certify, certify_exact, kohlberg_exact, max_difference, recover_allocation,
};
use exact::{certify_tolerance, compare_distance};
pub use kohlberg::{KohlbergReport, kohlberg};

use num_traits::{Signed, Zero};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::{check_imputation_set, feasibility_violation};
use crate::game::exact::{ExactGame, Rational, to_f64};
use crate::game::{
    Coalition, ExplicitGame, PlayerSet, SetFunction, default_tolerance, value_scale,
};
use crate::linalg::max_abs_difference;
use crate::nucleolus::{self, Options};
use crate::rng::SplitMix64;
use crate::solution::{Guarantee, Solution, Unverified};

/// 厳密な検証を行う人数の上限 (全提携の値の表と、有理数の LP を使う)。
pub const MAX_CERTIFY_PLAYERS: usize = 20;

/// 組ごとのカーネル条件の総当たりを行う人数の上限 (1 組あたり `2^(n-2)` 回の評価)。
pub const MAX_PAIR_CHECK_PLAYERS: usize = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct VerifyOptions {
    /// 多人数のときに調べる組の数。
    pub pairs: usize,
    pub seed: u64,
}

impl Default for VerifyOptions {
    fn default() -> VerifyOptions {
        VerifyOptions { pairs: 8, seed: 0 }
    }
}

/// 検証の結論。
#[derive(Clone, Debug, PartialEq)]
pub enum Check {
    /// 厳密な検証に合格した。厳密な配分 (有理数) を持つ。
    Certified { exact: Vec<Rational> },
    /// 解でないと確定した。理由を持つ。
    Refuted(String),
    /// 正しいとも誤りとも確定できなかった。理由を持つ。
    Undecided(String),
}

/// 仮定付きの結果を検証した結論。
#[derive(Clone, Debug, PartialEq)]
pub enum Verified {
    /// 厳密な検証に合格し、[`Guarantee::Certified`] に昇格した。配分は厳密な配分を浮動小数点数にしたもの。
    Certified(Solution),
    /// 解でないと確定した (保証は仮定付きのまま)。
    Refuted { solution: Solution, reason: String },
    /// 確定できなかった。仮定付きのまま返す。
    Undecided {
        solution: Unverified<Solution>,
        reason: String,
    },
}

impl Unverified<Solution> {
    /// ゲームの値を使って検証する (判定は [`check`] と同じ)。
    ///
    /// 結果を求めたときと同じゲームを渡すこと。検証はゲームの値だけを使うので、別のゲームを渡すと、
    /// 配分がそのゲームの仁 (プレ仁) かどうかを判定する (合格すれば、渡したゲームの厳密な仁が
    /// [`Verified::Certified`] の解になる)。
    ///
    /// `self` は借りるだけなので、エラーを返しても結果は手元に残る。
    ///
    /// # Errors
    ///
    /// 入力が不正なときだけエラーを返す ([`check`] と同じ): `game` のプレイヤー数が配分の長さと異なる場合と、
    /// [`MAX_CERTIFY_PLAYERS`] 人以下で全提携の値の表を作れない (有限でない値がある) 場合。
    /// 基準の仁を求める LP が失敗したなど、検証が成り立たないときは [`Verified::Undecided`] を返す。
    pub fn verify<G: SetFunction + ?Sized>(
        &self,
        game: &G,
        options: VerifyOptions,
    ) -> Result<Verified> {
        let solution = self.peek();
        let check = check(game, &solution.allocation, solution.domain, options)?;
        Ok(self.clone().conclude(check))
    }

    /// 有理数で構築したゲームの値を使って検証する (判定は [`check_exact`] と同じ)。
    ///
    /// [`Unverified::verify`] と同じく、結果を求めたときと同じゲームを渡すこと。
    ///
    /// # Errors
    ///
    /// 入力が不正なときだけエラーを返す ([`check_exact`] と同じ): `game` のプレイヤー数が配分の長さと異なる場合と、
    /// `game` の値を浮動小数点数にできない (範囲を超える) 場合。
    pub fn verify_exact(&self, game: &ExactGame, options: VerifyOptions) -> Result<Verified> {
        let solution = self.peek();
        let check = check_exact(game, &solution.allocation, solution.domain, options)?;
        Ok(self.clone().conclude(check))
    }

    fn conclude(self, check: Check) -> Verified {
        match check {
            Check::Certified { exact } => {
                let mut solution = self.accept_unverified();
                solution.allocation = to_f64(&exact);
                solution.guarantee = Guarantee::Certified;
                Verified::Certified(solution)
            }
            Check::Refuted(reason) => Verified::Refuted {
                solution: self.accept_unverified(),
                reason,
            },
            Check::Undecided(reason) => Verified::Undecided {
                solution: self,
                reason,
            },
        }
    }
}

/// 配分 `x` がゲーム `game` の仁 (`domain` が [`Domain::Imputation`]) か
/// プレ仁 ([`Domain::Preimputation`]) かを確かめる。
///
/// 許容誤差 `T` は、[`MAX_CERTIFY_PLAYERS`] 人以下では全提携の値の最大絶対値の `1e-6` 倍 ([`certify`] と同じ)、
/// それより多い人数では [`value_scale`] の `1e-6` 倍。
///
/// [`MAX_CERTIFY_PLAYERS`] 人以下での結論は次のとおり。
///
/// - `Certified`: 有理数の Kohlberg 判定 ([`kohlberg_exact`]) に合格した厳密な配分 `x*` があり、`x` との差
///   (各成分の差の絶対値の最大を有理数で測る) が `T` 以下。`x*` は、`x` から復元したもの ([`certify`])、
///   または `x` が [`certify`] に合格しないときに、逐次 LP で求めた仁から復元したもの。仁は一意なので、
///   どちらも同じ仁である。
/// - `Refuted`: 和や個人合理性が `T` を超えて破れている、`x` に有限でない成分がある、仁で配分集合が空である、
///   または基準の仁から離れている。配分集合は、逐次 LP ([`nucleolus::nucleolus`]) と同じく、`sum v({i})` が
///   `v(N)` を既定の許容誤差 ([`default_tolerance`]) を超えて上回るときに空とする。基準の仁が厳密に検証できれば
///   差が `T` を超えるとき、浮動小数点でしか検証できなければ差が `100 T` を超えるときに否定する。
/// - `Undecided`: 上のどれにも当たらない (基準の仁を求める LP が失敗した、基準も検証に合格しない、
///   浮動小数点でだけ検証した基準との差が `100 T` 以下など)。仁で、2 進数の値どおりの有理数では
///   `sum v({i}) > v(N)` だが、差が既定の許容誤差以下の場合も未決にする (`0.1 + 0.2 + 0.3` と `0.6` のように、
///   10 進数では等しい値で起こる)。逐次 LP はこのゲームを配分集合が 1 点のゲームとして解くが、
///   有理数の値では配分が存在せず、厳密な判定に合格する配分がないためである。
///
/// 21 人から [`MAX_PAIR_CHECK_PLAYERS`] 人まではカーネル条件の反例で否定するか未決、それより多いと未決にする。
///
/// # Errors
///
/// 入力が不正なときだけエラーを返す: `x` の長さがプレイヤー数と異なれば [`Error::InvalidArgument`]、
/// [`MAX_CERTIFY_PLAYERS`] 人以下で全提携の値の表を作れない (有限でない値がある) 場合はその表を作るときのエラー。
pub fn check<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    domain: Domain,
    options: VerifyOptions,
) -> Result<Check> {
    let n = game.players();
    check_length(n, x)?;
    if n <= MAX_CERTIFY_PLAYERS {
        let explicit = ExplicitGame::tabulate(game)?;
        let exact = ExactGame::from_explicit(&explicit)?;
        Ok(check_small(&explicit, &exact, x, domain, Values::Float))
    } else if n <= MAX_PAIR_CHECK_PLAYERS {
        let tolerance = 1e-6 * value_scale(game);
        if let Some(refuted) = check_feasibility(game, x, domain, tolerance) {
            return Ok(refuted);
        }
        Ok(check_pairs(game, x, domain, tolerance, options))
    } else {
        Ok(Check::Undecided(format!(
            "{n} 人は検証できる人数 ({MAX_PAIR_CHECK_PLAYERS}) を超える"
        )))
    }
}

/// 有理数で構築したゲームについて配分を確かめる (値を浮動小数点数にしたことによる誤差がない)。
///
/// 結論の意味は [`check`] の [`MAX_CERTIFY_PLAYERS`] 人以下の場合と同じ (人数によらず全提携の値の表で検証する)。
/// ただし、仁の配分集合が空かどうかは有理数の値で判定し、`sum v({i}) > v(N)` ならわずかな差でも否定する。
/// 和・個人合理性の確認と段の読み取りには値を浮動小数点数にしたゲームを、判定には有理数の値を使う。
///
/// # Errors
///
/// 入力が不正なときだけエラーを返す: `x` の長さがプレイヤー数と異なれば [`Error::InvalidArgument`]、
/// `exact` の値を浮動小数点数にできない (範囲を超える) 場合はその変換のエラー。
pub fn check_exact(
    exact: &ExactGame,
    x: &[f64],
    domain: Domain,
    _options: VerifyOptions,
) -> Result<Check> {
    let explicit = exact.to_explicit()?;
    check_length(explicit.players(), x)?;
    Ok(check_small(&explicit, exact, x, domain, Values::Rational))
}

/// 検証するゲームの値の由来。配分集合が空かどうかの判定を決める。
#[derive(Clone, Copy, PartialEq)]
enum Values {
    /// 浮動小数点数のゲーム。逐次 LP ([`nucleolus::nucleolus`]) と同じく、`sum v({i})` が `v(N)` を
    /// 既定の許容誤差を超えて上回るときに配分集合を空とする。
    Float,
    /// 有理数で作ったゲーム。値に誤差がないので、`sum v({i}) > v(N)` なら配分集合を空とする。
    Rational,
}

/// 全提携の値の表を使う検証。`explicit` は `exact` の値を浮動小数点数にしたゲーム。`x` の長さは確かめてあること。
fn check_small(
    explicit: &ExplicitGame,
    exact: &ExactGame,
    x: &[f64],
    domain: Domain,
    values: Values,
) -> Check {
    // 有理数の値で sum v({i}) が v(N) を超える量。正なら、有理数の値では配分集合が空で、仁は存在しない。
    let gap = match domain {
        Domain::Imputation => imputation_gap(exact),
        Domain::Preimputation => Rational::zero(),
    };
    if gap.is_positive() {
        let solver_tolerance = default_tolerance(explicit);
        let solver_solves = values == Values::Float
            && check_imputation_set(explicit, domain, solver_tolerance).is_ok();
        if !solver_solves {
            return Check::Refuted(match values {
                Values::Float => format!(
                    "配分集合が空 (sum v({{i}}) が v(N) を既定の許容誤差 {solver_tolerance:e} を超えて上回る) なので、\
                     仁は存在しない"
                ),
                Values::Rational => {
                    "配分集合が空 (有理数の値で sum v({i}) > v(N)) なので、仁は存在しない".into()
                }
            });
        }
    }
    let tolerance = certify_tolerance(explicit);
    // 実行可能性 (和・個人合理性) は浮動小数点数の値で確かめ、残りは厳密な値で確かめる。
    if let Some(refuted) = check_feasibility(explicit, x, domain, tolerance) {
        return refuted;
    }
    if gap.is_positive() {
        // 逐次 LP は配分集合を 1 点とみなして解くが、有理数の値では効率性と個人合理性を同時に満たす配分がなく、
        // 厳密な判定に合格する配分は存在しない。
        return Check::Undecided(format!(
            "2 進数の値では sum v({{i}}) が v(N) を {:e} 上回り、配分集合が空になる。差が既定の許容誤差以下なので\
             逐次 LP は配分集合を 1 点とみなすが、厳密には検証できない",
            to_f64(&[gap])[0]
        ));
    }
    certify_small(explicit, exact, x, domain, tolerance)
}

/// 有理数の値での `sum v({i}) - v(N)`。正なら配分集合が空で、仁が存在しない。
fn imputation_gap(exact: &ExactGame) -> Rational {
    let n = exact.players();
    let lower = (0..n).fold(Rational::zero(), |sum, i| {
        sum + exact.value(Coalition::singleton(i))
    });
    lower - exact.value(Coalition::grand(n))
}

/// 和・個人合理性を満たす有限な配分 `x` を厳密に検証し、不合格なら逐次 LP で求めた仁と比べる。
///
/// `Certified` を返すのは、有理数の Kohlberg 判定に合格した配分 (仁) と `x` の差が `tolerance` 以下のときだけ。
/// 検証の途中の失敗 (LP が解けないなど) は `Undecided` にする。
fn certify_small(
    explicit: &ExplicitGame,
    exact_game: &ExactGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Check {
    let report = match certify_exact(exact_game, x, domain) {
        Ok(report) => report,
        Err(error) => return Check::Undecided(format!("厳密な検証を続行できない: {error}")),
    };
    if report.satisfied {
        return Check::Certified {
            exact: report.allocation,
        };
    }
    let reason = report.reason.unwrap_or_default();
    // 基準: 逐次 LP の仁。厳密に検証できればその厳密な配分と比べる。
    let reference = match nucleolus::nucleolus_with(explicit, Options::new(explicit, domain)) {
        Ok(reference) => reference.allocation,
        Err(error) => {
            return Check::Undecided(format!(
                "候補は厳密な検証に合格せず ({reason})、比べる基準の仁も求められない: {error}"
            ));
        }
    };
    if let Ok(exact_reference) = certify_exact(exact_game, &reference, domain)
        && exact_reference.satisfied
    {
        // 判定に合格した配分は仁 (一意) なので、x との差だけで合否が決まる。
        return match compare_distance(&exact_reference.allocation, x, tolerance) {
            Ok((true, _)) => Check::Certified {
                exact: exact_reference.allocation,
            },
            Ok((false, distance)) => Check::Refuted(format!(
                "厳密に検証した仁との差が {distance:e} ある ({reason})"
            )),
            Err(error) => Check::Undecided(format!("厳密な仁との差を測れない: {error}")),
        };
    }
    // 値が浮動小数点の和で作られていると、有理数では等しくない超過が近接して厳密な検証が
    // 成り立たないことがある。その場合は浮動小数点の Kohlberg 判定に合格した LP の仁を基準にし、
    // 数値誤差より十分大きい差 (`100 * tolerance`) があるときだけ否定する。
    match kohlberg(explicit, &reference, domain) {
        Ok(kohlberg_report) if kohlberg_report.satisfied => {
            let distance = max_abs_difference(&reference, x);
            if distance > 100.0 * tolerance {
                Check::Refuted(format!(
                    "浮動小数点で検証した仁との差が {distance:e} ある ({reason})"
                ))
            } else {
                Check::Undecided(format!(
                    "浮動小数点で検証した仁との差 {distance:e} は否定できる大きさ (許容誤差の 100 倍) 以下だが、\
                     候補は厳密な検証に合格しない ({reason})"
                ))
            }
        }
        Ok(_) => Check::Undecided(format!("候補も基準の仁も検証に合格しない ({reason})")),
        Err(error) => Check::Undecided(format!(
            "候補は厳密な検証に合格せず ({reason})、基準の仁の検証も続行できない: {error}"
        )),
    }
}

fn check_length(n: usize, x: &[f64]) -> Result<()> {
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    Ok(())
}

/// 和が `v(N)` か、仁なら個人合理性を満たすかを確かめる。満たさなければ否定の結論を返す。
fn check_feasibility<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Option<Check> {
    feasibility_violation(game, x, domain, tolerance).map(Check::Refuted)
}

/// 最大余剰 `s_ij = max { e(S, x) : i in S, j not in S }` を総当たりで求める。
pub(crate) fn max_surplus_exhaustive<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    i: usize,
    j: usize,
) -> f64 {
    let n = game.players();
    let others: Vec<usize> = (0..n).filter(|&k| k != i && k != j).collect();
    let mut best = f64::NEG_INFINITY;
    for mask in 0u64..(1u64 << others.len()) {
        let mut members = vec![i];
        members.extend(
            others
                .iter()
                .enumerate()
                .filter(|(bit, _)| mask >> bit & 1 == 1)
                .map(|(_, &k)| k),
        );
        let set = PlayerSet::from_players(n, &members);
        best = best.max(game.value(&set) - set.sum(x));
    }
    best
}

fn check_pairs<G: SetFunction + ?Sized>(
    game: &G,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
    options: VerifyOptions,
) -> Check {
    let n = game.players();
    let mut rng = SplitMix64::new(options.seed);
    for _ in 0..options.pairs {
        let i = rng.range(0, n as u64 - 1) as usize;
        let mut j = rng.range(0, n as u64 - 2) as usize;
        if j >= i {
            j += 1;
        }
        let (forward, backward) = (
            max_surplus_exhaustive(game, x, i, j),
            max_surplus_exhaustive(game, x, j, i),
        );
        // s_ij > s_ji なら j は i に払うべきで、j が下限に張り付いていない限りカーネル条件が破れる。
        for (a, b, gap) in [(i, j, forward - backward), (j, i, backward - forward)] {
            let at_lower = domain == Domain::Imputation
                && x[b] <= game.value(&PlayerSet::singleton(n, b)) + tolerance;
            if gap > tolerance && !at_lower {
                return Check::Refuted(format!(
                    "組 ({a}, {b}) でカーネル条件が成り立たない: s_ab - s_ba = {gap:e}"
                ));
            }
        }
    }
    Check::Undecided(format!(
        "{} 組でカーネル条件の反例は見つからなかった (正しいとは確定しない)",
        options.pairs
    ))
}
