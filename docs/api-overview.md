# 機能一覧

coopgame のモジュールと関数、使っている手法の一覧である。
各関数の詳しい説明は [docs.rs](https://docs.rs/coopgame) を参照。

LP ソルバーは純 Rust の [microlp](https://crates.io/crates/microlp) を使う。外部の商用ソルバーは不要。

## モジュールの構成

| 目的 | モジュール |
|---|---|
| ゲームの表現 | `game` (提携、特性関数、全提携の表)、`game::exact` (有理数のゲーム)、`game::oracle` (オラクルの能力) |
| 特定のクラスのゲーム | `games` (`bankruptcy`・`airport`・`spanning_tree`・`production`・`voting`・`graph`・`cost`) |
| 手法の自動選択 | `auto` |
| 解 | `nucleolus` (下に `exact`・`oracle`・`convex`・`sampled`・`variants`)、`kernel`、`values`、`compromise`、`partition`、`communication`、`power` |
| 検証と性質 | `verify`、`bargaining`、`properties`、`solution` (結果と保証の種類) |
| 分析 | `analysis` (`explain`・`uncertainty`・`plot`・`search`)、`surplus` |
| 入出力と生成 | `io` (feature `io`)、`generators` |

同じ解を別の手法で求めるものは、解のモジュールの下のサブモジュールに置いている。

モジュール間の依存の向きは [CONTRIBUTING.md](../CONTRIBUTING.md) にまとめている。

## 解を求める

| 対象 | 関数 | 手法 |
|---|---|---|
| 仁・プレ仁 | `nucleolus::nucleolus`, `nucleolus::prenucleolus`, `nucleolus::nucleolus_with` | 逐次 LP (Kopelowitz 方式) + 制約生成 |
| 最小コア | `nucleolus::least_core` | LP 1 回 |
| 仁・プレ仁の厳密な計算 (10 人まで) | `nucleolus::exact::nucleolus` | 逐次 LP を有理数の 2 段階単体法 (Bland の規則) と制約生成で解く。許容誤差を使わない |
| カーネル・プレカーネルの 1 点 | `kernel::kernel_point` | Maschler/Stearns の transfer scheme |
| カーネル・プレカーネル全体 (6 人まで) | `kernel::kernel_set`、`KernelSet::merge_collinear_segments` | 最大余剰を与える提携の場合分けを LP で枝刈りしながら探索し、多面体の和集合として返す。同じ直線上でつながる線分は 1 本にまとめられる |
| Shapley 値・Banzhaf 値 | `values::{shapley, banzhaf, shapley_sampling, banzhaf_sampling, normalize}` | 厳密計算 (`O(n 2^n)`) と、任意人数のゲームでのサンプリング推定 (標準誤差付き) |
| solidarity 値 | `values::solidarity` | 定義どおりに全提携を走査 (`O(n 2^n)`)。限界貢献の代わりに提携内の限界貢献の平均を使う (Nowak & Radzik 1994) |
| tau 値・Gately 点 | `compromise::{tau_value, gately_point, utopia_payoffs, minimal_rights}` | 理想の支払い `M_i = v(N) - v(N \ {i})` と最小の権利から閉じた形で求める。tau 値は準平衡なゲームだけで定義し、それ以外はエラー (Tijs 1981、Gately 1974) |
| Myerson 値 | `communication::{myerson, graph_restricted}` | 通信グラフで制限したゲーム (連結成分ごとの値の和) の Shapley 値 (Myerson 1977) |
| 投票力指数 | `power::SimpleGame::{johnston, deegan_packel, public_good, coleman_prevent, coleman_initiative, coleman_collectivity}` | 単純ゲームの勝利提携・最小勝利提携・決定票を数えて求める。Shapley–Shubik 指数と Banzhaf 指数は `values` |
| per capita 仁・比例仁・modiclus | `nucleolus::variants::{per_capita_nucleolus, proportional_nucleolus, modiclus}` | 不満をアフィン関数に一般化した逐次 LP (`nucleolus::variants::lexicographic_minimum`)。modiclus は 7 人まで |
| disruption nucleolus | `nucleolus::variants::disruption_nucleolus` | 抜ける傾向を、コアの上で `e(S, x) / (v(N) - v(S) - v(N \ S))` の辞書式最小化に直して同じ逐次 LP で解く (Littlechild & Vaidya 1976)。コアが空でないゲームに限る |
| anti-prenucleolus・anti-nucleolus | `nucleolus::variants::{anti_prenucleolus, anti_nucleolus}`、`ExplicitGame::dual` | 超過を小さい順に辞書式に最大化する解。双対ゲーム `v*(S) = v(N) - v(N \ S)` のプレ仁・仁として求める (Funaki & Meinhardt 2006) |
| 提携構造・事前の連合 | `partition::{CoalitionStructure, aumann_dreze, owen, nucleolus, quotient_game}` | Aumann–Drèze 値 (ブロック内の Shapley 値)、提携構造つきの仁 (各ブロックで `x(B) = v(B)`)、Owen 値 (事前の連合) |
| 超過・最大余剰 `s_ij` | `surplus::excesses`, `surplus::max_surplus` | 超過の上位の提携だけを部分ソートして走査 |

## 結果を検証する

| 対象 | 関数 | 手法 |
|---|---|---|
| 仁・プレ仁の検証 | `verify::kohlberg` | Kohlberg 基準 |
| 仁・プレ仁の厳密な検証 | `verify::{certify, certify_exact, kohlberg_exact, recover_allocation}` | 超過の段から厳密な配分を有理数で復元し、有理数の単体法 (Bland の規則) で Kohlberg 基準を判定。`game::exact::ExactGame` で値を有理数のまま構築できる |
| カーネル・プレカーネルの検証 | `kernel::is_in_kernel`, `kernel::kernel_violation` | 最大余剰の釣り合い条件 |
| 交渉集合・プレ交渉集合への所属 | `bargaining::{check, is_in_bargaining_set}` | 組 `(i, j)` と提携 `S` ごとに、反論のない異議があるかを LP で判定。異議があればその支払いを返す |
| ゲームの性質 | `properties::{is_superadditive, is_convex, is_zero_monotonic, is_in_core}`、`nucleolus::has_nonempty_core` | 定義どおりの判定、コアが空でないかは最小コアの LP |
| 結果の事後検証 | `verify::{check, check_exact}`、`Unverified::verify` | 20 人以下は有理数の厳密な検証、26 人以下は組ごとのカーネル条件で否定 |
| 保証のある手法の自動選択 | `auto::AutoNucleolus` | ゲームの型から、保証のある手法のうち最も速いものを選ぶ |

結果に付く保証の種類 (`exact`・`proven`・`assumed`・`certified`・`approximate`) は [保証の種類と、性質に基づく手法の使い分け](guarantees.md) を参照。

## 大きいゲームを扱う

| 対象 | 関数 | 手法 |
|---|---|---|
| 仁・プレ仁・最小コア (オラクル) | `nucleolus::oracle::{nucleolus, prenucleolus, least_core}` | 超過の大きい順に提携を返すオラクルで制約生成。破産ゲーム (`games::bankruptcy`)・重み付き投票ゲーム (`games::voting`) のオラクルを用意 |
| 凸ゲームの仁 | `nucleolus::convex::nucleolus` | カーネル = 仁 (凸ゲーム) を使い、最大余剰を劣モジュラ最小化で求める transfer scheme |
| サンプリングした提携による仁・最小コア | `nucleolus::sampled::{nucleolus, least_core, SampledGame}` | 提携とその補集合を組でサンプルし、サンプルした提携だけで逐次 LP を解く。全提携をサンプルすれば真の仁に一致 |

使い分けは [大きいゲーム](large-games.md) を参照。

## 特定のゲームのクラス

| 対象 | 関数 | 手法 |
|---|---|---|
| 破産問題の配分規則 | `games::bankruptcy::{talmud_rule, constrained_equal_awards, constrained_equal_losses}`、`games::bankruptcy::BankruptcyGame::nucleolus` | 閉じた形の解 (`O(n log n)`)。タルムード則は破産ゲームの仁に一致する。`f64` と有理数のどちらでも計算できる |
| 費用ゲーム | `games::cost::CostGame` | 費用 `c(S)` を節約ゲームに直して仁を求め、費用の分担に戻す |
| 空港ゲーム | `games::airport::AirportGame` | Shapley 値は Littlechild–Owen の式 (`O(n log n)`)、仁は節約ゲームが凸なので凸ゲームの手法 |
| 最小全域木ゲーム | `games::spanning_tree::SpanningTreeGame` | Bird 規則 (Prim 法、コアに属する)。仁は一般に NP 困難なので全提携の表の逐次 LP |
| 線形生産ゲーム | `games::production::LinearProductionGame` | 双対 LP の影の価格による Owen 配分 (コアに属する) |
| 重み付き投票ゲーム | `games::voting::WeightedVotingGame` | オラクル。`ExplicitGame::tabulate` で全提携の表に直せる |

## 分析と可視化

| 対象 | 関数 | 手法 |
|---|---|---|
| 配分の説明・比較 | `analysis::explain::{report, render, compare}` | 不満 (超過) の大きい提携の段、コアに属するか、最小コアの値、プレイヤーごとの最も不満な提携。複数の配分を安定性で比べる |
| 値の不確かさの分析 | `analysis::uncertainty::{monte_carlo, IntervalGame, interval_monte_carlo, influence, key_coalitions}` | 値の分布・区間から配分の分布 (平均・標準偏差・分位点) を求め、どの提携の値が配分を決めているかを感度 `dx/dv(S)` で示す |
| 配分集合の図 | `analysis::plot::{imputation_figure, PlotOptions, core_vertices}` | 3-4 人のゲームの配分集合 (三角形・四面体) にコア・カーネル・仁・Shapley 値・任意の点を描いた SVG。ライト・ダークの両方に対応 |
| 反例の探索と縮小 | `analysis::search::{search, shrink}` | 性質を満たさないゲームをランダムに探し、プレイヤーの除去・値の単純化で反例を小さくする。実例は `examples/counterexample_search.rs`・`data/analysis/counterexamples.txt` |

## 入出力とゲームの生成

| 対象 | 関数 | 手法 |
|---|---|---|
| プレイヤー名付きの入出力 (feature `io`) | `io::{parse_json, parse_csv, GameData, PartialGame}` | JSON・CSV の読み書き。値が分からない提携はエラー・0・分かる提携だけで解く、から選ぶ ([入力形式](input-format.md)) |
| ゲーム生成 | `generators::{bnf, bankruptcy, weighted_voting, random_convex, random_superadditive, random_weighted_voting}` | BNF 実装と同じ分布など |

## 保証の種類ごとのアルゴリズム

ゲームの構造によって、仁をより強い保証で速く求めるアルゴリズムがある。
このライブラリは、保証が成り立つと確認できたものを実装し、構造ごとに切り替えて使えるようにする。

| ゲームのクラス | 保証 | アルゴリズム | 状態 |
|---|---|---|---|
| 一般のゲーム (明示ベクトル) | 厳密 (時間は `2^n` に比例) | 逐次 LP + 制約生成 (`nucleolus`) | 実装済み |
| 超過の大きい順に提携を列挙できるゲーム | 厳密 (計算量の保証なし) | 逐次 LP + オラクルによる制約生成 (`nucleolus::oracle`) | 実装済み |
| 破産ゲーム | 閉じた形、`O(n log n)` | タルムード則 (Aumann & Maschler 1985) | 実装済み (`games::bankruptcy::talmud_rule`) |
| 凸ゲーム | 厳密 (計算量の上界は示していない) | カーネル = 仁 + 劣モジュラ最小化による transfer scheme | 実装済み (`nucleolus::convex::nucleolus`)。Faigle, Kern & Kuipers (2001) の楕円体法は本文を確認できず未実装 |
| 重み付き投票ゲーム | 重みについて擬多項式時間 | Pashkovich (2022) | 未実装 |
| 重み付きマッチングゲーム | 多項式時間 | Könemann, Pashkovich, Toth (2020) | 未実装 |
| 動的計画法で特性関数を書けるゲーム | 枠組み | Könemann & Toth (2020) | 未実装 |

未実装の 3 つは、原論文の本文で保証を確かめられていないため実装していない (方針は [CONTRIBUTING.md](../CONTRIBUTING.md))。
文献は [references.md](references.md) を参照。
