/-!
# Identity patch — 改正単位を identity で書く（ADR-0013）

`Lawean.Basic` / `Lawean.Apply` の `Op` は「第N条第M項」という**番号**で対象を指す。番号は改正で動くので、
未確定の施行日や他法令の割り込みで発射台が変わると別の項を指してしまい、可換になりようがない。

ここでは対象を **stable_id** で指す。番号は状態に持たず、描画時に計算する（`paraNum`）。

- **独立**（触る id が交わらない）な 2 操作 / 2 改正単位は可換 — `applyOp_comm` / `applyUnit_comm`
- **衝突**（期待した本文と違う）は失敗ではなく `Node.conflicts` に値として残る。解消は `resolve`（調整規定）
- **依存**（相手が作った id を触る）は半順序。施行順序はその線形拡張でなければならない — `scheduleOk`
- 対象の id が無ければ `none`（発射台に無いものを触っている = 依存先が未施行）

Pijul / Darcs のパッチ理論と同じ構図で、収束型 CRDT のように衝突を黙って解決することはしない。
-/

namespace Lawean.Ident

abbrev NodeId := String

/-- 項。`art` は所属する条（描画用で、代数には効かない）。`conflicts` は期待した本文と違ったため当てられなかった新本文 -/
structure Node where
  id        : NodeId
  art       : Nat
  text      : String
  conflicts : List String := []
deriving Repr, DecidableEq, Inhabited

/-- 法令の 1 リビジョン = 文書順の項の列。番号は持たない -/
structure Revision where
  nodes : List Node
deriving Repr, DecidableEq, Inhabited

/-- 改正の操作。すべて id で対象を指す -/
inductive Op where
  /-- id の本文が expected なら new に改める。違えば衝突として記録する -/
  | replace     (id : NodeId) (expected new : String)
  /-- anchor の直後に newId の項を加える -/
  | insertAfter (anchor newId : NodeId) (art : Nat) (text : String)
  /-- id の項を削る -/
  | delete      (id : NodeId)
  /-- 調整規定: 衝突を解消して本文を確定する -/
  | resolve     (id : NodeId) (text : String)
deriving Repr, DecidableEq

/-- 操作が読む・書く id -/
def Op.touches : Op → List NodeId
  | .replace id _ _       => [id]
  | .insertAfter a n _ _  => [a, n]
  | .delete id            => [id]
  | .resolve id _         => [id]

/-- 操作が新しく作る id -/
def Op.creates : Op → List NodeId
  | .insertAfter _ n _ _ => [n]
  | _ => []

/-- 操作が探す id（列の中で最初に見つかる要素を書き換える） -/
def Op.key : Op → NodeId
  | .replace id _ _      => id
  | .insertAfter a _ _ _ => a
  | .delete id           => id
  | .resolve id _        => id

/-- 見つかった項を、0 個以上の項の列に置き換える -/
def Op.edit : Op → Node → List Node
  | .replace _ expected new, x =>
      if x.text = expected then [{ x with text := new }]
      else [{ x with conflicts := x.conflicts ++ [new] }]
  | .insertAfter _ n art text, x => [x, { id := n, art, text }]
  | .delete _, _ => []
  | .resolve _ text, x => [{ x with text, conflicts := [] }]

/-- 最初に id が k の要素を見つけ、g で要素の列に置き換える。無ければ none -/
def editAt (k : NodeId) (g : Node → List Node) : List Node → Option (List Node)
  | [] => none
  | x :: xs => if x.id = k then some (g x ++ xs) else (editAt k g xs).map (x :: ·)

def applyOp (r : Revision) (op : Op) : Option Revision :=
  (editAt op.key op.edit r.nodes).map Revision.mk

/-- 改正単位 = 操作の列 -/
abbrev AmendUnit := List Op

def applyUnit (r : Revision) (u : AmendUnit) : Option Revision :=
  u.foldlM applyOp r

-- 検査 ------------------------------------------------------------

