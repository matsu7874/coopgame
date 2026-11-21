# coopgame (Python バインディング)

Rust ライブラリ coopgame の機能を Python から使うためのバインディング (PyO3)。
Python 3.9 以降で動く (abi3)。

## インストール

PyPI での配布名は `coopgame-py` で、import 名は `coopgame` である
(PyPI の `coopgame` は別のパッケージが使っている)。

```bash
pip install coopgame-py
```

## ビルド

```bash
python3 -m venv .venv && . .venv/bin/activate
pip install maturin pytest
cd python
maturin develop --release   # 現在の環境にインストール
pytest tests                # 文献の数値例などのテスト
```

## 例

```python
from fractions import Fraction
import coopgame

# 辞書式順 ({1},{2},{3},{1,2},{1,3},{2,3},{1,2,3}) で与える。既定はビット順 (order="binary")。
game = coopgame.Game([0, 0, 0, 10, 0, 0, 2], order="lex")
coopgame.nucleolus(game)["allocation"]                      # [1.0, 1.0, 0.0]
coopgame.prenucleolus(game)["allocation"]                   # [3.0, 3.0, -4.0]
coopgame.certify(game, [3, 3, -4], "preimputation")         # {'allocation': [Fraction(3, 1), ...], 'satisfied': True, ...}
coopgame.kernel_set(game, "preimputation")["pieces"]        # 多面体の一覧

coopgame.talmud(100, [100, 200, 300], exact=True)           # [Fraction(100, 3)] * 3

# JSON・CSV の提携はプレイヤー番号 (0 始まり) で書く。
loaded = coopgame.parse_game('{"values": {"0": 0, "1": 0, "0,1": 4}}')
coopgame.nucleolus(loaded["game"])["allocation"]            # [2.0, 2.0]

# 全提携を列挙できないゲームは Python の関数で与える (提携はプレイヤー番号のタプル)。
majority = coopgame.FunctionGame(101, lambda s: float(len(s) >= 51))
coopgame.shapley_sampling(majority, 2000, seed=0)           # values, standard_errors, evaluations
coopgame.sampled_nucleolus(majority, 500, seed=0)           # サンプルした提携だけで解いたプレ仁
```

## API

