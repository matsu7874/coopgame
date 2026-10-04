//! coopgame の Python バインディング。
//!
//! - 明示ベクトルのゲームは [`Game`]、Python の関数で値を返すゲームは [`FunctionGame`] で表す。
//! - 提携はプレイヤー番号 (0 始まり) の列で受け取り、Python の関数にはタプルで渡す。
//! - 領域は文字列 `"imputation"` (仁・カーネル) か `"preimputation"` (プレ仁・プレカーネル) で指定する。
//! - 結果は辞書で返す。有理数は `fractions.Fraction` で返す。

use std::cell::RefCell;

use coopgame::auto::AutoNucleolus;
use coopgame::convex::{self, ConvexOptions, ConvexStats};
use coopgame::cost::CostGame;
use coopgame::exact::{self, Rational};
use coopgame::explain as explain_core;
use coopgame::io as io_core;
use coopgame::kernel::{self, TransferOptions};
use coopgame::kernel_set::{self as kernel_set_core, Row, SetOptions};
use coopgame::nucleolus::{self as nucleolus_core, Method, Options};
use coopgame::oracle::airport::AirportGame;
use coopgame::oracle::graph::InducedSubgraphGame;
use coopgame::oracle::production::LinearProductionGame;
use coopgame::oracle::spanning_tree::SpanningTreeGame;
use coopgame::oracle::{PlayerSet, SetFunction};
use coopgame::partition;
use coopgame::plot as plot_core;
use coopgame::search as search_core;
use coopgame::structure::{Assume, ConvexChecked};
use coopgame::uncertainty as uncertainty_core;
use coopgame::verify::{self as verify_core, Check, Verified, VerifyOptions};
use coopgame::{Coalition, Domain, ExplicitGame, bankruptcy, generators, kohlberg, properties};
use coopgame::{Concept, Guarantee, Solution};
use coopgame::{bargaining, communication, compromise, power, sampled, values, variants};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

fn to_py_err(error: coopgame::Error) -> PyErr {
    match error {
        coopgame::Error::Lp(_) | coopgame::Error::Numerical(_) => {
            PyRuntimeError::new_err(error.to_string())
        }
        _ => PyValueError::new_err(error.to_string()),
    }
}

fn parse_domain(domain: &str) -> PyResult<Domain> {
    match domain {
        "imputation" => Ok(Domain::Imputation),
        "preimputation" => Ok(Domain::Preimputation),
        _ => Err(PyValueError::new_err(format!(
            "domain は \"imputation\" か \"preimputation\": {domain:?}"
        ))),
    }
}

fn coalition_of(players: usize, members: &[usize]) -> PyResult<Coalition> {
    Coalition::try_from_players(players, members).map_err(to_py_err)
}

fn fraction<'py>(py: Python<'py>, value: &Rational) -> PyResult<Bound<'py, PyAny>> {
    py.import("fractions")?
        .getattr("Fraction")?
        .call1((exact::format_rational(value),))
}

fn fractions<'py>(py: Python<'py>, values: &[Rational]) -> PyResult<Vec<Bound<'py, PyAny>>> {
    values.iter().map(|v| fraction(py, v)).collect()
}

/// int・float・`Fraction`・`"8/3"` のような文字列を有理数にする (`str()` を経由する)。
fn rational_of(value: &Bound<'_, PyAny>) -> PyResult<Rational> {
    let text: String = value.str()?.extract()?;
    exact::parse_rational(&text).map_err(to_py_err)
}

/// `order` が `"lex"` (辞書式順) なら真、`"binary"` (ビット順) なら偽。
fn is_lex(order: &str) -> PyResult<bool> {
    match order {
        "binary" => Ok(false),
        "lex" => Ok(true),
        _ => Err(PyValueError::new_err(format!(
            "order は \"binary\" か \"lex\": {order:?}"
        ))),
    }
}

/// 有理数の値 (int・Fraction・"8/3"・"0.1") の列から、`order` の並びのゲームを作る。
fn exact_game(values: &Bound<'_, PyAny>, order: &str) -> PyResult<exact::ExactGame> {
    let values: Vec<Rational> = values
        .try_iter()?
        .map(|value| rational_of(&value?))
        .collect::<PyResult<_>>()?;
    if is_lex(order)? {
        exact::ExactGame::from_lex(values)
    } else {
        exact::ExactGame::from_binary(values)
    }
    .map_err(to_py_err)
}

/// 明示ベクトルで与える TU ゲーム。
///
/// `values` は空提携を除く長さ `2^n - 1` のベクトル。`order="binary"` (既定) はビット順
/// (提携 `S` の値が `values[mask(S) - 1]`)、`order="lex"` は CoopGame・TUGLab と同じ辞書式順。
#[pyclass(module = "coopgame", frozen)]
struct Game {
    inner: ExplicitGame,
}

#[pymethods]
impl Game {
    #[new]
    #[pyo3(signature = (values, order = "binary"))]
    fn new(values: Vec<f64>, order: &str) -> PyResult<Game> {
        let inner = if is_lex(order)? {
            ExplicitGame::from_lex(&values)
        } else {
            ExplicitGame::from_binary(&values)
        };
        Ok(Game {
            inner: inner.map_err(to_py_err)?,
        })
    }

    /// `function(coalition)` (提携はプレイヤー番号のタプル) を全ての空でない提携で評価して作る。
    #[staticmethod]
    fn from_function(players: usize, function: &Bound<'_, PyAny>) -> PyResult<Game> {
        if players == 0 || players > coopgame::game::MAX_PLAYERS {
            return Err(PyValueError::new_err(format!(
                "人数は 1 以上 {} 以下",
                coopgame::game::MAX_PLAYERS
            )));
        }
        let values: Vec<f64> = (1u64..(1 << players))
            .map(|mask| {
                let members: Vec<usize> = (0..players).filter(|i| mask >> i & 1 == 1).collect();
                function
                    .call1((PyTuple::new(function.py(), members)?,))?
                    .extract::<f64>()
            })
            .collect::<PyResult<_>>()?;
        Game::new(values, "binary")
    }

    /// 重み付き投票ゲーム: 重みの和が `quota` 以上の提携の値が 1、他は 0。
    #[staticmethod]
    fn weighted_voting(weights: Vec<f64>, quota: f64) -> PyResult<Game> {
        let inner = generators::weighted_voting(&weights, quota).map_err(to_py_err)?;
        Ok(Game { inner })
    }

    /// 破産ゲーム `v(S) = max(0, estate - claims(N \ S))`。
    #[staticmethod]
    fn bankruptcy(estate: f64, claims: Vec<f64>) -> PyResult<Game> {
        let inner = generators::bankruptcy(estate, &claims).map_err(to_py_err)?;
        Ok(Game { inner })
    }

    /// Benedek, Fliege & Nguyen (2021) のゲームタイプ 1-5 の乱数ゲーム。
    #[staticmethod]
    fn bnf(kind: u8, players: usize, seed: u64) -> PyResult<Game> {
        let inner = generators::bnf(kind, players, seed).map_err(to_py_err)?;
        Ok(Game { inner })
    }

    #[getter]
    fn players(&self) -> usize {
        self.inner.players()
    }

    /// 提携 (プレイヤー番号の列) の値。空の提携は 0。
    fn value(&self, coalition: Vec<usize>) -> PyResult<f64> {
        let coalition = coalition_of(self.inner.players(), &coalition)?;
        Ok(self.inner.value(coalition))
    }

    /// ビット順のベクトル (空提携を除く)。
    fn values(&self) -> Vec<f64> {
        self.inner.values()[1..].to_vec()
    }

    fn is_superadditive(&self) -> bool {
        properties::is_superadditive(&self.inner)
    }

    fn is_convex(&self) -> bool {
        properties::is_convex(&self.inner)
    }

    fn is_zero_monotonic(&self) -> bool {
        properties::is_zero_monotonic(&self.inner)
    }

