//! カーネル・プレカーネル全体 (説明は [`super::kernel_set`])。

use microlp::Variable;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::allocation::check_imputation_set;
use crate::game::{Coalition, ExplicitGame, default_tolerance};
use crate::linalg::{Span, for_each_combination, solve_square};
use crate::lp::{self, Cmp, Counter};

/// [`kernel_set`] が扱うプレイヤー数の上限。
pub const MAX_KERNEL_SET_PLAYERS: usize = 6;

/// 頂点の総当たりで調べる行の組み合わせ数の上限。これを超える多面体は頂点を返さない。
const MAX_VERTEX_COMBINATIONS: usize = 2_000_000;

/// 一次式の行 `coefficients . x (<= または =) rhs`。
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub coefficients: Vec<f64>,
    pub rhs: f64,
}

impl Row {
    pub fn lhs(&self, x: &[f64]) -> f64 {
        self.coefficients.iter().zip(x).map(|(a, b)| a * b).sum()
    }
}

/// カーネルを構成する多面体の 1 つ。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KernelPiece {
    /// アフィン包の次元。
    pub dimension: usize,
    /// 多面体に属する点の 1 つ。
    pub point: Vec<f64>,
    /// 頂点。非有界な場合と、行の組み合わせが多すぎて列挙できない場合は `None`。
    pub vertices: Option<Vec<Vec<f64>>>,
    pub equalities: Vec<Row>,
    pub inequalities: Vec<Row>,
}

impl KernelPiece {
    pub fn contains(&self, x: &[f64], tolerance: f64) -> bool {
        self.equalities
            .iter()
            .all(|row| (row.lhs(x) - row.rhs).abs() <= tolerance)
            && self
                .inequalities
                .iter()
                .all(|row| row.lhs(x) <= row.rhs + tolerance)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct KernelSet {
    /// 他の多面体に含まれるものを除いた多面体の一覧(次元の降順)。
    pub pieces: Vec<KernelPiece>,
    /// 探索した節点の数。
    pub nodes: usize,
    pub lp_solves: usize,
}

impl KernelSet {
    pub fn contains(&self, x: &[f64], tolerance: f64) -> bool {
        self.pieces.iter().any(|piece| piece.contains(x, tolerance))
    }

    /// 同じ直線上にあり、重なるか端点を共有する線分 (頂点が 2 つの 1 次元の多面体) を 1 本にまとめる。
    ///
    /// 探索は最大余剰を与える提携の場合分けごとに多面体を作るので、1 本の線分が
    /// 場合分けの境界で複数の多面体に分かれることがある。まとめた線分は、元の線分の等式
    /// (アフィン包) と、直線の向き `d` に沿った 2 本の不等式 `t_min <= d . x <= t_max` で表す。
    /// 他の形の多面体はそのまま残す。
    pub fn merge_collinear_segments(&self, tolerance: f64) -> KernelSet {
        let mut segments: Vec<Segment> = Vec::new();
        let mut others: Vec<KernelPiece> = Vec::new();
        for piece in &self.pieces {
            match Segment::from_piece(piece, tolerance) {
                Some(segment) => segments.push(segment),
                None => others.push(piece.clone()),
            }
        }
        // 1 回の合併で他の線分とつながることがあるので、変化がなくなるまで繰り返す。
        let mut merged = true;
        while merged {
            merged = false;
            'search: for a in 0..segments.len() {
                for b in a + 1..segments.len() {
                    if let Some(union) = segments[a].union(&segments[b], tolerance) {
                        segments[a] = union;
                        segments.remove(b);
                        merged = true;
                        break 'search;
                    }
                }
            }
        }
        let mut pieces: Vec<KernelPiece> = segments.into_iter().map(Segment::into_piece).collect();
        pieces.extend(others);
        pieces.sort_by_key(|piece| std::cmp::Reverse(piece.dimension));
        KernelSet {
            pieces,
            nodes: self.nodes,
            lp_solves: self.lp_solves,
        }
    }
}

/// [`KernelSet::merge_collinear_segments`] で扱う線分。`d . x` が `low..=high` を動く。
struct Segment {
    equalities: Vec<Row>,
    /// 単位ベクトル。
    direction: Vec<f64>,
    /// `d . x = low` と `d . x = high` の端点。
    ends: [Vec<f64>; 2],
    low: f64,
    high: f64,
}

impl Segment {
    fn from_piece(piece: &KernelPiece, tolerance: f64) -> Option<Segment> {
        let vertices = piece.vertices.as_ref()?;
        if piece.dimension != 1 || vertices.len() != 2 {
            return None;
        }
        let delta: Vec<f64> = vertices[1]
            .iter()
            .zip(&vertices[0])
            .map(|(a, b)| a - b)
            .collect();
        let length = dot(&delta, &delta).sqrt();
        if length <= tolerance {
            return None;
        }
        let direction: Vec<f64> = delta.iter().map(|v| v / length).collect();
        Some(Segment {
            equalities: piece.equalities.clone(),
            low: dot(&direction, &vertices[0]),
            high: dot(&direction, &vertices[1]),
            direction,
            ends: [vertices[0].clone(), vertices[1].clone()],
        })
    }

