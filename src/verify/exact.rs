//! 有理数による厳密な検証 (説明は [`super::certify`])。

use std::cmp::Ordering;

use num_traits::{One, Signed, Zero};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::allocation_defect;
use crate::game::exact::{ExactGame, Rational, format_rational, to_f64, to_rational};
use crate::game::scale::{check_tolerance, positive_scale};
use crate::game::{Coalition, ExplicitGame};
use crate::levels::{Reading, SortedExcess};
use crate::rational::{ExactEchelon, Inserted, indicator, simplex};

/// 厳密な検証の結果。
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ExactReport {
    /// 判定した配分 (復元に失敗した場合は空)。
    pub allocation: Vec<Rational>,
    pub satisfied: bool,
    pub reason: Option<String>,
    /// 平衡性を判定した超過の段の数。
    pub levels_checked: usize,
}

impl ExactReport {
    fn fail(allocation: Vec<Rational>, reason: String, levels_checked: usize) -> ExactReport {
        ExactReport {
            allocation,
            satisfied: false,
            reason: Some(reason),
            levels_checked,
        }
    }
}

/// 有理数による仁・プレ仁の厳密な検証。
///
/// 浮動小数点で求めた配分 `x` を、次の 2 段階で厳密に検証する。
///
/// 1. 復元 ([`recover_allocation`]): `x` での超過の段 (同じ超過を持つ提携の族) を読み取り、
///    「同じ段の提携の超過は等しい」「`x(N) = v(N)`」「下限に張り付いた `x_i = v({i})`」を
///    有理数の連立一次方程式として解き、厳密な配分 `x*` を求める。
///    `x*` が仁なら、Kohlberg 基準の平衡性からこの連立方程式の解は一意になる。
/// 2. 判定 ([`kohlberg_exact`]): `x*` で全提携の超過を有理数で計算し、超過の各段の提携族が
///    平衡であることを有理数の単体法 (Bland の規則) で判定する。
///
/// 判定は復元に使った段の読み取りに依存しない。読み取りを誤っても、誤った `x*` は判定で不合格になる。
/// 仁は一意なので、判定に合格した `x*` が仁であり、`x*` と異なる配分は仁ではない。
///
/// 許容誤差は 2 種類ある。
///
/// - 合格の許容誤差 `T = 1e-6 * max|v(S)|` (全ての値が 0 なら `1e-6`): 判定に合格した `x*` と入力の `x` の差
///   (各成分の差の絶対値の最大。有理数で計算する) が `T` 以下のときだけ合格にする。`x*` が仁でも、
///   `x` が `T` より離れていれば不合格にする。
/// - 段の読み取りの許容誤差: 同じ段とみなす超過の差と、下限に張り付いた (`x_i = v({i})`) とみなす
///   `|x_i - v({i})|` の上限。`T` から 1/10 ずつ小さくしながら、超過を浮動小数点数で計算したときの丸め誤差の目安
///   `4 n ε (max|v(S)| + Σ|x_i|)` (`ε` は `f64::EPSILON`) を下回らない範囲で試し、最後に目安そのものでも試す。
///   判定に合格した最初の `x*` を使う。
///   値の大きいゲームでは超過の差が `T` 以下の別の段があり、`T` で読むとそれらを 1 つの段にまとめて
///   誤った `x*` を復元するためである。読み取りの許容誤差を変えても判定と合格の条件は変わらないので、
///   試す回数を増やしても誤った配分を合格にすることはない。
///   `T` より小さい許容誤差での読み取りは、読み取った段を上から順に合わせた提携族がどれも平衡
///   (浮動小数点数の LP で判定) のときだけ、復元と判定に進む。仁の段はこの条件 (Kohlberg 基準) を満たすので、
///   満たさない読み取りは仁の段ではない。仁でない `x` について、有理数の計算を繰り返さないためである。
///   このため、仁ではないが仁から `T` 以内にある `x` は、`T` での読み取りから仁を復元できなければ不合格になる
///   ([`crate::verify::check`] は逐次 LP の仁からも復元するので、そのような `x` も合格にする)。
///
/// どの読み取りでも合格しなければ不合格 (`satisfied = false`) にする。`allocation` と `reason` には、
/// 判定に合格した `x*` が `x` から離れていた場合はその `x*` と差を、そうでなければ最も大きい許容誤差の読み取りで
/// 判定に不合格になった `x*` とその理由を、どの読み取りからも `x*` を得られなければ空の配分と復元できない旨を入れる。
///
/// ゲームの値は浮動小数点数の 2 進数の値どおりの有理数として扱う。
///
/// # Errors
///
/// `x` の長さがプレイヤー数と異なるか、有限でない成分があれば [`Error::InvalidArgument`]。
/// 段を読む超過に比較できない値 (NaN) があれば [`Error::Numerical`] (有限な値と配分からは生じない。
/// 超過は大きすぎても `±inf` になるだけである)。
pub fn certify(game: &ExplicitGame, x: &[f64], domain: Domain) -> Result<ExactReport> {
    certify_exact(&ExactGame::from_explicit(game)?, x, domain)
}