    fn has_nonempty_core(&self) -> PyResult<bool> {
        properties::has_nonempty_core(&self.inner).map_err(to_py_err)
    }

    #[pyo3(signature = (x, tolerance = 1e-9))]
    fn is_in_core(&self, x: Vec<f64>, tolerance: f64) -> bool {
        properties::is_in_core(&self.inner, &x, tolerance)
    }

    fn __repr__(&self) -> String {
        format!("Game(players={})", self.inner.players())
    }
}

/// Python の関数 `function(coalition) -> float` で値を返すゲーム (提携はプレイヤー番号のタプル)。
///
/// 全提携を列挙できない人数 (サンプリングによる推定・サンプルした提携の仁) に使う。
/// `v(空集合) = 0` とし、関数は空でない提携だけで呼ぶ。
#[pyclass(module = "coopgame", frozen)]
struct FunctionGame {
    players: usize,
    function: Py<PyAny>,
}

#[pymethods]
impl FunctionGame {
    #[new]
    fn new(players: usize, function: Py<PyAny>) -> PyResult<FunctionGame> {
        if players == 0 {
            return Err(PyValueError::new_err("人数は 1 以上"));
        }
        Ok(FunctionGame { players, function })
    }

    #[getter]
    fn players(&self) -> usize {
        self.players
    }

    fn __repr__(&self) -> String {
        format!("FunctionGame(players={})", self.players)
    }
}

/// Python の関数を [`SetFunction`] として呼ぶアダプタ。最初の例外を保存し、以降は NaN を返す。
struct Callback<'a, 'py> {
    py: Python<'py>,
    players: usize,
    function: &'a Py<PyAny>,
    error: RefCell<Option<PyErr>>,
}

impl Callback<'_, '_> {
    fn call(&self, coalition: &PlayerSet) -> PyResult<f64> {
        let members: Vec<usize> = coalition.members().collect();
        self.function
            .bind(self.py)
            .call1((PyTuple::new(self.py, members)?,))?
            .extract::<f64>()
    }

    /// 呼び出し中に起きた例外があれば返す。
    fn finish<T>(self, result: T) -> PyResult<T> {
        match self.error.into_inner() {
            Some(error) => Err(error),
            None => Ok(result),
        }
    }
}

impl SetFunction for Callback<'_, '_> {
    fn players(&self) -> usize {
        self.players
    }

    fn value(&self, coalition: &PlayerSet) -> f64 {
        if coalition.is_empty() || self.error.borrow().is_some() {
            return if coalition.is_empty() { 0.0 } else { f64::NAN };
        }
        match self.call(coalition) {
            Ok(value) => value,
            Err(error) => {
                *self.error.borrow_mut() = Some(error);
                f64::NAN
            }
        }
    }
}

/// [`Game`] と [`FunctionGame`] のどちらかを受け取り、[`SetFunction`] として計算を実行する。
fn with_set_function<T>(
    game: &Bound<'_, PyAny>,
    run: impl FnOnce(&dyn SetFunction) -> PyResult<T>,
) -> PyResult<T> {
    if let Ok(explicit) = game.cast::<Game>() {
        return run(&explicit.get().inner);
    }
    if let Ok(graph) = game.cast::<GraphGame>() {
        return run(&graph.get().inner);
    }
    if let Ok(function) = game.cast::<FunctionGame>() {
        let function = function.get();
        let callback = Callback {
            py: game.py(),
            players: function.players,
            function: &function.function,
            error: RefCell::new(None),
        };
        let result = run(&callback);
        return match result {
            Ok(value) => callback.finish(value),
            Err(error) => callback.finish(()).and(Err(error)),
        };
    }
    Err(PyValueError::new_err(
        "game は coopgame.Game、coopgame.FunctionGame、coopgame.InducedSubgraphGame のいずれか",
    ))
}

fn nucleolus_dict<'py>(
    py: Python<'py>,
    result: nucleolus_core::NucleolusResult,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("allocation", result.allocation)?;
    dict.set_item("levels", result.levels)?;
    dict.set_item("lp_solves", result.lp_solves)?;
    dict.set_item("rows_added", result.rows_added)?;
    dict.set_item("guarantee", result.guarantee.to_string())?;
    Ok(dict)
}

/// 仁 (`domain="imputation"`) またはプレ仁を逐次 LP で求める。`method` は `"cg"` (制約生成) か `"full"`。
#[pyfunction]
#[pyo3(signature = (game, domain = "imputation", method = "cg"))]
fn nucleolus<'py>(
    py: Python<'py>,
    game: &Game,
    domain: &str,
    method: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let mut options = Options::new(&game.inner, parse_domain(domain)?);
    options.method = match method {
        "cg" => Method::ConstraintGeneration,
        "full" => Method::Full,
        _ => {
            return Err(PyValueError::new_err(format!(
                "method は \"cg\" か \"full\": {method:?}"
            )));
        }
    };
    let result = nucleolus_core::nucleolus_with(&game.inner, options).map_err(to_py_err)?;
    nucleolus_dict(py, result)
}

/// プレ仁 (`nucleolus(game, "preimputation")` と同じ)。
#[pyfunction]
fn prenucleolus<'py>(py: Python<'py>, game: &Game) -> PyResult<Bound<'py, PyDict>> {
    nucleolus(py, game, "preimputation", "cg")
}

/// 最小コアの epsilon と、それを達成する配分の 1 つ。
#[pyfunction]
#[pyo3(signature = (game, domain = "imputation"))]
fn least_core<'py>(py: Python<'py>, game: &Game, domain: &str) -> PyResult<Bound<'py, PyDict>> {
    let result =
        nucleolus_core::least_core(&game.inner, parse_domain(domain)?).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("epsilon", result.epsilon)?;
    dict.set_item("allocation", result.allocation)?;
    dict.set_item("guarantee", result.guarantee.to_string())?;
    Ok(dict)
}

/// transfer scheme でカーネル (プレカーネル) の 1 点を求める。
#[pyfunction]
#[pyo3(signature = (game, domain = "imputation", start = None, max_iterations = None))]
fn kernel_point<'py>(
    py: Python<'py>,
    game: &Game,
    domain: &str,
    start: Option<Vec<f64>>,
    max_iterations: Option<usize>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut options = TransferOptions::for_game(&game.inner);
    if let Some(max_iterations) = max_iterations {
        options.max_iterations = max_iterations;
    }
    let result = kernel::kernel_point(
        &game.inner,
        parse_domain(domain)?,
        start.as_deref(),
        options,
    )
    .map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("allocation", result.allocation)?;
    dict.set_item("iterations", result.iterations)?;
    dict.set_item("violation", result.violation)?;
    dict.set_item("converged", result.converged)?;
    dict.set_item("guarantee", result.guarantee.to_string())?;
    Ok(dict)
}

/// 最大余剰の不釣り合い `max 2 delta_ij`。カーネルに属すれば 0。
#[pyfunction]
#[pyo3(signature = (game, x, domain = "imputation"))]
fn kernel_violation(game: &Game, x: Vec<f64>, domain: &str) -> PyResult<f64> {
    check_length(game, &x)?;
    Ok(kernel::kernel_violation(
        &game.inner,
        &x,
        parse_domain(domain)?,
    ))
}

/// `x` がカーネル (プレカーネル) に属するか。
#[pyfunction]
#[pyo3(signature = (game, x, domain = "imputation", tolerance = None))]
fn is_in_kernel(game: &Game, x: Vec<f64>, domain: &str, tolerance: Option<f64>) -> PyResult<bool> {
    let tolerance = tolerance.unwrap_or_else(|| coopgame::default_tolerance(&game.inner));
    Ok(kernel::is_in_kernel(
        &game.inner,
        &x,
        parse_domain(domain)?,
        tolerance,
    ))
}

