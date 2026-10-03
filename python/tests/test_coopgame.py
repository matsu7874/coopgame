"""Python バインディングのテスト。

Rust 側のテスト (tests/literature.rs など) で確かめた文献の数値例を、バインディング経由で再現する。
数値例の出典と手計算は docs/literature-cases.md を参照。
"""
import itertools
import math
from fractions import Fraction

import pytest

import coopgame


def close(actual, expected, tolerance=1e-7):
    return len(actual) == len(expected) and all(abs(a - e) <= tolerance for a, e in zip(actual, expected))


# ---------------------------------------------------------------- 仁・プレ仁


@pytest.mark.parametrize(
    "values, domain, expected",
    [
        # Peleg & Sudhölter (2007) Example 5.5.12
        ([0, 0, 0, 10, 0, 0, 2], "preimputation", [3, 3, -4]),
        ([0, 0, 0, 10, 0, 0, 2], "imputation", [1, 1, 0]),
        # Ferguson の講義ノート (CoopGame のテスト 49.1 が引用)
        ([-1, 0, 1, 3, 4, 2, 5], "imputation", [8 / 3, 2 / 3, 5 / 3]),
        # CoopGame のテスト 67.1
        ([2, 6, 5, 15, 1, 18, 14], "imputation", [2, 7, 5]),
        ([2, 6, 5, 15, 1, 18, 14], "preimputation", [-1, 13, 2]),
        # CoopGame のヘルプの 4 人ゲーム
        ([0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21], "imputation", [3.5, 4.5, 5.5, 7.5]),
    ],
)
def test_literature_nucleolus(values, domain, expected):
    game = coopgame.Game(values, order="lex")
    for method in ("cg", "full"):
        result = coopgame.nucleolus(game, domain, method)
        assert close(result["allocation"], expected), (method, result)
    assert coopgame.verify(game, expected, domain)["satisfied"]
    assert coopgame.is_in_kernel(game, expected, domain)
    assert any(piece_contains(piece, expected) for piece in coopgame.kernel_set(game, domain)["pieces"])


def piece_contains(piece, x, tolerance=1e-7):
    def lhs(coefficients):
        return sum(a * b for a, b in zip(coefficients, x))

    return all(abs(lhs(c) - rhs) <= tolerance for c, rhs in piece["equalities"]) and all(
        lhs(c) <= rhs + tolerance for c, rhs in piece["inequalities"]
    )


def test_prenucleolus_and_certify_with_fractions():
    game = coopgame.Game([-1, 0, 1, 3, 4, 2, 5], order="lex")
    x = coopgame.prenucleolus(game)["allocation"]
    report = coopgame.certify(game, x, "preimputation")
    assert report["satisfied"]
    assert report["allocation"] == [Fraction(8, 3), Fraction(2, 3), Fraction(5, 3)]
    # ずらした配分は不合格
    assert not coopgame.certify(game, [x[0] + 0.1, x[1] - 0.1, x[2]], "preimputation")["satisfied"]


def test_talmud_cases_match_nucleolus():
    claims = [100, 200, 300]
    for estate, expected in [
        (100, [Fraction(100, 3)] * 3),
        (200, [50, 75, 75]),
        (300, [50, 100, 150]),
    ]:
        assert coopgame.talmud(estate, claims, exact=True) == expected
        game = coopgame.Game.bankruptcy(estate, claims)
        assert close(coopgame.nucleolus(game)["allocation"], [float(e) for e in expected])


def test_bankruptcy_rules_accept_fractions_and_strings():
    claims = [Fraction(1, 3), "2/3", 1]
    shares = coopgame.constrained_equal_awards(Fraction(1, 2), claims, exact=True)
    assert shares == [Fraction(1, 6)] * 3
    shares = coopgame.constrained_equal_losses(1, claims, exact=True)
    assert sum(shares) == 1 and shares[0] == 0
    with pytest.raises(ValueError):
        coopgame.talmud(10, [1, 2])


