//! 求めた解の事後検証: 厳密な検証による昇格と、反例による否定。
//!
//! 性質を仮定して求めた結果 ([`Unverified`]) や任意の [`Solution`] を、ゲームの値だけを使って確かめる。
//!
//! - 人数が [`MAX_CERTIFY_PLAYERS`] 以下: 全提携の値の表を作り、有理数の Kohlberg 判定
//!   ([`crate::verify::certify`]) で検証する。合格すれば [`Guarantee::Certified`] に昇格する。
//!   不合格なら、逐次 LP で求めて検証した仁と比べ、離れていれば否定 (誤り) と確定する。
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
pub(crate) use kohlberg::feasibility_violation;
pub use kohlberg::{KohlbergReport, kohlberg};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::ExplicitGame;
use crate::game::exact::{ExactGame, Rational, to_f64};
use crate::game::{PlayerSet, SetFunction, value_scale};
use crate::generators::SplitMix64;
use crate::linalg::max_abs_difference;
use crate::nucleolus::{self, Options};
use crate::solution::Solution;
use crate::solution::{Guarantee, Unverified};

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
    /// ゲームの値を使って検証する。
    pub fn verify<G: SetFunction + ?Sized>(
        self,
        game: &G,
        options: VerifyOptions,
    ) -> Result<Verified> {
        let check = check(game, self.peek(), options)?;
        Ok(self.conclude(check))
    }

    /// 有理数で構築したゲームの値を使って検証する。
    pub fn verify_exact(self, game: &ExactGame, options: VerifyOptions) -> Result<Verified> {
        let check = check_exact(game, self.peek(), options)?;
        Ok(self.conclude(check))
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

/// 解 `solution` がゲーム `game` の解 (仁またはプレ仁) かを確かめる。
pub fn check<G: SetFunction + ?Sized>(
    game: &G,
    solution: &Solution,
    options: VerifyOptions,
) -> Result<Check> {
    let n = game.players();
    let x = &solution.allocation;
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    let domain = solution.concept.domain();
    let tolerance = 1e-6 * value_scale(game);
    if let Some(refuted) = check_feasibility(game, solution)? {
        return Ok(refuted);
    }
    if n <= MAX_CERTIFY_PLAYERS {
        let explicit = ExplicitGame::tabulate(game)?;
        let exact = ExactGame::from_explicit(&explicit)?;
        certify_small(&explicit, &exact, x, domain, tolerance)
    } else if n <= MAX_PAIR_CHECK_PLAYERS {
        Ok(check_pairs(game, x, domain, tolerance, options))
    } else {
        Ok(Check::Undecided(format!(
            "{n} 人は検証できる人数 ({MAX_PAIR_CHECK_PLAYERS}) を超える"
        )))
    }
}

/// 有理数で構築したゲームについて解を確かめる (値を浮動小数点数にしたことによる誤差がない)。
pub fn check_exact(
    exact: &ExactGame,
    solution: &Solution,
    _options: VerifyOptions,
) -> Result<Check> {
    let explicit = exact.to_explicit()?;
    // 実行可能性 (和・個人合理性) は浮動小数点数の値で確かめ、残りは厳密な値で確かめる。
    match check_feasibility(&explicit, solution)? {
        Some(refuted) => Ok(refuted),
        None => {
            let tolerance = 1e-6 * value_scale(&explicit);
            certify_small(
                &explicit,
                exact,
                &solution.allocation,
                solution.concept.domain(),
                tolerance,
            )
        }
    }
}

fn certify_small(
    explicit: &ExplicitGame,
    exact_game: &ExactGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<Check> {
    let report = certify_exact(exact_game, x, domain)?;
    if report.satisfied {
        return Ok(Check::Certified {
            exact: report.allocation,
        });
    }
    // 基準: 逐次 LP の仁。厳密に検証できればその厳密な配分と比べる。
    // 値が浮動小数点の和で作られていると、有理数では等しくない超過が近接して厳密な検証が
    // 成り立たないことがある。その場合は浮動小数点の Kohlberg 判定に合格した LP の仁を基準にし、
    // 数値誤差より十分大きい差 (`100 * tolerance`) があるときだけ否定する。
    let reference = nucleolus::nucleolus_with(explicit, Options::new(explicit, domain))?;
    let exact_reference = certify_exact(exact_game, &reference.allocation, domain)?;
    let (distance, threshold, basis) = if exact_reference.satisfied {
        (
            max_difference(&exact_reference.allocation, x)?,
            tolerance,
            "厳密に検証した仁",
        )
    } else if kohlberg(explicit, &reference.allocation, domain)?.satisfied {
        let distance = max_abs_difference(&reference.allocation, x);
        (distance, 100.0 * tolerance, "浮動小数点で検証した仁")
    } else {
        return Ok(Check::Undecided("候補も基準の仁も検証に合格しない".into()));
    };
    Ok(if distance > threshold {
        Check::Refuted(format!(
            "{basis}との差が {distance:e} ある ({})",
            report.reason.unwrap_or_default()
        ))
    } else {
        Check::Undecided(format!(
            "{basis}との差 {distance:e} は許容誤差以下だが、候補は厳密な検証に合格しない"
        ))
    })
}

/// 和が `v(N)` か、仁なら個人合理性を満たすかを確かめる。満たさなければ否定の結論を返す。
fn check_feasibility<G: SetFunction + ?Sized>(
    game: &G,
    solution: &Solution,
) -> Result<Option<Check>> {
    let n = game.players();
    let x = &solution.allocation;
    if x.len() != n {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {n} と異なる",
            x.len()
        )));
    }
    let tolerance = 1e-6 * value_scale(game);
    let total = game.value(&PlayerSet::full(n));
    if (x.iter().sum::<f64>() - total).abs() > tolerance {
        return Ok(Some(Check::Refuted(format!(
            "配分の和 {} が v(N) = {total} と異なる",
            x.iter().sum::<f64>()
        ))));
    }
    if solution.concept.domain() == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let single = game.value(&PlayerSet::from_players(n, &[i]));
            if *xi < single - tolerance {
                return Ok(Some(Check::Refuted(format!(
                    "プレイヤー {i} の受取 {xi} が v({{{i}}}) = {single} より小さい"
                ))));
            }
        }
    }
    Ok(None)
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
                && x[b] <= game.value(&PlayerSet::from_players(n, &[b])) + tolerance;
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