fn rows<'py>(py: Python<'py>, rows: &[Row]) -> PyResult<Vec<Bound<'py, PyTuple>>> {
    rows.iter()
        .map(|row| {
            PyTuple::new(
                py,
                [
                    row.coefficients.clone().into_pyobject(py)?.into_any(),
                    row.rhs.into_pyobject(py)?.into_any(),
                ],
            )
        })
        .collect()
}

/// カーネル (プレカーネル) 全体を多面体の和として求める (6 人まで)。
/// `merge=True` なら、同じ直線上でつながる線分を 1 本にまとめる。
///
/// 多面体ごとに `dimension`、`point` (属する点)、`vertices` (3 次元以下で有界なら頂点、他は None)、
/// `equalities`・`inequalities` (`(係数, 右辺)` の列で、`係数 . x = 右辺`・`係数 . x <= 右辺`) を返す。
#[pyfunction]
#[pyo3(signature = (game, domain = "imputation", max_nodes = None, merge = false))]
fn kernel_set<'py>(
    py: Python<'py>,
    game: &Game,
    domain: &str,
    max_nodes: Option<usize>,
    merge: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let mut options = SetOptions::for_game(&game.inner);
    if let Some(max_nodes) = max_nodes {
        options.max_nodes = max_nodes;
    }
    let mut set = kernel_set_core::kernel_set(&game.inner, parse_domain(domain)?, options)
        .map_err(to_py_err)?;
    if merge {
        set = set.merge_collinear_segments(10.0 * options.tolerance);
    }
    let pieces = set
        .pieces
        .iter()
        .map(|piece| {
            let dict = PyDict::new(py);
            dict.set_item("dimension", piece.dimension)?;
            dict.set_item("point", piece.point.clone())?;
            dict.set_item("vertices", piece.vertices.clone())?;
            dict.set_item("equalities", rows(py, &piece.equalities)?)?;
            dict.set_item("inequalities", rows(py, &piece.inequalities)?)?;
            Ok(dict)
        })
        .collect::<PyResult<Vec<_>>>()?;
    let dict = PyDict::new(py);
    dict.set_item("pieces", pieces)?;
    dict.set_item("nodes", set.nodes)?;
    dict.set_item("lp_solves", set.lp_solves)?;
    Ok(dict)
}

fn check_length(game: &Game, x: &[f64]) -> PyResult<()> {
    if x.len() != game.inner.players() {
        return Err(PyValueError::new_err(format!(
            "配分の長さ {} がプレイヤー数 {} と異なる",
            x.len(),
            game.inner.players()
        )));
    }
    Ok(())
}

/// Kohlberg 基準で `x` が仁 (プレ仁) かを浮動小数点数で判定する。
#[pyfunction]
#[pyo3(signature = (game, x, domain = "imputation"))]
fn verify<'py>(
    py: Python<'py>,
    game: &Game,
    x: Vec<f64>,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    check_length(game, &x)?;
    let report = kohlberg::verify(&game.inner, &x, parse_domain(domain)?).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("satisfied", report.satisfied)?;
    dict.set_item("reason", report.reason)?;
    dict.set_item("levels_checked", report.levels_checked)?;
    Ok(dict)
}

/// 浮動小数点数の配分から厳密な配分を復元し、有理数で Kohlberg 基準を判定する。
///
/// `allocation` は `fractions.Fraction` の列 (復元できなければ空)。特性関数の値は f64 の 2 進数の値どおりに有理数にする。
#[pyfunction]
#[pyo3(signature = (game, x, domain = "imputation"))]
fn certify<'py>(
    py: Python<'py>,
    game: &Game,
    x: Vec<f64>,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    check_length(game, &x)?;
    let report = exact::certify(&game.inner, &x, parse_domain(domain)?).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("allocation", fractions(py, &report.allocation)?)?;
    dict.set_item("satisfied", report.satisfied)?;
    dict.set_item("reason", report.reason)?;
    dict.set_item("levels_checked", report.levels_checked)?;
    Ok(dict)
}

/// 有理数で与えたゲームについて、浮動小数点数の配分 `x` を厳密に検証する。
///
/// `values` は int・`Fraction`・`"8/3"` や `"0.1"` (10 進数の値どおり) の列。`order` は `"binary"` か `"lex"`。
/// 値を浮動小数点数にすると丸め誤差で厳密な検証が成り立たないゲーム (浮動小数点数の和で作った値など) に使う。
#[pyfunction]
#[pyo3(signature = (values, x, domain = "imputation", order = "binary"))]
fn certify_rational<'py>(
    py: Python<'py>,
    values: &Bound<'py, PyAny>,
    x: Vec<f64>,
    domain: &str,
    order: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let game = exact_game(values, order)?;
    if x.len() != game.players() {
        return Err(PyValueError::new_err("配分の長さがプレイヤー数と異なる"));
    }
    let report = exact::certify_exact(&game, &x, parse_domain(domain)?).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("allocation", fractions(py, &report.allocation)?)?;
    dict.set_item("satisfied", report.satisfied)?;
    dict.set_item("reason", report.reason)?;
    dict.set_item("levels_checked", report.levels_checked)?;
    Ok(dict)
}

/// 配分 `x` を説明する: 不満 (超過) の大きい提携の段、コアに属するか、最小コアの値、
/// プレイヤーごとの最も不満な提携。`text` に日本語の説明文が入る。
#[pyfunction]
#[pyo3(signature = (game, x, levels = 3, names = None))]
fn explain<'py>(
    py: Python<'py>,
    game: &Game,
    x: Vec<f64>,
    levels: usize,
    names: Option<Vec<String>>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut options = explain_core::ExplainOptions::default();
    options.levels = levels;
    let report = explain_core::report(&game.inner, &x, options).map_err(to_py_err)?;
    let members = |c: Coalition| c.players().collect::<Vec<_>>();
    let dict = PyDict::new(py);
    dict.set_item("text", explain_core::render(&report, names.as_deref()))?;
    dict.set_item("max_excess", report.max_excess)?;
    dict.set_item("least_core_epsilon", report.least_core_epsilon)?;
    dict.set_item("in_core", report.in_core)?;
    dict.set_item("efficiency_gap", report.efficiency_gap)?;
    let levels = report
        .levels
        .iter()
        .map(|level| {
            let d = PyDict::new(py);
            d.set_item("excess", level.excess)?;
            d.set_item("count", level.count)?;
            d.set_item(
                "coalitions",
                level
                    .coalitions
                    .iter()
                    .map(|c| members(*c))
                    .collect::<Vec<_>>(),
            )?;
            Ok(d)
        })
        .collect::<PyResult<Vec<_>>>()?;
    dict.set_item("levels", levels)?;
    let players = report
        .players
        .iter()
        .map(|p| {
            let d = PyDict::new(py);
            d.set_item("payoff", p.payoff)?;
            d.set_item("standalone", p.standalone)?;
            d.set_item("gain", p.gain)?;
            d.set_item("worst_with", (members(p.worst_with.0), p.worst_with.1))?;
            d.set_item(
                "worst_without",
                (members(p.worst_without.0), p.worst_without.1),
            )?;
            Ok(d)
        })
        .collect::<PyResult<Vec<_>>>()?;
    dict.set_item("players", players)?;
    Ok(dict)
}

/// 複数の配分 (`{名前: 配分}`) を安定性の指標で比べる。`text` に表が入る。
#[pyfunction]
#[pyo3(signature = (game, candidates, names = None))]
fn compare<'py>(
    py: Python<'py>,
    game: &Game,
    candidates: Vec<(String, Vec<f64>)>,
    names: Option<Vec<String>>,
) -> PyResult<Bound<'py, PyDict>> {
    let rows = explain_core::compare(&game.inner, &candidates).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item(
        "text",
        explain_core::render_comparison(&rows, names.as_deref()),
    )?;
    let table = rows
        .iter()
        .map(|row| {
            let d = PyDict::new(py);
            d.set_item("name", row.name.clone())?;
            d.set_item("allocation", row.allocation.clone())?;
            d.set_item("max_excess", row.max_excess)?;
            d.set_item("in_core", row.in_core)?;
            d.set_item("blocking_coalitions", row.blocking_coalitions)?;
            Ok(d)
        })
        .collect::<PyResult<Vec<_>>>()?;
    dict.set_item("rows", table)?;
    Ok(dict)
}

