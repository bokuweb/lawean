import Lawean.Sem

/-!
# 実行例 — 借地借家法第3条（docs/10 M1）

手書き Semantic IR（`lawean-semantic/src/examples`、docs/03-examples/art-03.md）を Lean のデータにし、
docs/07 で Z3 に問うた性質を Lean で確かめる。証明の手段（docs/10 §3）を決めるための実験。
-/

namespace Lawean.Sem

/-- 第3条: 借地権の存続期間は三十年とする。ただし、契約でこれより長い期間を定めたときは、その期間とする。
層化順: 例外 R3-2 が先、原則 R3-1 が後 -/
def art3 : Model :=
  { rules :=
      [ { id := "R3-2",
          cond := .cmp (.var "a:契約で定めた期間") .gt (.int 360),
          effect := .set "a:存続期間" (.var "a:契約で定めた期間"),
          overrides := ["R3-1"],
          source := "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:2" },
        { id := "R3-1",
          cond := .tt,
          effect := .set "a:存続期間" (.int 360),
          exceptions := ["R3-2"],
          source := "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:1" } ] }

example : art3.wf = true := by decide

/-- 性質が触る変数だけを持つ世界。他は 0 / false -/
def w3 (contract duration : Int) : World :=
  { ints := fun n => if n = "a:契約で定めた期間" then contract else if n = "a:存続期間" then duration else 0,
    bools := fun _ => false }

-- 具体例（native_decide） ------------------------------------------------------------

/-- 契約で 40 年と定めた世界: R3-2 が適用され R3-1 は退く -/
example : applies art3 (w3 480 480) "R3-2" = true ∧ applies art3 (w3 480 480) "R3-1" = false := by native_decide
/-- 定めが無い（0）世界: 原則が適用される -/
example : applies art3 (w3 0 360) "R3-1" = true := by native_decide
/-- 模型であること -/
example : consistent art3 (w3 480 480) = true := by native_decide
example : consistent art3 (w3 0 360) = true := by native_decide
/-- 模型でない: 40 年と定めたのに存続期間が 30 年 -/
example : consistent art3 (w3 480 360) = false := by native_decide

-- 性質 ------------------------------------------------------------

/-- **第3条: 契約期間が何であれ 存続期間 ≥ 30 年**（docs/07 の 1 件目、Z3 では証明）。
評価器を展開して線形算術に落とし omega。Model が小さいので全部展開できる（大きい Model では `Properties.lean` の局所的なやり方） -/
theorem art3_ge_30 : ∀ contract duration : Int,
    consistent art3 (w3 contract duration) = true → 360 ≤ duration := by
  intro x y h
  simp [consistent, run, applies1, evalE, evalV, holds, Env.get, art3, w3, CmpOp.eval, List.foldl, List.all, List.find?] at h
  omega

/-- **第3条: 存続期間 = 30 年（常に）は偽**（docs/07 の 2 件目、Z3 が反例を出した）。
反例は Z3（または人）が見つけ、Lean はそれが模型であることを検証する -/
theorem art3_not_always_30 :
    ∃ contract duration : Int, consistent art3 (w3 contract duration) = true ∧ duration ≠ 360 :=
  ⟨480, 480, by native_decide, by decide⟩

#print axioms art3_ge_30
#print axioms art3_not_always_30

end Lawean.Sem
