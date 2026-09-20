import Lawean.SemTheorems
import Lawean.Data.Sem_403AC0000000090_hand

/-!
# 法令の内容の性質 — 手書き IR（docs/10 M2）

docs/07 で Z3 に問うた 6 性質を、同じデータ（`Data/Sem_403AC0000000090_hand.lean`、Rust が出力）について Lean で。
世界 `w : World` は全部の自由変数について量化する（Z3 と同じ強さ）。

証明は局所的: `consistent_rule` で関係する Rule の「適用されれば帰結が成り立つ」を取り出し、
`applies_spec` で applies をその Rule の条件と例外に開き、`simp` で評価器を展開して `omega`。
Model 全体を展開しない（Rule 数の 2 乗で膨らむ）。
-/

namespace Lawean.Data
open Lawean.Sem

abbrev M := sem_403AC0000000090_hand

theorem M_wf : M.wf = true := by decide

/-- Rule r の「適用されれば帰結」を、条件と例外に開いた形で取り出す -/
theorem rule_of (w : World) (h : consistent M w = true) (r : Rule) (hr : r ∈ M.rules) :
    (evalE M w (run M w) r.cond && r.exceptions.all fun x => !(applies M w x)) = true →
      holds M w r.effect = true := by
  intro ha
  exact consistent_rule M w h r hr (by rw [applies_spec M w M_wf r hr]; exact ha)

/-- applies を条件と例外に開く（id を文字列で指す版） -/
theorem applies_of (w : World) (r : Rule) (hr : r ∈ M.rules) (id : RuleId) (hid : r.id = id) :
    applies M w id = (evalE M w (run M w) r.cond && r.exceptions.all fun x => !(applies M w x)) := by
  rw [← hid]; exact applies_spec M w M_wf r hr

