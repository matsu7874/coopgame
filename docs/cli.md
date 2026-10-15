# CLI

`coopgame` コマンドは feature `cli` を有効にしてインストールする。

```bash
cargo install coopgame --features cli
```

入力ファイルの形式は [入力形式](input-format.md) を参照。
以下の例の `v.txt` は特性関数、`sol.txt` は配分 (1 行 1 値) である。

プレイヤーは入力・出力・オプションのすべてで 0 始まりの番号で表す。
配分は 1 行 1 値で、`i` 行目 (0 始まり) がプレイヤー `i` の値である。
提携は `{0, 2}` の形で出す。

## 例

3 人ゲーム `v({0,1}) = 4`、`v({0,2}) = 3`、`v({1,2}) = 2`、`v(N) = 6` (1 人提携は 0) を `v.txt` にビット順で書く。

```text
0
0
4
0
3
2
6
```

```console
$ coopgame nucleolus v.txt > sol.txt
time=0.000166s lp_solves=3 rows_added=0 levels=[-1.0]
$ cat sol.txt
3
2
1
$ coopgame explain v.txt sol.txt
配分:
  プレイヤー 0: 3.000000 (単独の値 0.000000、協力による増分 +3.000000)
  プレイヤー 1: 2.000000 (単独の値 0.000000、協力による増分 +2.000000)
  プレイヤー 2: 1.000000 (単独の値 0.000000、協力による増分 +1.000000)

安定性:
  最大の不満 (超過) は -1.000000。どの提携も離脱して得をしないので、コアに属する。
  最小コアの値は -1.000000。最大の不満をこれより下げられる配分はない (この配分は最大の不満を最小にしている)。

不満の大きい提携 (超過の大きい順の段):
  段 1: 超過 -1.000000 の提携 {0, 1} {2} {0, 2} {1, 2}
  段 2: 超過 -2.000000 の提携 {1}
  段 3: 超過 -3.000000 の提携 {0}

プレイヤーごとの最も不満な提携:
  プレイヤー 0: 含む提携 {0, 1} (超過 -1.000000)、含まない提携 {2} (超過 -1.000000)
  プレイヤー 1: 含む提携 {0, 1} (超過 -1.000000)、含まない提携 {2} (超過 -1.000000)
  プレイヤー 2: 含む提携 {2} (超過 -1.000000)、含まない提携 {0, 1} (超過 -1.000000)
$ coopgame structure v.txt --blocks "0,1|2"
player	aumann_dreze	structured_nucleolus	owen
0	2	2.5	2.75
1	2	1.5	2.25
2	0	0	1
```

統計 (`time=` など) は標準エラーに出る。`time` の値は実行ごとに変わる。

## 解を求める

```bash
coopgame nucleolus v.txt > sol.txt          # 仁 (オプションは下の表)
coopgame least-core v.txt                   # 最小コア
coopgame kernel v.txt                       # カーネルの 1 点 (--pre でプレカーネル)
coopgame kernel-set v.txt                   # カーネル全体を多面体ごとの頂点で出す (--merge で線分をまとめる)
coopgame shapley v.txt                      # Shapley 値 (--samples N でサンプリング推定)
coopgame banzhaf v.txt --normalize          # 正規化 Banzhaf 指数
coopgame disruption v.txt                   # disruption nucleolus (コアが空でないゲーム)
coopgame anti-nucleolus v.txt               # anti-nucleolus (--pre で anti-prenucleolus)
coopgame solidarity v.txt                   # solidarity 値
coopgame tau v.txt                          # tau 値 (準平衡でなければエラー)
coopgame gately v.txt                       # Gately 点
coopgame myerson v.txt --edges 0-1,1-2      # 通信グラフ (プレイヤーの番号の組) のもとでの Myerson 値
coopgame power v.txt                        # 単純ゲームの投票力指数の表 (Shapley–Shubik・Banzhaf・Johnston・Deegan–Packel・Public Good・Coleman)
coopgame per-capita v.txt                   # per capita 仁 (proportional で比例仁、modiclus で modiclus)
coopgame structure v.txt --blocks "0,1|2"   # 提携構造の Aumann–Drèze 値・提携構造つきの仁・Owen 値
coopgame convex-nucleolus v.txt             # 凸ゲームの手法で仁 (凸でなければエラー。--assume で凸と仮定して試し、事後検証する)
```

`nucleolus` の主なオプション:

| オプション | 内容 |
|---|---|
| `--pre` | プレ仁を求める |
| `--cost` | 費用ゲームとして読み、仁による費用の分担を出す (JSON・CSV の `type` が `cost` なら付けなくてよい) |
| `--exact` | 値を有理数として読み、有理数だけで仁を求めて分数で出す (JSON・CSV の数も字面どおり。`--full` とは併用できない) |
| `--full` | 全提携の行を最初から入れる LP で解く (制約生成との比較用) |
| `--missing zero\|ignore` | 値が分からない提携の扱い ([入力形式](input-format.md#値が分からない提携)) |

`least-core`・`kernel`・`shapley`・`banzhaf`・`compare` も `--cost` で費用ゲームを扱う ([費用ゲーム](input-format.md#費用ゲーム))。
他のコマンドに費用のゲーム (`type` が `cost`) を渡すとエラーにする。

## 特定のゲームを解く

```bash
coopgame talmud --estate 100 --claims 100,200,300 --exact   # 破産ゲームの仁 (タルムード則) を分数で出す
coopgame airport --costs 1,2,3,6                            # 空港ゲームの費用分担 (Shapley 値と仁)
```

```console
$ coopgame airport --costs 1,2,3,6
player	cost	shapley	nucleolus
0	1	0.25	0.5
1	2	0.5833333333333333	0.75
2	3	1.0833333333333333	0.875
3	6	4.083333333333333	3.875
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
coopgame explain v.txt sol.txt              # 不満の大きい提携、安定性、プレイヤーごとの不満
coopgame compare v.txt                      # 仁・プレ仁・Shapley 値・per capita 仁・modiclus を安定性で比べる
coopgame influence v.txt                    # 配分を決めている提携と、その値に対する感度
coopgame uncertainty v.txt --relative 0.1 --samples 200   # 各値 ±10% の誤差での配分の分布
coopgame plot v.txt > figure.svg            # 3-4 人の配分集合の図 (SVG)。頂点などの表を標準エラーに出す
```

上の例の `v.txt` での `compare` と `influence` の出力:

```console
$ coopgame compare v.txt
解	プレイヤー 0	プレイヤー 1	プレイヤー 2	最大の不満	コア	離脱で得をする提携の数
0	3.000000	2.000000	1.000000	-1.000000	属する	0
1	3.000000	2.000000	1.000000	-1.000000	属する	0
2	2.500000	2.000000	1.500000	-0.500000	属する	0
3	3.000000	2.000000	1.000000	-1.000000	属する	0
4	2.333333	2.333333	1.333333	-0.666667	属する	0

解の番号:
0	仁
1	プレ仁
2	Shapley 値
3	per capita 仁
4	modiclus
$ coopgame influence v.txt
coalition	max|dx/dv|	dx/dv (プレイヤー順)
{0, 1}	0.5833	0.2917 0.2917 -0.5833
{0, 2}	0.5833	0.4167 -0.5833 0.1667
{1, 2}	0.5833	-0.5833 0.4167 0.1667
{0, 1, 2}	0.4167	0.2917 0.2917 0.4167
{2}	0.2500	-0.1250 -0.1250 0.2500
{1}	0.0000	0.0000 0.0000 -0.0000
```

`plot` が標準エラーに出す表は、行頭がその点の種類 (`core_vertex`・`kernel_piece_0` など・`nucleolus`・`shapley`)、続く値がプレイヤー 0, 1, ... の順の配分である。

```console
$ coopgame plot v.txt > figure.svg
core_vertex	4 0 2
core_vertex	1 3 2
core_vertex	3 3 0
core_vertex	4 2 0
kernel_piece_0	3 2 1
nucleolus	3 2 1
shapley	2.5 2 1.5
```

## ゲームの生成と計測

```bash
coopgame generate --type 1 --n 5 --seed 1 > v.txt   # BNF タイプ 1 のゲーム
coopgame bench --types 1,2,3,4,5 --n 4..12 --seeds 3 > bench.csv
```

`bench` は `type,n,seed,method,seconds,lp_solves,rows_added,iterations,verified,violation,error` の CSV を出す。
`--methods` には `nucleolus`, `nucleolus-full`, `prenucleolus`, `prenucleolus-full`, `kernel`, `prekernel` を指定できる。
仁の `verified` は Kohlberg 基準、カーネルの `verified` は収束したかどうか。
