//! 3-4 人のゲームの配分集合の図 (SVG)。コア・カーネル・仁・Shapley 値などを同じ図に描く。
//!
//! - 3 人: 配分集合 (三角形) を正三角形に描く。頂点 `i` はプレイヤー `i` が協力の余剰を全て受け取る配分。
//! - 4 人: 配分集合 (四面体) を斜めから見た図に描く。コアは稜線で描く。
//!
//! 座標は配分集合の重心座標 `λ_i = (x_i - v({i})) / (v(N) - sum v({j}))` で、配分集合の外の点
//! (準配分) も同じ式で描く (λ が負になる)。配分集合が 1 点以下 (余剰が 0 以下) のゲームは描けない。
//!
//! 色は検証済みの 3 色 (コア・カーネル・仁) だけに使い、その他の点 (Shapley 値、利用者の点) は
//! 文字色の印と形・直接ラベルで区別する。各点には `<title>` (ホバーで値を表示) を付ける。
//! 明暗のテーマは `prefers-color-scheme` で切り替える。

use std::fmt::Write as _;

use crate::error::{Error, Result};
use crate::game::{Coalition, ExplicitGame, player_name};
use crate::kernel::{SetOptions, kernel_set};
use crate::linalg::{for_each_combination, indicator, solve_square};
use crate::{Domain, nucleolus, values};

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PlotOptions {
    /// プレイヤーの名前 (なければ 1 始まりの番号)。
    pub names: Option<Vec<String>>,
    /// カーネル全体を描くか (6 人まで計算できるが、図は 3-4 人)。
    pub kernel: bool,
    /// Shapley 値を描くか。
    pub shapley: bool,
    /// 追加で描く点 (ラベル, 配分)。
    pub points: Vec<(String, Vec<f64>)>,
    pub title: Option<String>,
}

impl Default for PlotOptions {
    fn default() -> PlotOptions {
        PlotOptions {
            names: None,
            kernel: true,
            shapley: true,
            points: Vec::new(),
            title: None,
        }
    }
}

/// 図に描いた要素 (表として確かめるため、SVG とあわせて返す)。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Figure {
    pub svg: String,
    /// コアの頂点 (空ならコアは空)。
    pub core_vertices: Vec<Vec<f64>>,
    /// カーネルの多面体の頂点 (多面体ごと)。
    pub kernel_pieces: Vec<Vec<Vec<f64>>>,
    /// 描いた点 (ラベル, 配分)。仁・Shapley 値・追加の点の順。
    pub points: Vec<(String, Vec<f64>)>,
}

const WIDTH: f64 = 640.0;
const HEIGHT: f64 = 600.0;

