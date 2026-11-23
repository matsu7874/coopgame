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
