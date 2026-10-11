# 保証の種類と、性質に基づく手法の使い分け

## 結論

- 結果には保証の種類 (`exact`・`proven(性質)`・`assumed(性質)`・`certified`・`approximate`) を付ける。
  性質を宣言しただけで求めた結果は `Unverified` 型で返し、保証付きの結果と取り違えないようにした (コンパイル時に区別される)。
- 凸ゲームの仁を全提携を列挙せずに求める手法を実装した。誘導部分グラフゲーム (構造的に凸) では 64 人で 17 秒だった。
- 凸と仮定して凸でないゲーム 347 個に使うと、88% (307 個) は手法自体が止まらないか数値的に失敗し、
  8% (29 個) は偶然に仁を返し、3% (11 個) は誤った配分を返した。誤った配分は全て事後検証で否定でき、
  検証に合格した誤答は 1 つもなかった。

## 設計

### 保証の種類 (`coopgame::Guarantee`)

| 種類 | 意味 | 例 |
|---|---|---|
| `exact` | 定義どおりに計算した (全提携の LP・総当たり。浮動小数点の許容誤差の範囲で) | `nucleolus::nucleolus`、オラクル版の仁 |
| `proven(性質)` | 性質が分かっているゲームに、その性質のもとで正しい専用手法を使った | 破産ゲームのタルムード則 (`proven(bankruptcy)`)、凸ゲームの手法 (`proven(convex)`) |
| `assumed(性質)` | 利用者が宣言した性質のもとで専用手法を使った。性質が成り立たなければ誤りうる | `nucleolus::convex::nucleolus(&Assume::convex(game))` |
| `certified` | 求めた後に有理数による厳密な検証に合格した | `Unverified::verify` の結果 |
| `approximate` | サンプリングなどの近似、または反復が収束しなかった | Shapley 値の推定、サンプルした仁 |

### 能力の階層 (`coopgame::game`、`coopgame::game::oracle`)

| 能力 | 求める機能 | 使える手法 |
|---|---|---|
| `game::SetFunction` | 提携の値 | `ExplicitGame::tabulate` で全提携の表を作れば全ての手法 (30 人まで)、サンプリング |
| `game::oracle::Separation` | 配分 `x` で超過がしきい値を超える提携を返す (除外する提携を指定できる) | オラクル版の仁 (制約生成) |
| `game::oracle::OracleGame` | 提携を超過の大きい順に返す | 同上。一括実装で自動的に `Separation` も満たす |

オラクル版の仁が要求する能力を `OracleGame` から `Separation` に弱めた。既存の `OracleGame` の型は何も変えずに使える。

### 性質の根拠 (`coopgame::properties`)

| 根拠 | 型 | 証明の種類 | 性質に基づく手法の戻り値 |
|---|---|---|---|
| 型が構造的に持つ | `BankruptcyGame`、`InducedSubgraphGame` (重みは非負) | `Proven` | `Solution` (`proven(convex)`) |
| 計算で確認した | `ConvexChecked::new(game)` (全提携で優モジュラ性を判定) | `Proven` | `Solution` |
| 利用者が宣言した | `Assume::convex(game)` | `Assumed` | `Unverified<Solution>` |

`Unverified<Solution>` の中身を使うには、次のどちらかを呼ぶ必要がある。

- `verify(game, options)` (または有理数のゲームで `verify_exact`): 検証して `Certified`・`Refuted`・`Undecided` に分ける。
- `accept_unverified()`: 検証せずに受け入れると明示する。

構造的な凸性の根拠:

- 破産ゲーム `v(S) = max(0, (E - D) + d(S))` は、加法的な `d(S)` (`d_i >= 0`) に凸で非減少な関数を合成したもので、優モジュラになる。
- 誘導部分グラフゲームでは `v(S ∪ T) + v(S ∩ T) - v(S) - v(T)` が `S \ T` と `T \ S` の間の辺の重みの和になり、重みが非負なら 0 以上になる。
- どちらもテスト (`structural_convexity_claims_hold`) で、ランダムな 30 ゲームずつ全提携の優モジュラ性を確かめた。

### 自動選択 (`coopgame::auto::AutoNucleolus`)

保証のある手法だけから選ぶ。`Assume` で包んだゲームは自動選択の対象にならない。

| 型 | 選ぶ手法 | 保証 |
|---|---|---|
| `ExplicitGame` | 逐次 LP + 制約生成 | `exact` |
| `ConvexChecked` | 16 人以下は逐次 LP、それより多いと凸ゲームの手法 | `exact` / `proven(convex)` |
| `BankruptcyGame` | タルムード則 | `proven(bankruptcy)` |
| `WeightedVotingGame` | オラクルによる制約生成 | `exact` |
| `InducedSubgraphGame` | 凸ゲームの手法 | `proven(convex)` |

