"""numpy の配列とスカラーを引数に渡したときの扱い (numpy がない環境ではスキップする)。"""
import pytest

import coopgame

np = pytest.importorskip("numpy")

VALUES = [0, 0, 0, 4, 0, 0, 4.0]


@pytest.mark.parametrize("dtype", [np.float64, np.float32, np.int64, np.int32, np.uint8])
def test_one_dimensional_numeric_arrays_are_number_sequences(dtype):
    game = coopgame.Game(np.array(VALUES, dtype=dtype), order="lex")
    assert game.values() == coopgame.Game(VALUES, order="lex").values()
    assert game.is_in_core(np.array([2, 2, 0], dtype=dtype))
    assert game.value(np.array([0, 1], dtype=np.int64)) == 4
    assert coopgame.verify(game, np.array([2, 2, 0], dtype=dtype))["satisfied"]
    assert coopgame.nucleolus_exact(np.array([0, 0, 7], dtype=dtype))["allocation"] == [3.5, 3.5]


def test_numpy_scalars_are_numbers():
    game = coopgame.Game(VALUES, order="lex")
    assert coopgame.nucleolus(game, tolerance=np.float64(1e-9))["allocation"] == pytest.approx([2, 2, 0])
    assert coopgame.shapley_sampling(game, np.int64(10), seed=np.uint64(1))["evaluations"] == 30


def test_bool_arrays_and_scalars_are_rejected():
    game = coopgame.Game(VALUES, order="lex")
    with pytest.raises(TypeError, match="bool"):
        coopgame.Game(np.array([True, False, True]))
    with pytest.raises(TypeError, match="bool"):
        game.is_in_core(np.array([True, True, False]))
    for flag in (np.bool_(True), np.bool_(False)):
        with pytest.raises(TypeError, match="bool"):
            coopgame.nucleolus(game, tolerance=flag)
        with pytest.raises(TypeError, match="bool"):
            coopgame.shapley_sampling(game, flag)
        with pytest.raises(TypeError, match="bool"):
            coopgame.verify(game, [2, 2, flag])
        with pytest.raises(TypeError, match="bool"):
            coopgame.talmud(1, [flag, 2], exact=True)


def test_arrays_of_other_shapes_and_kinds_are_rejected():
    game = coopgame.Game(VALUES, order="lex")
    # 2 次元の配列は数の列として読まない ([[2], [2], [0]] を [2, 2, 0] と読まない)。
    with pytest.raises(TypeError, match="次元"):
        game.is_in_core(np.array([[2.0], [2.0], [0.0]]))
    with pytest.raises(TypeError, match="次元"):
        coopgame.Game(np.array(VALUES).reshape(7, 1))
    with pytest.raises(TypeError):
        coopgame.Game(np.array(["0", "0", "4"]))
    with pytest.raises(TypeError):
        coopgame.Game(np.array([0, 0, 4], dtype=complex))
    # 行列の引数 (列の列) には 2 次元の配列を渡せる。
    costs = np.array([[0, 10, 10, 10], [10, 0, 3, 8], [10, 3, 0, 4], [10, 8, 4, 0]], dtype=float)
    assert coopgame.spanning_tree(costs)["bird"] == coopgame.spanning_tree(costs.tolist())["bird"]