macro "mem" : tactic => `(tactic| simp [M, sem_403AC0000000090_hand])

/-- `ruleValue` の展開に使う: 参照先の Rule。閉じた項なので `rfl` で決まる -/
theorem rv_R4_1 : M.rule? "R4-1" = some «R4-1» := rfl
theorem rv_R4_1' : M.rule? "R4-1'" = some «R4-1'» := rfl

/-- 評価器の展開 -/
syntax "unfold_sem" (Lean.Parser.Tactic.location)? : tactic
macro_rules
  | `(tactic| unfold_sem $[$loc]?) =>
    `(tactic| simp only [evalE, evalAll, evalAny, evalV, evalV.evalV1, holds, holds.holds1, CmpOp.eval,
        List.all, Bool.and_eq_true, Bool.or_eq_true, Bool.not_eq_true',
        Bool.not_eq_false', decide_eq_true_eq, decide_eq_false_iff_not, beq_iff_eq, bne_iff_ne,
        Bool.and_true, Bool.true_and, Bool.not_false, Bool.not_true, and_imp, and_true, true_and,
        gt_iff_lt, ge_iff_le, Bool.true_eq_false, Bool.false_eq_true, false_or, or_false,
        List.all_cons, List.all_nil, Int.not_lt, Int.not_le, imp_false, true_implies, false_implies,
        Bool.false_and, Bool.and_false] $[$loc]?)

-- 第3条 ------------------------------------------------------------

/-- **第3条: 契約期間が何であれ 存続期間 ≥ 30 年**（Z3: 証明） -/
theorem art3_ge_30 (w : World) (h : consistent M w = true) : 360 ≤ w.ints "a:存続期間" := by
  have h1 := rule_of w h «R3-1» (by mem)
  have h2 := rule_of w h «R3-2» (by mem)
  simp only [«R3-1», «R3-2», List.all_cons, List.all_nil, Bool.and_true] at h1 h2
  rw [applies_of w «R3-2» (by mem) "R3-2" rfl] at h1
  simp only [«R3-2», List.all_cons, List.all_nil, Bool.and_true] at h1
  unfold_sem at h1 h2
  omega

/-- **第3条: 存続期間 = 30 年（常に）は偽**（Z3: 反例 = 契約で 40 年）。反例を Lean が模型として検証する -/
theorem art3_not_always_30 :
    ∃ w : World, consistent M w = true ∧ w.ints "a:存続期間" ≠ 360 :=
  ⟨{ ints := fun n => if n = "a:契約で定めた期間" then 480 else if n = "a:存続期間" then 480 else 0,
     -- 第6条は「正当の事由が無い」世界で常に適用されるので、その印も立てておく
     bools := fun n => n = "eff:R6" },
   by native_decide, by decide⟩

-- 第4条 ------------------------------------------------------------

/-- **第4条: 更新後の期間 ≥ 10 年**（Z3: 証明） -/
theorem art4_ge_10 (w : World) (h : consistent M w = true) (hr : w.bools "p:更新する" = true) :
    120 ≤ w.ints "a:更新後の期間" := by
  have h1 := rule_of w h «R4-1» (by mem)
  have h1' := rule_of w h «R4-1'» (by mem)
  have h2 := rule_of w h «R4-2» (by mem)
  have h2' := rule_of w h «R4-2'» (by mem)
  simp only [«R4-1», «R4-1'», «R4-2», «R4-2'», List.all_cons, List.all_nil, Bool.and_true] at h1 h1' h2 h2'
  rw [applies_of w «R4-1'» (by mem) "R4-1'" rfl, applies_of w «R4-2» (by mem) "R4-2" rfl] at h1
  simp only [«R4-1'», «R4-2», List.all_cons, List.all_nil, Bool.and_true] at h1
  rw [applies_of w «R4-2'» (by mem) "R4-2'" rfl] at h1 h1'
  simp only [«R4-2'», List.all_cons, List.all_nil, Bool.and_true] at h1 h1'
  unfold_sem at h1 h1' h2 h2'
  simp only [rv_R4_1, rv_R4_1', «R4-1», «R4-1'», evalV.evalV1, hr] at h1 h1' h2 h2'
  cases hf : w.bools "p:最初の更新" <;> simp only [hf] at h1 h1' h2 h2' <;> unfold_sem at h1 h1' h2 h2' <;> omega

/-- **第4条: 最初の更新なら 更新後の期間 ≥ 20 年**（Z3: 手書き IR の当初版では反例、修正後は証明） -/
theorem art4_first_ge_20 (w : World) (h : consistent M w = true) (hr : w.bools "p:更新する" = true)
    (hf : w.bools "p:最初の更新" = true) : 240 ≤ w.ints "a:更新後の期間" := by
  have h1' := rule_of w h «R4-1'» (by mem)
  have h2' := rule_of w h «R4-2'» (by mem)
  simp only [«R4-1'», «R4-2'», List.all_cons, List.all_nil, Bool.and_true] at h1' h2'
  rw [applies_of w «R4-2'» (by mem) "R4-2'" rfl] at h1'
  simp only [«R4-2'», List.all_cons, List.all_nil, Bool.and_true] at h1'
  unfold_sem at h1' h2'
  simp only [rv_R4_1', «R4-1'», evalV.evalV1, hr, hf] at h1' h2'
  unfold_sem at h1' h2'
  omega

-- 第22条 × 第9条 ------------------------------------------------------------

/-- **存続期間 ≥ 50 年なら 第9条は適用されない**（定期借地権の特約は強行規定に反しない。Z3: 証明） -/
theorem art22_blocks_art9 (w : World) (h50 : 600 ≤ w.ints "a:存続期間") :
    applies M w "R9" = false := by
  rw [applies_of w «R9» (by mem) "R9" rfl]
  simp only [«R9», List.all_cons, List.all_nil, Bool.and_true]
  rw [applies_of w «R22-1a» (by mem) "R22-1a" rfl]
  simp only [«R22-1a», List.all_cons, List.all_nil, Bool.and_true]
  unfold_sem
  simp [h50]

-- 第9条 + 補題 ------------------------------------------------------------

/-- **第9条: 契約期間 < 30 年 ⇒ 特約は無効**。補題「30 年未満の特約はこの節の規定に反し借地権者に不利」を仮定する
（評価概念を人が埋める。Z3 と同じ補題）。存続期間 ≥ 50 年の特約（第22条）は別なので、その前提も要る -/
theorem art9_voids_short (w : World) (h : consistent M w = true)
    (lemma1 : w.ints "a:契約で定めた期間" < 360 →
      w.bools "p:反する(target=Scope(StableId(\"403AC0000000090/main/chap:2/sec:1\")))" = true ∧
      w.bools "p:不利(to=D:借地権者,judgement=Unknown(UnknownExpr { kind: Intentional, text: \"不利\" }))" = true)
    (hshort : w.ints "a:契約で定めた期間" < 360) :
    w.bools "void:特約" = true := by
  have h9 := rule_of w h «R9» (by mem)
  simp only [«R9», List.all_cons, List.all_nil, Bool.and_true] at h9
  rw [applies_of w «R22-1a» (by mem) "R22-1a" rfl] at h9
  simp only [«R22-1a», List.all_cons, List.all_nil, Bool.and_true] at h9
  -- 第22条の特約は存続期間 ≥ 50 年が要件。契約期間 < 30 年ならそもそも存続期間は 30 年（第3条）
  have h1 := rule_of w h «R3-1» (by mem)
  have h2 := rule_of w h «R3-2» (by mem)
  simp only [«R3-1», «R3-2», List.all_cons, List.all_nil, Bool.and_true] at h1 h2
  rw [applies_of w «R3-2» (by mem) "R3-2" rfl] at h1
  simp only [«R3-2», List.all_cons, List.all_nil, Bool.and_true] at h1
  unfold_sem at h9 h1 h2
  obtain ⟨l1, l2⟩ := lemma1 hshort
  have : w.ints "a:存続期間" = 360 := by omega
  simp only [l1, l2, this] at h9
  simpa using h9 trivial (by decide)

#print axioms art3_ge_30
#print axioms art4_first_ge_20
#print axioms art22_blocks_art9
#print axioms art9_voids_short

end Lawean.Data
