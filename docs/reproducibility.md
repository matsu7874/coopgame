# 実験の再現と検証

ドキュメントに書いた数値の元データ (`data/`) がどのコードと環境から作られたか、どう再現し、
ドキュメントの数値とどう照合するかをまとめる。

## すぐにできる照合

```bash
sha256sum -c data/SHA256SUMS                # 保存したデータが改変されていないか
python3 scripts/verify_claims.py            # ドキュメントの数値を CSV から再計算して照合
```

`verify_claims.py` は `docs/comparison.md`、`docs/guarantees.md`、`docs/performance.md` の数値を
1 つずつ CSV から再計算し、食い違えば終了コード 1 を返す。
インスタンスを作ってから `--instances` を付けると、配分そのものも検証する。

```bash
cargo build --release --features cli
scripts/compare/make_instances.sh scripts/compare/work
(cd scripts/compare/work && sha256sum -c ../instances.sha256)   # インスタンスが同じか
python3 scripts/verify_claims.py --instances scripts/compare/work
```

配分の検証では、coopgame-rs の全 576 配分を Kohlberg 基準で判定し、
CoopGame の誤りと判定した 4 件について、超過を降順に並べたベクトルを辞書式に比べる。

## データの出所

| ファイル | 内容 | 生成したコード | 生成コマンド |
|---|---|---|---|
| `data/bench/results-n4-12.csv` | 明示ベクトルの計測 (`docs/performance.md` の計測例) | コミット `3f85dc1` | `coopgame bench --n 4..12 --seeds 2 --methods nucleolus,nucleolus-full,prenucleolus,kernel,prekernel` |
| `data/bench/results-n13-16.csv` | 同上 | コミット `3f85dc1` | `coopgame bench --n 13..16 --seeds 1 --methods nucleolus,nucleolus-full,kernel` |
| `data/bench/results-n17-22.csv` | 同上 | コミット `3f85dc1` | `coopgame bench --n 17..22 --seeds 1 --methods nucleolus,kernel` |
| `data/bench/oracle-scaling.csv` | オラクル版の仁の計測 (`docs/performance.md` のオラクルの計測) | オラクル版の確定判定の LP で行を一律に緩めるのをやめたコミット | `bench_oracle bankruptcy 10,20,40,70,100 3`、`bankruptcy-wide 40,70,100 10`、`majority 9,13,17,19,21 1`、`voting 10,15,20,25 2` (`cargo run --release --example bench_oracle -- ...`。Apple M4 Pro・macOS・rustc 1.99.1 で、他の重い処理を止めて計測。秒の列は実行ごとに変わる) |
| `data/bench/oracle-tolerance.csv` | オラクル版の仁の、許容誤差を変えたときの時間と追加した行 (`docs/performance.md` の「上限と既定値の根拠」) | オラクル版の確定判定の LP で行を一律に緩めるのをやめたコミット | `cargo run --release --example bench_oracle_tolerance > data/bench/oracle-tolerance.csv` (Apple M4 Pro・macOS・rustc 1.99.1 で計測。秒の列は実行ごとに変わる) |
| `data/bench/uncertainty.csv` | 値の不確かさの分析で、仁を既定の手順で解く場合と 1 回だけ解く場合の時間 (`docs/performance.md` の「値の不確かさの分析の時間」) | 分析の既定を仁の既定の手順にしたコミット | `cargo run --release --example bench_uncertainty > data/bench/uncertainty.csv` (Apple M4 Pro・macOS・rustc 1.99.1 で計測。秒の列は実行ごとに変わる) |
| `data/bench/voting-oracle-before.csv` | 重み付き投票ゲームのオラクル版の仁の時間と最大メモリ、修正前 (`docs/performance.md` の「拒否権のある重み付き投票ゲーム」) | コミット `a1c3d93` のライブラリに、`examples/bench_voting.rs` を加えてビルドしたもの | `cargo build --release --example bench_voting && target/release/examples/bench_voting > data/bench/voting-oracle-before.csv` (Apple M4 Pro・macOS・rustc 1.99.1 で、他の重い処理を止めて計測。最大メモリは `/usr/bin/time -l`。秒の列は実行ごとに変わる) |
| `data/bench/voting-oracle-after.csv` | 同上、修正後 | 確定済みの空間を使った探索を加えたコミット | `target/release/examples/bench_voting > data/bench/voting-oracle-after.csv` (同じ環境) |
| `data/bench/voting-budget.csv` | 確定済みの空間を使った探索の前に読む上限 (`ENUMERATION_BUDGET`) を変えたときの時間・最大メモリ・1 回の探索で読んだ提携の数 (同上) | 同じコミットで、`ENUMERATION_BUDGET` を 64・256・1024・4096 に変え、確定済みの提携のある探索ごとに読んだ数を標準エラーに出すようにした一時的なビルド | 各ビルドで `/usr/bin/time -l target/release/examples/bench_voting <ゲーム> <人数>` を実行し、`searches` (探索の回数)・`max_read` (1 回で読んだ最大の数)・`searches_over_budget` (上限まで読んだ探索の数) を数えた (同じ環境) |
| `data/compare/certified.csv` | 比較の全配分の有理数による検証 | 有理数の検証を追加したコミット | `python3 scripts/compare/certify_results.py scripts/compare/work` |
| `data/analysis/sampled-convergence.csv` | サンプリングした仁の収束 | サンプリングした仁を追加したコミット | `cargo run --release --example study_data_valuation -- convergence` |
| `data/analysis/assumption-study.csv` | 凸と仮定した手法を一般のゲームに使う実験 ([guarantees.md](guarantees.md)) | 保証の種類を追加したコミット | `cargo run --release --example study_assumption -- 4,5,6,7,8,9,10 10` |
| `data/bench/convex-graph.csv`、`data/bench/convex-bankruptcy.csv` | 凸ゲームの手法の計測 (同上) | 同上 | `bench_convex graph 10,16,24,32,48,64 3`、`bench_convex bankruptcy 10,20,30,40 3` (`cargo run --release --example ...`) |
| `data/bench/auto-threshold.csv` | 自動選択の境目の計測 (同上) | 同上 | `cargo run --release --example bench_auto_threshold -- 8,10,12,14,16,18,20 3` |
| `data/bench/exact-scaling.csv` | 有理数だけの仁のソルバーの計測 | 厳密なソルバーを追加したコミット | `cargo run --release --example bench_exact -- 3,4,5,6,7,8,9,10` |
| `data/bench/exact-scaling-integer.csv` | 有理数だけの仁のソルバーの、整数ゲームでの計測 ([performance.md](performance.md) の「上限と既定値の根拠」) | 有理数への自動の切り替えを 8 人までにしたコミット | `cargo run --release --example bench_exact -- 7,8,9,10 integer > data/bench/exact-scaling-integer.csv` (10 人は各 scale で 1 ゲーム。Apple M4 Pro・macOS・rustc 1.99.1 で計測。秒の列は実行ごとに変わる) |
| `data/bench/variant-limits.csv` | 仁の変種と交渉集合の判定の時間と最大メモリ (`docs/performance.md` の上限の根拠) | 上限を追加したコミット。上限より 1 人多い人数は `MAX_*_PLAYERS` を一時的に大きくしたビルド | `cargo run --release --example bench_limits > data/bench/variant-limits.csv` (Apple M4 Pro・macOS・rustc 1.99.1 で計測。最大メモリは `/usr/bin/time -l`) |
| `data/analysis/tolerance-study.csv` | 仁の許容誤差の決め方の比較 ([performance.md](performance.md)) | 許容誤差の決め方を追加したコミット | `cargo run --release --example study_tolerance > data/analysis/tolerance-study.csv` (Apple M4 Pro・macOS・rustc 1.99.1 で計測。秒の列は実行ごとに変わる) |
| `data/analysis/refinement-rows.csv` | 許容誤差を下げて解き直すときに追加する行の数 (同上) | 解き直しの行の上限を追加したコミット | `cargo run --release --example study_tolerance -- rows > data/analysis/refinement-rows.csv` (同じ環境で計測。秒の列は実行ごとに変わる) |
| `data/analysis/counterexamples.txt` | 反例の探索と縮小の実例 | 反例探索を追加したコミット | `cargo run --release --example study_counterexamples` (約 1 分) |
| `data/compare/small.csv` | 既存実装との比較 (n = 4-12) | coopgame-rs はコミット `3f85dc1` の CLI | `python3 scripts/compare/run.py work/small` |
| `data/compare/tuglab-retry-tol1e-8.csv` | TUGLab の再実行 | 同上 | `IMPLS=TUGLab python3 scripts/compare/run.py <失敗したインスタンス> 1e-8` |
| `data/compare/large.csv` | 既存実装との比較 (n = 13-18) | 同上 | `python3 scripts/compare/run.py work/large 1e-8` |