def test_least_core_and_properties():
    # 3 人の多数決ゲーム: コアは空、最小コアは epsilon = 1/3 で等分
    game = coopgame.Game.weighted_voting([1, 1, 1], 2)
    least = coopgame.least_core(game)
    assert math.isclose(least["epsilon"], 1 / 3, abs_tol=1e-9)
    assert close(least["allocation"], [1 / 3] * 3)
    assert not game.has_nonempty_core()
    assert game.is_superadditive() and not game.is_convex()


def test_kernel_point_converges():
    game = coopgame.Game.bnf(1, 6, 0)
    result = coopgame.kernel_point(game)
    assert result["converged"]
    assert coopgame.kernel_violation(game, result["allocation"]) <= 1e-6


# ---------------------------------------------------------------- Shapley 値・Banzhaf 値


def test_ibn_ezra_shapley_value():
    game = coopgame.Game([120, 60, 40, 30, 120, 120, 120, 60, 60, 40, 120, 120, 120, 60, 120], order="lex")
    assert close(coopgame.shapley(game), [80 + 5 / 6, 20 + 5 / 6, 10 + 5 / 6, 7.5])


def test_gambarelli_banzhaf_value():
    game = coopgame.Game([0, 0, 0, 1, 2, 1, 3], order="lex")
    assert close(coopgame.banzhaf(game), [1.25, 0.75, 1.25])


@pytest.mark.parametrize(
    "quota, expected",
    [(1, [1 / 3] * 3), (51, [0.6, 0.2, 0.2]), (52, [0.5, 0.5, 0.0]), (100, [1 / 3] * 3)],
)
def test_normalized_banzhaf_index(quota, expected):
    game = coopgame.Game.weighted_voting([50, 49, 1], quota)
    assert close(coopgame.normalize(coopgame.banzhaf(game)), expected)


def test_from_function_matches_explicit_values():
    def glove(coalition):
        lefts = sum(1 for i in coalition if i == 0)
        return float(min(lefts, len(coalition) - lefts))

    game = coopgame.Game.from_function(3, glove)
    assert game.players == 3
    assert game.value([0, 1]) == 1 and game.value([1, 2]) == 0 and game.value([]) == 0
    assert close(coopgame.shapley(game), [2 / 3, 1 / 6, 1 / 6])


def test_sampling_on_python_function_game():
    n = 101
    game = coopgame.FunctionGame(n, lambda coalition: float(len(coalition) >= 51))
    shapley = coopgame.shapley_sampling(game, 2000, seed=3)
    assert math.isclose(sum(shapley["values"]), 1.0, abs_tol=1e-9)
    assert all(abs(v - 1 / n) <= 5 * se + 1e-12 for v, se in zip(shapley["values"], shapley["standard_errors"]))
    banzhaf = coopgame.banzhaf_sampling(game, 2000, seed=4)
    exact = math.comb(100, 50) / 2**100
    assert all(abs(v - exact) <= 5 * se for v, se in zip(banzhaf["values"], banzhaf["standard_errors"]))


def test_sampling_explicit_and_function_games_agree():
    explicit = coopgame.Game.bnf(2, 8, 1)
    function = coopgame.FunctionGame(8, explicit.value)
    for run in (coopgame.shapley_sampling, coopgame.banzhaf_sampling):
        assert run(explicit, 50, seed=7) == run(function, 50, seed=7)


def test_callback_exception_is_raised():
    def broken(coalition):
        raise KeyError("broken")

    with pytest.raises(KeyError):
        coopgame.shapley_sampling(coopgame.FunctionGame(4, broken), 10)
    with pytest.raises(KeyError):
        coopgame.sampled_nucleolus(coopgame.FunctionGame(4, broken), 10)


# ---------------------------------------------------------------- サンプリングした仁


def test_sampled_nucleolus_with_all_coalitions_is_exact():
    game = coopgame.Game.bnf(4, 7, 11)
    exact = coopgame.prenucleolus(game)["allocation"]
    result = coopgame.sampled_nucleolus(coopgame.FunctionGame(7, game.value), 2000, seed=5)
    assert result["evaluations"] == 2**7 - 1
    assert close(result["allocation"], exact, 1e-6 * max(map(abs, game.values())))


