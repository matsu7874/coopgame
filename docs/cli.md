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
time=0.000166s lp_solves=3 rows_added=0 solver=lp tolerance=6e-7 refinements=0 guarantee=exact levels=[-1.0]
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
  (コアの判定は、ゲームの値と配分を 2 進数の値どおりに読んだ厳密な判定。
  許容誤差を指定すると、その幅まで許して判定する。)
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
coopgame shapley v.txt                      # Shapley 値 (--samples N でサンプリング推定。--seed S は --samples と併用する)
coopgame banzhaf v.txt --normalize          # 正規化 Banzhaf 指数
coopgame disruption v.txt                   # disruption nucleolus (コアが空でないゲーム)
coopgame anti-nucleolus v.txt               # anti-nucleolus (--pre で anti-prenucleolus)
coopgame solidarity v.txt                   # solidarity 値
coopgame tau v.txt                          # tau 値 (準平衡でなければエラー)
coopgame gately v.txt                       # Gately 点
coopgame myerson v.txt --edges 0-1,1-2      # 通信グラフ (プレイヤーの番号の組) のもとでの Myerson 値 (辺のないグラフは --edges "")
coopgame power v.txt                        # 単純ゲームの投票力指数の表 (Shapley–Shubik・Banzhaf・Johnston・Deegan–Packel・Public Good・Coleman)
coopgame per-capita v.txt                   # per capita 仁 (proportional で比例仁、modiclus で modiclus)。既定は配分集合の版で、原典の準配分の版は --pre
coopgame structure v.txt --blocks "0,1|2"   # 提携構造の Aumann–Drèze 値・提携構造つきの仁・Owen 値
coopgame convex-nucleolus v.txt             # 凸ゲームの手法で仁 (凸でなければエラー。--assume で凸と仮定して試し、事後検証する)
```

`power` は、定義されない指数 (勝利提携がないゲームの Johnston 指数など) の列を空欄にし、理由を標準エラーに出す (Python の `power_indices` の `None` に当たる)。

`nucleolus` の主なオプション:

| オプション | 内容 |
|---|---|
| `--pre` | プレ仁を求める |
| `--cost` | 費用ゲームとして読み、仁による費用の分担を出す (JSON・CSV の `type` が `cost` なら付けなくてよい) |
| `--exact` | 値を有理数として読み、有理数だけで仁を求めて分数で出す (JSON・CSV の数も字面どおり。`--full` とは併用できない) |
| `--full` | 全提携の行を最初から入れる LP で解く (制約生成との比較用) |
| `--tolerance T` | 逐次 LP の許容誤差を値の単位で指定し、1 回だけ解く (`--exact` とは併用できない)。`auto` は、指定しない場合と同じ既定の手順を明示して選ぶ |
| `--rel-tolerance R` | 逐次 LP の許容誤差を `max\|v(S)\|` に対する比で指定し、1 回だけ解く (`--tolerance` とはどちらか一方) |
| `--missing zero\|ignore` | JSON・CSV で値が分からない提携の扱い ([入力形式](input-format.md#値が分からない提携))。`ignore` は `nucleolus` (`--exact` なし) だけで使える。1 行 1 値のファイルには使えない |

`--tolerance`・`--rel-tolerance` を指定しなければ、正しい仁を返すことを優先する。
比 `1e-7` の許容誤差で解いた配分が、Kohlberg 基準を細かい許容誤差でも満たすかを確かめる (20 人まで。21 人以上は確かめないので `approximate` で返す)。
満たさなければ許容誤差を下げて解き直し、それでも満たさなければ 8 人以下は有理数の逐次 LP で求める (10 秒ほどかかることがある)。
9 人以上で満たさないままなら、保証を `approximate` にして返す。
21 人以上で、1 回だけ解いた配分を確かめずに使うなら `--rel-tolerance 1e-7` を指定する (保証は `exact`)。
値が `1e12` 程度のゲームを 1 単位まで正しく求めるには `--exact` を使う
(手順・計測・見分けられない場合は [性能と制約](performance.md) の「仁の許容誤差の決め方」)。
`--missing ignore` で値が分かる提携だけを使うときは、指定がなくても 1 回だけ解く。
標準エラーの統計に、どう求めたかを出す。

```console
$ coopgame nucleolus v.txt
time=0.000167s lp_solves=3 rows_added=0 solver=lp tolerance=6e-7 refinements=0 guarantee=exact levels=[-1.0]
3
2
1
```

| 項目 | 内容 |
|---|---|
| `solver` | `lp` (逐次 LP)、`rational` (有理数の逐次 LP に切り替えた) |
| `tolerance` | 返した配分を求めた許容誤差 (値の単位。`rational` では 0) |
| `refinements` | 許容誤差を下げて解き直しを試みた回数 (失敗した試行を含む) |
| `guarantee` | `exact`、または Kohlberg 基準を細かい許容誤差では満たさないまま返した `approximate` (既定の手順で確かめない 21 人以上も `approximate`) |

`structure` (提携構造つきの仁) と、仁の変種 (`per-capita`・`proportional`・`modiclus`・`disruption`) も `--tolerance`・`--rel-tolerance` を受け付け、標準エラーに `tolerance` と `guarantee` を出す。
指定しなければ、`structure` は全体 1 ブロックなら `nucleolus` と同じ既定の手順で解き、全員が単独なら `exact`、それ以外は比 `1e-7` で 1 回だけ解いて、確かめていないので `approximate` にする。
仁の変種は比 `1e-7` で 1 回だけ解き、確かめていないので `approximate` にする。
許容誤差を指定すると、どれも 1 回だけ解いて `exact` にする (指定した許容誤差の範囲で成り立つ。[保証の設計](guarantees.md#既定の経路ごとの保証))。

`least-core`・`kernel`・`shapley`・`banzhaf`・`compare` も `--cost` で費用ゲームを扱う ([費用ゲーム](input-format.md#費用ゲーム))。
他のコマンドに費用のゲーム (`type` が `cost`) を渡すとエラーにする。
JSON の `type`・CSV の見出しで価値 (`value`) と明示したファイルに `--cost` を付けると、どちらの種類で読むかが食い違うのでエラーにする (`type` を省いた JSON と 1 行 1 値のファイルには付けられる)。
価値として読んだゲームで配分集合が空になると (`sum v({i}) > v(N)`)、エラーに `--cost` の案内を添える (費用を価値として読んだときに起きやすいため)。
費用のゲームで `--missing zero` を使うと、値が分からない提携の費用を 0 とみなす ([入力形式](input-format.md#値が分からない提携))。

同じオプションを 2 回書くとエラーにする (どちらの値を使うかが曖昧になるため)。
`--lex` は 1 行 1 値のファイル (`v.txt`) の並び順の指定なので、JSON・CSV の入力に付けるとエラーにする (どのコマンドでも同じ)。
`--missing` は JSON・CSV で値が分からない提携の扱いの指定なので、1 行 1 値のファイルに付けるとエラーにする (既定と同じ `--missing error` も同じ。どのコマンドでも同じ)。
コマンドが受け付けないオプションは、効果がないので未知のオプションとしてエラーにする。
位置引数 (`v.txt`・`sol.txt`) の数はコマンドごとに決まっていて、余分な位置引数はエラーにする。
`-` で始まる位置引数 (`-pre` など) は、オプションの書き間違いとして未知のオプションのエラーにする。
ゲームのファイルを読まないコマンド (`talmud`・`airport`・`generate`・`bench`) に `--lex`・`--missing` を付けた場合も同じである。

## 特定のゲームを解く

```bash
coopgame talmud --estate 100 --claims 100,200,300 --exact   # 破産ゲームの仁 (タルムード則) を分数で出す
coopgame airport --costs 1,2,3,6                            # 空港ゲームの費用分担 (Shapley 値と仁)
```

```console
$ coopgame airport --costs 1,2,3,6
nucleolus: method=convex-transfer guarantee=proven(convex)
player	cost	shapley	nucleolus
0	1	0.25	0.5
1	2	0.5833333333333333	0.75
2	3	1.0833333333333333	0.875
3	6	4.083333333333333	3.875
```

## 結果を検証する

```bash
coopgame verify v.txt sol.txt               # Kohlberg 基準とカーネル条件 (浮動小数点数の判定)
coopgame certify v.txt sol.txt              # 有理数による厳密な検証 (厳密な仁を分数で出す)
coopgame certify v.txt sol.txt --rational   # v.txt の値を 10 進数・分数の値どおりの有理数として検証する
coopgame check v.txt sol.txt                # 配分が仁かを検証し、結論 (certified・refuted・undecided) を出す
coopgame check v.txt sol.txt --reference lp # 基準の仁の求め方を選ぶ (auto・exact・lp)
coopgame bargaining v.txt sol.txt           # 交渉集合に属するか (属さなければ反論のない異議を出す)
```

`verify` と `bargaining` は浮動小数点数で判定し、使った許容誤差を `tolerance=` の行に出す。
既定の許容誤差は `max|v(S)|` の `1e-11` 倍で、`verify` の Kohlberg 基準とカーネル条件、`bargaining`、
Python の `verify`・`is_in_kernel`・`bargaining_set` で同じである (仁の既定の手順が確かめる幅以上なので、`nucleolus` の出力は合格する)。
`--check-tolerance T` で値の単位の許容誤差を指定できる。
合格しても仁であることの証明ではない。厳密に確かめるには `certify`・`check` を使う。

```console
$ coopgame verify v.txt sol.txt
tolerance=6e-11
kohlberg=true (1 levels, 1 LPs)
kernel=true (violation=0e0)
```

`certify` は、`sol.txt` の配分自身から厳密な配分を復元できたときだけ合格にする。
`check` は、`certify` に合格しない配分を基準の仁と比べ、結論を標準エラーに出す (`check=certified` など)。
合格なら厳密な仁を分数で標準出力に出す。`--rational`・`--pre` は `certify` と同じ。

| 結論 | 標準エラー | 終了コード |
|---|---|---|
| 合格 | `check=certified` | 0 |
| 否定 (配分は仁でない) | `check=refuted` と理由 | 2 |
| 未決 (仁とも仁でないとも確定できない) | `check=undecided` と理由 | 3 |
| 入力の誤りなどのエラー | `エラー: ...` | 1 |

`--reference` は基準の仁の求め方である (結論の条件は [保証の種類](guarantees.md) の「検証の範囲」)。
どれも、まず逐次 LP の仁から厳密な配分を復元する。

| 値 | 逐次 LP の仁から復元できないとき |
|---|---|
| `auto` (既定) | 8 人以下のときだけ、有理数の逐次 LP で求め直す |
| `exact` | 10 人まで、有理数の逐次 LP で求め直す (9・10 人では数分かかることがある。[性能と制約](performance.md)) |
| `lp` | 求め直さない (逐次 LP の仁だけを使う) |

`convex-nucleolus --assume` の事後検証でも、`--reference` で同じ選択ができる。

## 配分を説明・比較する

```bash
coopgame explain v.txt sol.txt              # 不満の大きい提携、安定性、プレイヤーごとの不満
coopgame compare v.txt                      # 仁・プレ仁・Shapley 値・per capita 仁・modiclus を安定性で比べる
coopgame influence v.txt                    # 配分を決めている提携と、その値に対する感度
coopgame uncertainty v.txt --relative 0.1 --samples 200   # 各値 ±10% の誤差での配分の分布 (--relative は必須)
coopgame plot v.txt > figure.svg            # 3-4 人の配分集合の図 (SVG)。頂点などの表を標準エラーに出す
```

上の例の `v.txt` での `compare` と `influence` の出力:

```console
$ coopgame compare v.txt
解	プレイヤー 0	プレイヤー 1	プレイヤー 2	最大の不満	コア	離脱で得をする提携の数
0	3.000000	2.000000	1.000000	-1.000000	属する	0
1	3.000000	2.000000	1.000000	-1.000000	属する	0
2	2.500000	2.000000	1.500000	-0.500000	属する	0
3	3.000000	2.000000	1.000000	-1.000000	属さない (和が v(N) と異なる)	0
4	2.000000	2.000000	2.000000	0.000000	属する	0