| 関数 | 内容 |
|---|---|
| `Game(values, order="binary" or "lex")`, `Game.from_function(n, f)`, `Game.weighted_voting`, `Game.bankruptcy`, `Game.bnf` | 明示ベクトルのゲーム (30 人まで)。`value`, `values`, `is_convex` などのメソッドを持つ |
| `FunctionGame(n, f)` | Python の関数で値を返すゲーム。受け付けるのは `shapley_sampling`・`banzhaf_sampling`・`sampled_nucleolus`・`sampled_least_core`・`convex_nucleolus(..., assume=True)`・`verify_solution` |
| `nucleolus(game, domain="imputation", method="cg" or "full", tolerance=None, rel_tolerance=None)`, `prenucleolus`, `least_core` | 仁・プレ仁・最小コア。`tolerance` (値の単位) か `rel_tolerance` (`max\|v(S)\|` に対する比) を指定すると、その許容誤差で 1 回だけ解く。指定しなければ、超過の段を確かめて必要なら解き直し、10 人以下は有理数の逐次 LP に切り替えて正しい仁を返す (時間がかかることがある)。戻り値の `tolerance`・`refinements`・`rational` で求め方が分かる |
| `per_capita_nucleolus`, `proportional_nucleolus`, `modiclus` | 仁の変種 (per capita 仁・比例仁は 17 人まで、modiclus は 7 人まで) |
| `kernel_point`, `kernel_violation`, `is_in_kernel`, `kernel_set(..., merge=False)` | カーネル・プレカーネル |
| `bargaining_set(game, x, domain)` | 交渉集合 (プレ交渉集合) への所属と、反論のない異議 (11 人まで) |
| `InducedSubgraphGame(n, edges)` | 誘導部分グラフゲーム (重みは非負、構造的に凸、人数の上限なし) |
| `convex_nucleolus(game, assume=False, verify=True)` | 凸ゲームの手法で仁。`assume=True` は凸と仮定して試し、`verdict` に検証結果 |
| `auto_nucleolus(game)` | 保証のある手法のうち最も速いものを選ぶ |
| `cost_nucleolus(game)`, `airport(costs)`, `spanning_tree(costs)`, `linear_production(technology, prices, resources)` | 費用ゲームと、構造を持つ費用ゲーム (空港・最小全域木・線形生産) |
| `uncertainty(game, relative=0.1)` / `uncertainty(lower, upper=upper)`, `influence(game)` | 値の不確かさによる配分の分布と、提携の値に対する感度。`influence` は `(提携, 感度, 差分の取り方)` の列で、差分の取り方は `"central"`、片側のゲームが解けないときは `"forward"`・`"backward"` |
| `explain(game, x, levels=3)`, `compare(game, [配分, ...])` | 配分の説明と比較 (`text` に日本語の説明)。`compare` の `rows` は入力と同じ順。nan・inf を含む配分は ValueError |
| `verify_solution(game, allocation, domain, reference="auto")` | 配分が仁 (プレ仁) かを事後検証 (`certified`・`refuted`・`undecided`)。`reference` は基準の仁の求め方 (`"lp"` は逐次 LP の仁だけ、`"exact"` は 10 人まで有理数で求め直す、`"auto"` は 8 人以下のときだけ求め直す) |
| `parse_game(text, format="json", missing="error")`, `partial_nucleolus(text, format)` | JSON・CSV の読み込み (提携はプレイヤー番号。同じ提携・同じキー・提携の中の同じ番号の重複は ValueError) と、値が分かる提携だけで解いた仁 (63 人まで。費用のゲームは節約ゲームで解いて費用の分担を返し、辞書の `kind` に値の種類を返す) |
| `aumann_dreze(game, blocks)`, `owen(game, unions)`, `structured_nucleolus(game, blocks)` | 提携構造・事前の連合のもとでの解 |
| `plot_svg(game, kernel=True, shapley=True, points=None, title=None)` | 3-4 人の配分集合の図。`points` は配分のリスト。SVG 文字列とコアの頂点などを辞書で返す |
| `shrink(game, predicate)` | 反例 (`predicate(game)` が真) のまま、ゲームを小さく単純にする |
| `nucleolus_exact(values, domain, order)` | 有理数だけの逐次 LP による厳密な仁 (10 人まで) |
| `certify_rational(values, x, domain, order)` | 有理数の値 (`Fraction`・`"8/3"`・`"0.1"`) のゲームで厳密に検証 |
| `verify`, `certify` | Kohlberg 基準による検証 (浮動小数点、有理数) |
| `shapley`, `banzhaf`, `normalize`, `shapley_sampling`, `banzhaf_sampling` | Shapley 値・Banzhaf 値。サンプリングの順列数・提携数は 1 以上 (0 なら ValueError) |
| `disruption_nucleolus(game)` | disruption nucleolus (Littlechild & Vaidya 1976、コアが空なら ValueError、16 人まで)。`per_capita_nucleolus` と同じ形の辞書 |
| `anti_prenucleolus(game)`, `anti_nucleolus(game)`, `dual(game)` | anti-prenucleolus・anti-nucleolus (双対ゲームのプレ仁・仁、Funaki & Meinhardt 2006) と双対ゲーム (双対ゲームの値 `v(N) - v(N \ S)` が f64 で表せなければ ValueError)。anti の 2 つは `nucleolus` と同じ形の辞書 |
| `solidarity(game)` | solidarity 値 (Nowak & Radzik 1994) |
| `tau_value(game)`, `utopia_payoffs(game)` | tau 値 (Tijs 1981、準平衡でなければ ValueError)。理想の支払いと最小の権利 |
| `gately_point(game)` | Gately 点 (Gately 1974) |
| `myerson(game, edges)`, `graph_restricted(game, edges)` | 通信グラフ (プレイヤー番号の組の列) のもとでの Myerson 値と、グラフ制限ゲーム |
| `power_indices(game)` | 単純ゲームの Johnston・Deegan–Packel・Public Good (Holler)・Coleman の指数、決定票の数、最小勝利提携 |
| `sampled_nucleolus`, `sampled_least_core` | サンプルした提携による仁・最小コア |
| `talmud`, `constrained_equal_awards`, `constrained_equal_losses` | 破産問題の規則 (`exact=True` で `Fraction`) |

- プレイヤーは 0 始まりの番号で表す。提携はプレイヤー番号のリスト (Python の関数にはタプル) で受け渡す。
- `domain` は `"imputation"` (仁・カーネル) か `"preimputation"` (プレ仁・プレカーネル)。
- 結果は辞書で返す。キーは Rust の結果の構造体のフィールド名と同じ。`guarantee` に保証の種類が入る。
- Python の関数が例外を投げると、計算を打ち切ってその例外をそのまま送出する。
- 入力の誤りは `ValueError`、LP の失敗などは `RuntimeError` になる。