def test_sampled_least_core_is_efficient():
    game = coopgame.FunctionGame(30, lambda coalition: math.sqrt(len(coalition)))
    result = coopgame.sampled_least_core(game, 200, seed=1)
    assert math.isclose(sum(result["allocation"]), math.sqrt(30), rel_tol=1e-9)


# ---------------------------------------------------------------- 入力の検査


def test_invalid_inputs_raise_value_error():
    with pytest.raises(ValueError):
        coopgame.Game([1, 2])  # 2^n - 1 の形ではない
    with pytest.raises(ValueError):
        coopgame.Game([0, 0, 1], order="colex")
    game = coopgame.Game([0, 0, 1])
    with pytest.raises(ValueError):
        coopgame.nucleolus(game, domain="core")
    with pytest.raises(ValueError):
        game.value([2])
    with pytest.raises(ValueError):
        coopgame.verify(game, [0.5])
    with pytest.raises(ValueError):
        coopgame.shapley_sampling("not a game", 10)


def test_binary_and_lex_orders_agree():
    lex = [1, 2, 3, 10, 11, 12, 20]
    game = coopgame.Game(lex, order="lex")
    # 辞書式順: {1},{2},{3},{1,2},{1,3},{2,3},{1,2,3}
    coalitions = [c for k in (1, 2, 3) for c in itertools.combinations(range(3), k)]
    for coalition, value in zip(coalitions, lex):
        assert game.value(list(coalition)) == value
    assert coopgame.Game(game.values()).values() == game.values()


# ---------------------------------------------------------------- 仁の変種・交渉集合


def test_variants_documented_examples():
    game = coopgame.Game([0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21], order="lex")
    assert close(coopgame.modiclus(game)["allocation"], [4.25, 5.25, 5.75, 5.75])
    young = coopgame.Game([0, 0, 0, 0, 9, 10, 12], order="lex")
    assert close(coopgame.per_capita_nucleolus(young)["allocation"], [2 / 3, 7 / 6, 61 / 6])
    with pytest.raises(ValueError):
        coopgame.proportional_nucleolus(coopgame.Game([-1, 0, 1, 3, 4, 2, 5], order="lex"))


def test_bargaining_set_of_majority_game():
    game = coopgame.Game.weighted_voting([1, 1, 1], 2)
    assert coopgame.bargaining_set(game, [1 / 3] * 3)["member"]
    report = coopgame.bargaining_set(game, [0.5, 0.5, 0.0])
    assert not report["member"]
    assert (report["objection"]["objector"], report["objection"]["target"]) == (2, 0)
    assert report["objection"]["coalition"] == [1, 2]


def test_kernel_set_merge_keeps_points():
    game = coopgame.Game.bnf(1, 5, 8)
    raw = coopgame.kernel_set(game, "preimputation")
    merged = coopgame.kernel_set(game, "preimputation", merge=True)
    assert len(merged["pieces"]) <= len(raw["pieces"])
    for piece in raw["pieces"]:
        for vertex in piece["vertices"]:
            assert any(piece_contains(other, vertex, 1e-6) for other in merged["pieces"])


# ---------------------------------------------------------------- 保証の種類・凸ゲームの手法・検証


def test_results_report_guarantee():
    game = coopgame.Game.bnf(1, 6, 0)
    assert coopgame.nucleolus(game)["guarantee"] == "exact"
    assert coopgame.shapley_sampling(game, 10)["guarantee"] == "approximate"
    assert coopgame.sampled_nucleolus(game, 20)["guarantee"] == "approximate"
    assert coopgame.kernel_point(game)["guarantee"] == "exact"