補足:

- `small.csv` は、`run.py` をリポジトリに入れる前の版で作った。この版は TUGLab のエラーを
  `error:Execution halted` と記録する。現在の版は R のエラー文 (`Error in ...`) を記録する。計算内容は同じである。
- `tuglab-retry-tol1e-8.csv` の対象は、`small.csv` で TUGLab が失敗・時間切れ・省略・誤りになった
  24 インスタンス (仁・プレ仁の両方を実行) である。

## 環境

| 項目 | 版 |
|---|---|
| OS | Ubuntu 24.04.4 LTS (Linux 6.18) |
| CPU | Intel Xeon 2.10GHz、4 コア (計測は単一スレッド) |
| Rust | rustc 1.94.1、cargo 1.94.1 |
| LP ソルバー | microlp 0.6.0 (`Cargo.lock` で固定) |
| R | 4.3.3 (r-base-core 4.3.3-2build2) |
| R パッケージ | rcdd 1.6-1、CoopGame 0.2.2、TUGLab 0.0.1 (コミットは `docs/references.md`、`scripts/compare/setup.sh` で固定) |
| apt パッケージ | r-cran-geometry 0.4.7-1、r-cran-gtools 3.9.5-1、libgmp-dev 6.3.0 |

## 再実行して照合する

### 既存実装との比較

