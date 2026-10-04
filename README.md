# coopgame

TU 協力ゲームの仁・カーネル・Shapley 値などの解を計算し、検証する Rust ライブラリと CLI。

## 機能

- **解を求める**
  - 仁・プレ仁・最小コア (`nucleolus`)。有理数による厳密な計算にも対応 (`exact`)
  - カーネル・プレカーネルの 1 点 (`kernel`) と、6 人までの全体 (`kernel_set`)
  - Shapley 値・Banzhaf 値の厳密計算とサンプリング推定、solidarity 値 (`values`)
  - tau 値・Gately 点 (`compromise`)、通信グラフのもとでの Myerson 値 (`communication`)
  - 単純ゲームの投票力指数: Johnston・Deegan–Packel・Public Good (Holler)・Coleman (`power`)
  - per capita 仁・比例仁・modiclus (`variants`)
  - 提携構造のある解: Aumann–Drèze 値・Owen 値・提携構造つきの仁 (`partition`)
- **結果を検証する**
  - 仁の Kohlberg 基準 (`kohlberg`) と、有理数による厳密な検証 (`exact`)
  - カーネル条件 (`kernel`)・交渉集合への所属 (`bargaining`)
  - 凸性・コアなどゲームの性質 (`properties`)
  - 結果に付く保証の種類と事後検証 (`verify`、`auto`)
- **大きいゲームを扱う**
  - 全提携を列挙しないオラクル (`oracle`)
  - 凸ゲームの仁 (`convex`)
  - 提携のサンプリングによる近似 (`sampled`)
- **特定のゲームを解く**
  - 破産ゲーム (タルムード則)、空港ゲーム、最小全域木ゲーム、線形生産ゲーム、重み付き投票ゲーム、費用ゲーム
- **配分を分析する**
  - 配分の説明と比較 (`explain`)、値の不確かさと感度 (`uncertainty`)
  - 3-4 人の配分集合の図 (`plot`)、性質の反例の探索 (`search`)
- **入出力**
  - プレイヤー名付きの JSON・CSV (`io`)、ランダムなゲームの生成 (`generators`)

関数と手法の一覧は [docs/api-overview.md](docs/api-overview.md) にある。
LP ソルバーは純 Rust の [microlp](https://crates.io/crates/microlp) を使うので、外部のソルバーは不要。

## インストール

ライブラリとして使う場合:

```toml
[dependencies]
coopgame = "0.1"
```

プレイヤー名付きの JSON・CSV を読み書きする `io` モジュールは、feature `io` で有効にする。

CLI (`coopgame` コマンド) を使う場合:

```bash
cargo install coopgame --features cli
```

## 使い方

```rust
use coopgame::{Domain, ExplicitGame, kernel, kohlberg, nucleolus, values};

fn main() -> coopgame::Result<()> {
    // 3 人ゲームの特性関数 (辞書式順: {1}, {2}, {3}, {1,2}, {1,3}, {2,3}, {1,2,3})
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0])?;

    let x = nucleolus::nucleolus(&game)?.allocation; // 仁 [2, 2, 0]
    assert!(kohlberg::verify(&game, &x, Domain::Imputation)?.satisfied);
    assert!(kernel::is_in_kernel(&game, &x, Domain::Imputation, 1e-9));

    let phi = values::shapley(&game); // Shapley 値 [2, 2, 0]
    assert!(phi.iter().zip(&x).all(|(a, b)| (a - b).abs() < 1e-9));
    Ok(())
}
```

CLI では、1 行 1 値のファイルで特性関数を与える。

```bash
coopgame generate --type 1 --n 5 --seed 1 > v.txt   # ランダムなゲーム
coopgame nucleolus v.txt > sol.txt                   # 仁
coopgame verify v.txt sol.txt                        # Kohlberg 基準とカーネル条件で検証
coopgame shapley v.txt                               # Shapley 値
```

全てのサブコマンドは [docs/cli.md](docs/cli.md)、入力ファイルの並び順と JSON・CSV の形式は [docs/input-format.md](docs/input-format.md) を参照。

## ドキュメント

- [チュートリアル](docs/tutorial/README.md): 古典的な論文 (Shapley–Shubik の投票力指数、タルムードの破産問題など) の結果を再現しながら使い方を学ぶ。
- [大きいゲーム](docs/large-games.md)・[性能と制約](docs/performance.md): 30 人を超えるゲームの扱い方、規模の上限、計測例。
- [保証の種類](docs/guarantees.md)・[既存実装との比較](docs/comparison.md)・[参考文献](docs/references.md): 結果の正しさの根拠。

全ての文書の一覧は [docs/README.md](docs/README.md) にある。
API リファレンスは [docs.rs](https://docs.rs/coopgame)。

## Python バインディング

`python/` に PyO3 によるバインディングがある (PyPI の配布名は `coopgame-py`、import 名は `coopgame`)。
ビルド方法と API は [python/README.md](python/README.md) を参照。

## コントリビューション

開発環境、テストの方針、リリースの手順は [CONTRIBUTING.md](CONTRIBUTING.md) にまとめている。

## ライセンス

MIT License。[LICENSE](LICENSE) を参照。