def test_convex_nucleolus_proven_and_assumed():
    graph = coopgame.InducedSubgraphGame(5, [(0, 1, 3.0), (1, 2, 1.0), (2, 3, 2.0), (3, 4, 4.0), (0, 4, 1.0)])
    proven = coopgame.convex_nucleolus(graph)
    assert proven["guarantee"] == "proven(convex)"
    explicit = coopgame.Game.from_function(5, graph.value)
    assert close(proven["allocation"], coopgame.nucleolus(explicit)["allocation"], 1e-6)
    assert coopgame.auto_nucleolus(graph)["guarantee"] == "proven(convex)"

    # 凸でないゲームは assume=True でしか実行できず、事後検証で否定される。
    game = coopgame.Game.bnf(2, 4, 0)
    with pytest.raises(ValueError):
        coopgame.convex_nucleolus(game)
    tried = coopgame.convex_nucleolus(game, assume=True)
    assert tried["guarantee"] == "assumed(convex)"
    assert tried["verdict"] == "refuted"

    # 凸な FunctionGame を凸と仮定すると、検証に合格して certified になる。
    certified = coopgame.convex_nucleolus(coopgame.FunctionGame(5, graph.value), assume=True)
    assert certified["verdict"] == "certified" and certified["guarantee"] == "certified"
    with pytest.raises(ValueError):
        coopgame.auto_nucleolus(coopgame.FunctionGame(5, graph.value))


def test_verify_solution():
    game = coopgame.Game([0, 0, 0, 10, 0, 0, 2], order="lex")
    report = coopgame.verify_solution(game, [1, 1, 0])
    assert report["check"] == "certified" and report["exact"] == [1, 1, 0]
    assert coopgame.verify_solution(game, [3, 3, -4], concept="prenucleolus")["check"] == "certified"
    assert coopgame.verify_solution(game, [1.5, 0.5, 0])["check"] == "refuted"


def test_certify_rational_reads_decimals_exactly():
    values = ["0.1", "0.2", "0.3", "0.5", "0.6", "0.7", 1]
    game = coopgame.Game([0.1, 0.2, 0.3, 0.5, 0.6, 0.7, 1.0], order="lex")
    x = coopgame.nucleolus(game)["allocation"]
    report = coopgame.certify_rational(values, x, order="lex")
    assert report["satisfied"]
    assert report["allocation"] == [Fraction(7, 30), Fraction(1, 3), Fraction(13, 30)]


# ---------------------------------------------------------------- 説明・比較


def test_explain_and_compare():
    game = coopgame.Game([0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21], order="lex")
    x = coopgame.nucleolus(game)["allocation"]
    report = coopgame.explain(game, x, names=["A", "B", "C", "D"])
    assert report["in_core"] and math.isclose(report["max_excess"], -0.5, abs_tol=1e-9)
    assert sorted(map(tuple, report["levels"][0]["coalitions"])) == [(0, 1, 2), (0, 1, 3), (0, 2, 3), (1, 2, 3)]
    assert "{A, B, C}" in report["text"]
    table = coopgame.compare(game, [("仁", x), ("Shapley", coopgame.shapley(game))])
    assert [r["in_core"] for r in table["rows"]] == [True, False]
    assert table["rows"][1]["blocking_coalitions"] == 1


# ---------------------------------------------------------------- 費用ゲーム


def test_cost_games():
    costs = coopgame.Game([15, 20, 55, 35, 61, 65, 78], order="lex")
    shares = coopgame.cost_nucleolus(costs)
    assert close(shares, [14, 18.5, 45.5], 1e-6)
    airport = coopgame.airport([1, 2, 3, 6])
    assert close(airport["shapley"], [0.25, 0.25 + 1 / 3, 0.25 + 1 / 3 + 0.5, 0.25 + 1 / 3 + 0.5 + 3])
    assert math.isclose(sum(airport["nucleolus"]), 6, abs_tol=1e-6)
    tree = coopgame.spanning_tree([[0, 10, 10, 10], [10, 0, 3, 8], [10, 3, 0, 4], [10, 8, 4, 0]])
    assert tree["bird"] == [10, 3, 4] and tree["total_cost"] == 17
    production = coopgame.linear_production([[1, 2], [3, 1]], [4, 5], [[2, 3], [4, 1], [0, 5]])
    assert math.isclose(sum(production["owen"]), production["value"], rel_tol=1e-9)


# ---------------------------------------------------------------- 値の不確かさ


def test_uncertainty_and_influence():
    game = coopgame.Game([0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21], order="lex")
    narrow = coopgame.uncertainty(game, relative=0.01, samples=50, seed=1)
    wide = coopgame.uncertainty(game, relative=0.1, samples=50, seed=1)
    assert all(w > n for w, n in zip(wide["std"], narrow["std"]))
    assert set(wide["quantiles"]) == {0.05, 0.5, 0.95}
    rows = coopgame.influence(game)
    assert max(abs(v) for v in rows[0][1]) > 0.5
    assert all(abs(v) < 1e-6 for v in coopgame.influence(game, coalitions=[[0]])[0][1])
    with pytest.raises(ValueError):
        coopgame.uncertainty(game)


