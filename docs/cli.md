# CLI

`coopgame` コマンドは feature `cli` を有効にしてインストールする。

```bash
cargo install coopgame --features cli
```

入力ファイルの形式は [入力形式](input-format.md) を参照。
以下の例の `v.txt` は特性関数、`sol.txt` は配分 (1 行 1 値) である。

## 解を求める

```bash
coopgame nucleolus v.txt > sol.txt          # 仁 (--pre でプレ仁)
coopgame least-core v.txt                   # 最小コア
coopgame kernel v.txt                       # カーネルの 1 点 (--pre でプレカーネル)
coopgame kernel-set v.txt                   # カーネル全体を多面体ごとの頂点で出す (--merge で線分をまとめる)
coopgame shapley v.txt                      # Shapley 値 (--samples N でサンプリング推定)
coopgame banzhaf v.txt --normalize          # 正規化 Banzhaf 指数
coopgame per-capita v.txt                   # per capita 仁 (proportional で比例仁、modiclus で modiclus)
coopgame structure v.txt --blocks "1,2|3,4,5"   # 提携構造の Aumann–Drèze 値・提携構造つきの仁・Owen 値
coopgame convex-nucleolus v.txt             # 凸ゲームの手法で仁 (凸でなければエラー。--assume で凸と仮定して試し、事後検証する)
```

`nucleolus` の主なオプション:

| オプション | 内容 |
|---|---|
| `--pre` | プレ仁を求める |
| `--cost` | 費用ゲームとして読み、仁による費用の分担を出す |
| `--exact` | 値を有理数として読み、有理数だけで仁を求めて分数で出す |
| `--full` | 全提携の行を最初から入れる LP で解く (制約生成との比較用) |
| `--missing zero\|ignore` | 値が分からない提携の扱い ([入力形式](input-format.md#値が分からない提携)) |

## 特定のゲームを解く

```bash
coopgame talmud --estate 100 --claims 100,200,300 --exact   # 破産ゲームの仁 (タルムード則) を分数で出す
coopgame airport --costs 1,2,3,6                            # 空港ゲームの費用分担 (Shapley 値と仁)
```

## 結果を検証する

```bash
coopgame verify v.txt sol.txt               # Kohlberg 基準とカーネル条件
coopgame certify v.txt sol.txt              # 有理数による厳密な検証 (厳密な仁を分数で出す)
coopgame certify v.txt sol.txt --rational   # v.txt の値を 10 進数・分数の値どおりの有理数として検証する
coopgame bargaining v.txt sol.txt           # 交渉集合に属するか (属さなければ反論のない異議を出す)
```

## 配分を説明・比較する

```bash
coopgame explain v.txt sol.txt --names A,B,C,D,E   # 不満の大きい提携、安定性、プレイヤーごとの不満
coopgame compare v.txt --names A,B,C,D,E           # 仁・プレ仁・Shapley 値・per capita 仁・modiclus を安定性で比べる
coopgame influence v.txt                           # 配分を決めている提携と、その値に対する感度
coopgame uncertainty v.txt --relative 0.1 --samples 200   # 各値 ±10% の誤差での配分の分布
coopgame plot v.txt --names A,B,C > figure.svg     # 3-4 人の配分集合の図 (SVG)。頂点などの表を標準エラーに出す
```

## ゲームの生成と計測

```bash
coopgame generate --type 1 --n 5 --seed 1 > v.txt   # BNF タイプ 1 のゲーム
coopgame bench --types 1,2,3,4,5 --n 4..12 --seeds 3 > bench.csv
```

`bench` は `type,n,seed,method,seconds,lp_solves,rows_added,iterations,verified,violation,error` の CSV を出す。
`--methods` には `nucleolus`, `nucleolus-full`, `prenucleolus`, `prenucleolus-full`, `kernel`, `prekernel` を指定できる。
仁の `verified` は Kohlberg 基準、カーネルの `verified` は収束したかどうか。