/// 有理数で構築したゲーム `exact` について、浮動小数点の配分 `x` を検証する ([`certify`] と同じ手順)。
///
/// 段の読み取り (復元) には `exact` の値を浮動小数点数にしたゲームを使い、判定は `exact` の値で行う。
///
/// # Errors
///
/// [`certify`] と同じ。加えて、`exact` の値を浮動小数点数にできなければ (範囲を超える) エラーを返す。
pub fn certify_exact(exact: &ExactGame, x: &[f64], domain: Domain) -> Result<ExactReport> {
    let game = exact.to_explicit()?;
    check_allocation(game.players(), x)?;
    let tolerance = certify_tolerance(&game);
    let sorted = SortedExcess::new(&game, x);
    let mut previous: Option<Reading> = None;
    let mut rejected: Vec<Vec<Rational>> = Vec::new();
    let mut first_failure: Option<ExactReport> = None;
    for (attempt, reading_tolerance) in reading_tolerances(&game, x, tolerance)
        .into_iter()
        .enumerate()
    {
        let Some(reading) = sorted.read(&game, x, domain, reading_tolerance)? else {
            continue;
        };
        // 許容誤差を小さくしても段の区切りが変わらなければ、同じ x* になる。
        if previous.as_ref() == Some(&reading) {
            continue;
        }
        // 2 回目以降の読み取りは、読み取った段が Kohlberg 基準を満たすときだけ復元して判定する。
        let recovered = if attempt == 0 || sorted.levels_are_balanced(&reading, game.players()) {
            solve(&sorted, exact, &reading)
        } else {
            None
        };
        previous = Some(reading);
        let Some(allocation) = recovered else {
            continue;
        };
        if rejected.contains(&allocation) {
            continue;
        }
        let report = kohlberg_exact(exact, &allocation, domain);
        if report.satisfied {
            // 仁は一意なので、別の読み取りで判定に合格する配分もこれと同じになり、合否はここで決まる。
            let (within, distance) = compare_distance(&report.allocation, x, tolerance)?;
            if within {
                return Ok(report);
            }
            return Ok(ExactReport::fail(
                report.allocation,
                format!(
                    "復元した厳密な仁と入力の配分の差 {distance:e} が許容誤差 {tolerance:e} を超える"
                ),
                report.levels_checked,
            ));
        }
        rejected.push(allocation);
        first_failure.get_or_insert(report);
    }
    Ok(first_failure.unwrap_or_else(|| {
        ExactReport::fail(
            Vec::new(),
            "超過の段から厳密な配分を復元できない (段の連立方程式の解が一意でないか、読み取った段が平衡でない)"
                .into(),
            0,
        )
    }))
}

