"""Python バインディングのテスト。

Rust 側のテスト (tests/literature.rs など) で確かめた文献の数値例を、バインディング経由で再現する。
数値例の出典と手計算は docs/literature-cases.md を参照。
"""
import itertools
import math
import time
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


@pytest.mark.parametrize("players", [1, 2, 10])
def test_sampling_evaluations_match_actual_calls(players):
    """evaluations は関数を実際に呼んだ回数。空提携は呼ばない。全体提携は順列ごとに呼ぶ。"""
    calls = []

    def value(coalition):
        calls.append(coalition)
        return float(len(coalition)) ** 1.5

    game = coopgame.FunctionGame(players, value)
    shapley = coopgame.shapley_sampling(game, 10, seed=1)
    assert shapley["evaluations"] == len(calls) == 10 * players
    assert calls.count(tuple(range(players))) == 10
    assert () not in calls
    for run in (coopgame.banzhaf_sampling, coopgame.sampled_nucleolus, coopgame.sampled_least_core):
        if players == 1 and run is not coopgame.banzhaf_sampling:
            continue  # サンプリングした仁・最小コアは 2 人以上
        calls.clear()
        result = run(game, 10, seed=1)
        assert result["evaluations"] == len(calls), run.__name__
        assert () not in calls, run.__name__


def test_sampling_averages_noise_of_grand_coalition():
    """呼ぶたびに値が揺れる関数では、全体提携も順列ごとに呼び直し、揺れを標本の分散に反映する。

    v(S) = |S| + ノイズ (全体提携だけ)。全体提携の値を使い回すと、最後に加わる人の限界貢献の揺れが
    消えず、推定値の和が 1 回目のノイズに固定される。
    """
    import random

    rng = random.Random(0)
    players = 3

    def value(coalition):
        return float(len(coalition)) + (rng.gauss(0.0, 1.0) if len(coalition) == players else 0.0)

    result = coopgame.shapley_sampling(coopgame.FunctionGame(players, value), 4000, seed=2)
    # 順列ごとのノイズの平均は 0 に近づく (標準偏差 1/sqrt(4000) の 5 倍以内)。
    assert abs(sum(result["values"]) - players) < 5 / math.sqrt(4000)
    # 限界貢献は、最後に加わる (確率 1/3) ときだけノイズを含むので分散は 1/3。標準誤差は sqrt(1/3) / sqrt(4000)。
    for se in result["standard_errors"]:
        assert se == pytest.approx(math.sqrt(1 / 3) / math.sqrt(4000), rel=0.2)


@pytest.mark.parametrize("players", [1, 2, 6])
def test_convex_evaluations_match_actual_calls(players):
    """convex_nucleolus の evaluations は、凸ゲームの手法で関数を実際に呼んだ回数 (事後検証は含まない)。"""
    calls = 0

    def value(coalition):
        nonlocal calls
        calls += 1
        return float(len(coalition)) ** 2

    result = coopgame.convex_nucleolus(coopgame.FunctionGame(players, value), assume=True, verify=False)
    assert result["evaluations"] == calls > 0


def test_callback_exception_is_raised():
    def broken(coalition):
        raise KeyError("broken")

    with pytest.raises(KeyError):
        coopgame.shapley_sampling(coopgame.FunctionGame(4, broken), 10)
    with pytest.raises(KeyError):
        coopgame.sampled_nucleolus(coopgame.FunctionGame(4, broken), 10)


# FunctionGame を受け付ける全ての関数 (README の表)。凸ゲームの手法は、凸な値 |S|^2 で途中まで進ませる。
FUNCTION_GAME_RUNS = {
    "shapley_sampling": (2000, lambda game: coopgame.shapley_sampling(game, 10**7)),
    "banzhaf_sampling": (2000, lambda game: coopgame.banzhaf_sampling(game, 10**6)),
    "sampled_nucleolus": (2000, lambda game: coopgame.sampled_nucleolus(game, 10**4)),
    "sampled_least_core": (2000, lambda game: coopgame.sampled_least_core(game, 10**4)),
    "convex_nucleolus": (12, lambda game: coopgame.convex_nucleolus(game, assume=True)),
    "convex_nucleolus(verify=False)": (
        12,
        lambda game: coopgame.convex_nucleolus(game, assume=True, verify=False),
    ),
    "verify_solution": (20, lambda game: coopgame.verify_solution(game, [20.0] * 20)),
}


