/-
coopgame が「構造的に成り立つ」として `Proven` の保証を付けている主張の証明。

* `bankruptcy_supermodular`       : 破産ゲームは凸 (src/games/bankruptcy.rs)
* `inducedSubgraph_supermodular`  : 辺の重みが非負の誘導部分グラフゲームは凸 (src/games/graph.rs)
* `airportSavings_supermodular`   : 空港ゲームの節約ゲームは凸 (src/games/airport.rs)
* `surplus_objective_submodular`  : 凸ゲームでは最大余剰の目的関数が劣モジュラ (src/nucleolus/convex.rs)
* `negPart_sum_le`                : 劣モジュラ最小化の双対ギャップの下界 (src/submodular.rs)

いずれも実数上の主張である。浮動小数点数の丸めは対象にしていない。
-/
import Mathlib

open Finset

namespace Coopgame

variable {ι : Type*} [DecidableEq ι]

/-- 優モジュラ (凸ゲーム): `v(S) + v(T) ≤ v(S ∪ T) + v(S ∩ T)`。 -/
def Supermodular (v : Finset ι → ℝ) : Prop :=
  ∀ S T, v S + v T ≤ v (S ∪ T) + v (S ∩ T)

/-- 劣モジュラ: `f(S ∪ T) + f(S ∩ T) ≤ f(S) + f(T)`。 -/
def Submodular (f : Finset ι → ℝ) : Prop :=
  ∀ S T, f (S ∪ T) + f (S ∩ T) ≤ f S + f T

/-! ## 破産ゲーム -/

/-- 破産ゲーム `v(S) = max(0, (E - D) + d(S))`。`c` が `E - D` に当たる。 -/
def bankruptcy (c : ℝ) (d : ι → ℝ) (S : Finset ι) : ℝ :=
  max 0 (c + ∑ i ∈ S, d i)

/-- 請求が非負なら、破産ゲームは凸である (`c` の符号は問わない)。 -/
theorem bankruptcy_supermodular (c : ℝ) (d : ι → ℝ) (hd : ∀ i, 0 ≤ d i) :
    Supermodular (bankruptcy c d) := by
  intro S T
  unfold bankruptcy
  have h := Finset.sum_union_inter (s₁ := S) (s₂ := T) (f := d)
  have hS : ∑ i ∈ S ∩ T, d i ≤ ∑ i ∈ S, d i :=
    Finset.sum_le_sum_of_subset_of_nonneg inter_subset_left (fun i _ _ => hd i)
  have hT : ∑ i ∈ S ∩ T, d i ≤ ∑ i ∈ T, d i :=
    Finset.sum_le_sum_of_subset_of_nonneg inter_subset_right (fun i _ _ => hd i)
  rcases max_cases 0 (c + ∑ i ∈ S, d i) with ⟨h1, _⟩ | ⟨h1, _⟩ <;>
  rcases max_cases 0 (c + ∑ i ∈ T, d i) with ⟨h2, _⟩ | ⟨h2, _⟩ <;>
  rcases max_cases 0 (c + ∑ i ∈ S ∪ T, d i) with ⟨h3, _⟩ | ⟨h3, _⟩ <;>
  rcases max_cases 0 (c + ∑ i ∈ S ∩ T, d i) with ⟨h4, _⟩ | ⟨h4, _⟩ <;>
  rw [h1, h2, h3, h4] <;> linarith

/-- コードの定義 `max(0, E - d(N \ S))` は `max(0, (E - D) + d(S))` に等しい。 -/
theorem bankruptcy_eq_complement_form [Fintype ι] (E : ℝ) (d : ι → ℝ) (S : Finset ι) :
    max 0 (E - ∑ i ∈ Sᶜ, d i) = bankruptcy (E - ∑ i, d i) d S := by
  unfold bankruptcy
  have h := Finset.sum_add_sum_compl S d
  congr 1
  linarith

/-! ## 誘導部分グラフゲーム -/

/-- 誘導部分グラフゲーム: 両端が `S` に入る辺の重みの和。辺は添字 `ε` で数え、多重辺を許す。 -/
def inducedSubgraph {ε : Type*} (edges : Finset ε) (src dst : ε → ι) (w : ε → ℝ)
    (S : Finset ι) : ℝ :=
  ∑ e ∈ edges, if src e ∈ S ∧ dst e ∈ S then w e else 0

/-- 辺の重みが非負なら、誘導部分グラフゲームは凸である。 -/
theorem inducedSubgraph_supermodular {ε : Type*} (edges : Finset ε) (src dst : ε → ι)
    (w : ε → ℝ) (hw : ∀ e, 0 ≤ w e) : Supermodular (inducedSubgraph edges src dst w) := by
  intro S T
  unfold inducedSubgraph
  rw [← Finset.sum_add_distrib, ← Finset.sum_add_distrib]
  apply Finset.sum_le_sum
  intro e _
  have := hw e
  by_cases h1 : src e ∈ S <;> by_cases h2 : src e ∈ T <;>
  by_cases h3 : dst e ∈ S <;> by_cases h4 : dst e ∈ T <;>
  simp [h1, h2, h3, h4, this]

/-! ## 空港ゲーム -/