/// 厳密な検証 ([`certify`]・[`crate::verify::check`]) の許容誤差: 値の最大絶対値の `1e-6` 倍 (全て 0 なら `1e-6`)。
pub(crate) fn certify_tolerance(game: &ExplicitGame) -> f64 {
    1e-6 * positive_scale(game.max_abs_value())
}

/// 段の読み取りの許容誤差を 1 回ごとに小さくする比。
const READING_STEP: f64 = 1e-1;

/// 段の読み取りを試す回数の上限。
///
/// 値が 0 でないゲームでは、`tolerance = 1e-6 * max|v(S)|` と丸め誤差の目安 `4 n ε (max|v(S)| + Σ|x_i|)` の比が
/// `1.2e9` 以下なので、目安を下回らない読み取りは 10 回以内、目安そのものを加えて 11 回以内になる。
/// 全ての値と `x` が 0 で目安が 0 になる場合にも止まるよう、上限を置く。
const MAX_READINGS: usize = 12;

/// 段の読み取りに使う許容誤差の列 (大きい順)。`tolerance` から [`READING_STEP`] 倍ずつ小さくし、
/// 超過の丸め誤差の目安を下回る前に止めて、最後に目安そのものを加える。`tolerance` は必ず含める。
///
/// 目安そのものを加えるのは、超過の段の間隔が目安の数倍しかないゲーム (値が 1e12 程度の整数ゲームなど) で、
/// [`READING_STEP`] 倍の列が段の間隔を飛び越えるからである (6 人で値が 6e12 なら目安は 0.064 で、
/// 列は 6 の次が 0.6、その次の 0.06 は目安を下回る。段の間隔が 0.25 のゲームは 0.6 でも分かれない)。
fn reading_tolerances(game: &ExplicitGame, x: &[f64], tolerance: f64) -> Vec<f64> {
    // x(S) は最大 n 項の和なので、超過 v(S) - x(S) の丸め誤差は n ε (max|v| + Σ|x_i|) 程度になる。
    let magnitude = game.max_abs_value() + x.iter().map(|v| v.abs()).sum::<f64>();
    let floor = 4.0 * game.players() as f64 * f64::EPSILON * magnitude;
    let mut readings = vec![tolerance];
    let mut reading = tolerance * READING_STEP;
    while reading > 0.0 && reading >= floor && readings.len() < MAX_READINGS {
        readings.push(reading);
        reading *= READING_STEP;
    }
    if floor > 0.0 && readings.last().is_some_and(|last| *last > floor) {
        readings.push(floor);
    }
    readings
}

/// 配分の長さと有限性を確かめる。
fn check_allocation(n: usize, x: &[f64]) -> Result<()> {
    match allocation_defect(n, x) {
        Some(defect) => Err(Error::InvalidArgument(defect)),
        None => Ok(()),
    }
}

/// 浮動小数点の配分 `x` の超過の段から、厳密な配分を復元する。一意に定まらなければ `None`。
///
/// 未知数は `x_1..x_n` と各段の超過 `alpha_k`。超過の大きい段から順に、段の提携を全て方程式
/// `x(S) + alpha_k = v(S)` として加え、提携の特性ベクトルが `R^n` を張った段で止める。
/// 段の先頭の超過から `tolerance` 以内の提携を同じ段とし、仁 ([`Domain::Imputation`]) では
/// `|x_i - v({i})| <= tolerance` のプレイヤーを下限に張り付いたとみなして `x_i = v({i})` も加える。
///
/// `game` は `exact` の値を浮動小数点数にしたゲーム (段の読み取りに使う)。方程式の右辺は `exact` の値を使う。
///
/// # Errors
///
/// `x` の長さがプレイヤー数と異なる、`x` に有限でない成分がある、`tolerance` が有限で 0 以上でない、
/// または `game` と `exact` のプレイヤー数が異なれば [`Error::InvalidArgument`]。
/// 段を読む超過に比較できない値 (NaN) があれば [`Error::Numerical`] (有限な値と配分からは生じない。
/// 超過は大きすぎても `±inf` になるだけである)。
pub fn recover_allocation(
    game: &ExplicitGame,
    exact: &ExactGame,
    x: &[f64],
    domain: Domain,
    tolerance: f64,
) -> Result<Option<Vec<Rational>>> {
    let n = game.players();
    if exact.players() != n {
        return Err(Error::InvalidArgument(format!(
            "有理数のゲームのプレイヤー数 {} が {n} と異なる",
            exact.players()
        )));
    }
    check_allocation(n, x)?;
    check_tolerance(tolerance)?;
    let sorted = SortedExcess::new(game, x);
    let Some(reading) = sorted.read(game, x, domain, tolerance)? else {
        return Ok(None);
    };
    Ok(solve(&sorted, exact, &reading))
}

