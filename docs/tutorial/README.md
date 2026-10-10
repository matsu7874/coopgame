# チュートリアル: 古典的な論文の結果を再現する

協力ゲーム理論の古典的な論文の結果を再現しながら、coopgame の使い方を学ぶ。

## 準備

`Cargo.toml` に次を書く。

```toml
[dependencies]
coopgame = "0.1"
```

各章のコードは、そのまま `src/main.rs` に貼れば動く完全なプログラムである。
リポジトリでは `cargo test --doc` で全てのコードを実行し、論文の値と一致することを `assert!` で確かめている。

## 章

| 章 | 内容 | 使う機能 |
|---|---|---|
| [1. ゲームの表し方](01-representing-games.md) | 特性関数と提携の表し方 | `ExplicitGame`、`Coalition` |
| [2. 投票力を測る](02-voting-power.md) | 国連安全保障理事会の投票力 (Shapley–Shubik 1954)、Nassau 郡の議会 (Banzhaf 1965) | `values::shapley`、`values::banzhaf`、`WeightedVotingGame` |
| [3. 公平に分ける](03-fair-division.md) | タルムードの破産問題 (Aumann–Maschler 1985)、空港ゲームと Ibn Ezra の相続 (Littlechild–Owen 1973、Aumann 2010) | `games::bankruptcy::talmud_rule`、`nucleolus::exact`、`AirportGame` |
| [4. 安定性と検証](04-stability-and-verification.md) | 凸ゲームのコアと Shapley 値 (Shapley 1971)、仁の検証 (Kohlberg 1971) | `properties`、`nucleolus`、`verify::kohlberg` |

1 章で基本の型を説明し、2-4 章はどの順に読んでもよい。

## 次に読むもの

- 全提携を列挙できない大きいゲーム、人数の多いゲームの近似: [大きいゲーム](../large-games.md)
- 同じ計算を CLI で行う: [CLI](../cli.md)
- プレイヤー名付きの JSON・CSV でゲームを読む: [入力形式](../input-format.md)
- 全てのモジュールと手法: [機能一覧](../api-overview.md)
