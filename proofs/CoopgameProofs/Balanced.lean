/-
Kohlberg 基準の判定 (src/verify/kohlberg.rs, src/verify/exact.rs) が使う 2 つの変形の証明。

* `PosCombination.mono_of_span_eq_top` : 族が全空間を張って平衡なら、それを含むどの族も平衡である
  (`PosCombination.union_of_mem_span` から導く)。判定はこれを根拠に、族の階数が n に達した段で打ち切る。
* `posCombination_iff_ge_one`        : 「正の重みで b を表せる」ことは、
  「1 以上の重みで b の正数倍を表せる」ことと同値である。`is_balanced` はこの形を LP で判定する。

ベクトル `a i` が提携の特性ベクトル、`b` が `1_N` (仁の場合は下限に張り付いた単集合の分を引いたもの) に当たる。
Kohlberg 基準そのもの (平衡性と仁・プレ仁の同値) は文献の定理であり、ここでは証明していない。
-/
import Mathlib

open Finset

namespace Coopgame

variable {ι V : Type*} [DecidableEq ι] [AddCommGroup V] [Module ℝ V]

/-- 族 `s` の全てのベクトルに正の重みを付けて `b` を表せる。 -/
def PosCombination (s : Finset ι) (a : ι → V) (b : V) : Prop :=
  ∃ w : ι → ℝ, (∀ i ∈ s, 0 < w i) ∧ ∑ i ∈ s, w i • a i = b