@pytest.mark.parametrize("name", list(FUNCTION_GAME_RUNS))
@pytest.mark.parametrize("error", [RuntimeError, KeyboardInterrupt])
def test_callback_exception_aborts_computation(name, error):
    """関数が例外 (Ctrl-C の KeyboardInterrupt を含む) を投げたら、計算を打ち切ってすぐ送出する。

    以前は例外を保存したまま NaN で計算を続けていた (shapley_sampling は全順列を回り、凸ゲームの手法は止まらなかった)。
    続けた計算は関数を呼ばないので、呼び出し回数では見分けられない。打ち切らない場合に数十秒以上かかる規模にして、
    十分に緩い時間の上限で確かめる。
    """
    players, run = FUNCTION_GAME_RUNS[name]
    calls = 0

    def value(coalition):
        nonlocal calls
        calls += 1
        if calls == 50:
            raise error("stop")
        return float(len(coalition)) ** 2

    start = time.perf_counter()
    with pytest.raises(error, match="stop"):
        run(coopgame.FunctionGame(players, value))
    assert time.perf_counter() - start < 5.0
    assert calls == 50


@pytest.mark.parametrize("bad", [math.nan, math.inf, -math.inf])
def test_callback_non_finite_value_is_rejected(bad):
    game = coopgame.FunctionGame(4, lambda coalition: bad if len(coalition) == 2 else 1.0)
    with pytest.raises(ValueError):
        coopgame.shapley_sampling(game, 10)
    with pytest.raises(ValueError):
        coopgame.sampled_nucleolus(game, 10)


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
    with pytest.raises(ValueError):
        game.is_in_core([1.0])  # 長さが人数と異なる
    assert game.is_in_core([0.5, 0.5])


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
    # CoopGame の modiclus のヘルプの 4 人ゲーム。CoopGame は空提携と全体提携を組に入れないので
    # (4.25, 5.25, 5.75, 5.75) を返すが、定義どおりの modiclus は (13/3, 16/3, 17/3, 17/3)。
    game = coopgame.Game([0, 0, 0, 0, 5, 5, 8, 9, 10, 8, 13, 15, 16, 17, 21], order="lex")
    assert close(coopgame.modiclus(game)["allocation"], [13 / 3, 16 / 3, 17 / 3, 17 / 3])
    young = coopgame.Game([0, 0, 0, 0, 9, 10, 12], order="lex")
    assert close(coopgame.per_capita_nucleolus(young)["allocation"], [2 / 3, 7 / 6, 61 / 6])
    # 仁の変種は、許容誤差を指定しなければ確かめていないので approximate、指定すれば exact。
    for solve in (
        lambda **kw: coopgame.per_capita_nucleolus(young, **kw),
        lambda **kw: coopgame.proportional_nucleolus(young, **kw),
        lambda **kw: coopgame.modiclus(young, **kw),
        lambda **kw: coopgame.disruption_nucleolus(young, **kw),
    ):
        default = solve()
        assert default["guarantee"] == "approximate"
        fixed = solve(rel_tolerance=1e-7)
        assert fixed["guarantee"] == "exact" and fixed["allocation"] == default["allocation"]
        assert fixed["tolerance"] == default["tolerance"]
        assert solve(tolerance=1e-6)["tolerance"] == 1e-6
        with pytest.raises(ValueError):
            solve(tolerance=1e-6, rel_tolerance=1e-7)
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