    /// `point` がこの線分を含む直線上にあるか。
    fn on_line(&self, point: &[f64], tolerance: f64) -> bool {
        let t = dot(&self.direction, point);
        let projected: Vec<f64> = self.ends[0]
            .iter()
            .zip(&self.direction)
            .map(|(p, d)| p + (t - self.low) * d)
            .collect();
        projected
            .iter()
            .zip(point)
            .all(|(a, b)| (a - b).abs() <= tolerance)
    }

    /// 同じ直線上で重なるか接していれば、合わせた線分を返す。
    fn union(&self, other: &Segment, tolerance: f64) -> Option<Segment> {
        if !other.ends.iter().all(|end| self.on_line(end, tolerance)) {
            return None;
        }
        let ts = other.ends.iter().map(|end| dot(&self.direction, end));
        let (other_low, other_high) = ts.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(t), hi.max(t))
        });
        if other_low > self.high + tolerance || other_high < self.low - tolerance {
            return None;
        }
        let pick = |t: f64| -> Vec<f64> {
            if (t - self.low).abs() <= tolerance {
                self.ends[0].clone()
            } else if (t - self.high).abs() <= tolerance {
                self.ends[1].clone()
            } else {
                other
                    .ends
                    .iter()
                    .find(|end| (dot(&self.direction, end) - t).abs() <= tolerance)
                    .expect("端点のどちらか")
                    .clone()
            }
        };
        let low = self.low.min(other_low);
        let high = self.high.max(other_high);
        Some(Segment {
            equalities: self.equalities.clone(),
            direction: self.direction.clone(),
            ends: [pick(low), pick(high)],
            low,
            high,
        })
    }

    fn into_piece(self) -> KernelPiece {
        let point = self.ends[0]
            .iter()
            .zip(&self.ends[1])
            .map(|(a, b)| (a + b) / 2.0)
            .collect();
        let inequalities = vec![
            Row {
                coefficients: self.direction.clone(),
                rhs: self.high,
            },
            Row {
                coefficients: self.direction.iter().map(|d| -d).collect(),
                rhs: -self.low,
            },
        ];
        let [first, second] = self.ends;
        KernelPiece {
            dimension: 1,
            point,
            vertices: Some(vec![first, second]),
            equalities: self.equalities,
            inequalities,
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct SetOptions {
    pub tolerance: f64,
    /// 探索する節点の数の上限。超えたらエラーを返す。
    pub max_nodes: usize,
}

impl SetOptions {
    pub fn for_game(game: &ExplicitGame) -> SetOptions {
        SetOptions {
            tolerance: default_tolerance(game),
            max_nodes: 200_000,
        }
    }
}

/// 小さいゲームのカーネル・プレカーネル全体を、多面体の和集合として求める。
///
/// 最大余剰 `s_ij(x)` は区分線形なので、各組 `(i, j)` で `s_ij` を与える提携
/// `S_ij`(`i in S, j not in S` で超過が最大の提携)を固定すると、その領域では
/// `s_ij(x) = e(S_ij, x)` が一次式になる。全ての組について `S_ij` と `S_ji` を選び、
///
/// - プレカーネル: `e(S_ij) = e(S_ji)`
/// - カーネル: 上に加えて、`e(S_ij) >= e(S_ji)` かつ `x_j = v({j})`、
///   または `e(S_ji) >= e(S_ij)` かつ `x_i = v({i})`
///
/// のいずれかを課すと、条件は全て一次の等式・不等式になる。
/// この選び方を組ごとに深さ優先で探索し、LP で実行不可能な枝を刈る。
/// 葉に残った多面体の和集合がカーネル(プレカーネル)に一致する。
///
/// 同点の提携による分岐の重複を避けるため、各節点で多面体のアフィン包
/// (明示的な等式と、LP で見つけた暗黙の等式)を求め、その上で超過が恒等的に
/// 等しくなる提携は同じ候補として 1 つだけ展開する。
///
/// 計算量は組の数と提携の数に対して指数的なので、プレイヤー数は [`MAX_KERNEL_SET_PLAYERS`] 以下に限る。
///
/// カーネル(`Domain::Imputation`)またはプレカーネル(`Domain::Preimputation`)全体を求める。
pub fn kernel_set(game: &ExplicitGame, domain: Domain, options: SetOptions) -> Result<KernelSet> {
    let n = game.players();
    if n > MAX_KERNEL_SET_PLAYERS {
        return Err(Error::TooManyPlayers {
            players: n,
            max: MAX_KERNEL_SET_PLAYERS,
        });
    }
    check_imputation_set(game, domain, options.tolerance)?;
    let mut search = Search {
        game,
        domain,
        tolerance: options.tolerance,
        max_nodes: options.max_nodes,
        n,
        pairs: (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .collect(),
        counter: Counter::default(),
        nodes: 0,
        leaves: Vec::new(),
    };

    let mut root = Polyhedron::new(n);
    let grand = game.grand();
    root.add_equality(indicator(grand, n), game.value(grand));
    if domain == Domain::Imputation {
        for i in 0..n {
            let mut coefficients = vec![0.0; n];
            coefficients[i] = -1.0;
            root.inequalities.push(Row {
                coefficients,
                rhs: -game.value(Coalition::singleton(i)),
            });
        }
    }
    if let Some(point) = search.feasible(&root)? {
        search.explore(0, root, point)?;
    }

    let mut pieces = Vec::with_capacity(search.leaves.len());
    for (polyhedron, point) in std::mem::take(&mut search.leaves) {
        pieces.push(search.piece(polyhedron, point)?);
    }
    Ok(KernelSet {
        pieces: remove_contained(pieces, 10.0 * options.tolerance),
        nodes: search.nodes,
        lp_solves: search.counter.solves,
    })
}

#[derive(Clone, Debug)]
struct Polyhedron {
    equalities: Vec<Row>,
    inequalities: Vec<Row>,
    /// 等式の係数ベクトルが張る空間。アフィン包の方向はこの直交補空間。
    span: Span,
}

impl Polyhedron {
    fn new(n: usize) -> Polyhedron {
        Polyhedron {
            equalities: Vec::new(),
            inequalities: Vec::new(),
            span: Span::new(n),
        }
    }

    /// 等式を加える。係数が既存の等式に一次従属でも、定数項が矛盾しうるので行は残す。
    fn add_equality(&mut self, coefficients: Vec<f64>, rhs: f64) {
        self.span.insert(&coefficients);
        self.equalities.push(Row { coefficients, rhs });
    }

    /// 実行可能な多面体で見つけた暗黙の等式を加える。
    /// 矛盾は起こりえないので、一次従属な行は捨てる。
    fn add_implied_equality(&mut self, coefficients: Vec<f64>, rhs: f64) {
        if self.span.insert(&coefficients) {
            self.equalities.push(Row { coefficients, rhs });
        }
    }
}

enum Case {
    /// 条件を付けない(`x_i = v({i})` と `x_j = v({j})` が既に成り立つ場合)。
    Free,
    Equal,
    /// `s_ij >= s_ji` かつ `x_j = v({j})`。
    FirstGreater,
    /// `s_ji >= s_ij` かつ `x_i = v({i})`。
    SecondGreater,
}

struct Search<'a> {
    game: &'a ExplicitGame,
    domain: Domain,
    tolerance: f64,
    max_nodes: usize,
    n: usize,
    pairs: Vec<(usize, usize)>,
    counter: Counter,
    nodes: usize,
    leaves: Vec<(Polyhedron, Vec<f64>)>,
}

impl Search<'_> {
    fn explore(&mut self, depth: usize, mut polyhedron: Polyhedron, point: Vec<f64>) -> Result<()> {
        self.nodes += 1;
        if self.nodes > self.max_nodes {
            return Err(Error::LimitExceeded(format!(
                "探索した節点が上限 {} を超えた",
                self.max_nodes
            )));
        }
        if depth == self.pairs.len() {
            self.leaves.push((polyhedron, point));
            return Ok(());
        }
        self.refresh_affine_hull(&mut polyhedron)?;
        let (i, j) = self.pairs[depth];
        // x_k = v({k}) が多面体上で恒等的に成り立つか。
        let at_lower_bound = |k: usize| {
            polyhedron.span.contains(&unit(k, self.n))
                && (point[k] - self.game.value(Coalition::singleton(k))).abs() <= self.tolerance
        };
        // 重なる場合分けは、他方に含まれるものを省く。
        let cases: &[Case] = match (self.domain, at_lower_bound(i), at_lower_bound(j)) {
            (Domain::Preimputation, _, _) => &[Case::Equal],
            // 両方が下限に張り付いていれば、組 (i, j) には条件が付かない。
            (Domain::Imputation, true, true) => &[Case::Free],
            // x_j = v({j}) なら「等しい」は「s_ij >= s_ji」に含まれる。
            (Domain::Imputation, false, true) => &[Case::FirstGreater, Case::SecondGreater],
            (Domain::Imputation, true, false) => &[Case::SecondGreater, Case::FirstGreater],
            (Domain::Imputation, false, false) => {
                &[Case::Equal, Case::FirstGreater, Case::SecondGreater]
            }
        };
        for (first, with_first) in self.argmax_candidates(i, j, &polyhedron)? {
            for (second, with_both) in self.argmax_candidates(j, i, &with_first)? {
                for case in cases {
                    let mut next = with_both.clone();
                    let difference = difference(first, second, self.n);
                    let gap = self.game.value(first) - self.game.value(second);
                    match case {
                        Case::Free => {}
                        Case::Equal => next.add_equality(difference, gap),
                        Case::FirstGreater => {
                            next.inequalities.push(Row {
                                coefficients: difference,
                                rhs: gap,
                            });
                            next.add_equality(
                                unit(j, self.n),
                                self.game.value(Coalition::singleton(j)),
                            );
                        }
                        Case::SecondGreater => {
                            next.inequalities.push(Row {
                                coefficients: difference.iter().map(|a| -a).collect(),
                                rhs: -gap,
                            });
                            next.add_equality(
                                unit(i, self.n),
                                self.game.value(Coalition::singleton(i)),
                            );
                        }
                    }
                    if let Some(point) = self.feasible(&next)? {
                        self.explore(depth + 1, next, point)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `i in S, j not in S` の提携 `S` のうち、多面体の中で超過が最大になりうるものと、
    /// 「`S` が最大」という条件を加えた多面体の組を返す。
    /// アフィン包の上で超過が恒等的に等しい提携は 1 つにまとめる。
    fn argmax_candidates(
        &mut self,
        i: usize,
        j: usize,
        polyhedron: &Polyhedron,
    ) -> Result<Vec<(Coalition, Polyhedron)>> {
        let group: Vec<Coalition> = (1..(1u64 << self.n))
            .map(Coalition)
            .filter(|s| s.contains(i) && !s.contains(j))
            .collect();
        let mut candidates: Vec<(Coalition, Polyhedron)> = Vec::new();
        for &coalition in &group {
            let duplicate = candidates.iter().any(|(chosen, _)| {
                polyhedron
                    .span
                    .contains(&difference(coalition, *chosen, self.n))
            });
            if duplicate {
                continue;
            }
            let mut next = polyhedron.clone();
            for &other in &group {
                if other == coalition {
                    continue;
                }
                // e(S) >= e(S') <=> x(S) - x(S') <= v(S) - v(S')
                // 方向が等式の張る空間に入る行もアフィン包上の定数条件として残す。
                next.inequalities.push(Row {
                    coefficients: difference(coalition, other, self.n),
                    rhs: self.game.value(coalition) - self.game.value(other),
                });
            }
            if self.feasible(&next)?.is_some() {
                candidates.push((coalition, next));
            }
        }
        Ok(candidates)
    }

    fn feasible(&mut self, polyhedron: &Polyhedron) -> Result<Option<Vec<f64>>> {
        let mut problem = lp::minimize();
        let x: Vec<Variable> = (0..self.n)
            .map(|_| problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY)))
            .collect();
        add_rows(&mut problem, &x, polyhedron, &[]);
        Ok(self
            .counter
            .solve(&problem)?
            .map(|solution| x.iter().map(|&var| solution[var]).collect()))
    }

    /// 暗黙の等式(多面体上で常に等号が成り立つ不等式)を等式に移す。
    ///
    /// 残りの不等式にスラック `s_k in [0, 1]` を付けて和を最大化し、
    /// 正のスラックを取れた不等式は暗黙の等式ではないとして外す。
    /// 最大値が 0 なら、残った不等式は全て暗黙の等式である。
    fn refresh_affine_hull(&mut self, polyhedron: &mut Polyhedron) -> Result<()> {
        let mut candidates: Vec<usize> = (0..polyhedron.inequalities.len()).collect();
        while !candidates.is_empty() {
            let mut problem = lp::maximize();
            let x: Vec<Variable> = (0..self.n)
                .map(|_| problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY)))
                .collect();
            let slacks: Vec<Variable> = candidates
                .iter()
                .map(|_| problem.add_var(1.0, (0.0, 1.0)))
                .collect();
            add_rows(
                &mut problem,
                &x,
                polyhedron,
                &candidates
                    .iter()
                    .copied()
                    .zip(slacks.iter().copied())
                    .collect::<Vec<_>>(),
            );
            let solution = self
                .counter
                .solve(&problem)?
                .ok_or_else(|| Error::Numerical("アフィン包の計算で多面体が空になった".into()))?;
            let before = candidates.len();
            let mut kept = Vec::new();
            for (&index, &slack) in candidates.iter().zip(&slacks) {
                if solution[slack] <= self.tolerance {
                    kept.push(index);
                }
            }
            candidates = kept;
            if candidates.len() == before {
                break;
            }
        }
        if candidates.is_empty() {
            return Ok(());
        }
        // 暗黙の等式を等式に移し、不等式からは外す。
        let mut implicit = vec![false; polyhedron.inequalities.len()];
        for &index in &candidates {
            implicit[index] = true;
        }
        let inequalities = std::mem::take(&mut polyhedron.inequalities);
        for (row, is_implicit) in inequalities.into_iter().zip(implicit) {
            if is_implicit {
                polyhedron.add_implied_equality(row.coefficients, row.rhs);
            } else {
                polyhedron.inequalities.push(row);
            }
        }
        Ok(())
    }

    fn piece(&mut self, mut polyhedron: Polyhedron, point: Vec<f64>) -> Result<KernelPiece> {
        self.refresh_affine_hull(&mut polyhedron)?;
        let basis = polyhedron.span.null_space();
        let dimension = basis.len();
        let vertices = self.vertices(&polyhedron, &point, &basis)?;
        Ok(KernelPiece {
            dimension,
            point,
            vertices,
            equalities: polyhedron.equalities,
            inequalities: polyhedron.inequalities,
        })
    }

    /// アフィン包を `x = point + sum z_l basis_l` と表し、`z` の空間で頂点を総当たりで求める。
    fn vertices(
        &mut self,
        polyhedron: &Polyhedron,
        point: &[f64],
        basis: &[Vec<f64>],
    ) -> Result<Option<Vec<Vec<f64>>>> {
        let dimension = basis.len();
        if dimension == 0 {
            return Ok(Some(vec![point.to_vec()]));
        }
        // z の空間での行 c . z <= r。正規化して重複を除く。
        let mut rows: Vec<(Vec<f64>, f64)> = Vec::new();
        for row in &polyhedron.inequalities {
            let c: Vec<f64> = basis.iter().map(|direction| row.lhs(direction)).collect();
            let norm = c.iter().map(|v| v * v).sum::<f64>().sqrt();
            if norm <= 1e-9 {
                continue;
            }
            let c: Vec<f64> = c.iter().map(|v| v / norm).collect();
            let r = (row.rhs - row.lhs(point)) / norm;
            match rows
                .iter_mut()
                .find(|(existing, _)| existing.iter().zip(&c).all(|(a, b)| (a - b).abs() <= 1e-9))
            {
                Some(existing) => existing.1 = existing.1.min(r),
                None => rows.push((c, r)),
            }
        }
        if !self.is_bounded(&rows, dimension)? {
            return Ok(None);
        }
        if combinations_count(rows.len(), dimension) > MAX_VERTEX_COMBINATIONS {
            return Ok(None);
        }
        let mut vertices: Vec<Vec<f64>> = Vec::new();
        for_each_combination(rows.len(), dimension, &mut |chosen| {
            let a = chosen.iter().map(|&k| rows[k].0.clone()).collect();
            let b = chosen.iter().map(|&k| rows[k].1).collect();
            let Some(z) = solve_square(a, b, 1e-9) else {
                return;
            };
            let feasible = rows.iter().all(|(c, r)| {
                c.iter().zip(&z).map(|(a, b)| a * b).sum::<f64>() <= r + self.tolerance
            });
            if !feasible {
                return;
            }
            let x: Vec<f64> = (0..point.len())
                .map(|i| point[i] + basis.iter().zip(&z).map(|(d, zl)| d[i] * zl).sum::<f64>())
                .collect();
            let known = vertices.iter().any(|v| {
                v.iter()
                    .zip(&x)
                    .all(|(a, b)| (a - b).abs() <= 10.0 * self.tolerance)
            });
            if !known {
                vertices.push(x);
            }
        });
        Ok(Some(vertices))
    }

    fn is_bounded(&mut self, rows: &[(Vec<f64>, f64)], dimension: usize) -> Result<bool> {
        for axis in 0..dimension {
            for sign in [1.0, -1.0] {
                let mut problem = lp::maximize();
                let z: Vec<Variable> = (0..dimension)
                    .map(|l| {
                        let objective = if l == axis { sign } else { 0.0 };
                        problem.add_var(objective, (f64::NEG_INFINITY, f64::INFINITY))
                    })
                    .collect();
                for (c, r) in rows {
                    let terms = z.iter().zip(c).map(|(&var, &coef)| (var, coef)).collect();
                    lp::add(&mut problem, terms, Cmp::Le, *r);
                }
                if self.counter.solve_bounded(&problem)?.is_none() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

/// 多面体の行を LP に入れる。`slacks` に含まれる不等式にはスラック変数を足す。
fn add_rows(
    problem: &mut microlp::Problem,
    x: &[Variable],
    polyhedron: &Polyhedron,
    slacks: &[(usize, Variable)],
) {
    let terms = |row: &Row| -> Vec<(Variable, f64)> {
        x.iter()
            .zip(&row.coefficients)
            .filter(|(_, coef)| **coef != 0.0)
            .map(|(&var, &coef)| (var, coef))
            .collect()
    };
    for row in &polyhedron.equalities {
        lp::add(problem, terms(row), Cmp::Eq, row.rhs);
    }
    for (index, row) in polyhedron.inequalities.iter().enumerate() {
        let mut row_terms = terms(row);
        if let Some((_, slack)) = slacks.iter().find(|(k, _)| *k == index) {
            row_terms.push((*slack, 1.0));
        }
        lp::add(problem, row_terms, Cmp::Le, row.rhs);
    }
}

/// 次元の大きい順に並べ、頂点が全て既出の多面体に含まれるものを除く。
fn remove_contained(mut pieces: Vec<KernelPiece>, tolerance: f64) -> Vec<KernelPiece> {
    pieces.sort_by_key(|piece| std::cmp::Reverse(piece.dimension));
    let mut kept: Vec<KernelPiece> = Vec::new();
    for piece in pieces {
        let contained = match &piece.vertices {
            Some(vertices) => kept.iter().any(|other| {
                vertices
                    .iter()
                    .all(|vertex| other.contains(vertex, tolerance))
            }),
            None => false,
        };
        if !contained {
            kept.push(piece);
        }
    }
    kept
}

fn indicator(coalition: Coalition, n: usize) -> Vec<f64> {
    crate::linalg::indicator(coalition, n)
}

fn unit(i: usize, n: usize) -> Vec<f64> {
    let mut vector = vec![0.0; n];
    vector[i] = 1.0;
    vector
}

/// `1_S - 1_T`。
fn difference(s: Coalition, t: Coalition, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| f64::from(u8::from(s.contains(i))) - f64::from(u8::from(t.contains(i))))
        .collect()
}

fn combinations_count(n: usize, k: usize) -> usize {
    let mut count: usize = 1;
    for i in 0..k {
        count = count.saturating_mul(n - i) / (i + 1);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_combinations() {
        assert_eq!(combinations_count(4, 2), 6);
    }

    #[test]
    fn pair_game_kernel_is_single_point() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let set = kernel_set(&game, Domain::Imputation, SetOptions::for_game(&game)).unwrap();
        assert_eq!(set.pieces.len(), 1, "{set:?}");
        let piece = &set.pieces[0];
        assert_eq!(piece.dimension, 0);
        let vertex = &piece.vertices.as_ref().unwrap()[0];
        for (a, e) in vertex.iter().zip([2.0, 2.0, 0.0]) {
            assert!((a - e).abs() < 1e-6, "{vertex:?}");
        }
    }

    #[test]
    fn two_player_prekernel() {
        // s_12 = v1 - x1, s_21 = v2 - x2 から x1 - x2 = v1 - v2。
        let game = ExplicitGame::from_binary(&[1.0, 3.0, 10.0]).unwrap();
        let set = kernel_set(&game, Domain::Preimputation, SetOptions::for_game(&game)).unwrap();
        assert_eq!(set.pieces.len(), 1);
        let vertex = &set.pieces[0].vertices.as_ref().unwrap()[0];
        assert!((vertex[0] - 4.0).abs() < 1e-9 && (vertex[1] - 6.0).abs() < 1e-9);
    }

    fn segment(a: [f64; 3], b: [f64; 3]) -> KernelPiece {
        // x1 + x2 + x3 = 3 の平面上の線分 (等式はアフィン包を表す 2 本)。
        let delta: Vec<f64> = b.iter().zip(&a).map(|(p, q)| p - q).collect();
        // delta と (1, 1, 1) に直交する法線
        let normal = vec![
            delta[1] - delta[2],
            delta[2] - delta[0],
            delta[0] - delta[1],
        ];
        KernelPiece {
            dimension: 1,
            point: a.iter().zip(&b).map(|(p, q)| (p + q) / 2.0).collect(),
            vertices: Some(vec![a.to_vec(), b.to_vec()]),
            equalities: vec![
                Row {
                    coefficients: vec![1.0; 3],
                    rhs: 3.0,
                },
                Row {
                    rhs: dot(&normal, &a),
                    coefficients: normal,
                },
            ],
            inequalities: Vec::new(),
        }
    }

    #[test]
    fn merges_touching_and_overlapping_collinear_segments() {
        let set = KernelSet {
            pieces: vec![
                segment([1.0, 1.0, 1.0], [2.0, 0.5, 0.5]),
                // 端点を共有する (向きは逆)
                segment([3.0, 0.0, 0.0], [2.0, 0.5, 0.5]),
                // 重なる
                segment([1.5, 0.75, 0.75], [0.0, 1.5, 1.5]),
                // 端点を共有するが別の直線
                segment([1.0, 1.0, 1.0], [1.0, 2.0, 0.0]),
                // 同じ直線だが離れている
                segment([-1.0, 2.0, 2.0], [-2.0, 2.5, 2.5]),
            ],
            nodes: 0,
            lp_solves: 0,
        };
        let merged = set.merge_collinear_segments(1e-9);
        assert_eq!(merged.pieces.len(), 3, "{merged:?}");
        let long = merged
            .pieces
            .iter()
            .find(|piece| piece.contains(&[3.0, 0.0, 0.0], 1e-9))
            .expect("合わせた線分");
        let mut ends = long.vertices.clone().unwrap();
        ends.sort_by(|a, b| a[0].total_cmp(&b[0]));
        assert_eq!(ends, vec![vec![0.0, 1.5, 1.5], vec![3.0, 0.0, 0.0]]);
        for x in [[0.0, 1.5, 1.5], [1.2, 0.9, 0.9], [3.0, 0.0, 0.0]] {
            assert!(long.contains(&x, 1e-9));
        }
        assert!(!long.contains(&[3.2, -0.1, -0.1], 1e-9));
        assert!(!long.contains(&[1.0, 2.0, 0.0], 1e-9));
        // 元の和集合に含まれる点は、まとめた後も含まれる
        for piece in &set.pieces {
            for vertex in piece.vertices.as_ref().unwrap() {
                assert!(merged.contains(vertex, 1e-9));
            }
        }
    }

    #[test]
    fn enumerates_vertices_of_four_dimensional_simplex() {
        // 配分集合 {x >= 0, sum x = 1} (5 人) は 4 次元の単体で、頂点は単位ベクトル。
        let game = ExplicitGame::from_binary(&[0.0; 31]).unwrap();
        let mut search = Search {
            game: &game,
            domain: Domain::Imputation,
            tolerance: 1e-9,
            max_nodes: 1,
            n: 5,
            pairs: Vec::new(),
            counter: Counter::default(),
            nodes: 0,
            leaves: Vec::new(),
        };
        let mut polyhedron = Polyhedron::new(5);
        polyhedron.add_equality(vec![1.0; 5], 1.0);
        for i in 0..5 {
            let mut coefficients = vec![0.0; 5];
            coefficients[i] = -1.0;
            polyhedron.inequalities.push(Row {
                coefficients,
                rhs: 0.0,
            });
        }
        let piece = search.piece(polyhedron, vec![0.2; 5]).unwrap();
        assert_eq!(piece.dimension, 4);
        let mut vertices = piece.vertices.expect("有界なので頂点を返す");
        vertices.sort_by(|a, b| {
            b.iter()
                .zip(a)
                .map(|(p, q)| p.total_cmp(q))
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        assert_eq!(vertices.len(), 5);
        for (i, vertex) in vertices.iter().enumerate() {
            for (k, value) in vertex.iter().enumerate() {
                let expected = if k == i { 1.0 } else { 0.0 };
                assert!((value - expected).abs() < 1e-9, "{vertices:?}");
            }
        }
    }

    #[test]
    fn rejects_large_games() {
        let game = crate::generators::bnf(2, MAX_KERNEL_SET_PLAYERS + 1, 0).unwrap();
        let err = kernel_set(&game, Domain::Imputation, SetOptions::for_game(&game)).unwrap_err();
        assert!(matches!(err, Error::TooManyPlayers { .. }));
    }
}
