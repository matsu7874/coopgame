# 大きいゲーム

全提携の値を並べる `ExplicitGame` は、時間とメモリが `2^n` に比例するため 30 人程度が上限である。
それより大きいゲームは、ゲームの構造に応じて次の方法で扱う。

| 方法 | モジュール | 使える場面 | 保証 |
|---|---|---|---|
| オラクル | `oracle` | 超過の大きい順に提携を列挙できるゲーム | 厳密 (計算量の保証なし) |
| 凸ゲームの手法 | `convex` | 凸ゲーム | 厳密 |
| 閉じた形の解 | `games::bankruptcy`、`games::airport` など | 破産ゲーム、空港ゲームなど特定のクラス | 厳密 |
| 提携のサンプリング | `sampled`、`values::shapley_sampling` | 人数の多いゲーム (データ評価など) | 近似 |

どの手法が使えるかをゲームの型から選ぶには `auto::AutoNucleolus` を使う。
保証の種類と、性質を仮定した場合の扱いは [保証の種類と、性質に基づく手法の使い分け](guarantees.md) を参照。

## オラクル

全提携を列挙しないゲームは、`game::oracle::OracleGame` を実装して扱う。
求める機能は、提携の値と「配分 `x` に対して真部分提携を超過の大きい順に返す」ことだけである。
人数の上限はない (提携は任意長のビット集合 `game::PlayerSet`)。

仁・プレ仁・最小コアは `nucleolus::oracle::{nucleolus, prenucleolus, least_core}` で求める。
破産ゲーム (`games::bankruptcy`) と重み付き投票ゲーム (`games::voting`) のオラクルを用意している。

小さいオラクルのゲームは、`ExplicitGame::tabulate` で全提携の表 (`ExplicitGame`) に直せば、明示ベクトル向けの全ての関数を使える
([チュートリアル 2 章](tutorial/02-voting-power.md) に例がある)。

## 凸ゲーム

凸ゲームではカーネルと仁が一致する。
`nucleolus::convex::nucleolus` はこれを使い、最大余剰を劣モジュラ最小化で求める transfer scheme で仁を求める。
誘導部分グラフゲームで 64 人を 17 秒で解いた。

## サンプリング

- Shapley 値・Banzhaf 値は、`values::{shapley_sampling, banzhaf_sampling}` で任意人数のゲームから標準誤差付きで推定できる。
- 仁・最小コアは、`nucleolus::sampled::{nucleolus, least_core}` でサンプルした提携だけを使って近似できる。
  仁は少数の提携で決まるため、その提携をサンプルするまで誤差が残る。

各方法の計測結果と規模の上限は [性能と制約](performance.md) にまとめている。
