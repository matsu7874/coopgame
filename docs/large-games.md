# 大きいゲーム

全提携の値を並べる `ExplicitGame` は、時間とメモリが `2^n` に比例するため 30 人程度が上限である。
それより大きいゲームは、ゲームの構造に応じて次の方法で扱う。

| 方法 | モジュール | 使える場面 | 保証 |
|---|---|---|---|
| オラクル | `oracle` | 超過の大きい順に提携を列挙できるゲーム | 厳密 (計算量の保証なし)。一部の提携だけを扱うオラクルでは近似 |
| 凸ゲームの手法 | `convex` | 凸ゲーム | 厳密 |
| 閉じた形の解 | `games::bankruptcy`、`games::airport` など | 破産ゲーム、空港ゲームなど特定のクラス | 厳密 |
| 提携のサンプリング | `sampled`、`values::shapley_sampling` | 人数の多いゲーム (データ評価など) | 近似 |

どの手法が使えるかをゲームの型から選ぶには `auto::AutoNucleolus` を使う。
全提携の表 (`ExplicitGame`) の仁は、既定の設定では 21 人以上で Kohlberg 基準を確かめないので、保証が `approximate` になる
([性能と制約](performance.md) の「仁の許容誤差の決め方」)。
保証の種類と、性質を仮定した場合の扱いは [保証の種類と、性質に基づく手法の使い分け](guarantees.md) を参照。

## オラクル

全提携を列挙しないゲームは、`game::oracle::OracleGame` を実装して扱う。
求める機能は、提携の値と「配分 `x` に対して真部分提携を超過の大きい順に返す」ことだけである。
人数の上限はない (提携は任意長のビット集合 `game::PlayerSet`)。

仁・プレ仁・最小コアは `nucleolus::oracle::{nucleolus, prenucleolus, least_core}` で求める。
破産ゲーム (`games::bankruptcy`) と重み付き投票ゲーム (`games::voting`) のオラクルを用意している。

仁の制約生成は、超過が確定した提携 (確定した提携の特性ベクトルが張る空間に入る提携) を違反の探索から除く。
超過順に列挙するだけのオラクルは、確定済みの提携を 1 つずつ読み飛ばすので、確定済みの提携が超過の上位を占めるゲーム
(仁が 2 段以上で決まるゲーム) では読む数が指数的に増えうる。
分離オラクル `Separation` の `violated_outside_span` (`OracleGame` では `search_outside_span`) は確定済みの提携を受け取るので、
確定済みの空間を使って探せるゲームはこれを上書きする。重み付き投票ゲームはこれを実装しており、
拒否権のある 60 人のゲームを 1.4 秒で解く (修正前は 24 人で 27 秒・3.7 GB。[性能と制約](performance.md#拒否権のある重み付き投票ゲーム))。
上書きしなければ、従来どおり超過の順に探す。

一部の提携しか扱わないオラクル (サンプルした提携だけ、値が分かる提携だけなど) は、`OracleGame::lists_all_coalitions`
(分離オラクル `Separation` では `covers_all_coalitions`) で `false` を返す。
このとき求まるのは扱った提携に限ったゲームの仁なので、結果の保証は近似 (`approximate`) になる。

小さいオラクルのゲームは、`ExplicitGame::tabulate` で全提携の表 (`ExplicitGame`) に直せば、明示ベクトル向けの全ての関数を使える
([チュートリアル 2 章](tutorial/02-voting-power.md) に例がある)。

## 凸ゲーム

凸ゲームではカーネルと仁が一致する。
`nucleolus::convex::nucleolus` はこれを使い、最大余剰を劣モジュラ最小化で求める transfer scheme で仁を求める。

使えるのは `properties::ConvexGame` を満たす型で、構造的に凸な型 (破産・誘導部分グラフ・空港ゲーム)、
凸であることを計算で確かめた `properties::ConvexChecked`、凸と宣言した `properties::Assume::convex` に限る。
利用者が定義したゲームは `Assume::convex` で包み、結果を検証して使う ([保証の種類と、性質に基づく手法の使い分け](guarantees.md))。

## サンプリング

- Shapley 値・Banzhaf 値は、`values::{shapley_sampling, banzhaf_sampling}` で任意人数のゲームから標準誤差付きで推定できる。
- 仁・最小コアは、`nucleolus::sampled::{nucleolus, least_core}` でサンプルした提携だけを使って近似できる。

各方法の計測結果、規模の上限、近似の誤差は [性能と制約](performance.md) にまとめている。