```bash
scripts/compare/setup.sh                       # R 環境 (コミット固定)
export R_LIBS=scripts/compare/work/rlib TUGLAB_DIR=scripts/compare/work/TUGLab/R
python3 scripts/compare/run.py scripts/compare/work/small > small.csv
python3 scripts/compare/run.py scripts/compare/work/large 1e-8 > large.csv
python3 scripts/compare/diff_results.py data/compare/small.csv small.csv
python3 scripts/compare/diff_results.py data/compare/large.csv large.csv
```

`diff_results.py` は状態の種類 (ok, error, timeout, skipped) と配分 (相対 1e-9) を比べる。
計算時間は環境で変わる。上限 300 秒に近いインスタンス (TUGLab の一部) は、
速い環境では時間切れにならず、遅い環境では新たに時間切れになりうる。

## 再現の確認結果 (2026-10-01、上の環境)

| 対象 | 方法 | 結果 |
|---|---|---|
| ドキュメントの数値 | `scripts/verify_claims.py --instances scripts/compare/work` | 104 項目全てが一致 (当時の項目数) |
| インスタンス | `make_instances.sh` で再生成し、比較に使ったファイルと照合 | 144 個全てが一致 (`instances.sha256` に記録) |
| 比較: coopgame-rs・CoopGame | `run.py` で再実行し `diff_results.py` で照合 | small 756 件、large 72 件の状態と配分が全て一致 (CoopGame の誤った配分も同じ値で再現) |
| 比較: TUGLab | 同上 | small 252 件は全て一致。large 36 件のうち 33 件が一致し、3 件は状態が変わった |
| 明示ベクトルの計測 | `verify_claims.py` (件数と検証結果) | 540 件全てが検証済み |
| Shapley 値・Banzhaf 値 | `scripts/compare/check_values.py` (CoopGame 0.2.2 と照合) | 12 ゲーム 24 件が相対 2.3e-15 以内で一致 |
| per capita 仁・比例仁・modiclus | `scripts/compare/check_variants.py` (CoopGame 0.2.2 と照合) | 41 ゲームで 3 つとも一致 (比例仁の 1 件は両方とも計算しない)。相対 9.1e-14 以内。当時の modiclus は CoopGame と同じく空でない真部分提携の組だけを使っていた。現在の modiclus は空提携と全体提携を含む組を使う文献の定義で、CoopGame とは一致しないので照合から外した |

TUGLab の large で状態が変わった 3 件は、いずれもタイプ 4 の仁で、時間の上限 (300 秒) の境目によるものである。

- n = 15: 前回は時間切れ、今回は 245.7 秒で完了した。
- n = 16, 17: n = 15 の結果に応じて「省略」から「完了」「時間切れ」に変わった。

今回完了した n = 15, 16 の配分は、coopgame-rs の配分 (Kohlberg 基準で検証済み) と最大 0.52、0.92 異なり、
正しい仁ではなかった。`docs/comparison.md` の「TUGLab はタイプ 2, 4 で誤った仁を返す」という結論は変わらない。

## 再実行の結果 (2026-11-21、macOS)

逐次 LP の確定値の決め方、仁の変種の定義、入力の検証を変えた後に、別の環境で一部を再実行した。

| 項目 | 版 |
|---|---|
| OS・CPU | macOS 27.2.1、Apple M4 Pro |
| Rust | rustc 1.99.1 |
| R | 4.6.2 (Homebrew) |
| R パッケージ | geometry 0.5.2、gtools 3.9.5 (CRAN)、rcdd 1.6-1、CoopGame 0.2.2 (`scripts/compare/setup.sh` で固定したコミット) |

| 対象 | 方法 | 結果 |
|---|---|---|
| インスタンス | `make_instances.sh` で再生成し、`instances.sha256` と照合 | 144 個全てが一致 |
| 比較: coopgame-rs の仁・プレ仁 | `IMPLS=coopgame-rs,coopgame-rs-full` で `run.py` を再実行し、`diff_results.py` で `data/compare/` と照合 | small 504 件、large 72 件の状態と配分 (相対 1e-9) が全て一致 |
| Shapley 値・Banzhaf 値 | `scripts/compare/check_values.py` | 12 ゲーム 24 件が相対 2.3e-15 以内で一致 |
| per capita 仁・比例仁 | `scripts/compare/check_variants.py` | per capita 仁は 41 ゲームで一致。比例仁は 40 ゲームで一致し、1 件は両方とも計算しない。相対 9.1e-14 以内 |
| modiclus | CoopGame の `modiclus` を直接実行 | 本実装と一致しない (値は [comparison.md](comparison.md#仁の変種-per-capita-仁比例仁modiclus)) |

CoopGame と TUGLab の仁 (`run.py` の残りの実装) は再実行していない。