omit [DecidableEq ι] in
/-- 有限個の正の数 `w i` に対して、`ε * c i < w i` を全て満たす正の `ε` がある。 -/
theorem exists_small_step (s : Finset ι) (w c : ι → ℝ) (hw : ∀ i ∈ s, 0 < w i) :
    ∃ ε : ℝ, 0 < ε ∧ ∀ i ∈ s, ε * c i < w i := by
  have hterm : ∀ j ∈ s, 0 ≤ |c j| / w j := fun j hj => div_nonneg (abs_nonneg _) (hw j hj).le
  have hnn : 0 ≤ ∑ j ∈ s, |c j| / w j := Finset.sum_nonneg hterm
  have hpos : 0 < 1 + ∑ j ∈ s, |c j| / w j := by linarith
  refine ⟨(1 + ∑ j ∈ s, |c j| / w j)⁻¹, inv_pos.mpr hpos, ?_⟩
  intro i hi
  rw [inv_mul_lt_iff₀ hpos]
  have hle : |c i| / w i ≤ ∑ j ∈ s, |c j| / w j :=
    Finset.single_le_sum (f := fun j => |c j| / w j) hterm hi
  have hci : |c i| ≤ (∑ j ∈ s, |c j| / w j) * w i := by
    have := mul_le_mul_of_nonneg_right hle (hw i hi).le
    rwa [div_mul_cancel₀ _ (hw i hi).ne'] at this
  nlinarith [le_abs_self (c i), hw i hi]

/-- 族の張る空間に入るベクトルを 1 つ足しても、正の重みで `b` を表せる。 -/
theorem PosCombination.insert_of_mem_span {s : Finset ι} {a : ι → V} {b : V}
    (h : PosCombination s a b) {j : ι}
    (hj : a j ∈ Submodule.span ℝ (a '' (s : Set ι))) :
    PosCombination (insert j s) a b := by
  by_cases hjs : j ∈ s
  · rwa [Finset.insert_eq_of_mem hjs]
  obtain ⟨w, hw, hsum⟩ := h
  obtain ⟨c, hc⟩ := (Submodule.mem_span_image_finset_iff_exists_fun' ℝ).mp hj
  obtain ⟨ε, hε, hsmall⟩ := exists_small_step s w c hw
  refine ⟨fun i => if i = j then ε else w i - ε * c i, ?_, ?_⟩
  · intro i hi
    rcases Finset.mem_insert.mp hi with rfl | his
    · simpa using hε
    · have hne : i ≠ j := fun h => hjs (h ▸ his)
      simpa [hne] using hsmall i his
  · rw [Finset.sum_insert hjs]
    have hrest : ∑ i ∈ s, (if i = j then ε else w i - ε * c i) • a i
        = ∑ i ∈ s, (w i • a i - ε • (c i • a i)) := by
      apply Finset.sum_congr rfl
      intro i his
      have hne : i ≠ j := fun h => hjs (h ▸ his)
      simp [hne, sub_smul, mul_smul]
    rw [hrest, Finset.sum_sub_distrib, ← Finset.smul_sum, hc, hsum]
    simp

/-- 族 `s` が正の重みで `b` を表し、`u` の全てのベクトルが `s` の張る空間に入るなら、
`s ∪ u` も正の重みで `b` を表す。`s` が全空間を張れば、`u` は任意でよい。 -/
theorem PosCombination.union_of_mem_span {s : Finset ι} {a : ι → V} {b : V}
    (h : PosCombination s a b) (u : Finset ι)
    (hu : ∀ j ∈ u, a j ∈ Submodule.span ℝ (a '' (s : Set ι))) :
    PosCombination (s ∪ u) a b := by
  induction u using Finset.induction_on with
  | empty => simpa using h
  | insert j u _ ih =>
    have ih' := ih (fun k hk => hu k (Finset.mem_insert_of_mem hk))
    have hj : a j ∈ Submodule.span ℝ (a '' ((s ∪ u : Finset ι) : Set ι)) := by
      refine Submodule.span_mono ?_ (hu j (Finset.mem_insert_self j u))
      apply Set.image_mono
      exact_mod_cast Finset.subset_union_left
    have := ih'.insert_of_mem_span hj
    rwa [Finset.union_insert]

/-- 打ち切りの根拠。族 `s` が全空間を張って正の重みで `b` を表すなら、
`s` を含むどの族も正の重みで `b` を表す。 -/
theorem PosCombination.mono_of_span_eq_top {s t : Finset ι} {a : ι → V} {b : V}
    (h : PosCombination s a b) (hspan : Submodule.span ℝ (a '' (s : Set ι)) = ⊤)
    (hst : s ⊆ t) : PosCombination t a b := by
  have := h.union_of_mem_span t (fun j _ => by rw [hspan]; exact Submodule.mem_top)
  rwa [Finset.union_eq_right.mpr hst] at this

omit [DecidableEq ι] in
/-- `is_balanced` の LP の形。空でない族 `s` について、正の重みで `b` を表せることは、
1 以上の重みで `b` の正数倍を表せることと同値である。 -/
theorem posCombination_iff_ge_one {s : Finset ι} (hs : s.Nonempty) {a : ι → V} {b : V} :
    PosCombination s a b ↔
      ∃ w : ι → ℝ, ∃ μ : ℝ, 0 < μ ∧ (∀ i ∈ s, 1 ≤ w i) ∧ ∑ i ∈ s, w i • a i = μ • b := by
  constructor
  · rintro ⟨w, hw, hsum⟩
    -- 最小の重みで割る。
    obtain ⟨m, hm, hmin⟩ := s.exists_min_image w hs
    have hmpos : 0 < w m := hw m hm
    refine ⟨fun i => w i / w m, (w m)⁻¹, inv_pos.mpr hmpos, ?_, ?_⟩
    · intro i hi
      rw [le_div_iff₀ hmpos, one_mul]
      exact hmin i hi
    · rw [← hsum, Finset.smul_sum]
      apply Finset.sum_congr rfl
      intro i _
      simp only [smul_smul, div_eq_inv_mul]
  · rintro ⟨w, μ, hμ, hw, hsum⟩
    refine ⟨fun i => w i / μ, fun i hi => div_pos (lt_of_lt_of_le one_pos (hw i hi)) hμ, ?_⟩
    have : ∑ i ∈ s, (w i / μ) • a i = μ⁻¹ • ∑ i ∈ s, w i • a i := by
      rw [Finset.smul_sum]
      apply Finset.sum_congr rfl
      intro i _
      simp only [smul_smul, div_eq_inv_mul]
    rw [this, hsum, smul_smul, inv_mul_cancel₀ hμ.ne', one_smul]

omit [DecidableEq ι] in
/-- `is_balanced` の LP は `μ ≥ 0` しか課さないが、族が空でなければ `μ ≥ 1` が従う。
`a i` は非負の成分を持ち (特性ベクトル)、`s` のどのベクトルにも 1 以上の成分がある (提携は空でない)。
`r` は下限に張り付いた単集合に非負の重みを付けた和に当たる。 -/
theorem one_le_scale_of_ge_one {κ : Type*} {s : Finset ι} (hs : s.Nonempty) (a : ι → κ → ℝ)
    (hnn : ∀ i k, 0 ≤ a i k) (hne : ∀ i ∈ s, ∃ k, 1 ≤ a i k) (r : κ → ℝ) (hr : ∀ k, 0 ≤ r k)
    (w : ι → ℝ) (hw : ∀ i ∈ s, 1 ≤ w i) (μ : ℝ)
    (hsum : ∀ k, ∑ i ∈ s, w i * a i k + r k = μ) : 1 ≤ μ := by
  obtain ⟨i₀, hi₀⟩ := hs
  obtain ⟨k, hk⟩ := hne i₀ hi₀
  have hterm : ∀ i ∈ s, 0 ≤ w i * a i k :=
    fun i hi => mul_nonneg (le_trans zero_le_one (hw i hi)) (hnn i k)
  have hsingle : w i₀ * a i₀ k ≤ ∑ i ∈ s, w i * a i k :=
    Finset.single_le_sum (f := fun i => w i * a i k) hterm hi₀
  have hprod : 1 ≤ w i₀ * a i₀ k := by nlinarith [hw i₀ hi₀]
  have := hsum k
  have := hr k
  linarith

end Coopgame