16 人の境目は計測で決めた (`data/bench/auto-threshold.csv`、ランダムな凸ゲーム、各 3 ゲームの最大時間)。

| n | 逐次 LP (秒) | 凸ゲームの手法 (秒) | 2 つの配分の差 (`max|v|` で割る) |
|---|---|---|---|
| 12 | 0.0035 | 0.0223 | 5.7e-17 |
| 16 | 0.0292 | 0.0597 | 7.6e-17 |
| 18 | 0.0863 | 0.0679 | 6.6e-12 |
| 20 | 0.3533 | 0.1136 | 7.8e-10 |

## 凸ゲームの手法 (`coopgame::nucleolus::convex`)

### 根拠と手順

文献の 3 つの定理を組み合わせた。

- 凸ゲームのカーネルは仁の 1 点である (Maschler, Peleg & Shapley 1971)。
- 凸ゲームは 0-単調なので、カーネルとプレカーネルは一致する (Maschler, Peleg & Shapley 1979)。
- transfer scheme はプレカーネルの点に収束する (Stearns 1968)。

手順:

1. 等分から始め、組 `(i, j)` を順に回り、最大余剰 `s_ij` と `s_ji` を釣り合わせる移転を行う。
2. 最大余剰 `s_ij(x) = max { v(S) - x(S) : i ∈ S, j ∉ S }` は、凸ゲームでは劣モジュラ関数の最小化になる。
   Fujishige–Wolfe の最小ノルム点法 (`coopgame::submodular`) で、全提携を列挙せずに求める。
3. 1 周の間に移転が起きなければ止める。最後の 1 周で全ての組を停止時の配分で調べているので、
   停止時の配分はプレカーネル条件を (許容誤差 `1e-9 * scale` の範囲で) 満たす。

劣モジュラ最小化は双対ギャップで止める。基多面体の点 `x` の負の成分の和は最小値の下界になるので、
見つけた集合の値と下界の差が許容誤差以下になったら止める。これにより、凸ゲームでは最大余剰の値に証明が付く。
ランダムな劣モジュラ関数 200 個で総当たりの最小値と一致することを確かめた (`submodular::tests`)。

### 当初の計画からの変更

当初は Faigle, Kern & Kuipers (2001) の楕円体法に基づく手法を実装する計画だった。
この環境から論文の本文を取得できないため、上の 3 つの定理と劣モジュラ最小化から導ける手法に変えた。
保証の内容 (凸ゲームでは仁を返す) は同じである。ただし計算量については、反復回数の上限を示していない。
劣モジュラ最小化の最小ノルム点法と transfer scheme の周回数に、多項式の上界を示していないため。

### 計測 (`data/bench/convex-graph.csv`、`data/bench/convex-bankruptcy.csv`、各 3 ゲームの最大)

| ゲーム | n | 時間 (秒) | 周回 | 特性関数の評価回数 | 基準との差 |
|---|---|---|---|---|---|
| 誘導部分グラフ | 16 | 0.048 | 4 | 16 万 | LP の仁と 8.5e-14 |
| 誘導部分グラフ | 24 | 0.34 | 6 | 73 万 | ランダムな 4 組のカーネル条件で反例なし |
| 誘導部分グラフ | 32 | 0.72 | 3 | 109 万 | (基準なし) |
| 誘導部分グラフ | 48 | 4.2 | 4 | 432 万 | (基準なし) |
| 誘導部分グラフ | 64 | 17.2 | 5 | 1,218 万 | (基準なし) |
| 破産 | 20 | 0.60 | 56 | 319 万 | タルムード則と 2.5e-7 |
| 破産 | 40 | 17.5 | 62 | 5,432 万 | タルムード則と 5.8e-7 |

- 誘導部分グラフゲームは 32 人を超えると明示ベクトルで扱えない (2^32 個の値)。この手法なら 64 人でも 17 秒で解ける。
- 破産ゲームは閉じた形 (タルムード則、`O(n log n)`) があるので、自動選択はタルムード則を使う。
  凸ゲームの手法は周回が多く遅い。破産ゲームでの計測は、閉じた形を基準にした正しさの確認のためである。

## 実験: 凸と仮定して一般のゲームに使う (`data/analysis/assumption-study.csv`)

`examples/study_assumption.rs` で、凸ゲームの手法を `Assume::convex` で包んで 6 クラスのゲームに使い、
`Unverified::verify` で検証した。n = 4-10、各クラス・各 n で seed 0-9 (計 420 ゲーム)。
`superadditive` は有理数で構築したゲーム (`random_superadditive_exact`) を使い、有理数の値で検証した。

