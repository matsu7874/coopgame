# examples

`examples/` には、使い方の例と、計測・分析のプログラムがある。
名前の接頭辞で分けている。

| 接頭辞 | 内容 |
|---|---|
| なし | 使い方の例。目的ごとに、ゲームの作成から結果の検証までを通して示す |
| `bench_` | 計測。計算時間と規模の関係を測り、CSV を出す |
| `study_` | 分析。手法やゲームの性質を調べ、CSV やテキストを出す |

## 使い方の例

解ごとの短いコード例は [docs/examples.md](../docs/examples.md) にある。
ここの例は、複数のモジュールを組み合わせる手順を、実行できるプログラムとして示す。

| 例 | 目的 | 主に使うモジュール |
|---|---|---|
| `quickstart` | 特性関数を作り、`auto` で仁を求め、結果の保証と手法を読む | `game`、`auto`、`solution`、`games` |
| `custom_game` | 自分の型をゲームにする。小さければ全提携の表にして全ての手法に渡し、大きければサンプリングで近似する | `game`、`values`、`nucleolus::sampled` |
| `nucleolus_methods` | 同じゲームの仁を手法ごとに求め、結果と保証を比べる | `nucleolus` (`exact`・`convex`・`variants`)、`game::exact`、`properties` |
| `large_games` | 100 人規模のゲームを、全提携を列挙せずに解く | `nucleolus::oracle`、`game::oracle`、`auto`、`games` |
| `verification` | 求めた配分を検証する | `verify`、`kernel`、`bargaining`、`properties`、`surplus` |
| `compare_solutions` | 同じゲームでいろいろな解を並べて比べる | `values`、`compromise`、`kernel`、`analysis::explain` |
| `voting_power` | 重み付き投票ゲームの投票力指数 | `power`、`values`、`games::voting` |
| `structured_games` | 構造のあるゲームの専用の解法 | `games` (`airport`・`spanning_tree`・`production`・`cost`・`bankruptcy`) |
| `cooperation_structures` | 提携構造と通信グラフで協力が制限される場合の解 | `partition`、`communication` |
| `analysis` | 配分を説明し、値の不確かさと感度を見て、図を描き、反例を探す | `analysis` (`explain`・`uncertainty`・`plot`・`search`)、`generators` |
| `io` | 名前付きの JSON・CSV を読み、値の欠けたゲームを扱う | `io` |

```bash
cargo run --example quickstart
cargo run --example io --features io     # io は feature `io` が要る
```

例は、[docs/examples.md](../docs/examples.md) と同じ入力 (3 人ゲーム `[0, 0, 0, 4, 6, 8, 12]` など) を使う。
既知の結果は `assert!` で確かめる。`cargo test` が使い方の例を全て実行するので (`Cargo.toml` の `test = true`)、例が壊れればテストが失敗する。
全ての公開モジュールが、どれかの例で使われていることを `tests/examples_coverage.rs` が確かめる。

## 計測と分析

実行の引数と、出力を保存した `data/` のファイルは [docs/reproducibility.md](../docs/reproducibility.md) にある。
各ファイルの冒頭のコメントにも、実行の方法を書いている。

| プログラム | 内容 |
|---|---|
| `bench_exact` | 有理数だけの仁のソルバーの計測 |
| `bench_oracle` | オラクル版の仁の計測 (破産・多数決・重み付き投票) |
| `bench_convex` | 凸ゲームの手法の計測 |
| `bench_auto_threshold` | 逐次 LP と凸ゲームの手法の時間の比較 (`auto` の切り替えの境目の根拠) |
| `study_assumption` | 凸と仮定した手法を一般のゲームに使い、事後検証で分類する |
| `study_counterexamples` | 反例の探索と縮小の実例 |
| `study_data_valuation` | サンプリングした仁の収束と、データ評価への応用 |
| `study_prekernel` | プレカーネル・カーネルの形とゲームの性質の関係 |
| `study_merge_segments` | `study_prekernel` の結果で、同じ直線上の線分をまとめた後の多面体の数 |
