# Lean による証明

coopgame がリポジトリ内で導いて使っている数学的な主張のうち、次の 2 種類を Lean 4 と Mathlib で証明している。

- 保証 (`proven`・`certified`) の根拠になる主張
- 実装が使っている式の変形

文献の定理に依拠する主張は [docs/references.md](../docs/references.md) にまとめている。

## 証明した主張

| 主張 | 使っている場所 | 定理 |
|---|---|---|
| 請求が非負なら、破産ゲーム `v(S) = max(0, (E - D) + d(S))` は凸 | `src/games/bankruptcy.rs` (`ConvexGame` の `Proven`) | `bankruptcy_supermodular`、`bankruptcy_eq_complement_form` (`Structural.lean`) |
| 辺の重みが非負なら、誘導部分グラフゲームは凸 | `src/games/graph.rs` (`ConvexGame` の `Proven`) | `inducedSubgraph_supermodular` (`Structural.lean`) |
| 空港ゲームの費用 `max` は劣モジュラで、節約ゲームは凸 | `src/games/airport.rs` (`ConvexGame` の `Proven`) | `airportCost_submodular`、`airportSavings_supermodular` (`Structural.lean`) |
| 凸ゲームでは、最大余剰の目的関数 `T ↦ -(v(T ∪ {i}) - x(T ∪ {i}))` は劣モジュラ | `src/nucleolus/convex.rs` | `surplus_objective_submodular` (`Structural.lean`) |
| `x(A) <= f(A)` (全ての `A`) なら、`x` の負の成分の和は `f` の最小値の下界。この前提は凸結合で保たれる | `src/submodular.rs` (双対ギャップによる停止) | `negPart_sum_le`、`polymatroid_ineq_convex_comb` (`Structural.lean`) |
| 提携の族が全空間を張って平衡なら、それを含むどの族も平衡 | `src/verify/kohlberg.rs`、`src/verify/exact.rs` (Kohlberg 判定の打ち切り) | `PosCombination.mono_of_span_eq_top` (`Balanced.lean`) |
| 「正の重みで `1_N` を表せる」は「1 以上の重みで `1_N` の正数倍を表せる」と同値。族が空でなければ倍率は 1 以上 | `src/verify/exact.rs`・`src/verify/kohlberg.rs` の `is_balanced` (平衡性の LP) | `posCombination_iff_ge_one`、`one_le_scale_of_ge_one` (`Balanced.lean`) |
| 確定した提携の張る空間が全空間でなければ、未確定の 1 人提携とその補集合があり、段の値は下に有界 | `src/nucleolus/mod.rs`・`exact.rs`・`oracle.rs` (逐次 LP の有界性) | `exists_single_not_mem`、`mem_of_compl_mem`、`level_lower_bound` (`SequentialLp.lean`) |
| 破産ゲームの超過は `max(-x(S), (E - D) + Σ_{i ∈ S} (d_i - x_i))` | `src/games/bankruptcy.rs` (オラクル) | `bankruptcy_excess` (`Identities.lean`) |
| 節約ゲームの超過は、費用の分担 `y_i = c_i - x_i` が提携の費用を超える額 `y(S) - c(S)` に等しい | `src/games/cost.rs` | `savings_excess_eq`、`savings_efficient_iff` (`Identities.lean`) |
| Gately 点の閉じた形は効率的で、抜ける傾向は `M_i ≠ v({i})` のプレイヤーの間で等しい (`(1 - t) / t`) | `src/compromise.rs` | `gately_efficient`、`gately_numerator`、`gately_propensity` (`Identities.lean`) |
| 効率的な配分では、双対ゲームの超過は補集合の超過の符号を変えたもの。modiclus の bi-excess の 2 つの書き方は等しい | `src/nucleolus/variants.rs` (`anti_prenucleolus`、`modiclus`) | `dual_excess`、`bi_excess_eq` (`Identities.lean`) |
| 抜ける傾向は、不満 `f = e(S, x) / b(S)` を使って `-1/f - 1` と書ける (`x(S) ≠ v(S)` のとき)。この関数は `f < 0` で狭義単調増加 | `src/nucleolus/variants.rs` (`disruption_nucleolus`) | `disruption_eq`、`disruption_strictMono` (`Identities.lean`) |

定理は全て名前空間 `Coopgame` にある。
Kohlberg 判定の 2 つの定理では、ベクトル `a i` が提携の特性ベクトル、`b` が `1_N` (仁の場合は、下限に張り付いた単集合に非負の重みを付けた分を引いたもの) に当たる。

## 証明の対象にしていないこと

- 逐次 LP の各段で、確定する提携が必ず見つかること (LP の双対と相補性による)。
- 不満に狭義単調増加な変換を掛けても、辞書式最小化の解が変わらないこと (disruption nucleolus の書き換えが使う)。
- 文献の定理: Kohlberg 基準と仁・プレ仁の同値、凸ゲームのカーネルが仁の 1 点であること、transfer scheme の収束、タルムード則と破産ゲームの仁の一致、貪欲法が基多面体の頂点を与えること (Edmonds) など。
- 浮動小数点数の丸めと許容誤差。定理は実数上の主張である。
- Rust のコードが定理の式どおりに計算していること。コードと式の対応はテストで確かめている。

## 確かめ方

Lean のツールチェーン管理に [elan](https://github.com/leanprover/elan) を使う。
Lean と Mathlib の版は `lean-toolchain` と `lake-manifest.json` で固定している。

```bash
cd proofs
lake exe cache get   # Mathlib のビルド済みファイルを取得する (展開後は約 7 GB)
lake build
```

`lake build` がエラーなく終われば、全ての定理が検査に通っている。
`sorry` は使っておらず、どの定理も Lean の標準の公理 (`propext`・`Classical.choice`・`Quot.sound`) だけに依存する。
依存する公理は `#print axioms Coopgame.bankruptcy_supermodular` のようにして確かめられる。