コアの判定の許容誤差: 0e0 (0 は値どおりの厳密な判定。--core-tolerance で指定)

解の番号:
0	仁
1	プレ仁
2	Shapley 値
3	per capita 仁
4	modiclus
$ coopgame influence v.txt
coalition	max|dx/dv|	dx/dv (プレイヤー順)	difference	guarantees
{0, 1}	0.5833	0.2917 0.2917 -0.5833	central	exact:2
{0, 2}	0.5833	0.4167 -0.5833 0.1667	central	exact:2
{1, 2}	0.5833	-0.5833 0.4167 0.1667	central	exact:2
{0, 1, 2}	0.4167	0.2917 0.2917 0.4167	central	exact:2
{2}	0.2500	-0.1250 -0.1250 0.2500	central	exact:2
{1}	0.0000	0.0000 0.0000 -0.0000	central	exact:2
```

`explain` と `compare` のコアの判定は、ゲームの値と配分を 2 進数の値どおりの有理数として読んだ厳密な判定である。
上の per capita 仁は LP の解で和が `v(N) = 6` にちょうど一致しない (3 などを丸めた値) ので、厳密な判定では属さない。
浮動小数点数の丸めの分を許すときは、`--core-tolerance T` で和の差と超過を 0 とみなす幅を指定する。

`compare` は、計算できない解を省き、省いた解と理由を標準エラーに出す。
省くのは、配分集合が空のゲームの仁と per capita 仁、18 人以上の per capita 仁、8 人以上の modiclus である
(人数の上限は [性能と制約](performance.md))。解の番号は、省いた解を詰めて振る。

仁・プレ仁の許容誤差は、`compare`・`influence`・`uncertainty` でも `--tolerance`・`--rel-tolerance` で指定できる。
指定しない場合は、どれも `nucleolus` と同じ既定の手順 (正しさを優先) で仁を求める。
`influence` と `uncertainty` は標本・差分の数だけ仁を解くので、解き直しや有理数の逐次 LP に切り替わるゲームでは時間が延びる
(実測は [性能と制約](performance.md#値の不確かさの分析の時間))。
速さを優先するなら、`--rel-tolerance 1e-7` などで許容誤差を指定し、確かめずに 1 回だけ解く
(値の大きい整数のゲームなどでは、仁と異なる配分が混ざりうる)。

`uncertainty` は誤差の幅 `--relative` (各値の前後の比。`0.1` で ±10%) が必須である。
標準エラーの `guarantees=` は解けた標本の保証の内訳 (`exact:198,approximate:2` など) で、
`approximate` は正しいと言えない解 (仁の既定の手順で Kohlberg 基準を確かめられなかった解など) である。
平均などは、保証によらず解けた標本を全て使って求める。
標本の解の許容誤差が、幅が正の区間のうち最も狭いものの幅の半分以上なら、その提携の揺れが許容誤差に埋もれるのでエラーにする
(`influence` の `--delta` と同じ判定。値が 0 の提携は幅が 0 なので比べない)。
そのため、値が丸め誤差程度の非 0 の提携 (`0.1 + 0.2 - 0.3` を計算した値など) を持つゲームや、値の桁が `1e6` 倍以上離れた提携を持つゲームでは、
小さい値の提携の幅が許容誤差 (既定は `max|v(S)|` の `1e-7` 倍) より狭くなり、エラーになりやすい。
その提携の揺れは許容誤差に埋もれて分布に現れないからである。値を 0 に丸めるか、より小さい許容誤差 (`--rel-tolerance`) で解く。
許容誤差は、LP を解かずに決まった厳密な解 (1 人ゲーム、`sum v({i}) = v(N)` のゲーム) と有理数の逐次 LP の解では 0 として比べる。

`influence` の `guarantees` の列は、差分に使った 2 つの解の保証の内訳である。
差分に使った解の許容誤差 (返した配分を求めた逐次 LP の許容誤差。有理数の逐次 LP で求めた解は 0) が `--delta` 以上なら、
配分の変化が許容誤差に埋もれるので、エラーにする。`--delta` を大きくするか、より小さい許容誤差で解く。
既定の手順で Kohlberg 基準を確かめた解も、最初に解いた許容誤差 (比 `1e-7`) で比べる。
確かめた解でも、値が `1e12` 程度のゲームでは仁と数単位ずれることがあるためである。

`influence` は提携の値を `±delta` 動かしたゲームを解き、`difference` の列に差分の取り方を書く。
`central` は中心差分 `(x(v(S) + δ) - x(v(S) - δ)) / 2δ` である。
片側のゲームが解けないときは、元のゲームの解との片側差分にする。
`forward` は `v(S) - δ` が解けなかった場合の `(x(v(S) + δ) - x(v)) / δ`、`backward` は `v(S) + δ` が解けなかった場合の `(x(v) - x(v(S) - δ)) / δ` である。
配分集合が 1 点のゲーム (`v(N) = sum v({i})`) では、`v(N)` を下げたり 1 人提携の値を上げたりすると配分集合が空になるので、片側差分になる。

```console
$ printf '1\n1\n2\n1\n2\n2\n3\n' > additive.txt
$ coopgame influence additive.txt
coalition	max|dx/dv|	dx/dv (プレイヤー順)	difference	guarantees
{0, 1, 2}	0.3333	0.3333 0.3333 0.3333	forward	exact:2
{0}	0.0000	0.0000 0.0000 0.0000	backward	exact:2
{1}	0.0000	0.0000 0.0000 0.0000	backward	exact:2
{0, 1}	0.0000	0.0000 0.0000 0.0000	central	exact:2
{2}	0.0000	0.0000 0.0000 0.0000	backward	exact:2
{0, 2}	0.0000	0.0000 0.0000 0.0000	central	exact:2
{1, 2}	0.0000	0.0000 0.0000 0.0000	central	exact:2
```

両側とも解けない提携があれば、その提携を示してエラーにする。

`plot` が標準エラーに出す表は、行頭がその点の種類 (`core_vertex`・`kernel_piece_0` など・`nucleolus`・`shapley`)、続く値がプレイヤー 0, 1, ... の順の配分である。
図のコアは、頂点の張る次元に合わせて、1 点なら円の印、線分なら太い線、平面に収まれば多角形、4 人の立体なら稜線で描き、凡例に `コア (1 点)`・`コア (線分)` のように形を書く。

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
`--types` は 1 から 5、`--n` は 2 以上、`--seeds` は 1 以上で、未知の手法とともにエラーにする。
タイプと人数の組でゲームを作れない場合 (タイプ 5 の `n < 7` など) は、その組の各手法の行の `error` 列に理由を書く。
