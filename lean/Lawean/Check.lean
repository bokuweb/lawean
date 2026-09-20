import Lawean.Ident

/-!
# `checkUnit` — エディタが実行する判定関数と、その正しさ（ADR-0014 / ADR-0015）

サービスが改正案ごとに走らせるのは証明ではなく、この関数の値。
`checkUnit r u = true` は「発射台 r に改正単位 u が溶け込み、id が重複せず、衝突も無い」と**同値**（`checkUnit_iff`）。
また、溶け込みが失敗する（`none`）のは、ある操作の時点でその対象 id が無いとき、それだけである（`applyOp_none_iff`）。
Rust の `lawean-check` の Base / Order / Conflict / Consolidate はこの 2 つの定理が言う条件をそのまま報告している。
-/

namespace Lawean.Ident

/-- 改正単位の検査: 溶け込めて、id が重複せず、衝突が無い -/
def checkUnit (r : Revision) (u : AmendUnit) : Bool :=
  match applyUnit r u with
  | some r' => r'.wf && !r'.hasConflict
  | none => false

theorem checkUnit_iff (r : Revision) (u : AmendUnit) :
    checkUnit r u = true ↔ ∃ r', applyUnit r u = some r' ∧ r'.wf = true ∧ r'.hasConflict = false := by
  unfold checkUnit
  cases h : applyUnit r u with
  | none => simp
  | some r' =>
    simp only [Bool.and_eq_true, Bool.not_eq_true', Option.some.injEq]
    constructor
    · rintro ⟨h1, h2⟩; exact ⟨r', rfl, h1, h2⟩
    · rintro ⟨r'', he, h1, h2⟩; cases he; exact ⟨h1, h2⟩

theorem editAt_none_iff (k : NodeId) (g : Node → List Node) :
    ∀ l : List Node, editAt k g l = none ↔ k ∉ l.map (·.id)
  | [] => by simp [editAt]
  | x :: xs => by
    simp only [editAt, List.map_cons, List.mem_cons]
    by_cases h : x.id = k
    · simp [h]
    · simp only [h, ite_false, Option.map_eq_none', editAt_none_iff k g xs]
      constructor
      · intro hn; rintro (e | e)
        · exact h e.symm
        · exact hn e
      · intro hn e; exact hn (Or.inr e)

/-- **失敗の特徴づけ**: 1 操作が失敗するのは、その対象 id が発射台に無いとき、そのときだけ -/
theorem applyOp_none_iff (r : Revision) (op : Op) :
    applyOp r op = none ↔ op.key ∉ r.nodes.map (·.id) := by
  simp only [applyOp, Option.map_eq_none']
  exact editAt_none_iff op.key op.edit r.nodes

/-- 改正単位が失敗するのは、途中のどこかの操作が、その時点の状態に無い id を触るとき、そのときだけ -/
theorem applyUnit_none_iff (r : Revision) : ∀ u : AmendUnit,
    applyUnit r u = none ↔
      ∃ pre op post, u = pre ++ op :: post ∧
        ∃ r', applyUnit r pre = some r' ∧ op.key ∉ r'.nodes.map (·.id)
  | [] => by
    rw [applyUnit_nil]
    constructor
    · intro h; cases h
    · rintro ⟨pre, op, post, h, _⟩; cases pre <;> simp at h
  | op :: rest => by
    rw [applyUnit_cons]
    cases h : applyOp r op with
    | none =>
      simp only [Option.none_bind, true_iff]
      exact ⟨[], op, rest, rfl, r, rfl, (applyOp_none_iff r op).mp h⟩
    | some r1 =>
      simp only [Option.some_bind]
      rw [applyUnit_none_iff r1 rest]
      constructor
      · rintro ⟨pre, op', post, hu, r', hp, hk⟩
        exact ⟨op :: pre, op', post, by simp [hu], r', by rw [applyUnit_cons, h]; exact hp, hk⟩
      · rintro ⟨pre, op', post, hu, r', hp, hk⟩
        cases pre with
        | nil =>
          simp only [List.nil_append, List.cons.injEq] at hu
          obtain ⟨rfl, rfl⟩ := hu
          simp only [applyUnit_nil, Option.some.injEq] at hp
          subst hp
          exact absurd ((applyOp_none_iff r op).mpr hk) (by simp [h])
        | cons op0 pre =>
          simp only [List.cons_append, List.cons.injEq] at hu
          obtain ⟨rfl, rfl⟩ := hu
          rw [applyUnit_cons, h, Option.some_bind] at hp
          exact ⟨pre, op', post, rfl, r', hp, hk⟩

end Lawean.Ident