/// 読み取った段の連立一次方程式を有理数で解き、配分を返す。矛盾するか一意に定まらなければ `None`。
fn solve(sorted: &SortedExcess, exact: &ExactGame, reading: &Reading) -> Option<Vec<Rational>> {
    let n = exact.players();
    // 未知数 (x_1..x_n, alpha_1..alpha_K) と右辺からなる行を加える。
    let unknowns = n + reading.ends.len();
    let mut echelon = ExactEchelon::default();
    let mut push = |mut row: Vec<Rational>, rhs: Rational| -> bool {
        row.push(rhs);
        !matches!(echelon.insert(row, unknowns), Inserted::Inconsistent)
    };
    let grand = Coalition::grand(n);
    let mut efficiency = indicator(grand, n);
    efficiency.resize(unknowns, Rational::zero());
    if !push(efficiency, exact.value(grand).clone()) {
        return None;
    }
    for &i in &reading.lower_bound {
        let mut row = vec![Rational::zero(); unknowns];
        row[i] = Rational::one();
        if !push(row, exact.value(Coalition::singleton(i)).clone()) {
            return None;
        }
    }
    let mut start = 0;
    for (k, &end) in reading.ends.iter().enumerate() {
        for &mask in &sorted.order[start..end] {
            let coalition = Coalition(mask as u64);
            let mut row = indicator(coalition, n);
            row.resize(unknowns, Rational::zero());
            row[n + k] = Rational::one();
            if !push(row, exact.value(coalition).clone()) {
                return None;
            }
        }
        start = end;
    }
    if echelon.rank() != unknowns {
        return None;
    }
    // 全ての列がピボットなので、既約な行階段形の右辺が解になる。
    let mut solution = vec![Rational::zero(); unknowns];
    for (pivot, row) in &echelon.rows {
        solution[*pivot] = row[unknowns].clone();
    }
    solution.truncate(n);
    Some(solution)
}