def test_default_nucleolus_is_approximate_beyond_check_limit():
    """既定の手順は 21 人以上で Kohlberg 基準を確かめないので approximate。許容誤差を指定すると exact。"""
    assert coopgame.nucleolus(coopgame.Game.bnf(1, 20, 3))["guarantee"] == "exact"
    game = coopgame.Game.bnf(1, 21, 3)
    auto = coopgame.nucleolus(game)
    assert auto["guarantee"] == "approximate"
    assert coopgame.prenucleolus(game)["guarantee"] == "approximate"
    fixed = coopgame.nucleolus(game, rel_tolerance=1e-7)
    assert fixed["guarantee"] == "exact"
    assert fixed["allocation"] == auto["allocation"]
    assert coopgame.auto_nucleolus(game)["guarantee"] == "approximate"


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
    # 事後検証の基準の仁の求め方 (既定は "auto")。事後検証をしない呼び方では指定できない。
    for reference in ["auto", "exact", "lp"]:
        assert coopgame.convex_nucleolus(game, assume=True, reference=reference)["verdict"] == "refuted"
    with pytest.raises(ValueError, match="reference"):
        coopgame.convex_nucleolus(game, assume=True, reference="fast")
    with pytest.raises(ValueError, match="assume=True"):
        coopgame.convex_nucleolus(graph, reference="lp")
    with pytest.raises(ValueError, match="assume=True"):
        coopgame.convex_nucleolus(game, assume=True, verify=False, reference="lp")
    with pytest.raises(ValueError):
        coopgame.auto_nucleolus(coopgame.FunctionGame(5, graph.value))


def test_nucleolus_tolerance():
    game = coopgame.Game([0, 0, 0, 4, 3, 2, 6], order="lex")
    # 既定: 最初に解いた配分が Kohlberg 基準を満たすゲームは、比 1e-7 の許容誤差で 1 回だけ解く。
    default = coopgame.nucleolus(game)
    assert default["allocation"] == pytest.approx([3, 2, 1])
    assert default["tolerance"] == pytest.approx(6e-7)
    assert default["refinements"] == 0 and default["rational"] is False
    # 指定した許容誤差で 1 回だけ解く。
    assert coopgame.nucleolus(game, rel_tolerance=1e-9)["tolerance"] == pytest.approx(6e-9)
    assert coopgame.nucleolus(game, tolerance=1e-3)["tolerance"] == pytest.approx(1e-3)
    with pytest.raises(ValueError, match="どちらか一方"):
        coopgame.nucleolus(game, tolerance=1e-6, rel_tolerance=1e-9)
    with pytest.raises(ValueError):
        coopgame.nucleolus(game, rel_tolerance=-1)
    # 値が 1e12 程度の整数ゲームは逐次 LP で超過の段を分けられないので、既定では有理数の逐次 LP に切り替える。
    large = coopgame.Game(
        [0, 0, 2000000000002, 0, 2000000000001, 2000000000006, 3000000000002, 0]
        + [2000000000006, 2000000000006, 3000000000005, 2000000000005]
        + [3000000000002, 3000000000002, 4000000000002]
    )
    solved = coopgame.nucleolus(large)
    assert solved["rational"] is True and solved["guarantee"] == "exact"
    exact = coopgame.nucleolus_exact([int(v) for v in large.values()])
    assert solved["allocation"] == [float(x) for x in exact["allocation"]]
    assert solved["allocation"] == [999999999998.5, 1000000000001.25, 999999999999.75, 1000000000002.5]
    fixed = coopgame.nucleolus(large, rel_tolerance=1e-7)
    assert fixed["rational"] is False and fixed["refinements"] == 0


