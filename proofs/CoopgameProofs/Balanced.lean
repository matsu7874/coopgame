/-
Kohlberg 基準の判定 (src/levels.rs, src/verify/exact.rs) が使う 2 つの変形の証明。

* `PosCombination.mono_of_span_insert_eq_top` : 族が正の重みで `b - r` を表し、族と `b` が全空間を張るなら、
  それを含むどの族も、ある正の数 `μ` について正の重みで `b - μ • r` を表す
  (`PosCombination.union_of_mem_span_insert` から導く)。判定はこれを根拠に、族と `1_N` の階数が n に達した段で
  打ち切る。
* `posCombination_iff_ge_one`        : 「正の重みで b を表せる」ことは、
  「1 以上の重みで b の正数倍を表せる」ことと同値である。`is_balanced` はこの形を LP で判定する。

ベクトル `a i` が提携の特性ベクトル、`b` が `1_N` に当たる。`r` は、仁の場合に下限に張り付いた単集合に
非負の重みを付けた和に当たる (プレ仁では 0)。`μ • r` も同じ単集合に非負の重みを付けた和なので、
正の重みで `b - μ • r` を表せることは、その族が平衡であることを意味する。
`posCombination_iff_ge_one` では、`b` が `1_N` から `r` を引いたものに当たる。
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
/-- 有限個の正の数 `w i` と実数 `β` に対して、`ε * c i < w i` を全て満たし、`0 < 1 + ε * β` も満たす
正の `ε` がある。 -/
theorem exists_small_step (s : Finset ι) (w c : ι → ℝ) (hw : ∀ i ∈ s, 0 < w i) (β : ℝ) :
    ∃ ε : ℝ, 0 < ε ∧ (∀ i ∈ s, ε * c i < w i) ∧ 0 < 1 + ε * β := by
  have hterm : ∀ j ∈ s, 0 ≤ |c j| / w j := fun j hj => div_nonneg (abs_nonneg _) (hw j hj).le
  have hnn : 0 ≤ ∑ j ∈ s, |c j| / w j := Finset.sum_nonneg hterm
  have hβ : 0 ≤ |β| := abs_nonneg β
  have hpos : 0 < 1 + |β| + ∑ j ∈ s, |c j| / w j := by linarith
  have hε : 0 < (1 + |β| + ∑ j ∈ s, |c j| / w j)⁻¹ := inv_pos.mpr hpos
  refine ⟨(1 + |β| + ∑ j ∈ s, |c j| / w j)⁻¹, hε, ?_, ?_⟩
  · intro i hi
    rw [inv_mul_lt_iff₀ hpos]
    have hle : |c i| / w i ≤ ∑ j ∈ s, |c j| / w j :=
      Finset.single_le_sum (f := fun j => |c j| / w j) hterm hi
    have hci : |c i| ≤ (∑ j ∈ s, |c j| / w j) * w i := by
      have := mul_le_mul_of_nonneg_right hle (hw i hi).le
      rwa [div_mul_cancel₀ _ (hw i hi).ne'] at this
    nlinarith [le_abs_self (c i), hw i hi, mul_nonneg hβ (hw i hi).le]
  · have hlt : (1 + |β| + ∑ j ∈ s, |c j| / w j)⁻¹ * |β| < 1 := by
      rw [inv_mul_lt_iff₀ hpos]
      linarith
    have hge : 0 ≤ (1 + |β| + ∑ j ∈ s, |c j| / w j)⁻¹ * (β + |β|) :=
      mul_nonneg hε.le (by linarith [neg_abs_le β])
    nlinarith

/-- 張る空間に `b` も数えてよい。族 `s` が正の重みで `b - r` を表し、`a j` が `s` と `b` の張る空間に入るなら、
ある正の数 `μ` について、`j` を足した族は正の重みで `b - μ • r` を表す。 -/
theorem PosCombination.insert_of_mem_span_insert {s : Finset ι} {a : ι → V} {b r : V}
    (h : PosCombination s a (b - r)) {j : ι}
    (hj : a j ∈ Submodule.span ℝ (insert b (a '' (s : Set ι)))) :
    ∃ μ : ℝ, 0 < μ ∧ PosCombination (insert j s) a (b - μ • r) := by
  by_cases hjs : j ∈ s
  · exact ⟨1, one_pos, by rwa [Finset.insert_eq_of_mem hjs, one_smul]⟩
  obtain ⟨w, hw, hsum⟩ := h
  obtain ⟨β, z, hz, hjeq⟩ := Submodule.mem_span_insert.mp hj
  obtain ⟨c, hc⟩ := (Submodule.mem_span_image_finset_iff_exists_fun' ℝ).mp hz
  obtain ⟨ε, hε, hsmall, hscale⟩ := exists_small_step s w c hw β
  -- `ε • a j + Σ (w i - ε c i) • a i = (1 + ε β) • b - r` を `1 + ε β` で割る。
  set μ : ℝ := (1 + ε * β)⁻¹ with hμdef
  have hμ : 0 < μ := inv_pos.mpr hscale
  have hμone : μ * ε * β + μ = 1 := by
    have : μ * ε * β + μ = μ * (1 + ε * β) := by ring
    rw [this, hμdef, inv_mul_cancel₀ hscale.ne']
  refine ⟨μ, hμ, fun i => if i = j then μ * ε else μ * (w i - ε * c i), ?_, ?_⟩
  · intro i hi
    rcases Finset.mem_insert.mp hi with rfl | his
    · simpa using mul_pos hμ hε
    · have hne : i ≠ j := fun h => hjs (h ▸ his)
      have hpos : 0 < w i - ε * c i := by linarith [hsmall i his]
      simpa [hne] using mul_pos hμ hpos
  · rw [Finset.sum_insert hjs]
    have hrest : ∑ i ∈ s, (if i = j then μ * ε else μ * (w i - ε * c i)) • a i
        = μ • (∑ i ∈ s, w i • a i) - (μ * ε) • ∑ i ∈ s, c i • a i := by
      rw [Finset.smul_sum, Finset.smul_sum, ← Finset.sum_sub_distrib]
      apply Finset.sum_congr rfl
      intro i his
      have hne : i ≠ j := fun h => hjs (h ▸ his)
      simp [hne, sub_smul, mul_smul, mul_sub]
    rw [hrest, hc, hsum, hjeq]
    simp only [ite_true]
    calc (μ * ε) • (β • b + z) + (μ • (b - r) - (μ * ε) • z)
        = (μ * ε * β + μ) • b - μ • r := by
          simp only [smul_add, smul_sub, smul_smul, add_smul]
          abel
      _ = b - μ • r := by rw [hμone, one_smul]

/-- 族 `s` が正の重みで `b - r` を表し、`u` の全てのベクトルが `s` と `b` の張る空間に入るなら、
ある正の数 `μ` について、`s ∪ u` は正の重みで `b - μ • r` を表す。 -/
theorem PosCombination.union_of_mem_span_insert {s : Finset ι} {a : ι → V} {b r : V}
    (h : PosCombination s a (b - r)) (u : Finset ι)
    (hu : ∀ j ∈ u, a j ∈ Submodule.span ℝ (insert b (a '' (s : Set ι)))) :
    ∃ μ : ℝ, 0 < μ ∧ PosCombination (s ∪ u) a (b - μ • r) := by
  induction u using Finset.induction_on with
  | empty => exact ⟨1, one_pos, by simpa using h⟩
  | insert j u _ ih =>
    obtain ⟨μ, hμ, ih'⟩ := ih (fun k hk => hu k (Finset.mem_insert_of_mem hk))
    have hj : a j ∈ Submodule.span ℝ (insert b (a '' ((s ∪ u : Finset ι) : Set ι))) := by
      refine Submodule.span_mono ?_ (hu j (Finset.mem_insert_self j u))
      apply Set.insert_subset_insert
      apply Set.image_mono
      exact_mod_cast Finset.subset_union_left
    obtain ⟨ν, hν, hins⟩ := ih'.insert_of_mem_span_insert hj
    refine ⟨ν * μ, mul_pos hν hμ, ?_⟩
    rw [Finset.union_insert, mul_smul]
    exact hins

/-- 打ち切りの根拠。族 `s` が正の重みで `b - r` を表し、`s` と `b` が全空間を張るなら、
`s` を含むどの族も、ある正の数 `μ` について正の重みで `b - μ • r` を表す。 -/
theorem PosCombination.mono_of_span_insert_eq_top {s t : Finset ι} {a : ι → V} {b r : V}
    (h : PosCombination s a (b - r))
    (hspan : Submodule.span ℝ (insert b (a '' (s : Set ι))) = ⊤)
    (hst : s ⊆ t) : ∃ μ : ℝ, 0 < μ ∧ PosCombination t a (b - μ • r) := by
  obtain ⟨μ, hμ, hunion⟩ :=
    h.union_of_mem_span_insert t (fun j _ => by rw [hspan]; exact Submodule.mem_top)
  exact ⟨μ, hμ, by rwa [Finset.union_eq_right.mpr hst] at hunion⟩

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