/// `game` を費用ゲーム `c(S)` として読み、仁による費用の分担を返す (節約ゲームの仁を費用に直す)。
/// Shapley 値は線形なので、費用ゲームにそのまま `shapley(game)` を使えば費用の分担になる。
#[pyfunction]
fn cost_nucleolus(game: &Game) -> PyResult<Vec<f64>> {
    CostGame::new(game.inner.clone())
        .nucleolus()
        .map_err(to_py_err)
}

/// 空港ゲーム (提携の費用 = 提携内の最大の費用) の費用分担。`shapley` は Littlechild & Owen (1973) の式、
/// `nucleolus` は凸ゲームの手法 (節約ゲームが凸)。人数の上限はない。
#[pyfunction]
fn airport<'py>(py: Python<'py>, costs: Vec<f64>) -> PyResult<Bound<'py, PyDict>> {
    let game = AirportGame::new(costs).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("shapley", game.shapley_costs())?;
    dict.set_item("nucleolus", game.nucleolus_costs().map_err(to_py_err)?)?;
    Ok(dict)
}

/// 最小全域木ゲーム (Bird 1976)。`costs` は供給元を頂点 0 とする `(n + 1) x (n + 1)` の対称な費用行列。
/// `bird` は Bird 規則 (コアに属する)、`nucleolus` は全提携の表の逐次 LP による仁 (30 人まで)。
#[pyfunction]
#[pyo3(signature = (costs, nucleolus = true))]
fn spanning_tree<'py>(
    py: Python<'py>,
    costs: Vec<Vec<f64>>,
    nucleolus: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let game = SpanningTreeGame::new(costs).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("bird", game.bird_rule())?;
    dict.set_item(
        "total_cost",
        game.cost(&PlayerSet::full(SetFunction::players(&game))),
    )?;
    if nucleolus {
        dict.set_item("nucleolus", game.nucleolus_costs().map_err(to_py_err)?)?;
    }
    Ok(dict)
}

/// 線形生産ゲーム (Owen 1975)。`technology` は資源 x 製品、`resources` は各人の資源。
/// `owen` は資源の影の価格で各人の資源を評価した配分 (コアに属する)。
#[pyfunction]
fn linear_production<'py>(
    py: Python<'py>,
    technology: Vec<Vec<f64>>,
    prices: Vec<f64>,
    resources: Vec<Vec<f64>>,
) -> PyResult<Bound<'py, PyDict>> {
    let game = LinearProductionGame::new(technology, prices, resources).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("shadow_prices", game.shadow_prices().map_err(to_py_err)?)?;
    dict.set_item("owen", game.owen_allocation().map_err(to_py_err)?)?;
    dict.set_item(
        "value",
        game.value(&PlayerSet::full(SetFunction::players(&game))),
    )?;
    Ok(dict)
}

fn solver_by_name(name: &str) -> PyResult<uncertainty_core::Solver> {
    uncertainty_core::solver(name).ok_or_else(|| {
        PyValueError::new_err(format!(
            "solver は {} のどれか: {name:?}",
            uncertainty_core::SOLVERS.join("・")
        ))
    })
}

/// 値が不確かなときの配分の分布。`relative` (例: 0.1 で各値 ±10%) か、`lower`・`upper` (Game) で区間を与え、
/// 区間内の一様分布から `samples` 個のゲームを引いて `solver` で解く。
/// 戻り値は `mean`・`std`・`quantiles` (`{0.05: [...], 0.5: [...], 0.95: [...]}`)・`samples`・`failures`。
#[pyfunction]
#[pyo3(signature = (game, relative = None, upper = None, samples = 200, seed = 0, solver = "nucleolus"))]
fn uncertainty<'py>(
    py: Python<'py>,
    game: &Game,
    relative: Option<f64>,
    upper: Option<PyRef<'py, Game>>,
    samples: usize,
    seed: u64,
    solver: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let interval = match (relative, upper) {
        (Some(relative), None) => uncertainty_core::IntervalGame::around(&game.inner, relative),
        (None, Some(upper)) => {
            uncertainty_core::IntervalGame::new(game.inner.clone(), upper.inner.clone())
        }
        _ => {
            return Err(PyValueError::new_err(
                "relative か upper (game を下限とする) のどちらか一方を指定する",
            ));
        }
    }
    .map_err(to_py_err)?;
    let d =
        uncertainty_core::interval_monte_carlo(&interval, samples, seed, solver_by_name(solver)?)
            .map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("mean", d.mean)?;
    dict.set_item("std", d.std)?;
    let quantiles = PyDict::new(py);
    for (q, values) in d.quantiles {
        quantiles.set_item(q, values)?;
    }
    dict.set_item("quantiles", quantiles)?;
    dict.set_item("samples", d.samples)?;
    dict.set_item("failures", d.failures)?;
    Ok(dict)
}

/// 提携の値を `±delta` 動かしたときの配分の変化率 `dx/dv(S)`。`coalitions` を省くと、
/// 配分で超過が大きい上位 `levels` 段の提携 (仁ではこれが配分を決める) と全体提携を調べる。
/// 戻り値は `(提携, 感度のリスト)` の列 (感度の大きい順)。
#[pyfunction]
#[pyo3(signature = (game, coalitions = None, solver = "nucleolus", levels = 2, delta = None))]
fn influence(
    game: &Game,
    coalitions: Option<Vec<Vec<usize>>>,
    solver: &str,
    levels: usize,
    delta: Option<f64>,
) -> PyResult<Vec<(Vec<usize>, Vec<f64>)>> {
    let n = game.inner.players();
    let mut solve = solver_by_name(solver)?;
    let coalitions: Vec<Coalition> = match coalitions {
        Some(list) => list
            .iter()
            .map(|members| coalition_of(n, members))
            .collect::<PyResult<_>>()?,
        None => {
            let x = solve(&game.inner).map_err(to_py_err)?;
            uncertainty_core::key_coalitions(&game.inner, &x, levels).map_err(to_py_err)?
        }
    };
    let delta = delta.unwrap_or(1e-4 * game.inner.max_abs_value().max(1.0));
    let mut rows =
        uncertainty_core::influence(&game.inner, &coalitions, delta, solve).map_err(to_py_err)?;
    rows.sort_by(|a, b| b.magnitude().total_cmp(&a.magnitude()));
    Ok(rows
        .into_iter()
        .map(|row| (row.coalition.players().collect(), row.sensitivity))
        .collect())
}

/// 有理数の値 (int・Fraction・"8/3"・"0.1") のゲームの仁 (プレ仁) を、有理数だけの逐次 LP で求める (10 人まで)。
/// 戻り値は `allocation`・`levels` (Fraction の列) と `lp_solves`。
#[pyfunction]
#[pyo3(signature = (values, domain = "imputation", order = "binary"))]
fn nucleolus_exact<'py>(
    py: Python<'py>,
    values: &Bound<'py, PyAny>,
    domain: &str,
    order: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let game = exact_game(values, order)?;
    let result =
        exact::nucleolus::nucleolus_exact(&game, parse_domain(domain)?).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("allocation", fractions(py, &result.allocation)?)?;
    dict.set_item("levels", fractions(py, &result.levels)?)?;
    dict.set_item("lp_solves", result.lp_solves)?;
    Ok(dict)
}

