/-
仁の逐次 LP (src/nucleolus/mod.rs, exact.rs, oracle.rs) が
「1 人提携とその補集合があれば LP は有界になる」として使っている主張の証明。

確定した提携の特性ベクトルが張る部分空間を `F` とする。`F` は `1_N` を含み、全空間ではない。

* `exists_single_not_mem` : `F` に入らない 1 人提携がある。
* `mem_of_compl_mem`      : 提携が `F` に入らなければ、その補集合も入らない (対偶の形で述べる)。
* `level_lower_bound`     : 1 人提携とその補集合の行があれば、段の値 `t` は下に有界である。

各段で確定する提携が必ず見つかること (LP の双対と相補性による) は証明していない。
-/
import Mathlib

namespace Coopgame

/-- `1_N` と `1_{N \ T} = 1_N - 1_T` が部分空間に入れば、`1_T` も入る。 -/
theorem mem_of_compl_mem {V : Type*} [AddCommGroup V] [Module ℝ V] (F : Submodule ℝ V)
    (one t : V) (hone : one ∈ F) (hcompl : one - t ∈ F) : t ∈ F := by
  have := F.sub_mem hone hcompl
  simpa using this

/-- 部分空間が全空間でなければ、入らない 1 人提携の特性ベクトルがある。 -/
theorem exists_single_not_mem {κ : Type*} [Fintype κ] [DecidableEq κ]
    (F : Submodule ℝ (κ → ℝ)) (h : F ≠ ⊤) : ∃ i, (Pi.single i 1 : κ → ℝ) ∉ F := by
  by_contra hcon
  push Not at hcon
  apply h
  rw [eq_top_iff, ← (Pi.basisFun ℝ κ).span_eq]
  apply Submodule.span_le.mpr
  rintro _ ⟨i, rfl⟩
  simpa using hcon i

/-- 効率的な `x` (`x_i + x(N \ {i}) = v(N)`) で、1 人提携 `{i}` とその補集合の超過がどちらも `t` 以下なら、
`t` は `(v({i}) + v(N \ {i}) - v(N)) / 2` 以上である。 -/
theorem level_lower_bound (vi vc vN xi xc t : ℝ) (heff : xi + xc = vN)
    (h1 : vi - xi ≤ t) (h2 : vc - xc ≤ t) : (vi + vc - vN) / 2 ≤ t := by
  linarith

end Coopgame