/// 配分集合の図を作る。
pub fn imputation_figure(game: &ExplicitGame, options: &PlotOptions) -> Result<Figure> {
    let n = game.players();
    if !(n == 3 || n == 4) {
        return Err(Error::InvalidArgument(format!(
            "図は 3 人か 4 人のゲームに限る ({n} 人)"
        )));
    }
    let singles = game.singleton_values();
    let surplus = game.value(game.grand()) - singles.iter().sum::<f64>();
    if surplus <= 1e-12 * game.max_abs_value().max(1.0) {
        return Err(Error::InvalidArgument(
            "配分集合が 1 点以下 (v(N) <= sum v({i})) なので描けない".into(),
        ));
    }
    let names: Vec<String> = match &options.names {
        Some(names) if names.len() == n => names.clone(),
        Some(_) => return Err(Error::InvalidArgument("名前の数が人数と異なる".into())),
        None => (0..n).map(|i| player_name(None, i)).collect(),
    };
    let projection = Projection::new(n, singles.clone(), surplus);

    let core_vertices = core_vertices(game);
    let kernel_pieces = if options.kernel {
        kernel_set(game, Domain::Imputation, SetOptions::for_game(game))?
            .pieces
            .into_iter()
            .filter_map(|piece| piece.vertices)
            .collect()
    } else {
        Vec::new()
    };
    let mut points: Vec<(String, Vec<f64>)> =
        vec![("仁".to_string(), nucleolus::nucleolus(game)?.allocation)];
    if options.shapley {
        points.push(("Shapley 値".to_string(), values::shapley(game)));
    }
    points.extend(options.points.iter().cloned());
    for (label, x) in &points {
        if x.len() != n {
            return Err(Error::InvalidArgument(format!(
                "点 {label:?} の長さが人数と異なる"
            )));
        }
    }

    // 画面の座標の範囲 (配分集合の頂点と全ての点を含む)。
    let corners: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut x = singles.clone();
            x[i] += surplus;
            x
        })
        .collect();
    let mut screen: Vec<(f64, f64)> = corners.iter().map(|x| projection.project(x)).collect();
    screen.extend(points.iter().map(|(_, x)| projection.project(x)));
    let scene = Scene {
        n,
        names: &names,
        view: View::fit(&screen),
        projection,
        corners: screen[..n].to_vec(),
    };
    let title = options
        .title
        .clone()
        .unwrap_or_else(|| format!("{n} 人ゲームの配分集合"));

    let mut svg = String::new();
    draw_header(&mut svg, &title);
    scene.draw_frame(&mut svg);
    scene.draw_core(&mut svg, game, &core_vertices);
    scene.draw_kernel(&mut svg, &kernel_pieces);
    scene.draw_vertex_labels(&mut svg);
    scene.draw_points(&mut svg, &points, options.shapley);
    draw_legend(
        &mut svg,
        !core_vertices.is_empty(),
        options.kernel,
        points.len() > 1,
    );
    svg.push_str("</svg>\n");
    Ok(Figure {
        svg,
        core_vertices,
        kernel_pieces,
        points,
    })
}

fn draw_header(svg: &mut String, title: &str) {
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {HEIGHT}" width="{WIDTH}" height="{HEIGHT}" role="img" aria-labelledby="t d" font-family="system-ui, -apple-system, 'Hiragino Sans', 'Noto Sans JP', sans-serif">"#
    );
    let _ = writeln!(svg, "<title id=\"t\">{}</title>", escape(title));
    let _ = writeln!(
        svg,
        "<desc id=\"d\">配分集合と、コア (青)・カーネル (橙)・仁 (水色) などの位置。各点の値は点に重ねたツールチップにある。</desc>"
    );
    svg.push_str(STYLE);
    let _ = writeln!(
        svg,
        r#"<rect class="surface" width="{WIDTH}" height="{HEIGHT}"/>"#
    );
    let _ = writeln!(
        svg,
        r#"<text class="title" x="24" y="36">{}</text>"#,
        escape(title)
    );
}

/// 線分の要素 (`tooltip` があれば `<title>` を付ける)。
fn line(svg: &mut String, class: &str, p: (f64, f64), q: (f64, f64), tooltip: Option<&str>) {
    let _ = write!(
        svg,
        r#"<line class="{class}" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}""#,
        p.0, p.1, q.0, q.1
    );
    match tooltip {
        Some(text) => {
            let _ = writeln!(svg, "><title>{text}</title></line>");
        }
        None => svg.push_str("/>\n"),
    }
}

/// 凸多角形の要素 (頂点は角度順に並べ直す)。
fn polygon(svg: &mut String, class: &str, points: &[(f64, f64)], tooltip: &str) {
    let path: Vec<String> = convex_order(points)
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect();
    let _ = writeln!(
        svg,
        r#"<polygon class="{class}" points="{}"><title>{tooltip}</title></polygon>"#,
        path.join(" ")
    );
}

/// 点の印の形。
#[derive(Clone, Copy)]
enum Marker {
    Circle,
    Diamond,
    Square,
}