/-- 空港ゲームの費用 `c(S) = max_{i ∈ S} c_i` (空提携は 0)。費用は非負なので `NNReal` で表す。 -/
noncomputable def airportCost (c : ι → NNReal) (S : Finset ι) : ℝ :=
  ((S.sup c : NNReal) : ℝ)

/-- 費用 `max` は劣モジュラである。 -/
theorem airportCost_submodular (c : ι → NNReal) : Submodular (airportCost c) := by
  intro S T
  unfold airportCost
  have hU : (S ∪ T).sup c = S.sup c ⊔ T.sup c := Finset.sup_union
  have hI : (S ∩ T).sup c ≤ S.sup c ⊓ T.sup c :=
    le_inf (Finset.sup_mono inter_subset_left) (Finset.sup_mono inter_subset_right)
  have hsum : S.sup c ⊔ T.sup c + S.sup c ⊓ T.sup c = S.sup c + T.sup c := max_add_min _ _
  have hI' : (((S ∩ T).sup c : NNReal) : ℝ) ≤ ((S.sup c ⊓ T.sup c : NNReal) : ℝ) := by
    exact_mod_cast hI
  have hsum' : ((S.sup c ⊔ T.sup c : NNReal) : ℝ) + ((S.sup c ⊓ T.sup c : NNReal) : ℝ)
      = ((S.sup c : NNReal) : ℝ) + ((T.sup c : NNReal) : ℝ) := by
    exact_mod_cast hsum
  rw [hU]
  linarith

/-- 節約ゲーム `s(S) = Σ_{i ∈ S} c_i - max_{i ∈ S} c_i`。 -/
noncomputable def airportSavings (c : ι → NNReal) (S : Finset ι) : ℝ :=
  (∑ i ∈ S, (c i : ℝ)) - airportCost c S

/-- 空港ゲームの節約ゲームは凸である。 -/
theorem airportSavings_supermodular (c : ι → NNReal) : Supermodular (airportSavings c) := by
  intro S T
  unfold airportSavings
  have h := Finset.sum_union_inter (s₁ := S) (s₂ := T) (f := fun i => (c i : ℝ))
  have hc := airportCost_submodular c S T
  linarith

/-! ## 最大余剰と劣モジュラ最小化 -/

/-- 凸ゲーム `v` と配分 `x` で、`T ↦ -(v(T ∪ {i}) - x(T ∪ {i}))` は劣モジュラである。
`convex.rs` はこの関数を最小化して最大余剰 `s_ij(x)` を求める。 -/
theorem surplus_objective_submodular (v : Finset ι → ℝ) (hv : Supermodular v) (x : ι → ℝ)
    (i : ι) : Submodular (fun T => -(v (insert i T) - ∑ k ∈ insert i T, x k)) := by
  intro S T
  have h := hv (insert i S) (insert i T)
  have hsum := Finset.sum_union_inter (s₁ := insert i S) (s₂ := insert i T) (f := x)
  have hU : insert i (S ∪ T) = insert i S ∪ insert i T := by
    ext k; simp only [mem_insert, mem_union]; tauto
  have hI : insert i (S ∩ T) = insert i S ∩ insert i T := by
    ext k; simp only [mem_insert, mem_inter]; tauto
  simp only [hU, hI]
  linarith

omit [DecidableEq ι] in
/-- 双対ギャップの下界。`x` が `x(A) ≤ f(A)` (全ての `A ⊆ V`) を満たすなら、
`x` の負の成分の和 `x⁻(V)` は `f` の最小値の下界である。
`submodular.rs` は `f(A) - x⁻(V)` が許容誤差以下になったら停止する。 -/
theorem negPart_sum_le (V : Finset ι) (f : Finset ι → ℝ) (x : ι → ℝ)
    (hx : ∀ A ⊆ V, ∑ k ∈ A, x k ≤ f A) (A : Finset ι) (hA : A ⊆ V) :
    ∑ k ∈ V, min (x k) 0 ≤ f A := by
  calc ∑ k ∈ V, min (x k) 0
      ≤ ∑ k ∈ A, min (x k) 0 :=
        Finset.sum_le_sum_of_subset_of_nonpos hA (fun k _ _ => min_le_right _ _)
    _ ≤ ∑ k ∈ A, x k := Finset.sum_le_sum (fun k _ => min_le_left _ _)
    _ ≤ f A := hx A hA

omit [DecidableEq ι] in
/-- 下界の前提 `x(A) ≤ f(A)` は凸結合で保たれる。最小ノルム点法の `x` は貪欲法の頂点の凸結合である。 -/
theorem polymatroid_ineq_convex_comb (V : Finset ι) (f : Finset ι → ℝ) (x y : ι → ℝ) (t : ℝ)
    (ht0 : 0 ≤ t) (ht1 : t ≤ 1)
    (hx : ∀ A ⊆ V, ∑ k ∈ A, x k ≤ f A) (hy : ∀ A ⊆ V, ∑ k ∈ A, y k ≤ f A)
    (A : Finset ι) (hA : A ⊆ V) :
    ∑ k ∈ A, (t * x k + (1 - t) * y k) ≤ f A := by
  rw [Finset.sum_add_distrib, ← Finset.mul_sum, ← Finset.mul_sum]
  have h1 := mul_le_mul_of_nonneg_left (hx A hA) ht0
  have h2 := mul_le_mul_of_nonneg_left (hy A hA) (sub_nonneg.mpr ht1)
  nlinarith

end Coopgame
