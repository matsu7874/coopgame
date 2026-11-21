//! 超過の測り方を変えた仁 (per capita 仁・比例仁・modiclus・disruption nucleolus・anti-nucleolus) を求める。
//! anti-nucleolus 以外は、一般化した逐次 LP で求める。
//!
//! 仁は「提携の不満 (超過) のベクトルを辞書式に最小化する配分」だった。不満をアフィン関数
//!
//! ```text
//! f_k(x) = (c_k - a_k . x) / w_k    (w_k > 0)
//! ```
//!
//! に一般化しても、逐次 LP (Kopelowitz 方式) はそのまま使える。
//!
//! - 第 k 段では、未確定の不満の最大値 `t` を最小化する: `c_k - a_k . x <= w_k t`。
//! - 最適値 `t*` を達成する不満のうち、最適解集合全体で `t*` に固定されるものを確定させる。
//!   確定の判定は、最適解集合の上で `a_k . x` を最大化する LP で行う。
//! - 確定した不満の係数 `a_k` が張る空間に入った不満は、以降は定数なので外す。
//! - 張る空間が `R^n` になれば配分は一意に決まる。不満が尽きても `R^n` にならなければ、
//!   辞書式最小は一意ではないのでエラーを返す。
//!
//! 許容誤差はゲームの値と同じ単位 (超過の単位) で、不満 `f_k` そのもの (比になりうる) ではなく
//! `c_k - a_k . x - w_k t` (段 `t` からの超過のずれ) に当てる。こうすると重み `w_k` の大きさによらず
//! 同じ基準で等号を判定できる。LP は絶対誤差で判定するので、定数 (`c_k`・配分集合の値) と重みをそれぞれ
//! 2 のべき乗で割って最大絶対値を `[1, 2)` にそろえてから解き、結果を元の大きさに戻す。
//!
//! 不満の選び方は文献の定義に合わせる (根拠は各関数の文書と `docs/references.md`):
//!
//! | 関数 | 不満 | 領域 |
//! |---|---|---|
//! | [`per_capita_nucleolus`] | `e(S, x) / abs(S)` (空でない真部分提携) | 指定。原典 (Young, Okada & Hashimoto 1982 の weak nucleolus、Young 1985 の per capita nucleolus) は準配分。配分は個人合理性を課した形 |
//! | [`proportional_nucleolus`] | `e(S, x) / v(S)` (`v(S) > 0` の真部分提携)。`v(S) = 0` の提携は制約 `x(S) >= 0` と、前者の後に辞書式に最小化する `-x(S)` | 指定。原典 (Young, Okada & Hashimoto 1982 の式 (9)) は準配分。配分は個人合理性を課した形。非負のゲームに限る |
//! | [`modiclus`] | `e(S, x) - e(T, x)` (`∅` と `N` を含む全ての提携の組。準配分の上で定数になる `S = T` と `{S, T} = {∅, N}` は除く) | 準配分 |
//! | [`disruption_nucleolus`] | `e(S, x) / b(S)`、`b(S) = v(N) - v(S) - v(N \ S)` (`b(S) > 0` の真部分提携) | コア (空でないゲームに限る) |
//!
//! 比例仁は不満を優先順の 2 組に分け、前の組を辞書式に最小化した配分の集合の上で後の組を辞書式に最小化する。
//!
//! disruption nucleolus は、提携 `S` の抜ける傾向 (英: propensity to disrupt)
//! `d(S, x) = (x(N \ S) - v(N \ S)) / (x(S) - v(S))` を大きい順に並べたベクトルを辞書式に最小化した配分である
//! (式は Gilles & Mallozzi, arXiv:2208.10189 の Definition 3.2 と Lemaire 1984 の 4.4 節で確認。
//! 提案した Littlechild & Vaidya 1976 の本文は未確認)。
//! コアの上では `x(N \ S) = v(N) - x(S)` なので、`d(S, x)` は不満 `f = (v(S) - x(S)) / b(S)` を使って
//! `-1/f - 1` と書ける (`x(S) ≠ v(S)` のとき)。この関数は `f < 0` で狭義単調増加なので、`d` の辞書式最小化は
//! 上の不満の辞書式最小化と同じになる (式の変形と狭義単調性は `proofs/` の `disruption_eq`・`disruption_strictMono`。
//! 狭義単調な変換で辞書式最小化の解が変わらないことは、証明の対象にしていない)。
//!
//! anti-prenucleolus は、超過を小さい順に並べたベクトルを準配分の上で辞書式に最大化した配分である。
//! 双対ゲーム `v*(S) = v(N) - v(N \ S)` では `e_{v*}(S, x) = -e_v(N \ S, x)` なので、
//! anti-prenucleolus は双対ゲームのプレ仁に一致する (Funaki & Meinhardt 2006。Sudhölter & Peleg の
//! Working Paper 262 は antinucleolus を `ν(v*)` (双対ゲームのプレ仁) と定義している)。
//! プレ仁は一般に双対ゲームのプレ仁と一致しないので、anti-prenucleolus はプレ仁とは別の解である。
//! anti-nucleolus は、同じ最大化を anti-imputation の集合 `{x : x(N) = v(N), x_i >= M_i}`
//! (`M_i = v(N) - v(N \ {i})`、双対ゲームの配分集合。Meinhardt "On the Replication of the Pre-Kernel and
//! Related Solutions" の式 (7.1)) の上で行った配分で、双対ゲームの仁に一致する。
//!
//! 不満の数は per capita 仁・比例仁で `2^n`、modiclus で約 `4^n` なので、明示的に全ての行を LP に入れる。
//! LP の時間とメモリが人数に対して指数的に増えるので、関数ごとにプレイヤー数の上限
//! ([`MAX_PER_CAPITA_PLAYERS`]・[`MAX_PROPORTIONAL_PLAYERS`]・[`MAX_DISRUPTION_PLAYERS`]・[`MAX_MODICLUS_PLAYERS`]) を設け、
//! 超えれば [`Error::TooManyPlayers`] を返す。per capita 仁・比例仁・disruption nucleolus の上限は、計測したゲームで
//! 1 回の計算が 1 分以内・最大メモリ 1 GB 以内に収まる人数である (計測は `docs/performance.md`)。
//! 比例仁は、`v(S) = 0` の提携が多いゲームでは行が増え、上限の人数でメモリがこれを超えることがある。

use microlp::{Problem, Variable};

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::lower_bounds;
use crate::game::scale::{check_tolerance, normalized_tolerance, normalizer, rounding_magnitude};
use crate::game::{Coalition, ExplicitGame, PlayerSet, default_tolerance};
use crate::linalg::{Span, indicator};
use crate::lp::{self, Cmp, Counter};
use crate::solution::Guarantee;

/// [`modiclus`] が扱うプレイヤー数の上限 (不満の数は `4^n - 2^n - 2`)。
pub const MAX_MODICLUS_PLAYERS: usize = 7;

/// [`per_capita_nucleolus`] が扱うプレイヤー数の上限 (不満の数は `2^n - 2`)。
pub const MAX_PER_CAPITA_PLAYERS: usize = 17;

/// [`proportional_nucleolus`] が扱うプレイヤー数の上限。LP の行は、`v(S) > 0` の提携ごとに不満が 1 つ、
/// `v(S) = 0` の提携ごとに制約と不満が 1 つずつで、`2^n - 2` 個から、その 2 倍までの間になる。
pub const MAX_PROPORTIONAL_PLAYERS: usize = 17;

/// [`disruption_nucleolus`] が扱うプレイヤー数の上限 (コアの制約が `2^n - 2` 個、不満がその数以下)。
pub const MAX_DISRUPTION_PLAYERS: usize = 16;

/// 不満 `f(x) = (constant - coefficients . x) / weight`。`weight` は正。
#[derive(Clone, Debug, PartialEq)]
pub struct Complaint {
    pub coefficients: Vec<f64>,
    pub constant: f64,
    pub weight: f64,
}

