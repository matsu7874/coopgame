# 既存実装との比較 (仁・プレ仁)

## 結論

- 速さ: n = 18 で coopgame-rs (制約生成) は 0.16-0.30 秒、CoopGame (R) は 2.8-8.4 秒、
  TUGLab (R) は 18-119 秒か時間切れだった。coopgame-rs は CoopGame より 13-50 倍速い。
- 正しさ: coopgame-rs の結果は全 288 件で Kohlberg 基準を満たし、有理数による厳密な検証にも全て合格した。
  CoopGame は n = 17, 18 の 4 件で、TUGLab は小さい n から誤った配分やエラーを返した。
- 無料の OSS で動かせる MatTuGames (Octave + GLPK) と pyCoopGame も別のマシンで比べた
  ([無料の OSS で動かした MatTuGames と pyCoopGame](#無料の-oss-で動かした-mattugames-と-pycoopgame))。
  MatTuGames は計算を終えた 270 件が全て正しかったが、n >= 14 の一部で数十秒以上かかるか時間切れになった。
  pyCoopGame は計算を終えた 92 件のうち 51 件で仁でない配分を返した。
- BNF 実装 (Benedek ら) は CPLEX 12.7 を必要とするため、比較できなかった。

## 比較対象

| 実装 | 版 | 仁の計算方法 | 備考 |
|---|---|---|---|
| coopgame-rs (制約生成) | この版 | 逐次 LP + 制約生成 (microlp) | 既定の方式 |
| coopgame-rs (全行) | この版 | 逐次 LP、2^n 行を全て入れる (microlp) | `--full` |
| CoopGame | 0.2.2 (2021-08-23) | 逐次 LP (rcdd の `lpcdd`、浮動小数点) | GitHub の CRAN ミラーからインストール |
| TUGLab | 0.0.1 (2025-06-10) | Potters ら (1996) の延長単体法 (R で実装) | 依存の volesti を避けるため `R/` 以下を source して使用 |

比較しなかった実装は次のとおり。

- BNF 実装 (blrzsvrzs/nucleolus): 全アルゴリズムが CPLEX 12.7 を必要とし、ライセンスがないため動かせない。
- MatTuGames を MATLAB で動かす比較: MATLAB のライセンスがないため、Octave で動かした (後述)。

## 方法

- インスタンス: BNF 実装と同じ分布のゲーム (`scripts/compare/make_instances.sh`)
  - small: タイプ 1-5、n = 4-12、seed 0-2 (タイプ 5 は n >= 7)、計 126 個
  - large: タイプ 1, 2, 4、n = 13-18、seed 0、計 18 個
- 全実装に同じファイル (ビット順) を渡した。CoopGame には辞書式順に並べ替えて渡した。
- 時間は計算部分だけを測った (coopgame-rs は CLI の内部計測、R は `system.time`)。
  1 回の上限は 300 秒で、時間切れになったら同じタイプのそれより大きい n は省いた。
- 正しさは coopgame-rs (制約生成) の配分と相対 1e-6 で比べ、食い違ったものは両方を Kohlberg 基準で判定した。
- TUGLab の許容誤差は small では既定値 (`100 * .Machine$double.eps`)、large では失敗を減らすため 1e-8 にした。
- 環境: Intel Xeon 2.10GHz (4 コア)、単一スレッド、R 4.3.3、Rust release ビルド。

## 結果

### 実行時間 (各 n の最大、秒)

| n | coopgame-rs 制約生成 | coopgame-rs 全行 | CoopGame | TUGLab |
|---|---|---|---|---|
| 8 | 0.004 | 0.015 | 0.005 | 0.13 |
| 10 | 0.017 | 0.058 | 0.032 | 0.15 |
| 12 | 0.025 | 0.28 | 0.51 | 0.87 |
| 14 | 0.036 | 2.0 | 0.63 | 20.4 |
| 16 | 0.11 | 13.7 | 2.0 | 8.1 (タイプ 4 は時間切れ後に省略) |
| 18 | 0.22 | 140 | 7.9 | 119 (タイプ 4 は省略) |

表は仁の値。プレ仁もほぼ同じで、n = 18 では制約生成 0.30 秒、CoopGame 8.4 秒だった。
全行の LP では coopgame-rs (microlp) は CoopGame (cddlib) より遅い。
coopgame-rs の速さは LP ソルバーではなく、制約生成で行数を 2^n から数百に減らしたことによる。

### 正しさ

| 実装 | インスタンス | 結果 |
|---|---|---|
| coopgame-rs (両方式) | 全 288 件 | 全て Kohlberg 基準を満たし、2 方式の配分は一致 |
| CoopGame | small 252 件 | 全て一致 |
| CoopGame | large 36 件 | 32 件一致、4 件 (タイプ 1 の n = 17, 18 の仁・プレ仁) が誤り |
| TUGLab (既定の許容誤差) | small 252 件 | 226 件一致、4 件誤り、17 件エラー、2 件時間切れ、3 件省略 |
| TUGLab (許容誤差 1e-8) | small で失敗した 26 件 | 16 件は正しく解けた。7 件エラー、1 件誤り、1 件時間切れ、1 件省略 |
| TUGLab (許容誤差 1e-8) | large 36 件 | 26 件一致、4 件誤り、2 件時間切れ、4 件省略 |

誤りと判定した根拠は次のとおり。

- CoopGame の 4 件: 超過を降順に並べたベクトルを比べると、coopgame-rs の方が辞書式に小さい
  (例: タイプ 1、n = 17 で 13 番目の超過が 116.56 と 115.65)。仁の定義から CoopGame の配分は仁ではない。
  最大超過は最小コアの値と一致しており、2 段目以降の LP で誤っている。
- TUGLab の誤り: 効率性 `x(N) = v(N)` を満たさない配分が 3 件 (例: x(N) = 341.9、v(N) = 296)。
  残りの 1 件も x(N) が v(N) を 1.2e-4 超えており、Kohlberg 基準を満たさない。
- TUGLab のエラーは `sistemaFamA` の「Compatible indeterminate system」と
  `nucleolusvalue` の `invalid 'times' argument` だった。後者は許容誤差を変えても解消しなかった。
  エラーはタイプ 1-4 の全てで起きた。誤った配分はタイプ 2, 4 (値の範囲が狭く同点の多いゲーム) で起きた。

## 有理数による厳密な検証

上の正しさの判定は浮動小数点の許容誤差に依存していた。そこで、計算に成功した全 1,124 配分を
有理数で検証した (`coopgame certify`、`scripts/compare/certify_results.py`)。

- 配分の超過の段から、厳密な配分を有理数の連立一次方程式として復元する。
- 復元した配分で全提携の超過を有理数で計算し、Kohlberg 基準の平衡性を有理数の単体法 (Bland の規則) で判定する。

| 実装 | 合格 | 不合格 | 厳密な仁 x* からの距離 |
|---|---|---|---|
| coopgame-rs (2 方式) | 576 | 0 | 2.9e-11 以下 |
| CoopGame | 284 | 4 | 合格は 8.3e-7 以下、不合格はタイプ 1 の n = 17, 18 で 2.17 と 0.10 |
| TUGLab | 252 | 8 | 合格は 8.3e-7 以下、不合格は 0.025-67.5 |

x* は、同じインスタンスで coopgame-rs の配分から復元し、合格した厳密な仁である。
仁は一意なので、x* が仁そのものである。不合格の 12 配分は x* から 0.025 以上離れており、仁ではない。
浮動小数点で「誤り」と判定した 12 件と、有理数で不合格になった 12 件は一致した。

## 無料の OSS で動かした MatTuGames と pyCoopGame

上の比較とは別のマシンで、同じ small・large のインスタンスを次の 2 実装と coopgame-rs (制約生成) で解いた。

| 実装 | 版 | 仁の計算方法 | 備考 |
|---|---|---|---|
| MatTuGames | 1.9.2.1 (commit 74b8ec7) | 逐次 LP (`nucl.m`、`PreNucl.m`)。2^n 行を全て入れる | MATLAB 用。Octave 11.3.0 で動かし、`linprog` は GLPK 5.0 で解く互換関数 (`scripts/compare/octave/linprog.m`) に替えた |
| pyCoopGame | 0.0.6 以降 (commit 72153c7) | 辞書式最小化を重み `0.5^(k-1)` の単一 LP で近似 (Pyomo + GLPK)。変数は約 4^n 個 | プレ仁の関数はない |

- MatTuGames のコードは書き換えていない。作者が想定する環境は MATLAB で、R2024a 以降は HiGHS の双対単体法を使う。
  時間は Octave + GLPK でのもので、MATLAB での時間ではない。
- Octave の `glpk` は 3 番目の戻り値がエラー番号で、解の状態は `extra.status` にある。互換関数はこれを MATLAB の
  `exitflag` に直す。作者の GLPK 版 (`glpk_cs_nucl.m`) は制約の種類を空で渡し、Octave の `glpk` では全て等式と
  解釈されて解けないため使わなかった。
- 環境: Apple M4 Pro、macOS 26、単一スレッド。時間は MatTuGames が Octave の `tic`/`toc`、pyCoopGame が
  `nucleolus()` の呼び出し全体 (Pyomo のモデル構築を含む)。上限と省略の規則は上と同じ。

### 実行時間 (仁、各 n の最大、秒)

| n | coopgame-rs 制約生成 | MatTuGames | pyCoopGame |
|---|---|---|---|
| 8 | 0.002 | 0.025 | 6.1 |
| 10 | 0.008 | 0.040 | 250 |
| 12 | 0.012 | 9.5 | 時間切れ後に省略 |
| 14 | 0.020 | 63 | - |
| 16 | 0.063 | 0.35 (タイプ 1 は時間切れ、タイプ 2 は省略) | - |
| 18 | 0.15 | 全タイプで時間切れ後に省略 | - |

MatTuGames の時間は、同じ n でもタイプで大きく違った (n = 15 でタイプ 1 は 0.15 秒、タイプ 4 は 100 秒、
タイプ 2 は時間切れ)。プレ仁もほぼ同じだった。

### 正しさ

coopgame-rs の配分と相対 1e-6 で比べ、食い違ったものは Kohlberg 基準で判定した。

| 実装 | 結果 |
|---|---|
| MatTuGames | 仁・プレ仁とも、計算を終えた 135 件ずつ (計 270 件) が全て一致 (差は相対 2.0e-12 以下)。各 3 件が時間切れ、6 件が省略 |
| pyCoopGame (仁) | 計算を終えた 92 件のうち 41 件が一致、51 件が仁でない。small で 5 件、large で 3 件が時間切れ、残りは省略 |

pyCoopGame の 51 件の内訳は次のとおり。

- 42 件: v(N) が提携の値の最大値より小さいゲーム。どの配分も x(N) ≠ v(N) で効率性を満たさない。
  pyCoopGame は全体の和を v(N) ではなく `max(game['value'])` に合わせている。
- 9 件: v(N) が最大のゲームで、効率性は満たすが仁と食い違った。原因は確かめていない。

`coopgame certify` は渡された配分から厳密な配分を復元して検証するので、効率性を満たさない pyCoopGame の配分でも
`certified=true` になることがある (例: タイプ 1、n = 4 で `max_difference=29`)。他の実装の配分の判定には、
`max_difference` か、上のように x* からの距離を合わせて見る必要がある。

## 仁の変種 (per capita 仁・比例仁・modiclus)

`nucleolus::variants::{per_capita_nucleolus, proportional_nucleolus, modiclus}` を CoopGame 0.2.2 の
`perCapitaNucleolus`・`proportionalNucleolus`・`modiclus` と比べた (`scripts/compare/check_variants.py`)。

- ゲーム: BNF タイプ 1, 2, 4 の n = 3, 4, 5、seed 0-3 (36 個) と、CoopGame のヘルプの例など 5 個。
- 結果: 3 つの解とも 41 ゲーム全てで一致した。差は相対 9.1e-14 以下。
- 比例仁は負の値を持つゲームでは定義しない。そのようなゲーム 1 個で、両方とも計算しなかった。

実装は、不満をアフィン関数に一般化した逐次 LP (`nucleolus::variants::lexicographic_minimum`) である。
不満の定義は CoopGame のソースと同じにした (`src/nucleolus/variants.rs` の表を参照)。

## 再現

```bash
cargo build --release --features cli
scripts/compare/setup.sh                      # R・rcdd・CoopGame・TUGLab を用意
scripts/compare/make_instances.sh             # scripts/compare/work/{small,large}
export R_LIBS=scripts/compare/work/rlib TUGLAB_DIR=scripts/compare/work/TUGLab/R
python3 scripts/compare/run.py scripts/compare/work/small > small.csv
python3 scripts/compare/run.py scripts/compare/work/large 1e-8 > large.csv
python3 scripts/compare/summarize.py small.csv scripts/compare/work/small
python3 scripts/compare/check_variants.py     # 仁の変種

scripts/compare/setup_oss.sh                  # Octave・MatTuGames・pyCoopGame を用意 (macOS)
export MATTUGAMES_DIR=scripts/compare/work/MatTuGames/mat_tugames PYCOOPGAME_DIR=scripts/compare/work/pyCoopGame \
  PYCOOPGAME_PYTHON=scripts/compare/work/pyenv/bin/python IMPLS=coopgame-rs,MatTuGames,pyCoopGame
python3 scripts/compare/run.py scripts/compare/work/small > small-oss.csv
python3 scripts/compare/run.py scripts/compare/work/large > large-oss.csv
```

生データは `data/compare/` にある。

- `small.csv`: small の全実装の結果 (TUGLab は既定の許容誤差)
- `tuglab-retry-tol1e-8.csv`: small で TUGLab が失敗・誤ったインスタンスの許容誤差 1e-8 での再実行
- `large.csv`: large の全実装の結果 (TUGLab は許容誤差 1e-8)
- `certified.csv`: small・large で計算に成功した全配分の有理数による検証結果
- `small-oss.csv`、`large-oss.csv`: coopgame-rs・MatTuGames・pyCoopGame の結果 (Apple M4 Pro)