/-- id に重複が無いか。改正単位の終わりで検査する -/
def Revision.wf (r : Revision) : Bool :=
  (r.nodes.map (·.id)).eraseDups.length = r.nodes.length

/-- 衝突が残っているか -/
def Revision.hasConflict (r : Revision) : Bool :=
  r.nodes.any fun n => !n.conflicts.isEmpty

def Revision.conflicts (r : Revision) : List (NodeId × String × List String) :=
  (r.nodes.filter fun n => !n.conflicts.isEmpty).map fun n => (n.id, n.text, n.conflicts)

/-- 番号は描画時に計算する: 同じ条の中での位置 + 1 -/
def paraNum (r : Revision) (id : NodeId) : Option Nat :=
  match r.nodes.find? (·.id = id) with
  | none => none
  | some n => ((r.nodes.filter (·.art = n.art)).findIdx? (·.id = id)).map (· + 1)

def AmendUnit.touches (u : AmendUnit) : List NodeId := u.flatMap Op.touches
def AmendUnit.creates (u : AmendUnit) : List NodeId := u.flatMap Op.creates

/-- 独立: 触る id が交わらない（Bool 版。証明には下の `Independent` を使う） -/
def independent (a b : Op) : Bool := a.touches.all fun i => !(b.touches.contains i)
def independentUnits (u v : AmendUnit) : Bool := u.touches.all fun i => !(v.touches.contains i)

/-- b は a に依存する: a が作った id を b が触る -/
def dependsOn (b a : AmendUnit) : Bool := b.touches.any (a.creates.contains ·)

/-- 施行順序が依存の半順序の線形拡張か: 先に施行される単位が、後の単位に依存していない -/
def scheduleOk : List AmendUnit → Bool
  | [] => true
  | u :: rest => (rest.all fun v => !(dependsOn u v)) && scheduleOk rest

-- メタ定理 ------------------------------------------------------------

/-- 独立（命題版） -/
def Independent (a b : Op) : Prop := ∀ i ∈ a.touches, i ∉ b.touches

def IndependentUnits (u v : AmendUnit) : Prop := ∀ a ∈ u, ∀ b ∈ v, Independent a b

instance (a b : Op) : Decidable (Independent a b) := by unfold Independent; infer_instance
instance (u v : AmendUnit) : Decidable (IndependentUnits u v) := by unfold IndependentUnits; infer_instance

theorem key_mem_touches (op : Op) : op.key ∈ op.touches := by
  cases op <;> simp [Op.key, Op.touches]

theorem creates_sub_touches (op : Op) : ∀ i ∈ op.creates, i ∈ op.touches := by
  cases op <;> simp [Op.creates, Op.touches]

/-- 書き換え結果の id は、元の id か、新しく作った id のどちらか -/
theorem edit_id (op : Op) (x : Node) : ∀ y ∈ op.edit x, y.id = x.id ∨ y.id ∈ op.creates := by
  intro y hy
  cases op with
  | replace id e n =>
    simp only [Op.edit] at hy
    split at hy <;> simp_all
  | insertAfter a n art t =>
    simp only [Op.edit, Op.creates, List.mem_cons, List.mem_singleton, List.not_mem_nil, or_false] at hy ⊢
    rcases hy with rfl | rfl <;> simp
  | delete id => simp [Op.edit] at hy
  | resolve id t => simp_all [Op.edit]

theorem editAt_cons (k : NodeId) (g : Node → List Node) (x : Node) (xs : List Node) :
    editAt k g (x :: xs) = if x.id = k then some (g x ++ xs) else (editAt k g xs).map (x :: ·) := rfl