/// 反例であることを保ったまま、ゲームをより単純にする (プレイヤーの除去、値を 0・整数・半分にする)。
/// `predicate(game)` は「反例であるか」を返す関数。戻り値は `(縮小したゲーム, 受け入れた変形の数)`。
#[pyfunction]
#[pyo3(signature = (game, predicate, budget = 2000))]
fn shrink(
    py: Python<'_>,
    game: &Game,
    predicate: &Bound<'_, PyAny>,
    budget: usize,
) -> PyResult<(Game, usize)> {
    let error: RefCell<Option<PyErr>> = RefCell::new(None);
    let mut call = |candidate: &ExplicitGame| -> bool {
        if error.borrow().is_some() {
            return false;
        }
        let result = Py::new(
            py,
            Game {
                inner: candidate.clone(),
            },
        )
        .and_then(|object| predicate.call1((object,)))
        .and_then(|value| value.is_truthy());
        match result {
            Ok(value) => value,
            Err(err) => {
                *error.borrow_mut() = Some(err);
                false
            }
        }
    };
    let (shrunk, steps) = search_core::shrink(&game.inner, &mut call, budget);
    if let Some(err) = error.into_inner() {
        return Err(err);
    }
    Ok((Game { inner: shrunk }, steps))
}

fn parse_game_data(text: &str, format: &str) -> PyResult<io_core::GameData> {
    match format {
        "json" => io_core::parse_json(text),
        "csv" => io_core::parse_csv(text),
        _ => {
            return Err(PyValueError::new_err(format!(
                "format は \"json\" か \"csv\": {format:?}"
            )));
        }
    }
    .map_err(to_py_err)
}

/// プレイヤー名付きのゲーム (JSON か CSV のテキスト) を読む。
///
/// 戻り値は `game` (Game。値が分からない提携は `missing="zero"` なら 0、`"error"` なら ValueError)、
/// `names`、`kind` (`"value"` か `"cost"`)、`missing` (値が分からない提携の名前のリスト)。
#[pyfunction]
#[pyo3(signature = (text, format = "json", missing = "error"))]
fn parse_game<'py>(
    py: Python<'py>,
    text: &str,
    format: &str,
    missing: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let data = parse_game_data(text, format)?;
    let policy = match missing {
        "error" => io_core::Missing::Error,
        "zero" => io_core::Missing::Zero,
        _ => return Err(PyValueError::new_err("missing は \"error\" か \"zero\"")),
    };
    let game = data.to_explicit(policy).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("game", Py::new(py, Game { inner: game })?)?;
    dict.set_item("names", data.names.clone())?;
    dict.set_item("kind", data.kind.as_str())?;
    let missing: Vec<Vec<&str>> = data
        .missing()
        .iter()
        .map(|&c| data.member_names(c))
        .collect();
    dict.set_item("missing", missing)?;
    Ok(dict)
}

/// 値が分かる提携だけで解いた仁 (JSON か CSV のテキストから)。1 人提携とその補集合の値が全て必要。
/// 真のゲームの仁とは限らないので `guarantee` は `"approximate"`。
#[pyfunction]
#[pyo3(signature = (text, format = "json", domain = "imputation"))]
fn partial_nucleolus<'py>(
    py: Python<'py>,
    text: &str,
    format: &str,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let data = parse_game_data(text, format)?;
    let domain = parse_domain(domain)?;
    let result = data
        .partial()
        .and_then(|partial| partial.nucleolus(domain))
        .map_err(to_py_err)?;
    let dict = nucleolus_dict(py, result)?;
    dict.set_item("names", data.names)?;
    dict.set_item("known", data.known.len())?;
    Ok(dict)
}

fn coalition_structure(
    game: &Game,
    blocks: Vec<Vec<usize>>,
) -> PyResult<partition::CoalitionStructure> {
    partition::CoalitionStructure::new(game.inner.players(), &blocks).map_err(to_py_err)
}

/// Aumann–Drèze 値: 提携構造 `blocks` (プレイヤー番号のリストのリスト、N の分割) の各ブロック内の Shapley 値。
#[pyfunction]
fn aumann_dreze(game: &Game, blocks: Vec<Vec<usize>>) -> PyResult<Vec<f64>> {
    let structure = coalition_structure(game, blocks)?;
    partition::aumann_dreze(&game.inner, &structure).map_err(to_py_err)
}

/// Owen 値: 事前の連合 `unions` (N の分割) のもとでの値 (連合の順序と連合内の順序を一様に選ぶ)。
#[pyfunction]
fn owen(game: &Game, unions: Vec<Vec<usize>>) -> PyResult<Vec<f64>> {
    let structure = coalition_structure(game, unions)?;
    partition::owen(&game.inner, &structure).map_err(to_py_err)
}

/// 提携構造 `blocks` のもとでの仁 (各ブロックで x(B) = v(B))。
#[pyfunction]
#[pyo3(signature = (game, blocks, domain = "imputation"))]
fn structured_nucleolus<'py>(
    py: Python<'py>,
    game: &Game,
    blocks: Vec<Vec<usize>>,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let structure = coalition_structure(game, blocks)?;
    let result =
        partition::nucleolus(&game.inner, &structure, parse_domain(domain)?).map_err(to_py_err)?;
    nucleolus_dict(py, result)
}

/// 3-4 人のゲームの配分集合の図 (SVG)。コア・カーネル・仁・Shapley 値と `points` の点を描く。
///
/// 辞書 `svg` (文字列)、`core_vertices`、`kernel_pieces`、`points` (ラベル, 配分) の組を返す。
#[pyfunction]
#[pyo3(signature = (game, names = None, kernel = true, shapley = true, points = None, title = None))]
fn plot_svg<'py>(
    py: Python<'py>,
    game: &Game,
    names: Option<Vec<String>>,
    kernel: bool,
    shapley: bool,
    points: Option<Vec<(String, Vec<f64>)>>,
    title: Option<String>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut options = plot_core::PlotOptions::default();
    options.names = names;
    options.kernel = kernel;
    options.shapley = shapley;
    options.points = points.unwrap_or_default();
    options.title = title;
    let figure = plot_core::imputation_figure(&game.inner, &options).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("svg", figure.svg)?;
    dict.set_item("core_vertices", figure.core_vertices)?;
    dict.set_item("kernel_pieces", figure.kernel_pieces)?;
    dict.set_item("points", figure.points)?;
    Ok(dict)
}

/// 厳密な Shapley 値。
#[pyfunction]
fn shapley(game: &Game) -> Vec<f64> {
    values::shapley(&game.inner)
}

/// 厳密な Banzhaf 値 (正規化しない)。正規化指数は `normalize(banzhaf(game))`。
#[pyfunction]
fn banzhaf(game: &Game) -> Vec<f64> {
    values::banzhaf(&game.inner)
}

/// solidarity 値 (Nowak & Radzik 1994)。
#[pyfunction]
fn solidarity(game: &Game) -> Vec<f64> {
    values::solidarity(&game.inner)
}

/// tau 値 (Tijs 1981)。準平衡でないゲームでは ValueError。
#[pyfunction]
fn tau_value(game: &Game) -> PyResult<Vec<f64>> {
    compromise::tau_value(&game.inner).map_err(to_py_err)
}

/// Gately 点 (Gately 1974)。
#[pyfunction]
fn gately_point(game: &Game) -> PyResult<Vec<f64>> {
    compromise::gately_point(&game.inner).map_err(to_py_err)
}

/// 理想の支払い `M_i = v(N) - v(N \ {i})` と最小の権利 `m_i` を `{"utopia", "minimal_rights"}` で返す。
#[pyfunction]
fn utopia_payoffs<'py>(py: Python<'py>, game: &Game) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("utopia", compromise::utopia_payoffs(&game.inner))?;
    dict.set_item("minimal_rights", compromise::minimal_rights(&game.inner))?;
    Ok(dict)
}