impl Complaint {
    pub fn value(&self, x: &[f64]) -> f64 {
        (self.constant - dot(&self.coefficients, x)) / self.weight
    }
}

/// 配分の集合: `sum x = total`、`x_i >= lower_i` (下限がある場合)、`a . x >= b` の制約。
#[derive(Clone, Debug, PartialEq)]
pub struct Feasible {
    pub total: f64,
    pub lower: Option<Vec<f64>>,
    /// `(a, b)` で `a . x >= b`。
    pub constraints: Vec<(Vec<f64>, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct LexicographicResult {
    pub(crate) allocation: Vec<f64>,
    pub(crate) levels: Vec<f64>,
    pub(crate) lp_solves: usize,
    pub(crate) guarantee: Guarantee,
}

impl LexicographicResult {
    pub fn allocation(&self) -> &[f64] {
        &self.allocation
    }

    pub fn into_allocation(self) -> Vec<f64> {
        self.allocation
    }

    /// 各段の不満の最大値。不満が 1 組なら非増加列である。
    /// 比例仁のように不満を優先順の組に分けて最小化した場合は、組の順に続けて並べる (組ごとに非増加)。
    pub fn levels(&self) -> &[f64] {
        &self.levels
    }

    pub fn lp_solves(&self) -> usize {
        self.lp_solves
    }

    /// 逐次 LP では [`Guarantee::Exact`]。
    pub fn guarantee(&self) -> Guarantee {
        self.guarantee
    }
}

/// 不満のベクトルを辞書式に最小化する配分を求める。
///
/// `tolerance` は定数 `c_k` (配分) と同じ単位で、等号の判定は `c_k - a_k . x - w_k t` に当てる。
///
/// 次のいずれかに当たれば [`Error::InvalidArgument`] を返す。
///
/// - `players` が 0、または `feasible.lower`・各制約の係数・各不満の係数の長さが `players` と異なる
/// - `feasible.total`・`feasible.lower`・制約の係数と右辺・不満の係数と定数・重みのいずれかが有限でない
/// - 重みが正でない
/// - `tolerance` が有限で 0 以上でない、または定数の大きさで割ると有限でない (定数に比べて大きすぎる)
pub fn lexicographic_minimum(
    players: usize,
    feasible: &Feasible,
    complaints: &[Complaint],
    tolerance: f64,
) -> Result<LexicographicResult> {
    lexicographic_minimum_in_order(players, feasible, &[complaints], tolerance)
}

/// 不満の組を優先順に辞書式に最小化する。前の組を辞書式に最小化した配分の集合の上で、次の組を辞書式に最小化する
/// (後の組の不満は、前の組のどの不満よりも小さいとみなして並べたベクトルの辞書式最小化)。
///
/// 前の組の辞書式最小の集合は、確定させた不満の等式 `c_k - a_k . x = w_k t_k` で正確に表して次の組へ渡す
/// (確定しなかった不満は、確定した不満の係数が張る空間に入っているので、その集合の上で定数である)。
/// 段の値 (`levels`) は、組の順に続けて返す。
fn lexicographic_minimum_in_order(
    players: usize,
    feasible: &Feasible,
    groups: &[&[Complaint]],
    tolerance: f64,
) -> Result<LexicographicResult> {
    let complaints: Vec<&Complaint> = groups.iter().flat_map(|group| group.iter()).collect();
    validate_problem(players, feasible, complaints.iter().copied(), tolerance)?;
    // 定数と重みをそれぞれ 2 のべき乗で割る (丸めは起きない)。
    // 配分は定数と同じ倍率、不満の値は 定数の倍率 / 重みの倍率 で変わる。
    // 重みは組ごとに割る。不満どうしを比べるのは同じ組の中だけなので、組ごとに別の数で割っても順序は変わらない。
    // 全ての組を同じ数で割ると、重みの大きさが組によって桁違いのとき (比例仁の `v(S)` と 1 など)、
    // 小さい方の重みが LP の中で 0 と区別できなくなる。
    let constants = complaints
        .iter()
        .map(|c| c.constant)
        .chain(std::iter::once(feasible.total))
        .chain(feasible.lower.iter().flatten().copied())
        .chain(feasible.constraints.iter().map(|(_, b)| *b))
        .fold(0.0, |acc: f64, v| acc.max(v.abs()));
    let value_divisor = normalizer(constants);
    let weight_divisors: Vec<f64> = groups
        .iter()
        .map(|group| normalizer(group.iter().fold(0.0, |acc: f64, c| acc.max(c.weight))))
        .collect();
    let normalized_feasible = Feasible {
        total: feasible.total / value_divisor,
        lower: feasible
            .lower
            .as_ref()
            .map(|lower| lower.iter().map(|v| v / value_divisor).collect()),
        constraints: feasible
            .constraints
            .iter()
            .map(|(a, b)| (a.clone(), b / value_divisor))
            .collect(),
    };
    let normalized_complaints: Vec<Complaint> = groups
        .iter()
        .zip(&weight_divisors)
        .flat_map(|(group, weight_divisor)| {
            group.iter().map(move |c| Complaint {
                coefficients: c.coefficients.clone(),
                constant: c.constant / value_divisor,
                weight: c.weight / weight_divisor,
            })
        })
        .collect();
    // 組ごとの不満の番号。後の組から順に取り出すので逆順に持つ。
    let mut start = 0;
    let mut pending: Vec<Vec<usize>> = groups
        .iter()
        .map(|group| {
            start += group.len();
            (start - group.len()..start).collect()
        })
        .collect();
    pending.reverse();
    let mut solver = Lexicographic {
        n: players,
        feasible: &normalized_feasible,
        complaints: &normalized_complaints,
        tolerance: normalized_tolerance(tolerance, value_divisor)?,
        open: pending.pop().unwrap_or_default(),
        pending,
        fixed: Vec::new(),
        span: Span::new(players),
        counter: Counter::default(),
    };
    solver.span.insert(&vec![1.0; players]);
    solver.drop_determined();
    let (mut result, level_groups) = solver.run()?;
    for value in &mut result.allocation {
        *value *= value_divisor;
    }
    for (level, group) in result.levels.iter_mut().zip(level_groups) {
        *level *= value_divisor / weight_divisors[group];
    }
    Ok(result)
}

/// [`lexicographic_minimum`] の入力の長さと値を確かめる。
fn validate_problem<'a>(
    players: usize,
    feasible: &Feasible,
    complaints: impl Iterator<Item = &'a Complaint>,
    tolerance: f64,
) -> Result<()> {
    let invalid = |message: String| Err(Error::InvalidArgument(message));
    if players == 0 {
        return invalid("プレイヤーが 0 人".into());
    }
    check_tolerance(tolerance)?;
    if !feasible.total.is_finite() {
        return invalid(format!("配分の総額 {} が有限でない", feasible.total));
    }
    if let Some(lower) = &feasible.lower {
        if lower.len() != players {
            return invalid(format!(
                "下限の長さ {} がプレイヤー数 {players} と異なる",
                lower.len()
            ));
        }
        if let Some(i) = lower.iter().position(|v| !v.is_finite()) {
            return invalid(format!("プレイヤー {i} の下限 {} が有限でない", lower[i]));
        }
    }
    for (k, (a, b)) in feasible.constraints.iter().enumerate() {
        if a.len() != players {
            return invalid(format!(
                "{k} 番目の制約の係数の長さ {} がプレイヤー数 {players} と異なる",
                a.len()
            ));
        }
        if !b.is_finite() || a.iter().any(|v| !v.is_finite()) {
            return invalid(format!("{k} 番目の制約の係数か右辺が有限でない"));
        }
    }
    for (k, complaint) in complaints.enumerate() {
        if complaint.coefficients.len() != players {
            return invalid(format!(
                "{k} 番目の不満の係数の長さ {} がプレイヤー数 {players} と異なる",
                complaint.coefficients.len()
            ));
        }
        if !complaint.constant.is_finite() || complaint.coefficients.iter().any(|v| !v.is_finite())
        {
            return invalid(format!("{k} 番目の不満の係数か定数が有限でない"));
        }
        if !(complaint.weight.is_finite() && complaint.weight > 0.0) {
            return invalid(format!(
                "{k} 番目の不満の重み {} が有限の正の数でない",
                complaint.weight
            ));
        }
    }
    Ok(())
}

struct Lexicographic<'a> {
    n: usize,
    feasible: &'a Feasible,
    complaints: &'a [Complaint],
    /// 超過 `c - a . x` の単位の許容誤差。
    tolerance: f64,
    /// 未確定の不満の番号 (いま最小化している組)。
    open: Vec<usize>,
    /// まだ最小化していない組 (最後の要素が次の組)。
    pending: Vec<Vec<usize>>,
    /// 確定した不満と、確定させた `a . x` の値 (確定した段の解での値)。
    fixed: Vec<(usize, f64)>,
    span: Span,
    counter: Counter,
}

