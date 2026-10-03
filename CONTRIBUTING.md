# コントリビューターガイド

coopgame の開発に参加する人向けに、開発環境、コミット前の確認、テストと文書の方針、計測の再現、リリースの手順をまとめる。
ライブラリと CLI の使い方は [README.md](README.md) と [docs/](docs/README.md) を参照。

## 開発環境

- Rust 1.88 以上 (edition 2024)。
- Python バインディングを変更する場合は、Python 3.9 以上と [maturin](https://www.maturin.rs/)、pytest。
- 計測結果を他の実装と比べ直す場合は、R と比較対象のパッケージ (`scripts/compare/setup.sh` が用意する)。

CLI は feature `cli` を有効にしたときだけビルドされる。

```bash
cargo build --release --features cli   # target/release/coopgame
```

## リポジトリの構成

| 場所 | 内容 |
|---|---|
| `src/` | ライブラリ |
| `src/bin/coopgame.rs` | CLI (feature `cli`) |
| `tests/` | 結合テスト。`tests/common/mod.rs` は独立に実装した基準 (タルムード則など) と補助関数で、examples からも `#[path]` で読み込む |
| `examples/` | 計測と分析のプログラム。文書が引用する出力は `data/` に保存している |
| `python/` | PyO3 による Python バインディング (配布名 `coopgame-py`、import 名 `coopgame`) |
| `data/` | 文書が引用する計測 (`bench/`)・他の実装との比較 (`compare/`)・分析 (`analysis/`) のデータ。`data/SHA256SUMS` で照合する |
| `docs/` | 利用者と、現状を理解したい人向けの文書 (チュートリアル、CLI と入力形式、性能と制約、保証の設計、他の実装との比較、文献と根拠、再現の手順)。一覧は `docs/README.md` |
| `scripts/` | 文書の数値の照合 (`verify_claims.py`) と、他の実装との比較のスクリプト (`compare/`) |

`data/`、`docs/`、`scripts/` は crates.io のパッケージに含めない (`Cargo.toml` の `exclude`)。

## コミット前の確認

CI は使っていないので、コミット前に次を実行する。

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features
# Python バインディングを変更した場合
(cd python && cargo fmt --check && cargo clippy -- -D warnings && maturin develop --release && pytest tests)
```

テストは release ビルドで実行する。
debug ビルドでは、大きいゲームを解くテスト (`tests/oracle.rs` など) に時間がかかる。

公開 API や rustdoc を変更した場合は、次も実行する。

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo test --doc                        # rustdoc とチュートリアルのコード例
cargo build --no-default-features       # feature なしでビルドできること
```

コミットメッセージは [Conventional Commits](https://www.conventionalcommits.org/ja/v1.0.0/) の形式で書く (説明は日本語でよい)。
公開 API を壊す変更は `refactor!:` のように `!` を付け、本文に `BREAKING CHANGE:` を書く。

## テストの方針

計算結果は、独立した基準どうしで突き合わせている。
同じ実装の結果どうしを比べても誤りは見つからないので、手計算、文献の値、閉じた形の解、別の解法、有理数による厳密な検証のいずれかを基準にする。

- 手計算できるゲームの仁・最小コア・カーネル
- 破産ゲームの仁がタルムード則 (Aumann–Maschler 1985) と一致すること
- 空港ゲーム(費用ゲーム)の仁
- 教科書・論文の数値例 (Peleg–Sudhölter Example 5.5.12 など) と、ランダムな破産問題 300 個でのタルムード則との一致 (`tests/literature.rs`)
- BNF タイプ 1-5 のランダムゲームで、仁が Kohlberg 基準を満たし、カーネルに属すること
- 仁を少し動かすと Kohlberg 基準が不成立になること
- transfer scheme の結果がカーネル条件を満たすこと
- 凸ゲームで transfer scheme の結果が仁と一致すること (Maschler–Peleg–Shapley 1971)
- 制約生成と全行の LP で仁が一致すること、部分ソートと定義どおりの走査で最大余剰が一致すること
- オラクル版の仁が明示版と一致すること、破産ゲームで n = 100 までタルムード則と一致すること (`tests/oracle.rs`)
- Shapley 値・Banzhaf 値が文献の例 (Ibn Ezra の相続問題、空港ゲーム、手袋ゲームなど) と一致すること
- タルムード則が、別実装・LP の仁 (n = 60)・有理数で検証した仁 (分数として完全一致) と一致すること (`tests/bankruptcy.rs`)
- サンプリングした仁が、サンプルを増やすと厳密なプレ仁に近づき、全提携で一致すること (`tests/sampled.rs`)
- per capita 仁・modiclus が文献の例と一致し、modiclus が定和ゲームでプレ仁に一致し凸ゲームでコアに属すること (Sudhölter 1997)、
  仁・カーネルの点が交渉集合に属し、凸ゲームで交渉集合がコアに一致すること (`tests/variants.rs`)
- Owen 値・Aumann–Drèze 値・提携構造つきの仁を、Shapley 値・仁・商ゲームとの関係で確かめる (`tests/partition.rs`)
- 費用ゲーム・空港ゲーム・最小全域木ゲーム・線形生産ゲームを、明示ゲームの計算とコアの定理で確かめる (`tests/cost_games.rs`)
- 保証の種類・分離オラクル・性質の宣言・凸ゲームの手法・事後検証・有理数で構築したゲーム (`tests/guarantees.rs`)
- 仁が厳密な検証に合格し、ずらした配分は不合格になること (`tests/exact.rs`)。有理数の単体法の判定が浮動小数点の LP と一致することは `exact::simplex` の単体テストで確かめる
- カーネル全体: 凸ゲームでは仁の 1 点になること、各多面体の頂点と重心がカーネル条件を満たすこと(健全性)、
  仁とランダムな初期点からの transfer scheme の到達点が和集合に含まれること(網羅性)
- チュートリアル (`docs/tutorial/`) のコード例が、論文の値と一致すること (doctest。`src/lib.rs` の `#[cfg(doctest)]` で読み込む)

## 公開 API の方針

crates.io で公開しているので、公開 API の変更は semver に従う。

- 内部でしか使わない関数やモジュールは `pub(crate)` にする。テストのためだけに公開しない (結合テストで足りなければ、単体テストに移す)。
- 後から変種を足しうる enum (`Error`、`Guarantee` など) と、後からフィールドを足しうる設定と結果の構造体には `#[non_exhaustive]` を付ける。
  三択で状態が確定している `Verified` と `Check`、2 値の `Domain` には付けない。
- 設定の構造体は、`Default` か `::new` で作ってからフィールドに代入する形で使う (`#[non_exhaustive]` の構造体は、クレートの外では構造体リテラルで作れない)。
- 既定の feature は空にし、ライブラリの依存を増やす機能は feature の後ろに置く。
  今は `serde_json` を使う `io` と、CLI の `cli` がある。
- 公開している型に出てくる依存 (`num-bigint`、`num-rational`、`num-traits`) は、lib.rs で再公開する。

## 文書と文献の方針

- `docs/` には、利用者と、現状を理解したい人向けの文書だけを置く。研究としての分析や開発の判断のための調査はリポジトリに置かない。
- 文献に依拠する主張は、[docs/references.md](docs/references.md) に文献と確認元を書き、計算で確かめられるものはテストかデータで確かめる。
- 文献の数値例を使うときは、出典と、答えを裏付けた方法 (原典、引用元、手計算) を書く ([docs/literature-cases.md](docs/literature-cases.md))。
- 保証つきのアルゴリズムは、原論文の本文で手順と保証を確かめてから実装する。
  要旨だけで実装すると、保証が成り立つことを確認できないからである。
  Pashkovich (2022)、Könemann, Pashkovich, Toth (2020)、Könemann & Toth (2020) は、本文を確認できていないため未実装にしている。
  Elkind & Pasechnik (2009) は、仁を計算する保証がないと後続研究で指摘されているので、実装の対象にしない。
- 文書に書いた計測と分析の数値は、`scripts/verify_claims.py` で CSV から計算し直して照合する。
  数値を変えたら、このスクリプトの期待値も更新する。

## 計測と再現

計測と分析のデータがどのコードと環境から作られたか、どう再現して照合するかは [docs/reproducibility.md](docs/reproducibility.md) にまとめている。
すぐにできる照合は次の 2 つである。

```bash
sha256sum -c data/SHA256SUMS             # 保存したデータが改変されていないか
python3 scripts/verify_claims.py         # 文書の数値を CSV から計算し直して照合
```

`data/` のデータを作り直したら、`data/SHA256SUMS` も更新する。
ベンチマークの記録 (`data/compare/*.csv` など) にある実装名 `coopgame-rs` は、リポジトリ名を変える前の名前だが、過去の計測結果の記録なので書き換えない。

## リリース

1. `Cargo.toml` と `python/pyproject.toml` の版を上げる。
2. 公開するファイルと、パッケージからビルドできることを確かめる。

   ```bash
   cargo package --list
   cargo publish --dry-run
   ```

3. `cargo publish` で crates.io に公開する。
4. Python バインディングは `python/` で `maturin build --release` を実行し、wheel を PyPI に `coopgame-py` として公開する。