/-- 先頭に k を含まない列が付いていても、その部分は素通りする -/
theorem editAt_append_skip (k : NodeId) (g : Node → List Node) :
    ∀ (ys xs : List Node), (∀ y ∈ ys, y.id ≠ k) → editAt k g (ys ++ xs) = (editAt k g xs).map (ys ++ ·) := by
  intro ys xs hys
  induction ys with
  | nil => simp only [List.nil_append]; cases editAt k g xs <;> simp
  | cons y ys ih =>
    have hy : y.id ≠ k := hys y (by simp)
    have hys' : ∀ z ∈ ys, z.id ≠ k := fun z hz => hys z (by simp [hz])
    rw [List.cons_append, editAt_cons, if_neg hy, ih hys']
    cases editAt k g xs <;> simp

/-- 鍵の違う 2 つの書き換えは、互いの鍵を作らない限り可換 -/
theorem editAt_comm (k₁ k₂ : NodeId) (h : k₁ ≠ k₂) (g₁ g₂ : Node → List Node)
    (h₁ : ∀ x, x.id = k₁ → ∀ y ∈ g₁ x, y.id ≠ k₂)
    (h₂ : ∀ x, x.id = k₂ → ∀ y ∈ g₂ x, y.id ≠ k₁) :
    ∀ (l : List Node), (editAt k₁ g₁ l).bind (editAt k₂ g₂) = (editAt k₂ g₂ l).bind (editAt k₁ g₁) := by
  intro l
  induction l with
  | nil => simp [editAt]
  | cons x xs ih =>
    by_cases hx1 : x.id = k₁
    · have hx2 : ¬ x.id = k₂ := by rw [hx1]; exact h
      rw [editAt_cons, editAt_cons, if_pos hx1, if_neg hx2]
      simp only [Option.some_bind]
      rw [editAt_append_skip k₂ g₂ (g₁ x) xs (h₁ x hx1)]
      cases editAt k₂ g₂ xs with
      | none => simp
      | some r => simp [editAt_cons, hx1]
    · by_cases hx2 : x.id = k₂
      · rw [editAt_cons, editAt_cons, if_neg hx1, if_pos hx2]
        simp only [Option.some_bind]
        rw [editAt_append_skip k₁ g₁ (g₂ x) xs (h₂ x hx2)]
        cases editAt k₁ g₁ xs with
        | none => simp
        | some r => simp [editAt_cons, hx2]
      · rw [editAt_cons, editAt_cons, if_neg hx1, if_neg hx2]
        cases hr1 : editAt k₁ g₁ xs with
        | none =>
          cases hr2 : editAt k₂ g₂ xs with
          | none => simp
          | some r2 =>
            have e := ih; rw [hr1, hr2] at e; simp at e
            simp [editAt_cons, hx1, ← e]
        | some r1 =>
          cases hr2 : editAt k₂ g₂ xs with
          | none =>
            have e := ih; rw [hr1, hr2] at e; simp at e
            simp [editAt_cons, hx2, e]
          | some r2 =>
            have e := ih; rw [hr1, hr2] at e; simp at e
            simp [editAt_cons, hx1, hx2, e]

theorem applyOp_bind (r : Revision) (a b : Op) :
    (applyOp r a).bind (applyOp · b) =
      ((editAt a.key a.edit r.nodes).bind (editAt b.key b.edit)).map Revision.mk := by
  simp only [applyOp]
  cases editAt a.key a.edit r.nodes <;> simp

/-- **独立な 2 操作は可換。** 全ての法令・全ての操作について、施行順が入れ替わっても結果は同じ -/
theorem applyOp_comm (r : Revision) (a b : Op) (h : Independent a b) :
    (applyOp r a).bind (applyOp · b) = (applyOp r b).bind (applyOp · a) := by
  have hk : a.key ≠ b.key := fun e => h a.key (key_mem_touches a) (e ▸ key_mem_touches b)
  rw [applyOp_bind, applyOp_bind, editAt_comm a.key b.key hk a.edit b.edit ?_ ?_ r.nodes]
  · intro x hx y hy
    rcases edit_id a x y hy with e | e
    · rw [e, hx]; exact hk
    · exact fun e' => h y.id (creates_sub_touches a y.id e) (e' ▸ key_mem_touches b)
  · intro x hx y hy
    rcases edit_id b x y hy with e | e
    · rw [e, hx]; exact hk.symm
    · exact fun e' => h a.key (key_mem_touches a) (e' ▸ creates_sub_touches b y.id e)

