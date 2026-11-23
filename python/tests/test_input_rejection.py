"""入力の誤りを黙って受け付けない (別の意味で読む・無視する・丸めることをしない) ことの回帰テスト。"""
from fractions import Fraction

import pytest

import coopgame

# 3 人ゲーム (docs/cli.md の例)。仁は (3, 2, 1)。
GAME = coopgame.Game([0, 0, 4, 0, 3, 2, 6])


# ---------------------------------------------------------------- 提携の中の重複した番号


def test_duplicate_members_are_rejected():
    with pytest.raises(ValueError, match="重複"):
        GAME.value([0, 1, 1])
    graph = coopgame.InducedSubgraphGame(3, [(0, 1, 1.0)])
    with pytest.raises(ValueError, match="重複"):
        graph.value([0, 0])
    assert graph.value([1, 0]) == 1.0
    with pytest.raises(ValueError, match="重複"):
        coopgame.influence(GAME, coalitions=[[0, 0]])


# ---------------------------------------------------------------- 配分の長さと有限でない値


@pytest.mark.parametrize("x", [[3, 2], [3, 2, 1, 0], [float("nan"), 2, 1], [3, float("inf"), 1]])
def test_allocation_checks_reject_wrong_length_and_non_finite_values(x):
    expected = "長さ|有限でない"
    with pytest.raises(ValueError, match=expected):
        coopgame.is_in_kernel(GAME, x)
    with pytest.raises(ValueError, match=expected):
        coopgame.kernel_violation(GAME, x)
    with pytest.raises(ValueError, match=expected):
        coopgame.verify(GAME, x)
    with pytest.raises(ValueError, match=expected):
        coopgame.verify_solution(GAME, x)


# ---------------------------------------------------------------- 有理数の列に文字列を渡す


@pytest.mark.parametrize("values", ["007", b"007", bytearray(b"007")])
def test_rational_sequences_reject_strings(values):
    # 文字列を 1 文字ずつの列として読まない ("007" を [0, 0, 7] として解かない)。
    with pytest.raises(TypeError, match="文字列"):
        coopgame.nucleolus_exact(values)
    with pytest.raises(TypeError, match="文字列"):
        coopgame.certify_rational(values, [0, 0])
    with pytest.raises(TypeError, match="文字列"):
        coopgame.talmud(2, values, exact=True)
    with pytest.raises(TypeError):
        coopgame.talmud(2, values)


def test_rational_sequences_still_accept_string_elements():
    # 要素としての "8/3"・"0.1" は従来どおり受け付ける。
    assert coopgame.talmud(2, ["1", "2"], exact=True) == coopgame.talmud(2, [1, 2], exact=True)
    assert coopgame.nucleolus_exact(("0", "0", "7"))["allocation"] == [Fraction(7, 2)] * 2


# ---------------------------------------------------------------- 効果のない指定


def test_convex_nucleolus_rejects_verify_without_assume():
    graph = coopgame.InducedSubgraphGame(3, [(0, 1, 1.0), (1, 2, 2.0)])
    for verify in (True, False):
        with pytest.raises(ValueError, match="verify"):
            coopgame.convex_nucleolus(graph, verify=verify)
    assert coopgame.convex_nucleolus(graph)["guarantee"] == "proven(convex)"
    assert coopgame.convex_nucleolus(graph, assume=True, verify=False)["verdict"] is None


def test_influence_rejects_levels_with_coalitions():
    with pytest.raises(ValueError, match="levels"):
        coopgame.influence(GAME, coalitions=[[0, 1]], levels=3)
    assert len(coopgame.influence(GAME, coalitions=[[0, 1]])) == 1
    assert coopgame.influence(GAME, levels=1)


@pytest.mark.parametrize("flag", [True, False])
def test_tolerance_rejects_bool(flag):
    # True を 1.0 として読まない。
    with pytest.raises(TypeError, match="bool"):
        coopgame.nucleolus(GAME, tolerance=flag)
    with pytest.raises(TypeError, match="bool"):
        coopgame.nucleolus(GAME, rel_tolerance=flag)


# ---------------------------------------------------------------- 費用のゲームを価値のゲームとして使わない

COST_JSON = '{"type": "cost", "values": {"0": 4, "1": 5, "0,1": 7}}'


