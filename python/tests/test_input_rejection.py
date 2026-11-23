"""入力の誤りを黙って受け付けない (別の意味で読む・無視する・丸めることをしない) ことの回帰テスト。"""
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