def test_analysis_and_partial_nucleolus_accept_tolerance():
    game = coopgame.Game([0, 0, 0, 4, 3, 2, 6], order="lex")
    # uncertainty と influence は、指定しなければ比 1e-7 の許容誤差で 1 回だけ解く。
    default = coopgame.influence(game)
    assert coopgame.influence(game, rel_tolerance=1e-7) == default
    assert coopgame.influence(game, tolerance=6e-7) == default
    assert coopgame.uncertainty(game, relative=0.1, samples=5, rel_tolerance=1e-9)["samples"] == 5
    with pytest.raises(ValueError, match="nucleolus"):
        coopgame.influence(game, solver="shapley", tolerance=1e-6)
    with pytest.raises(ValueError, match="どちらか一方"):
        coopgame.uncertainty(game, relative=0.1, tolerance=1e-6, rel_tolerance=1e-9)
    # 許容誤差の値は解く前に確かめる (標本の失敗に紛れさせない)。
    for bad in [-1.0, float("nan"), float("inf")]:
        for keyword in ["tolerance", "rel_tolerance"]:
            with pytest.raises(ValueError, match="0 以上の有限"):
                coopgame.uncertainty(game, relative=0.1, samples=5, **{keyword: bad})
            with pytest.raises(ValueError, match="0 以上の有限"):
                coopgame.influence(game, **{keyword: bad})
            with pytest.raises(ValueError, match="0 以上の有限"):
                coopgame.nucleolus(game, **{keyword: bad})
    # 値に比べて大きすぎる比は解くときに分かる。標本が 1 つも解けなければ、最初の失敗の理由を出す。
    with pytest.raises(ValueError, match="大きすぎる"):
        coopgame.uncertainty(game, relative=0.1, samples=5, rel_tolerance=1e308)
    # 既定の手順と比 1e-7 の 1 回の解で仁が異なるゲーム (値が 1e12 程度の 4 人の整数ゲーム) で、既定を見分ける。
    large = coopgame.Game(
        [0, 0, 2000000000002, 0, 2000000000001, 2000000000006, 3000000000002, 0]
        + [2000000000006, 2000000000006, 3000000000005, 2000000000005, 3000000000002, 3000000000002, 4000000000002]
    )
    assert coopgame.nucleolus(large)["rational"] is True
    assert coopgame.nucleolus(large)["allocation"] != coopgame.nucleolus(large, rel_tolerance=1e-7)["allocation"]
    assert coopgame.influence(large) == coopgame.influence(large, rel_tolerance=1e-7)
    assert coopgame.uncertainty(large, relative=0.1, samples=5) == coopgame.uncertainty(
        large, relative=0.1, samples=5, rel_tolerance=1e-7
    )
    # tolerance="auto" は、既定の手順を明示して選ぶ。nucleolus では指定しない場合と同じ。
    assert coopgame.nucleolus(large, tolerance="auto") == coopgame.nucleolus(large)
    # uncertainty・influence では、1 回だけ解く既定の代わりに既定の手順で解く。値をほとんど動かさない標本では、
    # 1 回だけ解いた分布と異なる。
    narrow = dict(relative=1e-13, samples=3)
    assert coopgame.uncertainty(large, tolerance="auto", **narrow) != coopgame.uncertainty(large, **narrow)
    assert coopgame.influence(large, tolerance="auto")
    with pytest.raises(ValueError, match="auto"):
        coopgame.influence(game, tolerance="fast")
    with pytest.raises(ValueError, match="どちらか一方"):
        coopgame.influence(game, tolerance="auto", rel_tolerance=1e-9)
    with pytest.raises(ValueError, match="nucleolus"):
        coopgame.influence(game, solver="shapley", tolerance="auto")
    with pytest.raises(TypeError):
        coopgame.influence(game, rel_tolerance="auto")
    # 値が分かる提携だけの仁も、許容誤差を指定できる。
    csv = "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|2,3\n1|2,2\n0|1|2,6\n"
    assert coopgame.partial_nucleolus(csv, format="csv", rel_tolerance=1e-9)["tolerance"] == pytest.approx(6e-9)
    assert coopgame.partial_nucleolus(csv, format="csv", tolerance=1e-3)["tolerance"] == pytest.approx(1e-3)
    with pytest.raises(ValueError):
        coopgame.partial_nucleolus(csv, format="csv", tolerance=-1)


def test_verify_solution():
    game = coopgame.Game([0, 0, 0, 10, 0, 0, 2], order="lex")
    report = coopgame.verify_solution(game, [1, 1, 0])
    assert report["check"] == "certified" and report["exact"] == [1, 1, 0]
    assert coopgame.verify_solution(game, [3, 3, -4], domain="preimputation")["check"] == "certified"
    assert coopgame.verify_solution(game, [1.5, 0.5, 0])["check"] == "refuted"
    # 基準の仁の求め方 (既定は "auto")。
    for reference in ["auto", "exact", "lp"]:
        assert coopgame.verify_solution(game, [1.5, 0.5, 0], reference=reference)["check"] == "refuted"
    with pytest.raises(ValueError, match="reference"):
        coopgame.verify_solution(game, [1, 1, 0], reference="fast")
    # 基準の求め方で結論が変わる例: 値が 1e9 程度の 4 人の整数ゲーム。厳密な仁から 2 だけずらした点は、
    # 逐次 LP の仁からは厳密な配分を復元できないので、"lp" では未決、有理数で求め直す "auto"・"exact" では合格。
    large = coopgame.Game(
        [0, 0, 2000000002, 0, 2000000000, 2000000001, 3000000000, 0]
        + [2000000005, 2000000002, 3000000000, 2000000003, 3000000001, 3000000004, 4000000001]
    )
    near = [1000000001.375, 999999998.25, 999999998.25, 1000000003.125]
    assert coopgame.verify_solution(large, near)["check"] == "certified"
    assert coopgame.verify_solution(large, near, reference="exact")["check"] == "certified"
    assert coopgame.verify_solution(large, near, reference="lp")["check"] == "undecided"


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
    report = coopgame.explain(game, x)
    assert report["in_core"] and math.isclose(report["max_excess"], -0.5, abs_tol=1e-9)
    assert sorted(map(tuple, report["levels"][0]["coalitions"])) == [(0, 1, 2), (0, 1, 3), (0, 2, 3), (1, 2, 3)]
    assert "{0, 1, 2}" in report["text"] and "プレイヤー 3" in report["text"]
    table = coopgame.compare(game, [x, coopgame.shapley(game)])
    assert [r["in_core"] for r in table["rows"]] == [True, False]
    assert close(table["rows"][0]["allocation"], x)
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
    with pytest.raises(ValueError):
        coopgame.influence(game, coalitions=[[]])  # 空提携の値は 0 に固定


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