impl Lexicographic<'_> {
    /// 結果と、各段がどの組 (0 始まり) の不満の段かを返す。
    fn run(mut self) -> Result<(LexicographicResult, Vec<usize>)> {
        if let Some(point) = self.single_point() {
            let result = LexicographicResult {
                allocation: point,
                levels: Vec::new(),
                lp_solves: 0,
                guarantee: Guarantee::Exact,
            };
            return Ok((result, Vec::new()));
        }
        let mut levels = Vec::new();
        let mut level_groups = Vec::new();
        let mut group = 0;
        let mut allocation = None;
        loop {
            while !self.open.is_empty() {
                let (level, x) = self.minimize_max_complaint()?;
                levels.push(level);
                level_groups.push(group);
                self.fix_tight(level, &x)?;
                self.drop_determined();
                allocation = Some(x);
            }
            // 前の組の辞書式最小の集合 (確定した等式) の上で、次の組を最小化する。
            match self.pending.pop() {
                Some(next) => {
                    self.open = next;
                    group += 1;
                    self.drop_determined();
                }
                None => break,
            }
        }
        let allocation = match allocation {
            Some(x) => x,
            None => self.any_feasible()?,
        };
        if !self.span.is_full() && !self.is_unique()? {
            return Err(Error::InvalidArgument(
                "不満の係数が R^n を張らず、辞書式最小の配分が一意に定まらない".into(),
            ));
        }
        let result = LexicographicResult {
            allocation,
            levels,
            lp_solves: self.counter.solves,
            guarantee: Guarantee::Exact,
        };
        Ok((result, level_groups))
    }

    /// 下限の和が `total` に (和の丸め誤差を除いて) 等しく、他の制約も許容誤差以内で満たすなら、
    /// 配分の集合は 1 点 `lower` で、それが辞書式最小。許容誤差以内で空の配分集合で下限を下げた場合もこれに当たる。
    ///
    /// 不満の差が許容誤差より小さくなり、LP の段で誤った値で確定しうるので先に判定する。
    fn single_point(&self) -> Option<Vec<f64>> {
        let lower = self.feasible.lower.as_ref()?;
        let sum: f64 = lower.iter().sum();
        let magnitude = lower
            .iter()
            .fold(self.feasible.total.abs(), |acc, v| acc.max(v.abs()));
        let rounding = 4.0 * f64::EPSILON * self.n as f64 * magnitude;
        let feasible = (sum - self.feasible.total).abs() <= rounding
            && self
                .feasible
                .constraints
                .iter()
                .all(|(a, b)| dot(a, lower) >= b - self.tolerance);
        feasible.then(|| lower.clone())
    }

    /// 係数が確定済みの空間に入った不満を外す (実行可能集合の上で定数になる)。
    fn drop_determined(&mut self) {
        let span = &self.span;
        let complaints = self.complaints;
        self.open
            .retain(|&k| !span.contains(&complaints[k].coefficients));
    }

    fn add_domain(&self, problem: &mut Problem, objective: Option<&[f64]>) -> Vec<Variable> {
        let x: Vec<Variable> = (0..self.n)
            .map(|i| {
                let lower = self
                    .feasible
                    .lower
                    .as_ref()
                    .map_or(f64::NEG_INFINITY, |lower| lower[i]);
                let coefficient = objective.map_or(0.0, |a| a[i]);
                problem.add_var(coefficient, (lower, f64::INFINITY))
            })
            .collect();
        lp::add(
            problem,
            x.iter().map(|&var| (var, 1.0)).collect(),
            Cmp::Eq,
            self.feasible.total,
        );
        for (a, b) in &self.feasible.constraints {
            lp::add(problem, terms(&x, a), Cmp::Ge, *b);
        }
        for &(k, value) in &self.fixed {
            lp::add(
                problem,
                terms(&x, &self.complaints[k].coefficients),
                Cmp::Eq,
                value,
            );
        }
        x
    }

    fn any_feasible(&mut self) -> Result<Vec<f64>> {
        let mut problem = lp::minimize();
        let x = self.add_domain(&mut problem, None);
        let solution = self
            .counter
            .solve(&problem)?
            .ok_or_else(|| Error::InvalidArgument("配分の集合が空".into()))?;
        Ok(x.iter().map(|&var| solution[var]).collect())
    }

    /// 配分の集合 (確定した等式を含む) が 1 点かを、各座標の最大・最小で確かめる。
    fn is_unique(&mut self) -> Result<bool> {
        for i in 0..self.n {
            let mut direction = vec![0.0; self.n];
            direction[i] = 1.0;
            let mut range = [0.0; 2];
            for (slot, sign) in [1.0, -1.0].into_iter().enumerate() {
                let mut problem = lp::maximize();
                let objective: Vec<f64> = direction.iter().map(|d| sign * d).collect();
                let x = self.add_domain(&mut problem, Some(&objective));
                self.add_open_rows(&mut problem, &x, Row::Unbounded);
                match self.counter.solve_bounded(&problem)? {
                    Some(solution) => range[slot] = sign * solution.objective(),
                    None => return Ok(false),
                }
            }
            if range[0] - range[1] > 10.0 * self.tolerance {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn add_open_rows(&self, problem: &mut Problem, x: &[Variable], row: Row) {
        for &k in &self.open {
            let complaint = &self.complaints[k];
            // c - a . x <= w t  <=>  a . x + w t >= c
            let mut row_terms = terms(x, &complaint.coefficients);
            let rhs = match row {
                Row::WithLevel(t) => {
                    row_terms.push((t, complaint.weight));
                    complaint.constant
                }
                Row::Unbounded => continue,
                // c - a . x <= w level + relax
                Row::AtMost { level, relax } => {
                    complaint.constant - complaint.weight * level - relax
                }
            };
            lp::add(problem, row_terms, Cmp::Ge, rhs);
        }
    }

    fn minimize_max_complaint(&mut self) -> Result<(f64, Vec<f64>)> {
        let mut problem = lp::minimize();
        let x = self.add_domain(&mut problem, None);
        let t = problem.add_var(1.0, (f64::NEG_INFINITY, f64::INFINITY));
        self.add_open_rows(&mut problem, &x, Row::WithLevel(t));
        let solution = self
            .counter
            .solve_bounded(&problem)?
            .ok_or_else(|| Error::Numerical("不満の最大値が下に有界でない".into()))?;
        Ok((solution[t], x.iter().map(|&var| solution[var]).collect()))
    }

    /// 不満の最大値が `level` 以下の配分の中で `a . x` を最大化する。
    fn maximize_direction(&mut self, a: &[f64], level: f64) -> Result<(f64, Vec<f64>)> {
        let mut problem = lp::maximize();
        let x = self.add_domain(&mut problem, Some(a));
        // 最適値の丸め誤差で実行不可能にならないよう、許容誤差の 1% だけ緩める。
        let relax = 0.01 * self.tolerance;
        self.add_open_rows(&mut problem, &x, Row::AtMost { level, relax });
        let solution = self
            .counter
            .solve_bounded(&problem)?
            .ok_or_else(|| Error::Numerical("確定判定の LP が非有界".into()))?;
        Ok((
            solution.objective(),
            x.iter().map(|&var| solution[var]).collect(),
        ))
    }

    /// 段 `level` からの超過のずれ `c - a . x - w level` (値の単位)。
    fn deviation(&self, k: usize, x: &[f64], level: f64) -> f64 {
        let complaint = &self.complaints[k];
        complaint.constant - dot(&complaint.coefficients, x) - complaint.weight * level
    }

    fn fix_tight(&mut self, level: f64, x: &[f64]) -> Result<()> {
        let tight: Vec<usize> = self
            .open
            .iter()
            .copied()
            .filter(|&k| self.deviation(k, x, level).abs() <= self.tolerance)
            .collect();
        let mut not_fixed = vec![false; tight.len()];
        let mut progressed = false;
        for position in 0..tight.len() {
            let k = tight[position];
            let complaint = &self.complaints[k];
            if not_fixed[position] || self.span.contains(&complaint.coefficients) {
                continue;
            }
            let fixed_value = complaint.constant - complaint.weight * level;
            let (max_value, witness) = self.maximize_direction(&complaint.coefficients, level)?;
            if max_value <= fixed_value + self.tolerance {
                // 確定値は `c - w level` ではなく、この段の解 `x` での値にする。許容誤差以内でタイトな不満は、
                // 最適解の集合の上で `c - w level` を取れないことがあり、その値で固定すると以後の LP が
                // 実行不可能になる。`x` での値なら、`x` 自身が全ての確定の等式を満たす実行可能点であり続ける。
                self.fixed.push((k, dot(&complaint.coefficients, x)));
                self.span.insert(&complaint.coefficients);
                progressed = true;
            } else {
                // witness で不満が level 未満になる不満も確定していない。
                for (later, flag) in tight.iter().zip(not_fixed.iter_mut()).skip(position + 1) {
                    if self.deviation(*later, &witness, level) < -self.tolerance {
                        *flag = true;
                    }
                }
            }
        }
        if !progressed {
            return Err(Error::Numerical(format!(
                "不満の最大値 {level} の段で確定する不満が見つからない (許容誤差 {} を見直す)",
                self.tolerance
            )));
        }
        let fixed: Vec<usize> = self.fixed.iter().map(|&(k, _)| k).collect();
        self.open.retain(|k| !fixed.contains(k));
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Row {
    /// `c - a . x <= w t` (`t` は変数)。
    WithLevel(Variable),
    /// `c - a . x <= w level + relax`。
    AtMost { level: f64, relax: f64 },
    /// 不満の行を入れない。
    Unbounded,
}

fn terms(x: &[Variable], a: &[f64]) -> Vec<(Variable, f64)> {
    x.iter()
        .zip(a)
        .filter(|(_, coefficient)| **coefficient != 0.0)
        .map(|(&var, &coefficient)| (var, coefficient))
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(p, q)| p * q).sum()
}

/// プレイヤー数が `max` を超えれば [`Error::TooManyPlayers`]。
fn check_players(game: &ExplicitGame, max: usize) -> Result<()> {
    let players = game.players();
    if players > max {
        return Err(Error::TooManyPlayers { players, max });
    }
    Ok(())
}

fn proper_coalitions(game: &ExplicitGame) -> impl Iterator<Item = Coalition> {
    (1..game.grand().0).map(Coalition)
}

/// 配分集合 (準配分集合)。配分集合が許容誤差を超えて空なら [`Error::EmptyImputationSet`]、
/// 許容誤差以内で空なら下限を下げる ([`crate::nucleolus::Options::tolerance`] と同じ扱い)。
fn feasible_set(game: &ExplicitGame, domain: Domain) -> Result<Feasible> {
    let grand = PlayerSet::grand(game.players());
    Ok(Feasible {
        total: game.value(game.grand()),
        lower: lower_bounds(game, &[grand], domain, default_tolerance(game))?,
        constraints: Vec::new(),
    })
}

/// 超過の complaint `e(S, x) / weight`。
fn excess_complaint(game: &ExplicitGame, coalition: Coalition, weight: f64) -> Complaint {
    Complaint {
        coefficients: indicator(coalition, game.players()),
        constant: game.value(coalition),
        weight,
    }
}

/// 全ての空でない真部分提携の超過 (重み 1) を不満にした場合。仁 (プレ仁) に一致する。
#[cfg(test)]
fn nucleolus_by_complaints(game: &ExplicitGame, domain: Domain) -> Result<LexicographicResult> {
    let feasible = feasible_set(game, domain)?;
    let complaints: Vec<Complaint> = proper_coalitions(game)
        .map(|s| excess_complaint(game, s, 1.0))
        .collect();
    lexicographic_minimum(
        game.players(),
        &feasible,
        &complaints,
        default_tolerance(game),
    )
}

/// per capita 仁: 超過を提携の人数で割った `e(S, x) / abs(S)` を辞書式に最小化する。
///
/// - `domain` が [`Domain::Preimputation`] なら原典の定義である。Young, Okada & Hashimoto (1982) の
///   weak nucleolus (4.2 節。超過を `(v(S) - x(S)) / |S|` とした仁で、`x(N) = v(N)` だけを課す) と、
///   Young (1985, "Methods and principles of cost allocation") の per capita nucleolus (normalized nucleolus) に当たる。
/// - [`Domain::Imputation`] は、個人合理性 `x_i >= v({i})` を課した形である
///   (Young, Okada & Hashimoto 1982 によれば Grotte 1970 の提案。Grotte の本文は未確認)。
///
/// プレイヤー数は [`MAX_PER_CAPITA_PLAYERS`] 以下に限る。
pub fn per_capita_nucleolus(game: &ExplicitGame, domain: Domain) -> Result<LexicographicResult> {
    check_players(game, MAX_PER_CAPITA_PLAYERS)?;
    let feasible = feasible_set(game, domain)?;
    let complaints: Vec<Complaint> = proper_coalitions(game)
        .map(|s| excess_complaint(game, s, s.len() as f64))
        .collect();
    lexicographic_minimum(
        game.players(),
        &feasible,
        &complaints,
        default_tolerance(game),
    )
}

/// disruption nucleolus (Littlechild & Vaidya 1976): 提携の抜ける傾向を辞書式に最小化するコアの点。
///
/// 抜ける傾向 `d(S, x) = (x(N \ S) - v(N \ S)) / (x(S) - v(S))` の式は Gilles & Mallozzi (arXiv:2208.10189) の
/// Definition 3.2 と Lemaire (1984) の 4.4 節で確認した。コアの上で不満 `e(S, x) / b(S)` の辞書式最小化に
/// 書き換えて解く (モジュールの文書を参照)。Littlechild & Vaidya の本文は未確認で、定義域 (配分集合の内部か、コアか)、
/// 分母 `x(S) - v(S)` が 0 以下の提携の扱い、コアが空のゲームの扱いは確認できていない。
/// この実装はコアの上で定義し、コアが空のゲームには解を返さない。
///
/// [`crate::nucleolus::has_nonempty_core`] が偽を返すゲームでは [`Error::InvalidArgument`] を返す。
/// `b(S) = v(N) - v(S) - v(N \ S) = 0` の提携は、コアの上で `x(S) = v(S)` に固定されるので不満にしない。
///
/// コアの制約は、最小コアの `epsilon` ([`crate::nucleolus::least_core`]) を使って
/// `x(S) >= v(S) - max(epsilon, 0)` とする。`epsilon <= 0` のゲームではコアそのもので、
/// `has_nonempty_core` が許容誤差の範囲で空でないと判定したゲーム (浮動小数点の `epsilon` が小さな正の値)
/// では、コアの制約を `epsilon` だけ緩めた最小コアになる。後者でも LP が実行可能になるようにするためである。
///
/// プレイヤー数は [`MAX_DISRUPTION_PLAYERS`] 以下に限る。
pub fn disruption_nucleolus(game: &ExplicitGame) -> Result<LexicographicResult> {
    let n = game.players();
    check_players(game, MAX_DISRUPTION_PLAYERS)?;
    let tolerance = default_tolerance(game);
    if !crate::nucleolus::has_nonempty_core(game)? {
        return Err(Error::InvalidArgument(
            "disruption nucleolus はコアが空でないゲームに限る".into(),
        ));
    }
    let relax = crate::nucleolus::least_core(game, Domain::Preimputation)?
        .epsilon()
        .max(0.0);
    let total = game.value(game.grand());
    let mut feasible = Feasible {
        total,
        lower: None,
        constraints: Vec::new(),
    };
    let mut complaints = Vec::new();
    for s in proper_coalitions(game) {
        feasible
            .constraints
            .push((indicator(s, n), game.value(s) - relax));
        let gap = total - game.value(s) - game.value(Coalition(game.grand().0 & !s.0));
        if gap > tolerance {
            complaints.push(excess_complaint(game, s, gap));
        }
    }
    lexicographic_minimum(n, &feasible, &complaints, tolerance)
}

/// anti-prenucleolus: 超過を小さい順に並べたベクトルを準配分の上で辞書式に最大化した配分。
/// 双対ゲームのプレ仁として求める (Funaki & Meinhardt 2006。Sudhölter & Peleg の Working Paper 262 の
/// antinucleolus `ν*(v) = ν(v*)` と同じ)。`levels` は双対ゲームの超過の段。
///
/// 双対ゲームの値が有限でない (`v(N) - v(N \ S)` が `f64` で表せない) ゲームでは [`Error::InvalidArgument`] を返す。
pub fn anti_prenucleolus(game: &ExplicitGame) -> Result<crate::nucleolus::NucleolusResult> {
    crate::nucleolus::prenucleolus(&game.dual()?)
}

/// anti-nucleolus: anti-imputation の集合 `{x(N) = v(N), x_i >= v(N) - v(N \ {i})}` の上での anti-prenucleolus。
/// この集合は双対ゲームの配分集合に等しい (Meinhardt "On the Replication of the Pre-Kernel and Related Solutions"
/// の式 (7.1))。
/// 双対ゲームの仁として求める。この集合が空 (`sum_i (v(N) - v(N \ {i})) > v(N)`) ならエラーを返す。
pub fn anti_nucleolus(game: &ExplicitGame) -> Result<crate::nucleolus::NucleolusResult> {
    let utopia: f64 = crate::compromise::utopia_payoffs(game).iter().sum();
    let total = game.value(game.grand());
    if utopia > total + default_tolerance(game) {
        return Err(Error::InvalidArgument(format!(
            "anti-imputation の集合が空: 理想の支払い v(N) - v(N \\ {{i}}) の和 {utopia} が v(N) = {total} を超える"
        )));
    }
    crate::nucleolus::nucleolus(&game.dual()?)
}

/// 比例仁 (Young, Okada & Hashimoto 1982 の 4.3 節): 超過を提携の値で割った `e(S, x) / v(S)` を辞書式に最小化する。
///
/// 原典の定義は、節約ゲームのような非負のゲームについて、式 (9)
/// `min t  s.t.  x(S) >= (1 - t) v(S) (S ⊂ N)、x(N) = v(N)` から始める逐次の最小化である。
/// `v(S) = 0` の提携は、式 (9) から常に制約 `x(S) >= 0` になり、互いの順序は取り決め
/// 「`v(S) = 0` なら `a/0 > b/0 if a > b`」(`a = v(S) - x(S) = -x(S)`) で決める。`a < 0` の `a/0` は有限の不満のどれよりも
/// 小さいので、`v(S) > 0` の不満を辞書式に最小化した配分の集合の上で、`v(S) = 0` の提携の `-x(S)` を
/// 辞書式に最小化する (`a = 0` の提携も後者に含める)。`levels` は前者の段 (比例超過の値) の後に後者の段 (`-x(S)` の値) が続く。
///
/// - `domain` が [`Domain::Preimputation`] なら原典の定義 (式 (9) は `x(N) = v(N)` だけを課す)。
///   1 人提携の値が 0 のゲームでは `x_i >= 0` が式 (9) から従う。
/// - [`Domain::Imputation`] は、個人合理性 `x_i >= v({i})` を足した形である。
///
/// 非負のゲームに限る。`v(S)` の絶対値が丸めの大きさ `4 n ε max|v|` 以下なら 0 とみなし
/// (浮動小数点数の和や差で作った値の丸め誤差)、それより負の値があれば [`Error::InvalidArgument`] を返す。
/// 解の精度のための許容誤差 (`1e-7 max|v|`) では比べない。値の大きさに比べて小さい正の値 (`v(N) = 1e7` に対する
/// `v(S) = 1` など) も、定義どおり比例超過の分母にする。
/// 費用から節約ゲームを作るときは [`crate::games::cost::CostGame::savings_game`] を使うと、和の丸めが 0 になる。
///
/// プレイヤー数は [`MAX_PROPORTIONAL_PLAYERS`] 以下に限る。
pub fn proportional_nucleolus(game: &ExplicitGame, domain: Domain) -> Result<LexicographicResult> {
    check_players(game, MAX_PROPORTIONAL_PLAYERS)?;
    let n = game.players();
    let tolerance = default_tolerance(game);
    // 0 とみなすのは丸めの範囲だけにする (解の精度のための許容誤差では、小さい正の値を落としてしまう)。
    let zero = rounding_magnitude(n, game.max_abs_value());
    if let Some(mask) = game.values().iter().position(|v| *v < -zero) {
        return Err(Error::InvalidArgument(format!(
            "比例仁は非負のゲームに限る: v({}) = {} (丸めとみなす範囲は {zero:e} まで)",
            Coalition(mask as u64),
            game.values()[mask]
        )));
    }
    let mut feasible = feasible_set(game, domain)?;
    let mut positive = Vec::new();
    let mut without_value = Vec::new();
    for s in proper_coalitions(game) {
        let value = game.value(s);
        if value > zero {
            positive.push(excess_complaint(game, s, value));
        } else {
            // v(S) = 0 とみなす: 制約 x(S) >= 0 と、第 2 の組の不満 a = -x(S)。
            feasible.constraints.push((indicator(s, n), 0.0));
            without_value.push(Complaint {
                coefficients: indicator(s, n),
                constant: 0.0,
                weight: 1.0,
            });
        }
    }
    lexicographic_minimum_in_order(n, &feasible, &[&positive, &without_value], tolerance)
}

/// modiclus (修正仁): 超過の差 (bi-excess) `e(S, x) - e(T, x)` のベクトルを辞書式に最小化する準配分。
///
/// 組 `(S, T)` は、空提携 `∅` と全体提携 `N` を含む全ての提携の組である
/// (Sudhölter & Peleg, "Nucleoli as maximizers of collective satisfaction functions", Working Paper 262 の式 (4.1)。
/// 満足 `f(S, x) = x(S) - v(S)` の差 `(f(S, x) - f(T, x))_{S, T ⊆ N}` を小さい順に並べて辞書式に最大化する)。
/// 準配分の上では `e(∅, x) = e(N, x) = 0` なので、組 `(S, ∅)`・`(∅, T)` は超過そのもの `e(S, x)`・`-e(T, x)` になる。
/// 準配分の上で定数になる組 (`S = T` と `{S, T} = {∅, N}`、係数 `1_S - 1_T` が `0` か `±1_N`) は、
/// 辞書式最小の点を変えないので不満に入れない。不満の数は `4^n - 2^n - 2`。
/// [`LexicographicResult::levels`] は、この定数の組を除いた不満の段である。
///
/// modiclus は、双対被覆ゲーム `w(S + T*) = max{v(S) + v*(T), v(T) + v*(S)}`
/// (プレイヤーは `N` とその複製 `N*`、`v*` は双対ゲーム) のプレ仁を `N` に射影したものに等しい
/// (Rosenmüller & Sudhölter, "Cartels via the modiclus", Working Paper 320)。
/// `tests/modiclus.rs` で、この関係、同じ論文の Theorem 2.4 の平衡性による特徴づけ、
/// Meinhardt ("On the Replication of the Pre-Kernel and Related Solutions") の Table 4.1 の数値例を確かめている。
///
/// CoopGame 0.2.2 の `modiclus` は組を空でない真部分提携に限るので、値が異なりうる。
///
/// プレイヤー数は [`MAX_MODICLUS_PLAYERS`] 以下に限る。
pub fn modiclus(game: &ExplicitGame) -> Result<LexicographicResult> {
    let n = game.players();
    check_players(game, MAX_MODICLUS_PLAYERS)?;
    let grand = game.grand();
    let empty = Coalition(0);
    // 空提携と全体提携を含む全ての提携 (game.value(∅) = 0)。
    let coalitions: Vec<Coalition> = (0..=grand.0).map(Coalition).collect();
    let mut complaints = Vec::with_capacity(coalitions.len() * coalitions.len());
    for &s in &coalitions {
        for &t in &coalitions {
            if s == t || (s == grand && t == empty) || (s == empty && t == grand) {
                continue;
            }
            // e(S) - e(T) = v(S) - v(T) - (1_S - 1_T) . x
            let coefficients = indicator(s, n)
                .iter()
                .zip(indicator(t, n))
                .map(|(a, b)| a - b)
                .collect();
            complaints.push(Complaint {
                coefficients,
                constant: game.value(s) - game.value(t),
                weight: 1.0,
            });
        }
    }
    lexicographic_minimum(
        n,
        &feasible_set(game, Domain::Preimputation)?,
        &complaints,
        default_tolerance(game),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generators, nucleolus};

    fn assert_close(actual: &[f64], expected: &[f64], tolerance: f64) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() <= tolerance, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn unit_weights_give_the_nucleolus() {
        for kind in 1..=4 {
            for n in [3, 5, 7] {
                let game = generators::bnf(kind, n, 2).unwrap();
                let tolerance = 1e-6 * game.max_abs_value().max(1.0);
                for domain in [Domain::Imputation, Domain::Preimputation] {
                    let expected =
                        match nucleolus::nucleolus_with(&game, nucleolus::Options::new(domain)) {
                            Ok(result) => result.allocation,
                            Err(Error::EmptyImputationSet) => continue,
                            Err(err) => panic!("{err}"),
                        };
                    let actual = nucleolus_by_complaints(&game, domain).unwrap().allocation;
                    assert_close(&actual, &expected, tolerance);
                }
            }
        }
    }

    /// 3 人ゲーム (ビット順の値 0, 1, 2, 3, 0, 2, 3、v(N) = 6) の超過を不満にした問題。
    fn excess_problem() -> (Feasible, Vec<Complaint>) {
        let values = [0.0, 1.0, 2.0, 3.0, 0.0, 2.0, 3.0];
        let complaints = (1..7u64)
            .map(|mask| Complaint {
                coefficients: indicator(Coalition(mask), 3),
                constant: values[mask as usize],
                weight: 1.0,
            })
            .collect();
        let feasible = Feasible {
            total: 6.0,
            lower: None,
            constraints: Vec::new(),
        };
        (feasible, complaints)
    }

    #[test]
    fn lexicographic_minimum_rejects_malformed_input() {
        let (feasible, complaints) = excess_problem();
        assert_close(
            lexicographic_minimum(3, &feasible, &complaints, 1e-7)
                .unwrap()
                .allocation(),
            &[2.0, 3.0, 1.0],
            1e-9,
        );
        let with_feasible = |edit: &dyn Fn(&mut Feasible)| {
            let mut feasible = feasible.clone();
            edit(&mut feasible);
            lexicographic_minimum(3, &feasible, &complaints, 1e-7)
        };
        let with_complaint = |edit: &dyn Fn(&mut Complaint)| {
            let mut complaints = complaints.clone();
            edit(&mut complaints[0]);
            lexicographic_minimum(3, &feasible, &complaints, 1e-7)
        };
        let cases: Vec<(&str, Result<LexicographicResult>)> = vec![
            ("total が NaN", with_feasible(&|f| f.total = f64::NAN)),
            (
                "total が無限大",
                with_feasible(&|f| f.total = f64::INFINITY),
            ),
            (
                "lower の長さが 2",
                with_feasible(&|f| f.lower = Some(vec![0.0, 0.0])),
            ),
            (
                "lower の長さが 4",
                with_feasible(&|f| f.lower = Some(vec![1.0, 1.0, 1.0, 3.0])),
            ),
            (
                "lower に NaN",
                with_feasible(&|f| f.lower = Some(vec![f64::NAN, 0.0, 0.0])),
            ),
            (
                "制約の長さが 1",
                with_feasible(&|f| f.constraints = vec![(vec![1.0], 5.0)]),
            ),
            (
                "制約の長さが 4",
                with_feasible(&|f| f.constraints = vec![(vec![1.0, 0.0, 0.0, 1.0], 5.0)]),
            ),
            (
                "制約の係数に NaN",
                with_feasible(&|f| f.constraints = vec![(vec![f64::NAN, 1.0, 0.0], 1.0)]),
            ),
            (
                "制約の右辺が無限大",
                with_feasible(&|f| f.constraints = vec![(vec![1.0, 0.0, 0.0], f64::INFINITY)]),
            ),
            (
                "不満の係数の長さが 2",
                with_complaint(&|c| c.coefficients = vec![1.0, 0.0]),
            ),
            (
                "不満の係数に NaN",
                with_complaint(&|c| c.coefficients[0] = f64::NAN),
            ),
            (
                "不満の定数が NaN",
                with_complaint(&|c| c.constant = f64::NAN),
            ),
            ("重みが 0", with_complaint(&|c| c.weight = 0.0)),
            ("重みが負", with_complaint(&|c| c.weight = -1.0)),
            (
                "重みが無限大",
                with_complaint(&|c| c.weight = f64::INFINITY),
            ),
            (
                "許容誤差が NaN",
                lexicographic_minimum(3, &feasible, &complaints, f64::NAN),
            ),
            (
                "許容誤差が負",
                lexicographic_minimum(3, &feasible, &complaints, -1.0),
            ),
            (
                "許容誤差が無限大",
                lexicographic_minimum(3, &feasible, &complaints, f64::INFINITY),
            ),
            (
                "プレイヤーが 0 人",
                lexicographic_minimum(0, &feasible, &[], 1e-7),
            ),
        ];
        for (label, result) in cases {
            assert!(
                matches!(result, Err(Error::InvalidArgument(_))),
                "{label}: {result:?}"
            );
        }
    }

    #[test]
    fn variants_reject_games_above_player_limits() {
        let too_many = |result: Result<LexicographicResult>, max: usize| {
            assert!(
                matches!(result, Err(Error::TooManyPlayers { max: m, .. }) if m == max),
                "{result:?}"
            );
        };
        // 凸ゲーム (コアが空でない) なので、上限がなければ全て計算を始める。
        let convex = |n: usize| ExplicitGame::from_fn(n, |s| (s.len() * s.len()) as f64).unwrap();
        let game = convex(MAX_PER_CAPITA_PLAYERS + 1);
        too_many(
            per_capita_nucleolus(&game, Domain::Imputation),
            MAX_PER_CAPITA_PLAYERS,
        );
        let game = convex(MAX_PROPORTIONAL_PLAYERS + 1);
        too_many(
            proportional_nucleolus(&game, Domain::Imputation),
            MAX_PROPORTIONAL_PLAYERS,
        );
        let game = convex(MAX_DISRUPTION_PLAYERS + 1);
        too_many(disruption_nucleolus(&game), MAX_DISRUPTION_PLAYERS);
        let game = convex(MAX_MODICLUS_PLAYERS + 1);
        too_many(modiclus(&game), MAX_MODICLUS_PLAYERS);
    }

    #[test]
    fn anti_prenucleolus_rejects_games_whose_dual_overflows() {
        let game = ExplicitGame::from_binary(&[-1e308, -1e308, 1e308]).unwrap();
        assert!(matches!(
            anti_prenucleolus(&game),
            Err(Error::InvalidArgument(_))
        ));
        assert!(matches!(
            anti_nucleolus(&game),
            Err(Error::InvalidArgument(_))
        ));
    }

    #[test]
    fn proportional_rejects_negative_games() {
        let game = ExplicitGame::from_lex(&[-1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0]).unwrap();
        assert!(proportional_nucleolus(&game, Domain::Imputation).is_err());
    }

    #[test]
    fn non_spanning_complaints_are_rejected() {
        // x >= 0 の上で不満が {0, 1} の超過だけなら、x_2 = 0 は決まるが x_0 と x_1 は決まらない。
        let (mut feasible, complaints) = excess_problem();
        feasible.lower = Some(vec![0.0; 3]);
        let err = lexicographic_minimum(3, &feasible, &complaints[2..3], 1e-7).unwrap_err();
        assert!(matches!(err, Error::InvalidArgument(_)), "{err}");
    }

    /// 比例仁の定義どおりの比較の鍵: `v(S) > 0` の比例超過を大きい順に並べ、続けて
    /// `v(S) = 0` の提携の `v(S) - x(S) = -x(S)` を大きい順に並べたベクトル (Young, Okada & Hashimoto 1982 の式 (9) と、
    /// `a/0 > b/0 if a > b` の取り決め)。
    fn proportional_key(game: &ExplicitGame, x: &[f64]) -> Vec<f64> {
        let sorted = |mut v: Vec<f64>| {
            v.sort_by(|a, b| b.total_cmp(a));
            v
        };
        let excess = |s: Coalition| game.value(s) - s.sum(x);
        let positive = proper_coalitions(game).filter(|&s| game.value(s) > 0.0);
        let zero = proper_coalitions(game).filter(|&s| game.value(s) == 0.0);
        let mut key = sorted(positive.map(|s| excess(s) / game.value(s)).collect());
        key.extend(sorted(zero.map(excess).collect()));
        key
    }

    /// `x` を実行可能な方向に少し動かして、鍵が辞書式に小さくならないことを確かめる。
    fn assert_proportional_minimum(game: &ExplicitGame, x: &[f64], seed: u64) {
        let n = game.players();
        let base = proportional_key(game, x);
        let feasible = |y: &[f64]| {
            proper_coalitions(game)
                .filter(|&s| game.value(s) == 0.0)
                .all(|s| s.sum(y) >= -1e-12)
        };
        let mut rng = crate::rng::SplitMix64::new(seed);
        let mut moved = 0;
        for _ in 0..2000 {
            // 一部のプレイヤーの間だけで移す方向 (他のプレイヤーは動かさないので、境界に沿って動ける)。
            let movers: Vec<bool> = (0..n).map(|_| rng.next_f64() < 0.5).collect();
            let count = movers.iter().filter(|m| **m).count();
            if count < 2 {
                continue;
            }
            let mut d: Vec<f64> = movers
                .iter()
                .map(|&m| if m { rng.next_f64() - 0.5 } else { 0.0 })
                .collect();
            let mean = d.iter().sum::<f64>() / count as f64;
            for (v, &m) in d.iter_mut().zip(&movers) {
                if m {
                    *v -= mean;
                }
            }
            // 最大の成分を 1 にそろえ、鍵の変化が丸め誤差より十分大きくなる歩幅で動かす。
            let largest = d.iter().fold(0.0, |acc: f64, v| acc.max(v.abs()));
            d.iter_mut().for_each(|v| *v /= largest);
            for step in [1e-3, 1e-2] {
                let y: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + step * b).collect();
                if !feasible(&y) {
                    continue;
                }
                moved += 1;
                let other = proportional_key(game, &y);
                let first = base
                    .iter()
                    .zip(&other)
                    .position(|(a, b)| (a - b).abs() > 1e-11);
                if let Some(k) = first {
                    assert!(
                        other[k] > base[k],
                        "{:?}: {x:?} を {y:?} に動かすと鍵の {k} 番目が {} から {} に小さくなる\n{base:?}\n{other:?}",
                        game.values(),
                        base[k],
                        other[k]
                    );
                }
            }
        }
        assert!(moved > 0);
    }

    #[test]
    fn proportional_orders_coalitions_with_zero_value() {
        // v(S) > 0 の真部分提携がないので、v(S) = 0 の提携の -x(S) だけで決まる (均等配分)。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]).unwrap();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let result = proportional_nucleolus(&game, domain).unwrap();
            assert_close(result.allocation(), &[1.0 / 3.0; 3], 1e-9);
        }
        // v({0, 1}) = 2、v(N) = 5、他は 0。第 1 段で x({0, 1}) = 5 (比例超過 -3/2) に決まり、
        // x_2 = x_3 = 0 になる。x_0 と x_1 は v(S) = 0 の提携の -x(S) で決まる。
        let game = ExplicitGame::from_fn(4, |s| match s.0 {
            0b0011 => 2.0,
            0b1111 => 5.0,
            _ => 0.0,
        })
        .unwrap();
        let result = proportional_nucleolus(&game, Domain::Preimputation).unwrap();
        assert_close(result.allocation(), &[2.5, 2.5, 0.0, 0.0], 1e-9);
        assert_close(&result.levels()[..1], &[-1.5], 1e-12);
        assert_proportional_minimum(&game, result.allocation(), 1);
        // 検査の確認: 第 1 段は同じでも、x_0 と x_1 の釣り合わない点は不合格になる。
        let wrong = std::panic::catch_unwind(|| {
            assert_proportional_minimum(&game, &[3.0, 2.0, 0.0, 0.0], 1)
        });
        assert!(wrong.is_err());
    }

    #[test]
    fn proportional_nucleolus_of_sparse_games_is_lexicographic_minimum() {
        // v(S) > 0 の提携が少なく、v(S) = 0 の提携の順序づけが配分を決めるゲーム。
        let mut rng = crate::rng::SplitMix64::new(1982);
        let mut checked = 0;
        for case in 0..30 {
            let n = 3 + case % 3;
            let grand = (1u64 << n) - 1;
            let game = ExplicitGame::from_fn(n, |s| {
                if s.0 == grand {
                    10.0
                } else if s.len() >= 2 && rng.range(0, 3) == 0 {
                    rng.range(1, 8) as f64
                } else {
                    0.0
                }
            })
            .unwrap();
            for domain in [Domain::Imputation, Domain::Preimputation] {
                let x = proportional_nucleolus(&game, domain).unwrap().allocation;
                assert!(x.iter().all(|v| *v >= -1e-9), "{x:?}");
                assert_proportional_minimum(&game, &x, case as u64);
                checked += 1;
            }
        }
        assert_eq!(checked, 60);
    }

    #[test]
    fn proportional_treats_rounding_noise_as_zero() {
        // 費用から浮動小数点数で作った節約ゲームでは、0 のはずの値が -3.6e-15 などになる。
        let exact = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.4, 0.0, 0.4, 1.6]).unwrap();
        let noisy = ExplicitGame::from_lex(&[-3.6e-16, 2e-16, 0.0, 0.4, -1e-16, 0.4, 1.6]).unwrap();
        for domain in [Domain::Imputation, Domain::Preimputation] {
            let x = proportional_nucleolus(&exact, domain).unwrap();
            let y = proportional_nucleolus(&noisy, domain).unwrap();
            assert_close(y.allocation(), x.allocation(), 1e-12);
            assert_close(x.allocation(), &[0.0, 1.6, 0.0], 1e-12);
        }
        // 丸めの大きさを超えて負の値は拒否する。
        for negative in [-1e-3, -1e-9, -1e-12] {
            let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.4, negative, 0.4, 1.6]).unwrap();
            let err = proportional_nucleolus(&game, Domain::Preimputation).unwrap_err();
            assert!(
                matches!(err, Error::InvalidArgument(_)),
                "{negative}: {err}"
            );
        }
    }

    #[test]
    fn proportional_keeps_small_positive_values() {
        // v({0, 1}) = 1、v(N) = 1e7、他は 0。式 (9) は x_0 + x_1 >= (1 - t) で、t を最小にするには
        // x_0 + x_1 = 1e7、x_2 = 0。残りは v(S) = 0 の提携の順序づけで x_0 = x_1 に決まる。
        // 値の大きさに比べて小さい正の値 (1e-7 max|v|) を 0 とみなすと、等分 (1e7 / 3 ずつ) になってしまう。
        for v01 in [1.0, 2.0, 1e-3] {
            let game = ExplicitGame::from_binary(&[0.0, 0.0, v01, 0.0, 0.0, 0.0, 1e7]).unwrap();
            let x = proportional_nucleolus(&game, Domain::Preimputation).unwrap();
            assert_close(x.allocation(), &[5e6, 5e6, 0.0], 1e-3);
        }
    }

    #[test]
    fn proportional_scales_with_the_game() {
        // 値を c 倍すると、配分は c 倍、比例超過の段はそのまま、v(S) = 0 の提携の段 (-x(S)) は c 倍になる。
        // 2 つの組の重み (v(S) と 1) を同じ数で割ると、値が 1 から大きく離れたゲームで片方の重みが
        // 潰れて LP が解けなくなる。
        let games: [(&[f64], &[f64]); 3] = [
            // YOH1982 の 3 自治体の節約ゲーム (ビット順)。比例超過だけで決まる。
            (&[0.0, 0.0, 0.4, 0.0, 0.0, 0.4, 1.6], &[0.0, 1.6, 0.0]),
            // v({0, 1}) = v(N) = 1: x_2 = 0 に固定され、残りは v(S) = 0 の提携の順序づけで決まる。
            (&[0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], &[0.5, 0.5, 0.0]),
            // v({1, 2}) = 4、v(N) = 5: 比例超過は x_1 + x_2 だけを決め、残りは順序づけで決まる。
            (&[0.0, 0.0, 0.0, 0.0, 0.0, 4.0, 5.0], &[0.0, 2.5, 2.5]),
        ];
        for (values, expected) in games {
            let base = ExplicitGame::from_binary(values).unwrap();
            let reference = proportional_nucleolus(&base, Domain::Preimputation).unwrap();
            assert_close(reference.allocation(), expected, 1e-9);
            for exponent in [-60, -40, -20, 20, 32, 40, 60] {
                let c = 2f64.powi(exponent);
                let scaled: Vec<f64> = values.iter().map(|v| v * c).collect();
                let game = ExplicitGame::from_binary(&scaled).unwrap();
                let result = proportional_nucleolus(&game, Domain::Preimputation)
                    .unwrap_or_else(|err| panic!("2^{exponent} 倍: {err}"));
                let back: Vec<f64> = result.allocation().iter().map(|v| v / c).collect();
                assert_close(&back, expected, 1e-9);
                assert_eq!(
                    result.levels().len(),
                    reference.levels().len(),
                    "2^{exponent}"
                );
            }
        }
    }

    #[test]
    fn disruption_nucleolus_matches_coopgame_example() {
        // CoopGame 0.2.2 の disruptionNucleolus のヘルプの例 (掲載値は小数 6 桁に丸めた値)。
        let game = ExplicitGame::from_lex(&[
            0.0, 0.0, 0.0, 0.0, 2.0, 3.0, 4.0, 1.0, 3.0, 2.0, 8.0, 11.0, 6.5, 9.5, 14.0,
        ])
        .unwrap();
        let x = disruption_nucleolus(&game).unwrap().allocation;
        for (a, e) in x.iter().zip([3.193548, 4.754839, 2.129032, 3.922581]) {
            assert!((a - e).abs() < 1e-6, "{x:?}");
        }
        // コアが空のゲームは対象外。
        let empty = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 80.0, 80.0, 80.0, 90.0]).unwrap();
        assert!(disruption_nucleolus(&empty).is_err());
    }

    #[test]
    fn disruption_nucleolus_solves_every_game_with_nonempty_core() {
        // v({i, j}) = 1、v(N) = 1.5 - eps。コアは eps = 0 で 1 点 (1/2, 1/2, 1/2)、eps > 0 で空になり、
        // 最小コアは eps 2/3 だけ緩めた 1 点 (1/2 - eps/3, ...) になる。
        // has_nonempty_core が真を返すゲームでは解を返し、偽を返すゲームでは InvalidArgument を返す。
        // eps = f64::EPSILON (v(N) が 1.5 の 1 つ下の f64) は入力の丸めの大きさに収まるので空でないと判定され、
        // 浮動小数点の最小コアの値がわずかに正でも LP が実行可能でなければならない。
        let mut solved = 0;
        for eps in [0.0, f64::EPSILON, 1e-12, 1e-9, 5e-8, 1e-7, 1e-6, 1e-3] {
            let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.5 - eps]).unwrap();
            let nonempty = crate::nucleolus::has_nonempty_core(&game).unwrap();
            assert_eq!(nonempty, eps <= f64::EPSILON, "eps {eps}");
            let result = disruption_nucleolus(&game);
            if !nonempty {
                assert!(
                    matches!(result, Err(Error::InvalidArgument(_))),
                    "eps {eps}"
                );
                continue;
            }
            let x = result
                .unwrap_or_else(|err| panic!("eps {eps}: {err}"))
                .allocation;
            assert_close(&x, &[0.5 - eps / 3.0; 3], 1e-7);
            solved += 1;
        }
        assert_eq!(solved, 2);
    }

    #[test]
    fn anti_prenucleolus_of_bankruptcy_example() {
        // Funaki & Meinhardt (2006) Example 4.1: E = 150、d = (60, 80, 120)。
        // 破産ゲーム v のプレ仁と、その双対 w = v* の anti-prenucleolus はともに (30, 40, 80)。
        let v = ExplicitGame::from_lex(&[0.0, 0.0, 10.0, 30.0, 70.0, 90.0, 150.0]).unwrap();
        let w = ExplicitGame::from_lex(&[60.0, 80.0, 120.0, 140.0, 150.0, 150.0, 150.0]).unwrap();
        assert_eq!(v.dual().unwrap().values(), w.values());
        assert_eq!(w.dual().unwrap().values(), v.values());
        let expected = [30.0, 40.0, 80.0];
        let close = |x: &[f64]| x.iter().zip(&expected).all(|(a, e)| (a - e).abs() < 1e-6);
        assert!(close(
            &crate::nucleolus::prenucleolus(&v).unwrap().allocation
        ));
        assert!(close(&anti_prenucleolus(&w).unwrap().allocation));
        assert!(close(&anti_nucleolus(&w).unwrap().allocation));
        // w のプレ仁は (30, 40, 80) ではない (プレ仁は双対をとると一般に変わる)。
        assert!(!close(
            &crate::nucleolus::prenucleolus(&w).unwrap().allocation
        ));
    }

    #[test]
    fn anti_prenucleolus_maximizes_the_smallest_excess() {
        // 最小の超過は、anti-prenucleolus で最大になる (定義の第 1 段)。プレ仁や Shapley 値と比べる。
        for seed in 0..5 {
            let game = crate::generators::bnf(1, 5, seed).unwrap();
            let min_excess = |x: &[f64]| {
                proper_coalitions(&game)
                    .map(|s| crate::surplus::excess(&game, s, x))
                    .fold(f64::INFINITY, f64::min)
            };
            let anti = anti_prenucleolus(&game).unwrap().allocation;
            let pre = crate::nucleolus::prenucleolus(&game).unwrap().allocation;
            let shapley = crate::values::shapley(&game);
            assert!(min_excess(&anti) >= min_excess(&pre) - 1e-6);
            assert!(min_excess(&anti) >= min_excess(&shapley) - 1e-6);
        }
    }
}
