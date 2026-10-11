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
| `data/bench/results-n4-12.csv` | フェーズ 2 の計測 (`docs/performance.md` の計測例) | コミット `3f85dc1` | `coopgame bench --n 4..12 --seeds 2 --methods nucleolus,nucleolus-full,prenucleolus,kernel,prekernel` |
| `data/bench/results-n13-16.csv` | 同上 | コミット `3f85dc1` | `coopgame bench --n 13..16 --seeds 1 --methods nucleolus,nucleolus-full,kernel` |
| `data/bench/results-n17-22.csv` | 同上 | コミット `3f85dc1` | `coopgame bench --n 17..22 --seeds 1 --methods nucleolus,kernel` |
| `data/bench/oracle-scaling.csv` | オラクル版の仁の計測 (`docs/performance.md` のオラクルの計測) | Shapley/Banzhaf 値を追加したコミット | `bench_oracle bankruptcy 10,20,40,70,100 3`、`majority 9,13,17,19,21 1`、`voting 10,15,20,25 2` (`cargo run --release --example bench_oracle -- ...`) |
| `data/compare/certified.csv` | 比較の全配分の有理数による検証 | 有理数の検証を追加したコミット | `python3 scripts/compare/certify_results.py scripts/compare/work` |
| `data/analysis/sampled-convergence.csv` | サンプリングした仁の収束 | サンプリングした仁を追加したコミット | `cargo run --release --example study_data_valuation -- convergence` |
| `data/analysis/assumption-study.csv` | 凸と仮定した手法を一般のゲームに使う実験 ([guarantees.md](guarantees.md)) | 保証の種類を追加したコミット | `cargo run --release --example study_assumption -- 4,5,6,7,8,9,10 10` |
| `data/bench/convex-graph.csv`、`data/bench/convex-bankruptcy.csv` | 凸ゲームの手法の計測 (同上) | 同上 | `bench_convex graph 10,16,24,32,48,64 3`、`bench_convex bankruptcy 10,20,30,40 3` (`cargo run --release --example ...`) |
| `data/bench/auto-threshold.csv` | 自動選択の境目の計測 (同上) | 同上 | `cargo run --release --example bench_auto_threshold -- 8,10,12,14,16,18,20 3` |
| `data/bench/exact-scaling.csv` | 有理数だけの仁のソルバーの計測 | 厳密なソルバーを追加したコミット | `cargo run --release --example bench_exact -- 3,4,5,6,7,8,9,10` |
| `data/analysis/counterexamples.txt` | 反例の探索と縮小の実例 | 反例探索を追加したコミット | `cargo run --release --example study_counterexamples` (約 1 分) |
| `data/compare/small.csv` | 既存実装との比較 (n = 4-12) | coopgame-rs はコミット `3f85dc1` の CLI | `python3 scripts/compare/run.py work/small` |
| `data/compare/tuglab-retry-tol1e-8.csv` | TUGLab の再実行 | 同上 | `IMPLS=TUGLab python3 scripts/compare/run.py <失敗したインスタンス> 1e-8` |
| `data/compare/large.csv` | 既存実装との比較 (n = 13-18) | 同上 | `python3 scripts/compare/run.py work/large 1e-8` |

補足:

- `data/bench/oracle-scaling.csv` を作った後、`oracle/voting.rs` のループを書き換えた (clippy の指摘)。
  再実行して、18 行で段数・LP の数・追加行数が一致することを確かめた。
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
| ドキュメントの数値 | `scripts/verify_claims.py --instances scripts/compare/work` | 104 項目全てが一致 (当時の項目数。その後、研究ノートとともにプレカーネルの分析とデータ評価の照合を外し、今は 53 項目) |
| インスタンス | `make_instances.sh` で再生成し、比較に使ったファイルと照合 | 144 個全てが一致 (`instances.sha256` に記録) |
| 比較: coopgame-rs・CoopGame | `run.py` で再実行し `diff_results.py` で照合 | small 756 件、large 72 件の状態と配分が全て一致 (CoopGame の誤った配分も同じ値で再現) |
| 比較: TUGLab | 同上 | small 252 件は全て一致。large 36 件のうち 33 件が一致し、3 件は状態が変わった |
| フェーズ 2 の計測 | `verify_claims.py` (件数と検証結果) | 540 件全てが検証済み |
| Shapley 値・Banzhaf 値 | `scripts/compare/check_values.py` (CoopGame 0.2.2 と照合) | 12 ゲーム 24 件が相対 2.3e-15 以内で一致 |
| per capita 仁・比例仁・modiclus | `scripts/compare/check_variants.py` (CoopGame 0.2.2 と照合) | 41 ゲームで 3 つとも一致 (比例仁の 1 件は両方とも計算しない)。相対 9.1e-14 以内 |

TUGLab の large で状態が変わった 3 件は、いずれもタイプ 4 の仁で、時間の上限 (300 秒) の境目によるものである。

- n = 15: 前回は時間切れ、今回は 245.7 秒で完了した。
- n = 16, 17: n = 15 の結果に応じて「省略」から「完了」「時間切れ」に変わった。

今回完了した n = 15, 16 の配分は、coopgame-rs の配分 (Kohlberg 基準で検証済み) と最大 0.52、0.92 異なり、
正しい仁ではなかった。`docs/comparison.md` の「TUGLab はタイプ 2, 4 で誤った仁を返す」という結論は変わらない。