/// Myerson 値: 通信グラフ `edges` (プレイヤー番号の組の列) で制限したゲームの Shapley 値 (Myerson 1977)。
#[pyfunction]
fn myerson(game: &Game, edges: Vec<(usize, usize)>) -> PyResult<Vec<f64>> {
    communication::myerson(&game.inner, &edges).map_err(to_py_err)
}

/// グラフ制限ゲーム `v^g(S) = sum_{C in S/g} v(C)`。
#[pyfunction]
fn graph_restricted(game: &Game, edges: Vec<(usize, usize)>) -> PyResult<Game> {
    let inner = communication::graph_restricted(&game.inner, &edges).map_err(to_py_err)?;
    Ok(Game { inner })
}

/// 単純ゲーム (値が 0 か 1) の投票力指数をまとめて返す。
/// 値は `johnston`・`deegan_packel`・`public_good` (Holler)・`coleman_prevent`・`coleman_initiative`
/// (定義されない場合は None)、`coleman_collectivity`、`swings` (決定票の数)、
/// `minimal_winning` (最小勝利提携、プレイヤー番号のリスト)。
#[pyfunction]
fn power_indices<'py>(py: Python<'py>, game: &Game) -> PyResult<Bound<'py, PyDict>> {
    let simple = power::SimpleGame::new(&game.inner).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("johnston", simple.johnston().ok())?;
    dict.set_item("deegan_packel", simple.deegan_packel().ok())?;
    dict.set_item("public_good", simple.public_good().ok())?;
    dict.set_item("coleman_prevent", simple.coleman_prevent().ok())?;
    dict.set_item("coleman_initiative", simple.coleman_initiative().ok())?;
    dict.set_item("coleman_collectivity", simple.coleman_collectivity())?;
    dict.set_item("swings", simple.swings.clone())?;
    let minimal: Vec<Vec<usize>> = simple
        .minimal_winning
        .iter()
        .map(|c| c.players().collect())
        .collect();
    dict.set_item("minimal_winning", minimal)?;
    Ok(dict)
}

/// disruption nucleolus (Littlechild & Vaidya 1976)。コアが空なら ValueError。
#[pyfunction]
fn disruption_nucleolus(game: &Game) -> PyResult<Vec<f64>> {
    Ok(variants::disruption_nucleolus(&game.inner)
        .map_err(to_py_err)?
        .allocation)
}

/// anti-prenucleolus (双対ゲームのプレ仁、Funaki & Meinhardt 2006)。
#[pyfunction]
fn anti_prenucleolus(game: &Game) -> PyResult<Vec<f64>> {
    Ok(variants::anti_prenucleolus(&game.inner)
        .map_err(to_py_err)?
        .allocation)
}

/// anti-nucleolus (双対ゲームの仁)。anti-imputation の集合が空なら ValueError。
#[pyfunction]
fn anti_nucleolus(game: &Game) -> PyResult<Vec<f64>> {
    Ok(variants::anti_nucleolus(&game.inner)
        .map_err(to_py_err)?
        .allocation)
}

/// 双対ゲーム `v*(S) = v(N) - v(N \ S)`。
#[pyfunction]
fn dual(game: &Game) -> Game {
    Game {
        inner: game.inner.dual(),
    }
}

/// 和が 1 になるように割る。
#[pyfunction]
fn normalize(values: Vec<f64>) -> Vec<f64> {
    values::normalize(&values)
}

fn estimate_dict<'py>(py: Python<'py>, estimate: values::Estimate) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("values", estimate.values)?;
    dict.set_item("standard_errors", estimate.standard_errors)?;
    dict.set_item("evaluations", estimate.evaluations)?;
    dict.set_item("guarantee", estimate.guarantee.to_string())?;
    Ok(dict)
}

/// ランダムな順列 `permutations` 個で Shapley 値を推定する (Castro et al. 2009)。
#[pyfunction]
#[pyo3(signature = (game, permutations, seed = 0))]
fn shapley_sampling<'py>(
    py: Python<'py>,
    game: &Bound<'py, PyAny>,
    permutations: usize,
    seed: u64,
) -> PyResult<Bound<'py, PyDict>> {
    let estimate = with_set_function(game, |g| {
        Ok(values::shapley_sampling(g, permutations, seed))
    })?;
    estimate_dict(py, estimate)
}

/// ランダムな提携 `samples` 個での限界貢献の平均で Banzhaf 値を推定する。
#[pyfunction]
#[pyo3(signature = (game, samples, seed = 0))]
fn banzhaf_sampling<'py>(
    py: Python<'py>,
    game: &Bound<'py, PyAny>,
    samples: usize,
    seed: u64,
) -> PyResult<Bound<'py, PyDict>> {
    let estimate = with_set_function(game, |g| Ok(values::banzhaf_sampling(g, samples, seed)))?;
    estimate_dict(py, estimate)
}

/// 提携を `pairs` 組 (補集合と組で) サンプルし、サンプルした提携だけで仁 (既定はプレ仁) を求める。
#[pyfunction]
#[pyo3(signature = (game, pairs, seed = 0, domain = "preimputation"))]
fn sampled_nucleolus<'py>(
    py: Python<'py>,
    game: &Bound<'py, PyAny>,
    pairs: usize,
    seed: u64,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let domain = parse_domain(domain)?;
    let result = with_set_function(game, |g| {
        sampled::sampled_nucleolus(g, pairs, seed, domain).map_err(to_py_err)
    })?;
    let dict = PyDict::new(py);
    dict.set_item("allocation", result.allocation)?;
    dict.set_item("epsilon", result.epsilon)?;
    dict.set_item("levels", result.levels)?;
    dict.set_item("evaluations", result.evaluations)?;
    dict.set_item("guarantee", result.guarantee.to_string())?;
    Ok(dict)
}

/// 提携を `pairs` 組サンプルし、サンプルした提携だけで最小コアの配分を 1 つ求める (Yan & Procaccia 2021)。
#[pyfunction]
#[pyo3(signature = (game, pairs, seed = 0, domain = "preimputation"))]
fn sampled_least_core<'py>(
    py: Python<'py>,
    game: &Bound<'py, PyAny>,
    pairs: usize,
    seed: u64,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let domain = parse_domain(domain)?;
    let (least, evaluations) = with_set_function(game, |g| {
        sampled::sampled_least_core(g, pairs, seed, domain).map_err(to_py_err)
    })?;
    let dict = PyDict::new(py);
    dict.set_item("allocation", least.allocation)?;
    dict.set_item("epsilon", least.epsilon)?;
    dict.set_item("evaluations", evaluations)?;
    dict.set_item("guarantee", least.guarantee.to_string())?;
    Ok(dict)
}

fn lexicographic_dict<'py>(
    py: Python<'py>,
    result: variants::LexicographicResult,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("allocation", result.allocation)?;
    dict.set_item("levels", result.levels)?;
    dict.set_item("lp_solves", result.lp_solves)?;
    dict.set_item("guarantee", result.guarantee.to_string())?;
    Ok(dict)
}

/// per capita 仁: `e(S, x) / |S|` を辞書式に最小化する。
#[pyfunction]
#[pyo3(signature = (game, domain = "imputation"))]
fn per_capita_nucleolus<'py>(
    py: Python<'py>,
    game: &Game,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let result =
        variants::per_capita_nucleolus(&game.inner, parse_domain(domain)?).map_err(to_py_err)?;
    lexicographic_dict(py, result)
}

/// 比例仁: `e(S, x) / v(S)` を辞書式に最小化する (非負のゲーム)。
#[pyfunction]
#[pyo3(signature = (game, domain = "imputation"))]
fn proportional_nucleolus<'py>(
    py: Python<'py>,
    game: &Game,
    domain: &str,
) -> PyResult<Bound<'py, PyDict>> {
    let result =
        variants::proportional_nucleolus(&game.inner, parse_domain(domain)?).map_err(to_py_err)?;
    lexicographic_dict(py, result)
}