impl Marker {
    fn svg(self, x: f64, y: f64) -> String {
        match self {
            Marker::Circle => {
                format!(r#"<circle class="nucleolus" cx="{x:.1}" cy="{y:.1}" r="5.5"/>"#)
            }
            Marker::Diamond => format!(
                r#"<rect class="other" x="{:.1}" y="{:.1}" width="9" height="9" transform="rotate(45 {x:.1} {y:.1})"/>"#,
                x - 4.5,
                y - 4.5
            ),
            Marker::Square => format!(
                r#"<rect class="other" x="{:.1}" y="{:.1}" width="9" height="9"/>"#,
                x - 4.5,
                y - 4.5
            ),
        }
    }
}

/// 図の座標系と名前。
struct Scene<'a> {
    n: usize,
    names: &'a [String],
    projection: Projection,
    view: View,
    /// 配分集合の頂点の (写像前の) 座標。
    corners: Vec<(f64, f64)>,
}

impl Scene<'_> {
    fn map(&self, x: &[f64]) -> (f64, f64) {
        self.view.map(self.projection.project(x))
    }

    /// 配分集合の輪郭。
    fn draw_frame(&self, svg: &mut String) {
        for a in 0..self.n {
            for b in a + 1..self.n {
                let (p, q) = (
                    self.view.map(self.corners[a]),
                    self.view.map(self.corners[b]),
                );
                line(svg, "frame", p, q, None);
            }
        }
    }

    /// コア (3 人は多角形、4 人は多面体の稜線)。
    fn draw_core(&self, svg: &mut String, game: &ExplicitGame, vertices: &[Vec<f64>]) {
        if vertices.is_empty() {
            return;
        }
        let mapped: Vec<(f64, f64)> = vertices.iter().map(|x| self.map(x)).collect();
        if self.n == 3 {
            let tooltip = format!("コア ({} 頂点)", vertices.len());
            polygon(svg, "core", &mapped, &tooltip);
        } else {
            for (a, b) in core_edges(game, vertices) {
                line(svg, "core-edge", mapped[a], mapped[b], None);
            }
        }
    }

    fn draw_kernel(&self, svg: &mut String, pieces: &[Vec<Vec<f64>>]) {
        for piece in pieces {
            let mapped: Vec<(f64, f64)> = piece.iter().map(|x| self.map(x)).collect();
            let tooltip = format!("カーネル ({} 頂点)", piece.len());
            match mapped.len() {
                1 => {}
                2 => line(svg, "kernel", mapped[0], mapped[1], Some(&tooltip)),
                _ => polygon(svg, "kernel-area", &mapped, &tooltip),
            }
        }
        // 1 点のカーネルは仁と重なることが多いので、点の印の下に描く。
        for piece in pieces.iter().filter(|p| p.len() == 1) {
            let (x, y) = self.map(&piece[0]);
            let _ = writeln!(
                svg,
                r#"<circle class="kernel-dot" cx="{x:.1}" cy="{y:.1}" r="9"><title>カーネル: {}</title></circle>"#,
                escape(&format_allocation(&piece[0], self.names))
            );
        }
    }

    /// 頂点の名前 (重心から外向きにずらす)。
    fn draw_vertex_labels(&self, svg: &mut String) {
        let center = {
            let (sx, sy) = self
                .corners
                .iter()
                .fold((0.0, 0.0), |acc, p| (acc.0 + p.0, acc.1 + p.1));
            self.view.map((sx / self.n as f64, sy / self.n as f64))
        };
        for (i, &corner) in self.corners.iter().enumerate() {
            let (x, y) = self.view.map(corner);
            let (dx, dy) = (x - center.0, y - center.1);
            let norm = (dx * dx + dy * dy).sqrt().max(1e-9);
            let (lx, ly) = (x + 22.0 * dx / norm, y + 22.0 * dy / norm + 5.0);
            let _ = writeln!(
                svg,
                r#"<text class="vertex" x="{lx:.1}" y="{ly:.1}" text-anchor="middle">{}</text>"#,
                escape(&self.names[i])
            );
        }
    }

    /// 点 (仁は水色の丸、Shapley 値は文字色のひし形、他は四角) とラベル。
    fn draw_points(&self, svg: &mut String, points: &[(String, Vec<f64>)], shapley: bool) {
        let placed: Vec<(f64, f64)> = points.iter().map(|(_, x)| self.map(x)).collect();
        let mut taken: Vec<Rect> = placed
            .iter()
            .map(|&(x, y)| Rect::around(x, y, 11.0))
            .collect();
        // 印は後で逆順に描く (仁が一番上に来るように)。
        let mut marks: Vec<String> = Vec::new();
        for (k, ((label, x), &(px, py))) in points.iter().zip(&placed).enumerate() {
            let marker = match k {
                0 => Marker::Circle,
                1 if shapley => Marker::Diamond,
                _ => Marker::Square,
            };
            let tooltip = format!("{label}: {}", format_allocation(x, self.names));
            marks.push(format!(
                r#"<g class="point">{}<circle class="hit" cx="{px:.1}" cy="{py:.1}" r="12"/><title>{}</title></g>"#,
                marker.svg(px, py),
                escape(&tooltip)
            ));
            let (lx, ly, anchor, rect) = place_label(px, py, text_width(label, 13.0), &taken);
            taken.push(rect);
            let _ = writeln!(
                svg,
                r#"<text class="label" x="{lx:.1}" y="{ly:.1}" text-anchor="{anchor}">{}</text>"#,
                escape(label)
            );
        }
        for mark in marks.iter().rev() {
            let _ = writeln!(svg, "{mark}");
        }
    }
}

