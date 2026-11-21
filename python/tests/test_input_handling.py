"""不正な入力と退化したゲームの扱い (Rust 側の tests/cli_input_handling.rs と同じ場合をバインディングで確かめる)。"""
import json
import math

import pytest

import coopgame

# 3 人ゲーム (docs/cli.md の例)。仁は (3, 2, 1)。
GAME = coopgame.Game([0, 0, 4, 0, 3, 2, 6])


# ---------------------------------------------------------------- 配分の説明・比較


@pytest.mark.parametrize("bad", [math.nan, math.inf, -math.inf])
def test_explain_and_compare_reject_non_finite_allocations(bad):
    with pytest.raises(ValueError, match="x_0"):
        coopgame.explain(GAME, [bad, 2, 1])
    with pytest.raises(ValueError, match="1 番目の配分"):
        coopgame.compare(GAME, [[3, 2, 1], [bad, 2, 1]])


# ---------------------------------------------------------------- 厳密な検証


@pytest.mark.parametrize("bad", [math.nan, math.inf, -math.inf])
def test_certify_rejects_non_finite_allocations(bad):
    # 段の読み取りが進まず止まらなくなることがなく、ValueError で終わる。
    with pytest.raises(ValueError, match="有限でない"):
        coopgame.certify(GAME, [bad, 2, 1])
    with pytest.raises(ValueError, match="有限でない"):
        coopgame.certify_rational([0, 0, 4, 0, 3, 2, 6], [bad, 2, 1])


# ---------------------------------------------------------------- 許容誤差つきの判定


@pytest.mark.parametrize("bad", [math.inf, -math.inf, math.nan, -1.0])
def test_membership_checks_reject_invalid_tolerance(bad):
    # 無限大ではどの配分も「属する」、負では仁も「属さない」と返していた。
    with pytest.raises(ValueError, match="許容誤差"):
        coopgame.is_in_kernel(GAME, [3, 2, 1], tolerance=bad)
    with pytest.raises(ValueError, match="許容誤差"):
        GAME.is_in_core([3, 2, 1], tolerance=bad)
    assert coopgame.is_in_kernel(GAME, [3, 2, 1], tolerance=1e-9)


# ---------------------------------------------------------------- 感度


def test_influence_uses_one_sided_difference_on_single_point_imputation_set():
    # 加法的なゲーム: 配分集合は 1 点 (1, 1, 1)。v(N) を下げる・1 人提携の値を上げると配分集合が空になる。
    additive = coopgame.Game([1, 1, 2, 1, 2, 2, 3])
    rows = {tuple(c): (s, d) for c, s, d in coopgame.influence(additive)}
    assert len(rows) == 7
    sensitivity, difference = rows[(0, 1, 2)]
    assert difference == "forward"
    assert all(abs(v - 1 / 3) < 1e-6 for v in sensitivity)
    for single in [(0,), (1,), (2,)]:
        assert rows[single][1] == "backward"
    assert rows[(0, 1)][1] == "central"
    # 中心差分で解けるゲームは central。
    assert {d for _, _, d in coopgame.influence(GAME)} == {"central"}


# ---------------------------------------------------------------- 図


def test_plot_draws_point_and_segment_cores():
    glove = coopgame.Game([0, 0, 1, 0, 1, 0, 1])
    figure = coopgame.plot_svg(glove)
    assert figure["core_vertices"] == [[1.0, 0.0, 0.0]]
    assert '<circle class="core-dot"' in figure["svg"]
    assert "コア (1 点)" in figure["svg"]
    pair = coopgame.Game([0, 0, 0, 1, 0, 0, 1], order="lex")
    figure = coopgame.plot_svg(pair)
    assert '<line class="core-segment"' in figure["svg"]
    assert "コア (線分)" in figure["svg"]


# ---------------------------------------------------------------- JSON・CSV の読み込み


@pytest.mark.parametrize(
    "text, format",
    [
        ('{"values": {"0": 0, "1": 0, "0,1": 4, "0,1": 5}}', "json"),
        ('{"values": {"0": 0, "1": 0, "0,1": 4, "1,0": 5}}', "json"),
        ("coalition,value\n0,0\n1,0\n0|1,4\n1|0,5\n", "csv"),
    ],
)
def test_parse_game_rejects_duplicate_coalitions(text, format):
    with pytest.raises(ValueError, match="重複"):
        coopgame.parse_game(text, format=format)


@pytest.mark.parametrize(
    "text, format",
    [
        ('{"values": {"0,0": 1, "1": 0, "0,1": 3}}', "json"),
        ('{"values": [{"coalition": [0, 0], "value": 1}, {"coalition": [0, 1], "value": 3}]}', "json"),
        ("coalition,value\n0|0,1\n1,0\n0|1,3\n", "csv"),
    ],
)
def test_parse_game_rejects_duplicate_members(text, format):
    with pytest.raises(ValueError, match="2 回以上ある"):
        coopgame.parse_game(text, format=format)


def test_parse_game_reports_huge_player_index():
    with pytest.raises(ValueError, match="2 行目.*範囲外"):
        coopgame.parse_game("coalition,value\n18446744073709551615,1\n", format="csv")


def test_parse_game_suggests_how_to_handle_missing_values():
    partial = "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|1|2,6\n"
    with pytest.raises(ValueError, match="partial_nucleolus"):
        coopgame.parse_game(partial, format="csv")
    loaded = coopgame.parse_game(partial, format="csv", missing="zero")
    assert loaded["missing"] == [[0, 2], [1, 2]]
    # partial_nucleolus は価値のゲームに限るので、費用のゲームには案内しない。
    cost = partial.replace("coalition,value", "coalition,cost")
    with pytest.raises(ValueError, match='missing="zero"') as raised:
        coopgame.parse_game(cost, format="csv")
    assert "partial_nucleolus" not in str(raised.value)
    with pytest.raises(ValueError, match="価値のゲーム"):
        coopgame.partial_nucleolus(cost, format="csv")


def test_partial_nucleolus_with_all_values_is_still_approximate():
    # 全ての提携の値があっても、値が分かる提携だけを扱う手法の結果なので近似とする。配分は仁と一致する。
    full = "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|2,3\n1|2,2\n0|1|2,6\n"
    result = coopgame.partial_nucleolus(full, format="csv")
    game = coopgame.parse_game(full, format="csv")["game"]
    assert result["guarantee"] == "approximate" and result["known"] == 7
    assert result["allocation"] == pytest.approx(coopgame.nucleolus(game)["allocation"])


def test_partial_nucleolus_beyond_table_limit():
    n = 31
    everyone = list(range(n))
    values = {str(i): 1 for i in everyone}
    for i in everyone:
        values[",".join(str(j) for j in everyone if j != i)] = 29.5
    values[",".join(map(str, everyone))] = 31
    result = coopgame.partial_nucleolus(json.dumps({"players": n, "values": values}))
    assert all(abs(x - 1) < 1e-6 for x in result["allocation"])
    assert result["guarantee"] == "approximate"


# ---------------------------------------------------------------- サンプリング


def test_sampling_requires_at_least_one_sample():
    with pytest.raises(ValueError, match="permutations は 1 以上"):
        coopgame.shapley_sampling(GAME, 0)
    with pytest.raises(ValueError, match="samples は 1 以上"):
        coopgame.banzhaf_sampling(GAME, 0)
    estimate = coopgame.shapley_sampling(GAME, 1)
    assert len(estimate["values"]) == 3