/// modiclus: 超過の差 `e(S, x) - e(T, x)` を辞書式に最小化する準配分 (7 人まで)。
#[pyfunction]
fn modiclus<'py>(py: Python<'py>, game: &Game) -> PyResult<Bound<'py, PyDict>> {
    let result = variants::modiclus(&game.inner).map_err(to_py_err)?;
    lexicographic_dict(py, result)
}

/// `x` が交渉集合 (`domain="imputation"`) かプレ交渉集合に属するか。
///
/// `objection` は反論のない異議 (`objector`, `target`, `coalition`, `payoff`, `margin`) か None。
#[pyfunction]
#[pyo3(signature = (game, x, domain = "imputation", tolerance = None))]
fn bargaining_set<'py>(
    py: Python<'py>,
    game: &Game,
    x: Vec<f64>,
    domain: &str,
    tolerance: Option<f64>,
) -> PyResult<Bound<'py, PyDict>> {
    let tolerance = tolerance.unwrap_or_else(|| 10.0 * coopgame::default_tolerance(&game.inner));
    let report =
        bargaining::check(&game.inner, &x, parse_domain(domain)?, tolerance).map_err(to_py_err)?;
    let dict = PyDict::new(py);
    dict.set_item("member", report.is_member())?;
    dict.set_item("lp_solves", report.lp_solves)?;
    match report.objection {
        None => dict.set_item("objection", py.None())?,
        Some(objection) => {
            let inner = PyDict::new(py);
            inner.set_item("objector", objection.objector)?;
            inner.set_item("target", objection.target)?;
            inner.set_item(
                "coalition",
                objection.coalition.players().collect::<Vec<_>>(),
            )?;
            inner.set_item("payoff", objection.payoff)?;
            inner.set_item("margin", objection.margin)?;
            dict.set_item("objection", inner)?;
        }
    }
    Ok(dict)
}

/// 誘導部分グラフゲーム: 提携の値は提携内で完結する辺の重みの和。重みは非負 (構造的に凸)。
///
/// `edges` は `(u, v, 重み)` の列。人数の上限はない。
#[pyclass(module = "coopgame", frozen, name = "InducedSubgraphGame")]
struct GraphGame {
    inner: InducedSubgraphGame,
}

#[pymethods]
impl GraphGame {
    #[new]
    fn new(players: usize, edges: Vec<(usize, usize, f64)>) -> PyResult<GraphGame> {
        let inner = InducedSubgraphGame::new(players, edges).map_err(to_py_err)?;
        Ok(GraphGame { inner })
    }

    #[getter]
    fn players(&self) -> usize {
        SetFunction::players(&self.inner)
    }

    fn value(&self, coalition: Vec<usize>) -> PyResult<f64> {
        let n = SetFunction::players(&self.inner);
        if let Some(&i) = coalition.iter().find(|&&i| i >= n) {
            return Err(PyValueError::new_err(format!(
                "プレイヤー番号 {i} が人数 {n} 以上"
            )));
        }
        Ok(self.inner.value(&PlayerSet::from_players(n, &coalition)))
    }

    fn __repr__(&self) -> String {
        format!(
            "InducedSubgraphGame(players={}, edges={})",
            SetFunction::players(&self.inner),
            self.inner.edges().len()
        )
    }
}

fn solution_dict<'py>(
    py: Python<'py>,
    solution: &Solution,
    stats: Option<ConvexStats>,
) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("allocation", solution.allocation.clone())?;
    dict.set_item("guarantee", solution.guarantee.to_string())?;
    dict.set_item("method", solution.method)?;
    if let Some(stats) = stats {
        dict.set_item("sweeps", stats.sweeps)?;
        dict.set_item("transfers", stats.transfers)?;
        dict.set_item("evaluations", stats.evaluations)?;
    }
    Ok(dict)
}

/// 凸ゲームの手法 (劣モジュラ最小化 + transfer scheme) で仁を求める。
///
/// - `assume=False`: 凸であることが分かっている場合だけ実行する。`InducedSubgraphGame` は構造的に凸、
///   `Game` は全提携で凸性を確かめる (凸でなければ ValueError)。`guarantee` は `"proven(convex)"`。
/// - `assume=True`: 凸と仮定して実行する (FunctionGame も可)。`guarantee` は `"assumed(convex)"`。
///   `verify=True` なら事後検証し、`verdict` に `"certified"`・`"refuted"`・`"undecided"`、`reason` に理由を入れる。
///   `certified` なら配分は厳密な配分になり、`guarantee` は `"certified"` になる。
#[pyfunction]
#[pyo3(signature = (game, assume = false, verify = true))]
fn convex_nucleolus<'py>(
    py: Python<'py>,
    game: &Bound<'py, PyAny>,
    assume: bool,
    verify: bool,
) -> PyResult<Bound<'py, PyDict>> {
    if !assume {
        let (solution, stats) = if let Ok(graph) = game.cast::<GraphGame>() {
            convex::nucleolus_with(&graph.get().inner, ConvexOptions::default())
        } else if let Ok(explicit) = game.cast::<Game>() {
            let checked = ConvexChecked::new(explicit.get().inner.clone()).map_err(|_| {
                PyValueError::new_err("ゲームが凸でない (凸と仮定して試すには assume=True)")
            })?;
            convex::nucleolus_with(&checked, ConvexOptions::default())
        } else {
            return Err(PyValueError::new_err(
                "凸であることが分からないゲーム (FunctionGame など) は assume=True で実行する",
            ));
        }
        .map_err(to_py_err)?;
        return solution_dict(py, &solution, Some(stats));
    }
    with_set_function(game, |g| {
        let assumed = Assume::convex(g);
        let (unverified, stats) =
            convex::nucleolus_with(&assumed, ConvexOptions::default()).map_err(to_py_err)?;
        if !verify {
            let dict = solution_dict(py, unverified.peek(), Some(stats))?;
            dict.set_item("verdict", py.None())?;
            return Ok(dict);
        }
        let verdict = unverified
            .verify(&g, VerifyOptions::default())
            .map_err(to_py_err)?;
        let (dict, label, reason) = match verdict {
            Verified::Certified(solution) => (
                solution_dict(py, &solution, Some(stats))?,
                "certified",
                None,
            ),
            Verified::Refuted { solution, reason } => (
                solution_dict(py, &solution, Some(stats))?,
                "refuted",
                Some(reason),
            ),
            Verified::Undecided { solution, reason } => (
                solution_dict(py, solution.peek(), Some(stats))?,
                "undecided",
                Some(reason),
            ),
        };
        dict.set_item("verdict", label)?;
        dict.set_item("reason", reason)?;
        Ok(dict)
    })
}

/// 保証のある手法のうち最も速いものを自動で選んで仁を求める。
///
/// `Game` は逐次 LP (`"exact"`)、`InducedSubgraphGame` は凸ゲームの手法 (`"proven(convex)"`)。
/// 保証のある手法がない `FunctionGame` は ValueError (仮定付きで試すなら `convex_nucleolus(..., assume=True)`)。
#[pyfunction]
fn auto_nucleolus<'py>(py: Python<'py>, game: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let solution = if let Ok(graph) = game.cast::<GraphGame>() {
        graph.get().inner.nucleolus_auto()
    } else if let Ok(explicit) = game.cast::<Game>() {
        explicit.get().inner.nucleolus_auto()
    } else {
        return Err(PyValueError::new_err(
            "保証のある手法を選べないゲーム (FunctionGame は性質が分からない)",
        ));
    }
    .map_err(to_py_err)?;
    solution_dict(py, &solution, None)
}