/// 有理数の配分 `x` が仁 (プレ仁) であるかを Kohlberg 基準で厳密に判定する。
///
/// `x` の長さがプレイヤー数と異なれば、不成立 (`satisfied = false`) とその理由を返す。
pub fn kohlberg_exact(game: &ExactGame, x: &[Rational], domain: Domain) -> ExactReport {
    let n = game.players();
    if x.len() != n {
        return ExactReport::fail(
            x.to_vec(),
            format!("配分の長さ {} がプレイヤー数 {n} と異なる", x.len()),
            0,
        );
    }
    let grand = Coalition::grand(n);
    let total = x.iter().fold(Rational::zero(), |acc, v| acc + v);
    if &total != game.value(grand) {
        return ExactReport::fail(
            x.to_vec(),
            format!(
                "効率性を満たさない: x(N) = {}, v(N) = {}",
                format_rational(&total),
                format_rational(game.value(grand))
            ),
            0,
        );
    }
    let mut at_lower_bound: Vec<Coalition> = Vec::new();
    if domain == Domain::Imputation {
        for (i, xi) in x.iter().enumerate() {
            let lower = game.value(Coalition::singleton(i));
            match xi.cmp(lower) {
                Ordering::Less => {
                    return ExactReport::fail(
                        x.to_vec(),
                        format!("個人合理性を満たさない: x_{i} < v({{{i}}})"),
                        0,
                    );
                }
                Ordering::Equal => at_lower_bound.push(Coalition::singleton(i)),
                Ordering::Greater => {}
            }
        }
    }

    // 全ての真部分提携の超過を有理数で求め、大きい順に並べる。
    let full = (1usize << n) - 1;
    let mut sums = vec![Rational::zero(); full + 1];
    for mask in 1..=full {
        let lowest = mask.trailing_zeros() as usize;
        sums[mask] = &sums[mask & (mask - 1)] + &x[lowest];
    }
    let excess: Vec<Rational> = (0..=full)
        .map(|mask| game.value(Coalition(mask as u64)) - &sums[mask])
        .collect();
    let mut order: Vec<usize> = (1..full).collect();
    order.sort_by(|a, b| excess[*b].cmp(&excess[*a]));

    let mut collection: Vec<Coalition> = Vec::new();
    // 打ち切りの判定に使う、提携族と `1_N` の張る空間。
    let mut span = ExactEchelon::default();
    span.insert(indicator(grand, n), n);
    let mut levels_checked = 0;
    let mut start = 0;
    while start < order.len() {
        let level = excess[order[start]].clone();
        let mut end = start;
        while end < order.len() && excess[order[end]] == level {
            let coalition = Coalition(order[end] as u64);
            collection.push(coalition);
            span.insert(indicator(coalition, n), n);
            end += 1;
        }
        levels_checked += 1;
        if !is_balanced(&collection, &at_lower_bound, n) {
            return ExactReport::fail(
                x.to_vec(),
                format!("超過 {} 以上の提携族が平衡でない", format_rational(&level)),
                levels_checked,
            );
        }
        // 族と `1_N` が R^n を張って平衡なら、それ以降の段も平衡 (`verify::kohlberg` の説明を参照)。
        if span.rank() == n {
            break;
        }
        start = end;
    }
    ExactReport {
        allocation: x.to_vec(),
        satisfied: true,
        reason: None,
        levels_checked,
    }
}

/// `collection` に重み 1 以上、`optional` に重み 0 以上を付けて `sum lambda_S 1_S = mu 1_N` とできるか。
///
/// `lambda_S = 1 + z_S` (`z_S >= 0`) と置き換え、各プレイヤー `i` について
/// `sum_{S ∋ i} z_S + sum_{T ∋ i} lambda_T - mu = -|{S in collection : i in S}|` の非負解を探す。
fn is_balanced(collection: &[Coalition], optional: &[Coalition], n: usize) -> bool {
    let extra: Vec<Coalition> = optional
        .iter()
        .filter(|c| !collection.contains(c))
        .copied()
        .collect();
    let columns: Vec<Coalition> = collection.iter().chain(&extra).copied().collect();
    let mut rows = Vec::with_capacity(n);
    let mut rhs = Vec::with_capacity(n);
    for i in 0..n {
        let mut row: Vec<Rational> = columns
            .iter()
            .map(|c| {
                if c.contains(i) {
                    Rational::one()
                } else {
                    Rational::zero()
                }
            })
            .collect();
        row.push(-Rational::one()); // mu
        rows.push(row);
        let count = collection.iter().filter(|c| c.contains(i)).count();
        rhs.push(-Rational::from_integer(num_bigint::BigInt::from(count)));
    }
    simplex::is_feasible(&rows, &rhs)
}

/// `a` と `b` の差の最大値 (有理数で計算し、浮動小数点数で返す)。
pub fn max_difference(a: &[Rational], b: &[f64]) -> Result<f64> {
    Ok(to_f64(&[max_difference_exact(a, b)?])[0])
}

/// `a` と `b` の差の最大値 (有理数)。
fn max_difference_exact(a: &[Rational], b: &[f64]) -> Result<Rational> {
    let mut worst = Rational::zero();
    for (x, y) in a.iter().zip(b) {
        let difference = (x - to_rational(*y)?).abs();
        if difference > worst {
            worst = difference;
        }
    }
    Ok(worst)
}