@pytest.mark.parametrize("error", [RuntimeError, KeyboardInterrupt])
def test_shrink_aborts_on_predicate_exception(error):
    """predicate が例外 (Ctrl-C の KeyboardInterrupt を含む) を投げたら、縮小を打ち切ってすぐ送出する。

    以前は例外を保存したまま predicate を False として縮小を続け、候補を全て作り直していた。
    続けた縮小は predicate を呼ばないので、打ち切らない場合に数秒以上かかる規模にして、緩い時間の上限で確かめる。
    """
    game = coopgame.Game([float(i % 7) for i in range(2**16 - 1)])
    calls = 0

    def predicate(candidate):
        nonlocal calls
        calls += 1
        if calls == 3:
            raise error("stop")
        return False

    start = time.perf_counter()
    with pytest.raises(error, match="stop"):
        coopgame.shrink(game, predicate, budget=10**9)
    assert time.perf_counter() - start < 2.0
    assert calls == 3


# ---------------------------------------------------------------- JSON・CSV の入力・値が分からない提携


def test_game_input_and_missing_values():
    text = '{"players": 3, "values": {"0": 0, "1": 0, "2": 0, "0,1": 4, "0,2": 3, "1,2": 2, "0,1,2": 6}}'
    loaded = coopgame.parse_game(text)
    assert loaded["game"].players == 3 and loaded["kind"] == "value" and loaded["missing"] == []
    assert coopgame.nucleolus(loaded["game"])["allocation"] == pytest.approx([3, 2, 1])
    # 人数を省くと最大の番号 + 1 人。提携を {"coalition": [...], "value": ...} の列で書いてもよい。
    listed = '{"values": [{"coalition": [0], "value": 1}, {"coalition": [1], "value": 1}, {"coalition": [0, 1], "value": 3}]}'
    assert coopgame.parse_game(listed)["game"].values() == [1, 1, 3]
    csv = "coalition,value\n0,1\n1,1\n2,1\n3,1\n0|1|2,4\n0|1|3,4\n0|2|3,4\n1|2|3,4\n0|1,3\n0|1|2|3,8\n"
    with pytest.raises(ValueError):
        coopgame.parse_game(csv, format="csv")
    filled = coopgame.parse_game(csv, format="csv", missing="zero")
    assert filled["missing"] == [[0, 2], [1, 2], [0, 3], [1, 3], [2, 3]]
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
    # 2 つ以上のブロックは確かめていないので approximate。許容誤差を明示すれば 1 回だけ解いて exact。
    assert result["guarantee"] == "approximate" and result["domain"] == "imputation"
    fixed = coopgame.structured_nucleolus(game, [[0, 1], [2, 3]], rel_tolerance=1e-7)
    assert fixed["guarantee"] == "exact" and fixed["allocation"] == result["allocation"]
    # 全体 1 ブロックは仁そのものなので、nucleolus の既定と同じ。
    grand = coopgame.structured_nucleolus(game, [[0, 1, 2, 3]])
    assert grand == coopgame.nucleolus(game)
    with pytest.raises(ValueError):
        coopgame.owen(game, [[0, 1], [1, 2, 3]])