/// 配分 `allocation` が仁 (`concept="nucleolus"`) かプレ仁かを、ゲームの値だけで検証する。
///
/// 20 人以下は厳密な検証、26 人以下はランダムな `pairs` 組のカーネル条件の総当たり (否定だけができる)。
/// 戻り値の `check` は `"certified"`・`"refuted"`・`"undecided"`。`certified` なら `exact` に Fraction の配分。
#[pyfunction]
#[pyo3(signature = (game, allocation, concept = "nucleolus", pairs = 8, seed = 0))]
fn verify_solution<'py>(
    py: Python<'py>,
    game: &Bound<'py, PyAny>,
    allocation: Vec<f64>,
    concept: &str,
    pairs: usize,
    seed: u64,
) -> PyResult<Bound<'py, PyDict>> {
    let concept = match concept {
        "nucleolus" => Concept::Nucleolus,
        "prenucleolus" => Concept::Prenucleolus,
        _ => {
            return Err(PyValueError::new_err(format!(
                "concept は \"nucleolus\" か \"prenucleolus\": {concept:?}"
            )));
        }
    };
    let candidate = Solution {
        concept,
        allocation,
        guarantee: Guarantee::Approximate,
        method: "user",
    };
    let check = with_set_function(game, |g| {
        let mut options = VerifyOptions::default();
        options.pairs = pairs;
        options.seed = seed;
        verify_core::check(g, &candidate, options).map_err(to_py_err)
    })?;
    let dict = PyDict::new(py);
    match check {
        Check::Certified { exact } => {
            dict.set_item("check", "certified")?;
            dict.set_item("exact", fractions(py, &exact)?)?;
            dict.set_item("reason", py.None())?;
        }
        Check::Refuted(reason) => {
            dict.set_item("check", "refuted")?;
            dict.set_item("exact", py.None())?;
            dict.set_item("reason", reason)?;
        }
        Check::Undecided(reason) => {
            dict.set_item("check", "undecided")?;
            dict.set_item("exact", py.None())?;
            dict.set_item("reason", reason)?;
        }
    }
    Ok(dict)
}

type Rule = fn(Rational, &[Rational]) -> coopgame::Result<Vec<Rational>>;
type FloatRule = fn(f64, &[f64]) -> coopgame::Result<Vec<f64>>;

fn apply_rule<'py>(
    py: Python<'py>,
    estate: &Bound<'py, PyAny>,
    claims: &Bound<'py, PyAny>,
    exact_rule: Rule,
    float_rule: FloatRule,
    exact: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if exact {
        let claims: Vec<Rational> = claims
            .try_iter()?
            .map(|claim| rational_of(&claim?))
            .collect::<PyResult<_>>()?;
        let shares = exact_rule(rational_of(estate)?, &claims).map_err(to_py_err)?;
        Ok(fractions(py, &shares)?.into_pyobject(py)?.into_any())
    } else {
        let claims: Vec<f64> = claims.extract()?;
        let shares = float_rule(estate.extract()?, &claims).map_err(to_py_err)?;
        Ok(shares.into_pyobject(py)?.into_any())
    }
}

/// タルムード則 (破産ゲームの仁、Aumann & Maschler 1985)。`exact=True` なら `Fraction` で厳密に計算する。
#[pyfunction]
#[pyo3(signature = (estate, claims, exact = false))]
fn talmud<'py>(
    py: Python<'py>,
    estate: &Bound<'py, PyAny>,
    claims: &Bound<'py, PyAny>,
    exact: bool,
) -> PyResult<Bound<'py, PyAny>> {
    apply_rule(
        py,
        estate,
        claims,
        bankruptcy::talmud_rule::<Rational>,
        bankruptcy::talmud_rule::<f64>,
        exact,
    )
}

/// 制約付き均等配分 (CEA)。
#[pyfunction]
#[pyo3(signature = (estate, claims, exact = false))]
fn constrained_equal_awards<'py>(
    py: Python<'py>,
    estate: &Bound<'py, PyAny>,
    claims: &Bound<'py, PyAny>,
    exact: bool,
) -> PyResult<Bound<'py, PyAny>> {
    apply_rule(
        py,
        estate,
        claims,
        bankruptcy::constrained_equal_awards::<Rational>,
        bankruptcy::constrained_equal_awards::<f64>,
        exact,
    )
}

/// 制約付き均等損失 (CEL)。
#[pyfunction]
#[pyo3(signature = (estate, claims, exact = false))]
fn constrained_equal_losses<'py>(
    py: Python<'py>,
    estate: &Bound<'py, PyAny>,
    claims: &Bound<'py, PyAny>,
    exact: bool,
) -> PyResult<Bound<'py, PyAny>> {
    apply_rule(
        py,
        estate,
        claims,
        bankruptcy::constrained_equal_losses::<Rational>,
        bankruptcy::constrained_equal_losses::<f64>,
        exact,
    )
}

#[pymodule]
#[pyo3(name = "coopgame")]
fn coopgame_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Game>()?;
    m.add_class::<FunctionGame>()?;
    m.add_class::<GraphGame>()?;
    m.add_function(wrap_pyfunction!(nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(prenucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(least_core, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_point, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_violation, m)?)?;
    m.add_function(wrap_pyfunction!(is_in_kernel, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_set, m)?)?;
    m.add_function(wrap_pyfunction!(verify, m)?)?;
    m.add_function(wrap_pyfunction!(certify, m)?)?;
    m.add_function(wrap_pyfunction!(certify_rational, m)?)?;
    m.add_function(wrap_pyfunction!(shapley, m)?)?;
    m.add_function(wrap_pyfunction!(banzhaf, m)?)?;
    m.add_function(wrap_pyfunction!(normalize, m)?)?;
    m.add_function(wrap_pyfunction!(solidarity, m)?)?;
    m.add_function(wrap_pyfunction!(tau_value, m)?)?;
    m.add_function(wrap_pyfunction!(gately_point, m)?)?;
    m.add_function(wrap_pyfunction!(utopia_payoffs, m)?)?;
    m.add_function(wrap_pyfunction!(myerson, m)?)?;
    m.add_function(wrap_pyfunction!(graph_restricted, m)?)?;
    m.add_function(wrap_pyfunction!(power_indices, m)?)?;
    m.add_function(wrap_pyfunction!(disruption_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(anti_prenucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(anti_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(dual, m)?)?;
    m.add_function(wrap_pyfunction!(shapley_sampling, m)?)?;
    m.add_function(wrap_pyfunction!(banzhaf_sampling, m)?)?;
    m.add_function(wrap_pyfunction!(sampled_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(sampled_least_core, m)?)?;
    m.add_function(wrap_pyfunction!(per_capita_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(proportional_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(modiclus, m)?)?;
    m.add_function(wrap_pyfunction!(bargaining_set, m)?)?;
    m.add_function(wrap_pyfunction!(convex_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(auto_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(verify_solution, m)?)?;
    m.add_function(wrap_pyfunction!(explain, m)?)?;
    m.add_function(wrap_pyfunction!(compare, m)?)?;
    m.add_function(wrap_pyfunction!(cost_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(airport, m)?)?;
    m.add_function(wrap_pyfunction!(spanning_tree, m)?)?;
    m.add_function(wrap_pyfunction!(linear_production, m)?)?;
    m.add_function(wrap_pyfunction!(uncertainty, m)?)?;
    m.add_function(wrap_pyfunction!(influence, m)?)?;
    m.add_function(wrap_pyfunction!(nucleolus_exact, m)?)?;
    m.add_function(wrap_pyfunction!(shrink, m)?)?;
    m.add_function(wrap_pyfunction!(parse_game, m)?)?;
    m.add_function(wrap_pyfunction!(partial_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(aumann_dreze, m)?)?;
    m.add_function(wrap_pyfunction!(owen, m)?)?;
    m.add_function(wrap_pyfunction!(structured_nucleolus, m)?)?;
    m.add_function(wrap_pyfunction!(plot_svg, m)?)?;
    m.add_function(wrap_pyfunction!(talmud, m)?)?;
    m.add_function(wrap_pyfunction!(constrained_equal_awards, m)?)?;
    m.add_function(wrap_pyfunction!(constrained_equal_losses, m)?)?;
    Ok(())
}