/// `a` と `b` の差の最大値が `tolerance` 以下か (有理数で比べる) と、差を浮動小数点数にしたもの (表示用)。
///
/// 差を浮動小数点数に丸めてから比べると、`tolerance` をわずかに超える差が `tolerance` に丸められて
/// 以下と判定されうるので、有理数のまま比べる。
pub(crate) fn compare_distance(a: &[Rational], b: &[f64], tolerance: f64) -> Result<(bool, f64)> {
    let distance = max_difference_exact(a, b)?;
    let within = distance <= to_rational(tolerance)?;
    Ok((within, to_f64(&[distance])[0]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nucleolus;

    fn r(numerator: i64, denominator: i64) -> Rational {
        Rational::new(numerator.into(), denominator.into())
    }

    #[test]
    fn recovers_exact_ferguson_nucleolus() {
        // 文献の例 (tests/literature.rs): 仁は (8/3, 2/3, 5/3)。
        let game = ExplicitGame::from_lex(&[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0]).unwrap();
        let x = nucleolus::nucleolus(&game).unwrap().allocation;
        let report = certify(&game, &x, Domain::Imputation).unwrap();
        assert!(report.satisfied, "{report:?}");
        assert_eq!(report.allocation, vec![r(8, 3), r(2, 3), r(5, 3)]);
    }

    #[test]
    fn rejects_non_nucleolus_exactly() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let exact = ExactGame::from_explicit(&game).unwrap();
        assert!(kohlberg_exact(&exact, &[r(2, 1), r(2, 1), r(0, 1)], Domain::Imputation).satisfied);
        let report = kohlberg_exact(&exact, &[r(1, 1), r(3, 1), r(0, 1)], Domain::Imputation);
        assert!(!report.satisfied);
        // 1/10^12 ずらしただけでも厳密には不合格になる。
        let tiny = r(1, 1_000_000_000_000);
        let report = kohlberg_exact(
            &exact,
            &[r(2, 1) + &tiny, r(2, 1) - &tiny, r(0, 1)],
            Domain::Imputation,
        );
        assert!(!report.satisfied);
    }

    #[test]
    fn reading_tolerances_decrease_and_are_bounded() {
        let game = ExplicitGame::from_binary(&[0.0, 1e7, 1e7 + 2.0]).unwrap();
        let tolerance = certify_tolerance(&game);
        let readings = reading_tolerances(&game, &[1.0, 1e7 + 1.0], tolerance);
        assert_eq!(readings[0], tolerance);
        assert!(readings.windows(2).all(|pair| pair[1] < pair[0]));
        // 1/10 より粗い飛ばし方をせず、最後に丸め誤差の目安そのものを試す。
        assert!(
            readings.windows(2).all(|pair| pair[1] >= pair[0] * 0.099),
            "{readings:?}"
        );
        let floor = 4.0 * 2.0 * f64::EPSILON * (1e7 + 2.0 + 1.0 + 1e7 + 1.0);
        assert_eq!(*readings.last().unwrap(), floor, "{readings:?}");
        assert!(readings.len() < MAX_READINGS, "{readings:?}");
        // 全ての値と配分が 0 なら丸め誤差の目安も 0 になるが、上限で止まる。
        let zero = ExplicitGame::from_binary(&[0.0; 7]).unwrap();
        let readings = reading_tolerances(&zero, &[0.0; 3], certify_tolerance(&zero));
        assert_eq!(readings.len(), MAX_READINGS);
    }

    #[test]
    fn distance_is_compared_exactly() {
        // 1 + 2^-60 は浮動小数点数にすると 1 に丸められるが、許容誤差 1 を超える。
        let above = [r(1, 1) + r(1, 1 << 60)];
        assert_eq!(compare_distance(&above, &[0.0], 1.0).unwrap(), (false, 1.0));
        assert_eq!(
            compare_distance(&[r(1, 1)], &[0.0], 1.0).unwrap(),
            (true, 1.0)
        );
    }
}