def test_nucleolus_exact():
    result = coopgame.nucleolus_exact(["0.1", "0.2", "0.3", "0.5", "0.6", "0.7", 1], order="lex")
    assert result["allocation"] == [Fraction(7, 30), Fraction(1, 3), Fraction(13, 30)]
    assert result["levels"] == [Fraction(-1, 15)]
    pre = coopgame.nucleolus_exact([0, 0, 0, 10, 0, 0, 2], domain="preimputation", order="lex")
    assert pre["allocation"] == [3, 3, -4]


def test_shrink_counterexample():
    values = [2, 2, 1] + [3] * 11 + [100]
    game = coopgame.Game(values)
    shrunk, steps = coopgame.shrink(game, lambda g: not g.is_superadditive())
    assert shrunk.players == 2 and steps > 0
    assert sum(abs(v) for v in shrunk.values()) == 1

    def broken(g):
        raise KeyError("x")

    with pytest.raises(KeyError):
        coopgame.shrink(game, broken)


# ---------------------------------------------------------------- 名前付きの入力・値が分からない提携


def test_named_input_and_missing_values():
    text = '{"players": ["A", "B", "C"], "values": {"A": 0, "B": 0, "C": 0, "A,B": 4, "A,C": 3, "B,C": 2, "A,B,C": 6}}'
    loaded = coopgame.parse_game(text)
    assert loaded["names"] == ["A", "B", "C"] and loaded["kind"] == "value" and loaded["missing"] == []
    assert coopgame.nucleolus(loaded["game"])["allocation"] == pytest.approx([3, 2, 1])
    csv = "coalition,value\nA,1\nB,1\nC,1\nD,1\nA|B|C,4\nA|B|D,4\nA|C|D,4\nB|C|D,4\nA|B,3\nA|B|C|D,8\n"
    with pytest.raises(ValueError):
        coopgame.parse_game(csv, format="csv")
    filled = coopgame.parse_game(csv, format="csv", missing="zero")
    assert len(filled["missing"]) == 5
    partial = coopgame.partial_nucleolus(csv, format="csv")
    assert partial["guarantee"] == "approximate" and partial["known"] == 10
    assert sum(partial["allocation"]) == pytest.approx(8)


# ---------------------------------------------------------------- 提携構造・事前の連合


def test_coalition_structures():
    game = coopgame.Game([0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21], order="lex")
    shapley = coopgame.shapley(game)
    assert close(coopgame.owen(game, [[0, 1, 2, 3]]), shapley)
    assert close(coopgame.aumann_dreze(game, [[0, 1, 2, 3]]), shapley)
    assert close(coopgame.aumann_dreze(game, [[0, 1], [2, 3]]), [2.5, 2.5, 4, 4])
    result = coopgame.structured_nucleolus(game, [[0, 1], [2, 3]])
    assert math.isclose(result["allocation"][0] + result["allocation"][1], 5, abs_tol=1e-9)
    with pytest.raises(ValueError):
        coopgame.owen(game, [[0, 1], [1, 2, 3]])


def test_plot_svg():
    # Ferguson のゲーム: 最初の点は仁 (8/3, 2/3, 5/3)、最後は追加した点。
    game = coopgame.Game([-1, 0, 1, 3, 4, 2, 5], order="lex")
    figure = coopgame.plot_svg(game, names=["A", "B", "C"], points=[("x", [2, 1, 2])])
    assert figure["svg"].lstrip().startswith("<svg")
    assert close(figure["points"][0][1], [8 / 3, 2 / 3, 5 / 3])
    assert "</svg>" in figure["svg"]
    assert [label for label, _ in figure["points"]][-1] == "x"
    assert len(figure["core_vertices"]) >= 1
    with pytest.raises(ValueError):
        coopgame.plot_svg(coopgame.Game([0, 0, 1], order="lex"))