| クラス | ゲーム数 | 実際に凸 | 合格 (仁) | 否定 | 手法が失敗 |
|---|---|---|---|---|---|
| convex | 70 | 70 | 70 | 0 | 0 |
| bnf1 | 70 | 0 | 1 | 1 | 68 |
| bnf2 | 70 | 0 | 0 | 2 | 68 |
| bnf4 | 70 | 0 | 0 | 1 | 69 |
| superadditive | 70 | 3 | 10 | 1 | 59 |
| voting | 70 | 0 | 21 | 6 | 43 |

読み取れること:

- 凸ゲーム 73 個 (convex 70、superadditive のうち凸の 3) は全て合格した。
- 凸でないゲーム 347 個では、307 個 (88%) で手法が止まらないか数値的に失敗した。
  内訳は、劣モジュラ最小化が進まない・退化する (numerical) が 198 個、周回・反復の上限 (limit) が 109 個。
  劣モジュラでない関数では最小ノルム点法の前提が崩れるためである。失敗はエラーとして返るので安全である。
- 凸でないのに仁を返したのは 29 個で、大半は小さい n (n = 4 が 16 個) だった。
- 誤った配分を返した 11 個は、全て事後検証で否定できた。
  - 3 個はプレ仁 (配分集合の外) に収束し、個人合理性を満たさないとして否定された。
  - 1 個はプレカーネルの点だが仁ではなかった (voting、n = 10)。プレカーネルが 1 点でないゲームでは、
    「カーネル = 仁」の定理が成り立たないためである。
  - 7 個はプレカーネル条件も満たしていなかった (劣モジュラ最小化が最大余剰を見誤った)。
- 検証に合格した結果は、全て LP の仁との差が 7.4e-10 以下だった。誤った配分が合格した例はない。

## 有理数で構築したゲーム

浮動小数点数の和で作ったゲーム (`random_superadditive`) では、有理数では等しいはずの値の間に丸め誤差が残る。
そのため、LP で求めた仁でも厳密な検証 (有理数の Kohlberg 判定) に合格しないことがある
(例: n = 4, seed 0。テスト `rational_construction_makes_certification_possible`)。次の入口で、値を有理数のまま扱える。

| 入口 | 内容 |
|---|---|
| `game::exact::ExactGame::from_binary` / `from_lex` | 有理数の値からゲームを作る。`to_explicit` で浮動小数点数のゲームにして LP などに渡す |
| `verify::certify_exact`、`verify::check_exact`、`Unverified::verify_exact` | 有理数のゲームの値で検証する |
| `generators::random_superadditive_exact` | `random_superadditive` と同じ乱数で、和と最大値を有理数で計算する |
| `game::exact::parse_rational_values`、CLI の `certify --rational` | `0.1` を `1/10` のように 10 進数・分数の値どおりに読む |
| Python の `certify_rational(values, x)` | 値を `Fraction`・`"8/3"`・`"0.1"` で渡す |

実験の superadditive クラスを有理数で構築したゲームに替えると、浮動小数点数で構築したときに「未決」だった 6 個が全て合格した。

## 検証の範囲 (`coopgame::verify`)

| 人数 | 方法 | 結論 |
|---|---|---|
| 20 人以下 | 全提携の表と有理数の Kohlberg 判定。不合格なら LP の仁と比べる | 合格 / 否定 / (検証自体が成り立たないときだけ) 未決 |
| 21-26 人 | ランダムな組のカーネル条件を総当たり (1 組 `2^(n-2)` 回の評価) | 否定 / 未決 (合格にはできない) |
| 27 人以上 | なし | 未決 |

仁はカーネルに、プレ仁はプレカーネルに属する (Davis & Maschler 1965、Maschler, Peleg & Shapley 1979) ので、
カーネル条件の破れは否定の根拠になる。

## 再現

```bash
cargo build --release --examples
target/release/examples/study_assumption 4,5,6,7,8,9,10 10 > data/analysis/assumption-study.csv   # 約 20 秒
target/release/examples/bench_convex graph 10,16,24,32,48,64 3 > data/bench/convex-graph.csv
target/release/examples/bench_convex bankruptcy 10,20,30,40 3 > data/bench/convex-bankruptcy.csv
target/release/examples/bench_auto_threshold 8,10,12,14,16,18,20 3 > data/bench/auto-threshold.csv
python3 scripts/verify_claims.py
```

`seconds` 以外の列は seed で決まる。