theorem applyUnit_nil (r : Revision) : applyUnit r [] = some r := rfl

theorem applyUnit_cons (r : Revision) (a : Op) (u : AmendUnit) :
    applyUnit r (a :: u) = (applyOp r a).bind (applyUnit · u) := rfl

theorem applyUnit_append (r : Revision) (u v : AmendUnit) :
    applyUnit r (u ++ v) = (applyUnit r u).bind (applyUnit · v) := by
  induction u generalizing r with
  | nil => simp [applyUnit_nil]
  | cons a u ih =>
    rw [List.cons_append, applyUnit_cons, applyUnit_cons, Option.bind_assoc]
    cases applyOp r a with
    | none => simp
    | some r' => simp [ih]

/-- 1 操作は、独立な改正単位を素通りできる -/
theorem applyOp_swap_unit (a : Op) (v : AmendUnit) (hv : ∀ b ∈ v, Independent a b) :
    ∀ r, (applyOp r a).bind (applyUnit · v) = (applyUnit r v).bind (applyOp · a) := by
  intro r
  induction v generalizing r with
  | nil => simp [applyUnit_nil]
  | cons b v ih =>
    have hab : Independent a b := hv b (by simp)
    have hv' : ∀ c ∈ v, Independent a c := fun c hc => hv c (by simp [hc])
    simp only [applyUnit_cons]
    calc (applyOp r a).bind (fun r1 => (applyOp r1 b).bind (applyUnit · v))
        = ((applyOp r a).bind (applyOp · b)).bind (applyUnit · v) := by rw [Option.bind_assoc]
      _ = ((applyOp r b).bind (applyOp · a)).bind (applyUnit · v) := by rw [applyOp_comm r a b hab]
      _ = (applyOp r b).bind (fun r1 => (applyOp r1 a).bind (applyUnit · v)) := by rw [Option.bind_assoc]
      _ = (applyOp r b).bind (fun r1 => (applyUnit r1 v).bind (applyOp · a)) := by
            cases applyOp r b with
            | none => rfl
            | some r1 => simp [ih hv' r1]
      _ = ((applyOp r b).bind (applyUnit · v)).bind (applyOp · a) := by rw [Option.bind_assoc]

/-- **独立な 2 改正単位は可換。** 施行順序が未確定でも、独立なら順序の列挙は要らない -/
theorem applyUnit_comm (r : Revision) (u v : AmendUnit) (h : IndependentUnits u v) :
    applyUnit r (u ++ v) = applyUnit r (v ++ u) := by
  induction u generalizing r with
  | nil => simp [applyUnit_append, applyUnit_nil]
  | cons a u ih =>
    have ha : ∀ b ∈ v, Independent a b := fun b hb => h a (by simp) b hb
    have hu : IndependentUnits u v := fun a' ha' b hb => h a' (by simp [ha']) b hb
    rw [List.cons_append, applyUnit_cons, applyUnit_append]
    simp only [applyUnit_cons]
    calc (applyOp r a).bind (applyUnit · (u ++ v))
        = (applyOp r a).bind (fun r1 => (applyUnit r1 u).bind (applyUnit · v)) := by
            cases applyOp r a with
            | none => rfl
            | some r1 => simp [applyUnit_append]
      _ = (applyOp r a).bind (fun r1 => (applyUnit r1 v).bind (applyUnit · u)) := by
            cases applyOp r a with
            | none => rfl
            | some r1 =>
              have e := ih r1 hu
              rw [applyUnit_append, applyUnit_append] at e
              simp [e]
      _ = ((applyOp r a).bind (applyUnit · v)).bind (applyUnit · u) := by rw [Option.bind_assoc]
      _ = ((applyUnit r v).bind (applyOp · a)).bind (applyUnit · u) := by rw [applyOp_swap_unit a v ha r]
      _ = (applyUnit r v).bind (fun r1 => (applyOp r1 a).bind (applyUnit · u)) := by rw [Option.bind_assoc]

end Lawean.Ident