def test_plot_svg():
    # Ferguson のゲーム。追加した点は列での位置 (0 始まり) で示す。
    game = coopgame.Game([-1, 0, 1, 3, 4, 2, 5], order="lex")
    figure = coopgame.plot_svg(game, points=[[2, 1, 2]])
    assert figure["svg"].lstrip().startswith("<svg")
    assert close(figure["nucleolus"], [8 / 3, 2 / 3, 5 / 3])
    assert close(figure["shapley"], coopgame.shapley(game))
    assert "</svg>" in figure["svg"] and "点 0" in figure["svg"]
    assert coopgame.plot_svg(game, shapley=False)["shapley"] is None
    assert len(figure["core_vertices"]) >= 1
    with pytest.raises(ValueError):
        coopgame.plot_svg(coopgame.Game([0, 0, 1], order="lex"))


# ---------------------------------------------------------------- tau 値・Gately 点・solidarity 値・Myerson 値・投票力指数
# 手計算は Rust 側の単体テスト (src/compromise.rs、src/values.rs、src/communication.rs、src/power.rs) と同じ。


def test_tau_value_and_utopia_payoffs():
    game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
    bounds = coopgame.utopia_payoffs(game)
    assert close(bounds["utopia"], [4, 6, 8])
    assert close(bounds["minimal_rights"], [0, 0, 2])
    assert close(coopgame.tau_value(game), [2.5, 3.75, 5.75])
    with pytest.raises(ValueError):
        coopgame.tau_value(coopgame.Game([0, 0, 0, 80, 80, 80, 90], order="lex"))


def test_gately_point_is_efficient():
    game = coopgame.Game([1, 0, 2, 4, 6, 8, 12], order="lex")
    assert close(coopgame.gately_point(game), [2.8, 3.6, 5.6])


def test_solidarity_gives_null_players_a_share():
    game = coopgame.Game([1, 0, 0, 1, 1, 0, 1], order="lex")
    assert close(coopgame.shapley(game), [1, 0, 0])
    assert close(coopgame.solidarity(game), [11 / 18, 7 / 36, 7 / 36])


def test_myerson_value_on_a_line():
    majority = coopgame.Game([0, 0, 0, 1, 1, 1, 1], order="lex")
    assert close(coopgame.myerson(majority, [(0, 1), (1, 2)]), [1 / 6, 2 / 3, 1 / 6])
    restricted = coopgame.graph_restricted(majority, [(0, 1), (1, 2)])
    assert close(coopgame.shapley(restricted), [1 / 6, 2 / 3, 1 / 6])


def test_power_indices_of_weighted_voting_game():
    game = coopgame.Game.weighted_voting([3, 2, 1], 4)
    power = coopgame.power_indices(game)
    assert power["swings"] == [3, 1, 1]
    assert sorted(power["minimal_winning"]) == [[0, 1], [0, 2]]
    assert close(power["johnston"], [2 / 3, 1 / 6, 1 / 6])
    assert close(power["deegan_packel"], [0.5, 0.25, 0.25])
    assert close(power["public_good"], [0.5, 0.25, 0.25])
    assert close(power["coleman_prevent"], [1, 1 / 3, 1 / 3])
    assert close(power["coleman_initiative"], [0.6, 0.2, 0.2])
    assert math.isclose(power["coleman_collectivity"], 3 / 8)


def test_disruption_and_anti_nucleolus():
    # CoopGame の disruptionNucleolus のヘルプの例 (小数 6 桁に丸めた掲載値)
    game = coopgame.Game([0, 0, 0, 0, 2, 3, 4, 1, 3, 2, 8, 11, 6.5, 9.5, 14], order="lex")
    assert close(coopgame.disruption_nucleolus(game)["allocation"], [3.193548, 4.754839, 2.129032, 3.922581], 1e-6)
    # Funaki & Meinhardt (2006) Example 4.1
    w = coopgame.Game([60, 80, 120, 140, 150, 150, 150], order="lex")
    assert close(coopgame.anti_prenucleolus(w)["allocation"], [30, 40, 80])
    assert close(coopgame.anti_nucleolus(w)["allocation"], [30, 40, 80])
    assert close(coopgame.prenucleolus(coopgame.dual(w))["allocation"], [30, 40, 80])