/// ラベルを右・左・下・上の順に、`taken` と重ならない位置に置く (どこも重なれば右)。
fn place_label(px: f64, py: f64, width: f64, taken: &[Rect]) -> (f64, f64, &'static str, Rect) {
    let candidates = [
        (px + 15.0, py + 4.5, "start"),
        (px - 15.0, py + 4.5, "end"),
        (px + 15.0, py + 22.0, "start"),
        (px + 15.0, py - 13.0, "start"),
        (px - 15.0, py + 22.0, "end"),
        (px - 15.0, py - 13.0, "end"),
    ];
    let with_rect =
        |(x, y, anchor): (f64, f64, &'static str)| (x, y, anchor, Rect::label(x, y, anchor, width));
    candidates
        .iter()
        .map(|&c| with_rect(c))
        .find(|(_, _, _, rect)| taken.iter().all(|other| !rect.overlaps(other)))
        .unwrap_or_else(|| with_rect(candidates[0]))
}

fn draw_legend(svg: &mut String, core: bool, kernel: bool, others: bool) {
    let legend_y = HEIGHT - 28.0;
    let mut lx = 24.0;
    let mut item = |svg: &mut String, swatch: &str, text: &str| {
        let _ = writeln!(
            svg,
            r#"<g transform="translate({lx:.0},{legend_y:.0})">{swatch}<text class="legend" x="18" y="5">{text}</text></g>"#
        );
        lx += 18.0 + text_width(text, 13.0) + 22.0;
    };
    item(
        svg,
        r#"<line class="frame" x1="0" y1="0" x2="12" y2="0"/>"#,
        "配分集合",
    );
    item(
        svg,
        r#"<rect class="core" x="0" y="-6" width="12" height="12"/>"#,
        if core { "コア" } else { "コア (空)" },
    );
    if kernel {
        item(
            svg,
            r#"<line class="kernel" x1="0" y1="0" x2="12" y2="0"/>"#,
            "カーネル",
        );
    }
    item(
        svg,
        r#"<circle class="nucleolus" cx="6" cy="0" r="5"/>"#,
        "仁",
    );
    if others {
        item(
            svg,
            r#"<rect class="other" x="2" y="-4" width="8" height="8"/>"#,
            "その他の点",
        );
    }
}

const STYLE: &str = r#"<style>
svg { --surface: #fcfcfb; --text-primary: #0b0b0b; --text-secondary: #52514e; --grid: #d6d5d0;
      --series-1: #2a78d6; --series-2: #eb6834; --series-3: #1baf7a; }
@media (prefers-color-scheme: dark) {
  svg { --surface: #1a1a19; --text-primary: #ffffff; --text-secondary: #c3c2b7; --grid: #4a4a46;
        --series-1: #3987e5; --series-2: #d95926; --series-3: #199e70; }
}
.surface { fill: var(--surface); }
.title { fill: var(--text-primary); font-size: 18px; font-weight: 600; }
.vertex { fill: var(--text-primary); font-size: 15px; font-weight: 600; }
.label, .legend { fill: var(--text-secondary); font-size: 13px; }
.label, .vertex { paint-order: stroke; stroke: var(--surface); stroke-width: 4px; stroke-linejoin: round; }
.frame { stroke: var(--text-secondary); stroke-width: 1; fill: none; }
.core { fill: var(--series-1); fill-opacity: 0.12; stroke: var(--series-1); stroke-width: 2; stroke-linejoin: round; }
.core-edge { stroke: var(--series-1); stroke-width: 2; stroke-linecap: round; }
.kernel { stroke: var(--series-2); stroke-width: 3; stroke-linecap: round; }
.kernel-area { fill: var(--series-2); fill-opacity: 0.15; stroke: var(--series-2); stroke-width: 2; }
.kernel-dot { fill: none; stroke: var(--series-2); stroke-width: 2.5; }
.nucleolus { fill: var(--series-3); stroke: var(--surface); stroke-width: 2; }
.other { fill: var(--text-primary); stroke: var(--surface); stroke-width: 2; }
.hit { fill: transparent; }
.point:hover .nucleolus, .point:hover .other { stroke: var(--text-primary); }
</style>
"#;

/// 文字列の幅の目安 (全角は 1 文字、半角は 0.55 文字)。
fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| if c.is_ascii() { 0.55 * size } else { size })
        .sum()
}

#[derive(Clone, Copy)]
struct Rect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Rect {
    fn around(x: f64, y: f64, radius: f64) -> Rect {
        Rect {
            left: x - radius,
            top: y - radius,
            right: x + radius,
            bottom: y + radius,
        }
    }

    /// `(x, y)` を基準線に置いたラベルの範囲 (`anchor` は `"start"` か `"end"`)。
    fn label(x: f64, y: f64, anchor: &str, width: f64) -> Rect {
        let left = if anchor == "start" { x } else { x - width };
        Rect {
            left,
            top: y - 11.0,
            right: left + width,
            bottom: y + 3.0,
        }
    }

    fn overlaps(&self, other: &Rect) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn format_allocation(x: &[f64], names: &[String]) -> String {
    x.iter()
        .zip(names)
        .map(|(v, name)| format!("{name} {v:.4}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// 重心座標から画面の 2 次元座標への写像。
struct Projection {
    singles: Vec<f64>,
    surplus: f64,
    corners: Vec<[f64; 3]>,
}

impl Projection {
    fn new(n: usize, singles: Vec<f64>, surplus: f64) -> Projection {
        let h = 3f64.sqrt() / 2.0;
        let corners = if n == 3 {
            vec![[0.5, 0.0, 0.0], [0.0, h, 0.0], [1.0, h, 0.0]]
        } else {
            // 正四面体を斜めから見る (z 軸の回りに回し、手前に傾ける)。
            let raw = [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.5, h, 0.0],
                [0.5, h / 3.0, (2.0f64 / 3.0).sqrt()],
            ];
            let (theta, phi) = (0.45f64, 0.35f64);
            raw.iter()
                .map(|p| {
                    let (x, y) = (p[0] - 0.5, p[1] - h / 3.0);
                    let rx = x * theta.cos() - y * theta.sin();
                    let ry = x * theta.sin() + y * theta.cos();
                    let sy = -(p[2] * phi.cos()) + ry * phi.sin();
                    [rx, sy, 0.0]
                })
                .collect()
        };
        Projection {
            singles,
            surplus,
            corners,
        }
    }

    fn project(&self, x: &[f64]) -> (f64, f64) {
        let mut point = (0.0, 0.0);
        for (i, corner) in self.corners.iter().enumerate() {
            let lambda = (x[i] - self.singles[i]) / self.surplus;
            point.0 += lambda * corner[0];
            point.1 += lambda * corner[1];
        }
        point
    }
}

/// 図の座標から SVG の座標への拡大・平行移動 (縦横比を保つ)。
struct View {
    scale: f64,
    offset: (f64, f64),
}

impl View {
    fn fit(points: &[(f64, f64)]) -> View {
        let (mut min, mut max) = (
            (f64::INFINITY, f64::INFINITY),
            (f64::NEG_INFINITY, f64::NEG_INFINITY),
        );
        for p in points {
            min = (min.0.min(p.0), min.1.min(p.1));
            max = (max.0.max(p.0), max.1.max(p.1));
        }
        let (left, top, right, bottom) = (70.0, 80.0, 90.0, 80.0);
        let width = (max.0 - min.0).max(1e-9);
        let height = (max.1 - min.1).max(1e-9);
        let scale = ((WIDTH - left - right) / width).min((HEIGHT - top - bottom) / height);
        let offset = (
            left + (WIDTH - left - right - scale * width) / 2.0 - scale * min.0,
            top + (HEIGHT - top - bottom - scale * height) / 2.0 - scale * min.1,
        );
        View { scale, offset }
    }

    fn map(&self, p: (f64, f64)) -> (f64, f64) {
        (
            self.offset.0 + self.scale * p.0,
            self.offset.1 + self.scale * p.1,
        )
    }
}

/// 凸多角形の頂点を重心の回りの角度順に並べる。
fn convex_order(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let count = points.len() as f64;
    let center = points.iter().fold((0.0, 0.0), |acc, p| {
        (acc.0 + p.0 / count, acc.1 + p.1 / count)
    });
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| {
        (a.1 - center.1)
            .atan2(a.0 - center.0)
            .total_cmp(&(b.1 - center.1).atan2(b.0 - center.0))
    });
    ordered
}

/// コアの制約 `x(S) >= v(S)` (真部分提携) の係数。
fn core_constraints(game: &ExplicitGame) -> Vec<(Coalition, f64)> {
    (1..game.grand().0)
        .map(|mask| (Coalition(mask), game.value(Coalition(mask))))
        .collect()
}

/// コアの頂点 (`n - 1` 本の制約が等号で成り立ち、全ての制約を満たす点)。
pub fn core_vertices(game: &ExplicitGame) -> Vec<Vec<f64>> {
    let n = game.players();
    let tolerance = 1e-9 * game.max_abs_value().max(1.0);
    let constraints = core_constraints(game);
    let total = game.value(game.grand());
    let mut vertices: Vec<Vec<f64>> = Vec::new();
    for_each_combination(constraints.len(), n - 1, &mut |chosen: &[usize]| {
        let mut a: Vec<Vec<f64>> = chosen
            .iter()
            .map(|&k| indicator(constraints[k].0, n))
            .collect();
        let mut b: Vec<f64> = chosen.iter().map(|&k| constraints[k].1).collect();
        a.push(vec![1.0; n]);
        b.push(total);
        let Some(x) = solve_square(a, b, 1e-12) else {
            return;
        };
        let feasible = constraints
            .iter()
            .all(|&(s, v)| coalition_sum(s, &x) >= v - tolerance);
        let duplicate = vertices.iter().any(|y| {
            y.iter()
                .zip(&x)
                .all(|(p, q)| (p - q).abs() <= 1e3 * tolerance)
        });
        if feasible && !duplicate {
            vertices.push(x);
        }
    });
    vertices
}

fn coalition_sum(coalition: Coalition, x: &[f64]) -> f64 {
    coalition.players().map(|i| x[i]).sum()
}

/// コアの稜線 (2 つの頂点で共通に等号が成り立つ制約が `n - 2` 本以上あり、独立なもの)。
fn core_edges(game: &ExplicitGame, vertices: &[Vec<f64>]) -> Vec<(usize, usize)> {
    let n = game.players();
    let tolerance = 1e-7 * game.max_abs_value().max(1.0);
    let constraints = core_constraints(game);
    let tight: Vec<Vec<usize>> = vertices
        .iter()
        .map(|x| {
            constraints
                .iter()
                .enumerate()
                .filter(|(_, (s, v))| (coalition_sum(*s, x) - v).abs() <= tolerance)
                .map(|(k, _)| k)
                .collect()
        })
        .collect();
    let mut edges = Vec::new();
    for a in 0..vertices.len() {
        for b in a + 1..vertices.len() {
            let common: Vec<usize> = tight[a]
                .iter()
                .copied()
                .filter(|k| tight[b].contains(k))
                .collect();
            // 共通の等号の制約の特性ベクトルが (全体と合わせて) n - 1 次元を張れば、2 点は同じ稜線上にある。
            let mut span = crate::linalg::Span::new(n);
            span.insert(&vec![1.0; n]);
            for k in common {
                span.insert_coalition(constraints[k].0);
            }
            if span.rank() >= n - 1 {
                edges.push((a, b));
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_player_figure_contains_core_kernel_and_points() {
        // Ferguson の例: コアは空でなく、仁は (8/3, 2/3, 5/3)。
        let game = ExplicitGame::from_lex(&[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0]).unwrap();
        let figure = imputation_figure(&game, &PlotOptions::default()).unwrap();
        assert!(figure.svg.starts_with("<svg"));
        assert!(figure.svg.contains("class=\"core\""));
        assert!(!figure.core_vertices.is_empty());
        for x in &figure.core_vertices {
            assert!((x.iter().sum::<f64>() - 5.0).abs() < 1e-9);
            assert!(crate::properties::is_in_core(&game, x, 1e-7));
        }
        assert_eq!(figure.points[0].0, "仁");
        assert!(figure.svg.contains("仁: 1 2.6667"));
    }

    #[test]
    fn names_are_escaped_in_every_tooltip() {
        // 3 人多数決: カーネルは 1 点なので、カーネルの点のツールチップにも名前が入る。
        let majority = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let options = PlotOptions {
            names: Some(vec!["<a>".into(), "b&c".into(), "d".into()]),
            ..PlotOptions::default()
        };
        let figure = imputation_figure(&majority, &options).unwrap();
        assert!(figure.svg.contains("kernel-dot"));
        assert!(!figure.svg.contains("<a>"));
        assert!(!figure.svg.contains("b&c"));
        assert!(figure.svg.contains("&lt;a&gt;"));
    }

    #[test]
    fn diamond_marks_only_the_shapley_value() {
        let game = ExplicitGame::from_lex(&[-1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 5.0]).unwrap();
        let options = PlotOptions {
            shapley: false,
            points: vec![("x".into(), vec![2.0, 1.0, 2.0])],
            ..PlotOptions::default()
        };
        let figure = imputation_figure(&game, &options).unwrap();
        assert!(!figure.svg.contains("rotate(45"));
        let figure = imputation_figure(&game, &PlotOptions::default()).unwrap();
        assert_eq!(figure.svg.matches("rotate(45").count(), 1);
    }

    #[test]
    fn empty_core_and_four_players() {
        // 3 人多数決: コアは空。
        let majority = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let figure = imputation_figure(&majority, &PlotOptions::default()).unwrap();
        assert!(figure.core_vertices.is_empty());
        assert!(figure.svg.contains("コア (空)"));
        // 4 人: コアの頂点は全てコアに属し、稜線が描かれる。
        let game = ExplicitGame::from_lex(&[
            0.0, 0.0, 0.0, 0.0, 5.0, 5.0, 8.0, 9.0, 10.0, 8.0, 13.0, 15.0, 16.0, 17.0, 21.0,
        ])
        .unwrap();
        let figure = imputation_figure(&game, &PlotOptions::default()).unwrap();
        assert!(figure.core_vertices.len() >= 4);
        assert!(figure.svg.contains("core-edge"));
        for x in &figure.core_vertices {
            assert!(crate::properties::is_in_core(&game, x, 1e-7));
        }
    }

    #[test]
    fn rejects_unsupported_games() {
        assert!(
            imputation_figure(
                &crate::generators::bnf(1, 5, 0).unwrap(),
                &PlotOptions::default()
            )
            .is_err()
        );
        let degenerate = ExplicitGame::from_lex(&[1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0]).unwrap();
        assert!(imputation_figure(&degenerate, &PlotOptions::default()).is_err());
    }
}