def test_parsed_cost_game_is_not_used_as_value_game():
    cost = coopgame.parse_game(COST_JSON)["game"]
    assert cost.kind == "cost" and repr(cost) == 'Game(players=2, kind="cost")'
    # 費用を価値として計算する関数は、費用のゲームを受け付ける関数を案内して ValueError にする。
    for run in (
        lambda g: coopgame.explain(g, [2, 5]),
        lambda g: coopgame.nucleolus(g),
        lambda g: coopgame.least_core(g),
        lambda g: coopgame.dual(g),
        lambda g: coopgame.convex_nucleolus(g, assume=True),
        lambda g: coopgame.auto_nucleolus(g),
        lambda g: coopgame.verify_solution(g, [2, 5]),
        lambda g: coopgame.nucleolus_exact(g),
        lambda g: g.is_convex(),
        lambda g: g.is_in_core([2, 5]),
    ):
        with pytest.raises(ValueError, match="cost_nucleolus"):
            run(cost)
    # 費用のゲームを受け付ける関数: 仁による費用の分担と、線形な値 (費用の分担になる)。
    assert coopgame.cost_nucleolus(cost)["allocation"] == pytest.approx([3, 4])
    assert coopgame.shapley(cost) == pytest.approx([3, 4])
    assert coopgame.banzhaf(cost) == pytest.approx([3, 4])
    # サンプリング版も線形な値の推定なので、費用の分担の推定値を返す (厳密版と同じ扱い)。
    for run in (coopgame.shapley_sampling, coopgame.banzhaf_sampling):
        estimate = run(cost, 200)
        assert estimate["values"] == pytest.approx([3, 4], abs=0.2)
    # 節約ゲームに直せば、価値のゲームの関数を使える。
    savings = cost.savings()
    assert savings.kind == "value" and savings.values() == [0, 0, 2]
    assert coopgame.nucleolus(savings)["allocation"] == pytest.approx([1, 1])
    with pytest.raises(ValueError, match="費用のゲーム"):
        savings.savings()


def test_game_kind_can_be_given_explicitly():
    assert coopgame.Game([0, 0, 4]).kind == "value"
    cost = coopgame.Game([4, 5, 7], kind="cost")
    assert cost.kind == "cost" and cost.values() == [4, 5, 7]
    with pytest.raises(ValueError, match="cost_nucleolus"):
        coopgame.nucleolus(cost)
    with pytest.raises(ValueError, match="kind"):
        coopgame.Game([4, 5, 7], kind="costs")


# ---------------------------------------------------------------- Game だけを受け付ける関数の案内


def test_game_only_functions_point_to_usable_functions():
    function_game = coopgame.FunctionGame(3, lambda s: float(len(s) >= 2))
    graph = coopgame.InducedSubgraphGame(3, [(0, 1, 1.0)])
    with pytest.raises(TypeError, match="shapley_sampling.*Game.from_function"):
        coopgame.nucleolus(function_game)
    with pytest.raises(TypeError, match="convex_nucleolus.*Game.from_function"):
        coopgame.shapley(graph)
    with pytest.raises(TypeError, match="coopgame.Game"):
        coopgame.explain([0, 0, 4], [2, 2])
    # 案内した変換で表にできる。
    table = coopgame.Game.from_function(graph.players, graph.value)
    assert coopgame.shapley(table) == pytest.approx([0.5, 0.5, 0])


def test_nucleolus_exact_accepts_game():
    game = coopgame.Game([0, 0, 0, 10, 0, 0, 2], order="lex")
    assert coopgame.nucleolus_exact(game)["allocation"] == [1, 1, 0]
    assert coopgame.nucleolus_exact(game, domain="preimputation")["allocation"] == [3, 3, -4]
    # Game は並び順を持つので、order を指定しても効果がない。
    with pytest.raises(ValueError, match="order"):
        coopgame.nucleolus_exact(game, order="lex")


# ---------------------------------------------------------------- 数の列と数の引数の型 (全ての引数で共通)


@pytest.mark.parametrize("text", ["007", b"\x00\x00\x07", bytearray(b"\x00\x00\x07")])
def test_number_sequences_reject_text(text):
    # bytes を 1 バイトずつの整数の列として読まない。
    for run in (
        lambda: coopgame.Game(text),
        lambda: GAME.value(text),
        lambda: coopgame.verify(GAME, text),
        lambda: coopgame.normalize(text),
        lambda: coopgame.Game.weighted_voting(text, 1),
        lambda: coopgame.aumann_dreze(GAME, [text]),
        lambda: coopgame.airport(text),
    ):
        with pytest.raises(TypeError, match="文字列"):
            run()


def test_number_arguments_reject_bool():
    for run in (
        lambda: coopgame.Game([0, 0, True]),
        lambda: coopgame.verify(GAME, [3, 2, True]),
        lambda: GAME.is_in_core([3, 2, 1], tolerance=True),
        lambda: coopgame.shapley_sampling(GAME, True),
        lambda: coopgame.shapley_sampling(GAME, 10, seed=False),
        lambda: coopgame.explain(GAME, [3, 2, 1], levels=True),
        lambda: coopgame.Game.bankruptcy(True, [1, 2]),
        lambda: coopgame.talmud(True, [1, 2]),
        lambda: coopgame.talmud(1, [True, 2], exact=True),
        lambda: coopgame.uncertainty(GAME, relative=True),
        lambda: coopgame.myerson(GAME, [(0, True)]),
        lambda: coopgame.InducedSubgraphGame(3, [(0, 1, True)]),
        lambda: coopgame.FunctionGame(True, len),
    ):
        with pytest.raises(TypeError, match="bool"):
            run()


def test_number_sequences_still_accept_sequences_of_numbers():
    import fractions

    assert coopgame.Game((0, 0, 4)).values() == [0, 0, 4]
    assert coopgame.verify(GAME, (3.0, 2, 1))["satisfied"]
    assert coopgame.talmud(fractions.Fraction(1), [1, 2]) == pytest.approx([0.5, 0.5])
    with pytest.raises(TypeError):
        coopgame.verify(GAME, {3, 2, 1})  # 順序のない集合は配分として読まない
