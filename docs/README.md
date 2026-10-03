# coopgame のドキュメント

## 学ぶ

- [チュートリアル](tutorial/README.md): 古典的な論文の結果を再現しながら、基本的な使い方を学ぶ。

## 使い方の詳細

- [機能一覧](api-overview.md): 全てのモジュール・関数と、使っている手法。
- [CLI](cli.md): `coopgame` コマンドのサブコマンドとオプション。
- [入力形式](input-format.md): 特性関数のファイルの並び順、プレイヤー名付きの JSON・CSV。
- [大きいゲーム](large-games.md): オラクル・凸ゲームの手法・サンプリングの使い分け。
- [性能と制約](performance.md): 規模の上限、数値の制約、計測例。

## 保証と根拠

- [保証の種類と、性質に基づく手法の使い分け](guarantees.md): 結果に付く保証の設計と、性質を仮定した手法の実験。
- [既存実装との比較](comparison.md): CoopGame・TUGLab (R) との速さと正しさの比較。
- [文献の例の再現](literature-cases.md): 教科書・論文の数値例の再現。
- [参考文献と、主張の根拠](references.md): 依拠した文献と、主張ごとの確認方法。
- [実験の再現と検証](reproducibility.md): 実験データの出所と、再現・照合の手順。
