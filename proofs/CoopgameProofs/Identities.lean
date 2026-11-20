/-
実装が使っている式の変形の証明。

* `bankruptcy_excess`            : 破産ゲームの超過は 2 つの加法的な関数の最大値 (src/games/bankruptcy.rs のオラクル)
* `savings_excess_eq`            : 節約ゲームの超過と費用の分担の対応 (src/games/cost.rs)
* `gately_efficient` ほか        : Gately 点の閉じた形 (src/compromise.rs)
* `disruption_eq` ほか           : disruption nucleolus の不満の変形 (src/nucleolus/variants.rs)
* `dual_excess`・`bi_excess_eq`  : 双対ゲームの超過と modiclus の bi-excess (src/nucleolus/variants.rs)
-/
import Mathlib

open Finset

namespace Coopgame

variable {ι : Type*}

/-! ## 破産ゲームのオラクル -/

/-- 破産ゲーム `v(S) = max(0, c + d(S))` (`c = E - D`) の超過 `v(S) - x(S)` は、
`-x(S)` と `c + Σ_{i ∈ S} (d_i - x_i)` の最大値である。 -/
theorem bankruptcy_excess (c : ℝ) (d x : ι → ℝ) (S : Finset ι) :
    max 0 (c + ∑ i ∈ S, d i) - ∑ i ∈ S, x i
      = max (-∑ i ∈ S, x i) (c + ∑ i ∈ S, (d i - x i)) := by
  rw [Finset.sum_sub_distrib]
  rcases max_cases 0 (c + ∑ i ∈ S, d i) with ⟨h1, _⟩ | ⟨h1, _⟩ <;>
  rcases max_cases (-∑ i ∈ S, x i) (c + (∑ i ∈ S, d i - ∑ i ∈ S, x i)) with ⟨h3, _⟩ | ⟨h3, _⟩ <;>
  rw [h1, h3] <;> linarith

/-! ## 費用ゲームと節約ゲーム -/

/-- 節約ゲーム `s(S) = Σ_{i ∈ S} c_i - c(S)` の配分 `x` での超過は、費用の分担 `y_i = c_i - x_i` が
提携の費用を超える額 `y(S) - c(S)` に等しい。超過に基づく解はこの対応で移り合う。 -/
theorem savings_excess_eq (standalone x : ι → ℝ) (cost : Finset ι → ℝ) (S : Finset ι) :
    ((∑ i ∈ S, standalone i) - cost S) - ∑ i ∈ S, x i
      = (∑ i ∈ S, (standalone i - x i)) - cost S := by
  rw [Finset.sum_sub_distrib]
  ring

/-- `x` が節約ゲームで効率的 (`x(N) = s(N)`) なら、分担 `y` は費用を過不足なく賄う (`y(N) = c(N)`)。 -/
theorem savings_efficient_iff (standalone x : ι → ℝ) (cost : Finset ι → ℝ) (N : Finset ι) :
    ∑ i ∈ N, x i = (∑ i ∈ N, standalone i) - cost N
      ↔ ∑ i ∈ N, (standalone i - x i) = cost N := by
  rw [Finset.sum_sub_distrib]
  constructor <;> intro h <;> linarith

/-! ## Gately 点 -/

/-- Gately 点 `x_i = v_i + t (M_i - v_i)`、`t = (v(N) - Σ v_j) / Σ (M_j - v_j)` は効率的である。 -/
theorem gately_efficient (N : Finset ι) (single utopia : ι → ℝ) (total : ℝ)
    (hspread : ∑ j ∈ N, (utopia j - single j) ≠ 0) :
    ∑ i ∈ N, (single i
        + (total - ∑ j ∈ N, single j) / (∑ j ∈ N, (utopia j - single j))
          * (utopia i - single i)) = total := by
  rw [Finset.sum_add_distrib, ← Finset.mul_sum, div_mul_cancel₀ _ hspread]
  ring

/-- 効率的な `x` では、`i` 以外の全員の取り分の余り `x(N \ {i}) - v(N \ {i})` は `M_i - x_i` に等しい
(`M_i = v(N) - v(N \ {i})`)。 -/
theorem gately_numerator (total xi others : ℝ) :
    (total - xi) - others = (total - others) - xi := by ring

/-- Gately 点では、抜ける傾向 `(M_i - x_i) / (x_i - v_i)` が `i` によらず `(1 - t) / t` になる。 -/
theorem gately_propensity (single utopia t : ℝ) (hgap : utopia - single ≠ 0) :
    (utopia - (single + t * (utopia - single))) / ((single + t * (utopia - single)) - single)
      = (1 - t) / t := by
  have h1 : utopia - (single + t * (utopia - single)) = (1 - t) * (utopia - single) := by ring
  have h2 : (single + t * (utopia - single)) - single = t * (utopia - single) := by ring
  rw [h1, h2, mul_div_mul_right _ _ hgap]

/-! ## disruption nucleolus -/

/-- 効率的な `x` (`x(N \ S) = v(N) - x(S)`) では、抜ける傾向
`(x(N \ S) - v(N \ S)) / (x(S) - v(S))` は、不満 `f = (v(S) - x(S)) / b(S)`
(`b(S) = v(N) - v(S) - v(N \ S)`) を使って `-1 / f - 1` と書ける。 -/
theorem disruption_eq (total vS vC xS : ℝ) (hg : xS - vS ≠ 0) :
    ((total - xS) - vC) / (xS - vS) = -1 / ((vS - xS) / (total - vS - vC)) - 1 := by
  have hg' : vS - xS ≠ 0 := fun h => hg (by linarith)
  field_simp
  ring

/-- `f ↦ -1 / f - 1` は `f < 0` の範囲で狭義単調増加である。
したがって、抜ける傾向の辞書式最小化は不満 `f` の辞書式最小化と同じ順序になる。 -/
theorem disruption_strictMono (f₁ f₂ : ℝ) (h₂ : f₂ < 0) (h : f₁ < f₂) :
    -1 / f₁ - 1 < -1 / f₂ - 1 := by
  have key : 1 / f₂ < 1 / f₁ := one_div_lt_one_div_of_neg_of_lt h₂ h
  rw [neg_div, neg_div]
  linarith

/-! ## 双対ゲームと bi-excess -/

/-- 効率的な `x` (`x(S) + x(N \ S) = v(N)`) では、双対ゲーム `v*(S) = v(N) - v(N \ S)` の超過は、
元のゲームの補集合の超過の符号を変えたものに等しい: `e(S, x, v*) = -e(N \ S, x, v)`。
anti-prenucleolus を双対ゲームのプレ仁として求めるのは、この等式による。 -/
theorem dual_excess (total vC xS xC : ℝ) (heff : xS + xC = total) :
    (total - vC) - xS = -(vC - xC) := by
  linarith

/-- modiclus の bi-excess の 2 つの書き方は、効率的な `x` で等しい:
`e(S, x, v) - e(T, x, v) = e(S, x, v) + e(N \ T, x, v*)`。 -/
theorem bi_excess_eq (total vS vT xS xT xTc : ℝ) (heff : xT + xTc = total) :
    (vS - xS) - (vT - xT) = (vS - xS) + ((total - vT) - xTc) := by
  linarith

end Coopgame
