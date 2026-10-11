# 入力形式

## 特性関数のベクトル

特性関数は長さ `2^n - 1` のベクトルで与える (空提携を除く)。並び順は 2 種類に対応する。

| 並び順 | 提携 `S` の位置 | 対応する既存実装 |
|---|---|---|
| ビット順 (既定) | `sum_{i in S} 2^i` 番目 (1 始まり) | BNF 実装 (blrzsvrzs/nucleolus) の `v.txt`、TUGLab の `binary = TRUE` |
| 辞書式順 (CLI は `--lex`、ライブラリは `ExplicitGame::from_lex`) | 大きさの昇順、同じ大きさ内は辞書式 | CoopGame、TUGLab の既定 |

プレイヤーは 0 始まりの番号 `0, 1, ..., n-1` で表す。3 人ゲームなら、次の順に書く。

- ビット順: `{0},{1},{0,1},{2},{0,2},{1,2},{0,1,2}`
- 辞書式順: `{0},{1},{2},{0,1},{0,2},{1,2},{0,1,2}`

ファイルは 1 行 1 値のテキストで、空行と `#` で始まる行は無視する。

## 費用ゲーム

費用 `c(S)` を与えるゲームは、ライブラリでは `games::cost::CostGame`、CLI では `--cost` で扱う。
節約ゲームに直して解き、費用の分担に戻す。

## 提携ごとの値の形式 (JSON・CSV)

拡張子が `.json`・`.csv` のファイルは、提携ごとの値で読む。
提携はメンバーのプレイヤーの番号 (0 始まり) で書く。
ライブラリでは feature `io` を有効にして `io` モジュールを使う。

JSON の `values` は、キーが提携 (番号をカンマでつなぐ。順序と空白は問わない) のオブジェクトで書く。

```json
{"players": 3, "type": "value",
 "values": {"0": 0, "1": 0, "2": 0, "0,1": 4, "0,2": 3, "1,2": 2, "0,1,2": 6}}
```

`{"coalition": [番号, ...], "value": 値}` の配列でもよい。
`players` (人数) を省くと、現れた最大の番号 + 1 人とする。

```json
{"type": "cost",
 "values": [{"coalition": [0], "value": 4}, {"coalition": [1], "value": 5}, {"coalition": [0, 1], "value": 7}]}
```

CSV は 1 行目を見出し `coalition,value` とし、提携の番号を `|` でつなぐ。
人数は現れた最大の番号 + 1 とする。

```csv
coalition,value
0,0
1,0
2,0
0|1,4
0|2,3
1|2,2
0|1|2,6
```

CLI は `v.txt` と同じく、配分を 1 行 1 値 (`i` 行目がプレイヤー `i`) で出す。
`type` が `cost` (CSV は見出しが `coalition,cost`) なら費用の分担を出す。

```console
$ coopgame nucleolus game.csv
time=0.000077s lp_solves=3 rows_added=0 levels=[-1.0]
3
2
1
$ coopgame nucleolus cost.json
time=0.000038s lp_solves=2 rows_added=0 levels=[-1.0]
3
4
```

### 値が分からない提携

値が分からない提携があると、既定ではエラーにして一覧を示す。
CLI では次のオプションで扱いを変えられる。

| オプション | 扱い |
|---|---|
| `--missing zero` | 0 とみなす |
| `--missing ignore` (`nucleolus` のみ) | 値が分かる提携だけで解く。1 人提携とその補集合の値は必要。結果の保証は `approximate` |

`{0, 2}` と `{1, 2}` の値がない `partial.csv` の例:

```csv
coalition,value
0,0
1,0
2,0
0|1,4
0|1|2,6
```

```console
$ coopgame nucleolus partial.csv
エラー: partial.csv: 引数が不正: 値が分からない提携が 2 個ある: {0, 2} {1, 2} (0 とみなすか、分かる提携だけで解く)
$ coopgame nucleolus partial.csv --missing zero
time=0.000109s lp_solves=5 rows_added=0 levels=[-1.0, -2.5]
2.5
2.5
1
```
