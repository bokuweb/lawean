import Lawean.Ident

/-!
# 部分施行 — 1 つの改正単位を施行期日ごとの部分に分けて当てる

附則第一条の号が、改正法の条の一部の改正規定だけを別の日に施行することがある（「第六条の規定（…の改正規定を除く。）」）。
`lawean-check::stage::parts_of` は単位の命令を部分に分け、施行日ごとに当てる。ここではその正しさの条件を定理にする。

- **分割**（`Split u ps`）: 各部分が `u` の部分列で、全部合わせると `u` の命令をちょうど 1 回ずつ含む。
  Rust 側はテスト `stage::tests_conservation` がこれを確かめる
- **独立**（`indepParts`）: 別の部分どうしが同じ id を触らない。Rust 側は `stage::overlaps`（条・項の粒度）が近似する
- 両方が成り立てば、**部分をどの順で当てても単位全体を一度に当てたのと同じ**（`applyParts_eq`）。
  施行日の順は部分の並べ方の 1 つにすぎないので、号の日と本文の日の前後によらない
- 独立でなければ結果は順序で変わりうる（`StageExamples.lean` に反例）
-/

namespace Lawean.Ident

/-- `u` は `a` と `b` の部分列への分割（各部分の中で元の順序を保つ） -/
inductive Interleave : AmendUnit → AmendUnit → AmendUnit → Prop
  | nil : Interleave [] [] []
  | left  {a b u : AmendUnit} (x : Op) : Interleave a b u → Interleave (x :: a) b (x :: u)
  | right {a b u : AmendUnit} (x : Op) : Interleave a b u → Interleave a (x :: b) (x :: u)

theorem independent_symm {a b : Op} (h : Independent a b) : Independent b a :=
  fun i hb ha => h i ha hb

theorem independentUnits_symm {u v : AmendUnit} (h : IndependentUnits u v) : IndependentUnits v u :=
  fun a ha b hb => independent_symm (h b hb a ha)

theorem independentUnits_sound {u v : AmendUnit} (h : independentUnits u v = true) :
    IndependentUnits u v := by
  intro a ha b hb i hi hj
  simp only [independentUnits, List.all_eq_true, Bool.not_eq_true', List.contains_iff_mem,
    AmendUnit.touches, List.mem_flatMap] at h
  have := h i ⟨a, ha, hi⟩
  have hc : i ∈ v.flatMap Op.touches := List.mem_flatMap.mpr ⟨b, hb, hj⟩
  simp [hc] at this

/-- **独立な部分への分割は、単位全体と同じ。** 部分列に分けても、別の部分の操作が独立なら `a` を当ててから `b` を当てるのと同じ -/
theorem applyUnit_interleave {a b u : AmendUnit} (h : Interleave a b u) (hi : IndependentUnits a b) :
    ∀ r, applyUnit r u = applyUnit r (a ++ b) := by
  induction h with
  | nil => intro r; rfl
  | @left a b u x _ ih =>
    intro r
    have hi' : IndependentUnits a b := fun a' ha' b' hb' => hi a' (by simp [ha']) b' hb'
    rw [List.cons_append, applyUnit_cons, applyUnit_cons]
    cases applyOp r x with
    | none => rfl
    | some r1 => simp [ih hi' r1]
  | @right a b u x _ ih =>
    intro r
    have hi' : IndependentUnits a b := fun a' ha' b' hb' => hi a' ha' b' (by simp [hb'])
    have hx : IndependentUnits [x] a :=
      fun y hy a' ha' => by
        simp at hy; subst hy; exact independent_symm (hi a' ha' y (by simp))
    rw [applyUnit_cons]
    calc (applyOp r x).bind (applyUnit · u)
        = (applyOp r x).bind (applyUnit · (a ++ b)) := by
            cases applyOp r x with
            | none => rfl
            | some r1 => simp [ih hi' r1]
      _ = applyUnit r ([x] ++ a ++ b) := by
            rw [List.append_assoc, List.singleton_append, applyUnit_cons]
      _ = (applyUnit r ([x] ++ a)).bind (applyUnit · b) := by rw [applyUnit_append]
      _ = (applyUnit r (a ++ [x])).bind (applyUnit · b) := by rw [applyUnit_comm r [x] a hx]
      _ = applyUnit r (a ++ x :: b) := by
            rw [← applyUnit_append, List.append_assoc, List.singleton_append]

/-- 2 部分なら、どちらを先に施行しても単位全体と同じ -/
theorem staged_eq_whole {a b u : AmendUnit} (h : Interleave a b u) (hi : IndependentUnits a b)
    (r : Revision) :
    applyUnit r u = (applyUnit r a).bind (applyUnit · b) ∧
    applyUnit r u = (applyUnit r b).bind (applyUnit · a) := by
  constructor
  · rw [applyUnit_interleave h hi r, applyUnit_append]
  · rw [applyUnit_interleave h hi r, applyUnit_comm r a b hi, applyUnit_append]

-- 実行できる検査（`native_decide` と Rust の突き合わせ用） ---------------------------------

/-- `u` から部分列 `p` を左から貪欲に取り除いた残り。`p` が部分列でなければ `none` -/
def removeSub : AmendUnit → AmendUnit → Option AmendUnit
  | [], u => some u
  | _ :: _, [] => none
  | x :: p, y :: u => if x = y then removeSub p u else (removeSub (x :: p) u).map (y :: ·)

/-- `ps` は `u` の部分列への分割か（並べ方は問わない。施行日の順に並べて渡す） -/
def isSplit : AmendUnit → List AmendUnit → Bool
  | u, [] => u.isEmpty
  | u, p :: ps => match removeSub p u with
    | some w => isSplit w ps
    | none => false

/-- 各部分が、後に並ぶ部分のすべてと独立 -/
def indepParts : List AmendUnit → Bool
  | [] => true
  | p :: ps => independentUnits p ps.flatten && indepParts ps

/-- 部分を並べた順に当てる -/
def applyParts (r : Revision) (ps : List AmendUnit) : Option Revision :=
  ps.foldlM applyUnit r

theorem removeSub_sound : ∀ {p u w : AmendUnit}, removeSub p u = some w → Interleave p w u
  | [], [], w, h => by simp [removeSub] at h; subst h; exact .nil
  | [], y :: u, w, h => by
    simp [removeSub] at h; subst h
    exact .right y (removeSub_sound (p := []) (u := u) (by simp [removeSub]))
  | _ :: _, [], _, h => by simp [removeSub] at h
  | x :: p, y :: u, w, h => by
    unfold removeSub at h
    by_cases e : x = y
    · subst e; simp at h; exact .left x (removeSub_sound h)
    · simp [e] at h
      obtain ⟨w', h', rfl⟩ := h
      exact .right y (removeSub_sound h')

theorem interleave_mem {a b u : AmendUnit} (h : Interleave a b u) : ∀ x ∈ u, x ∈ a ∨ x ∈ b := by
  induction h with
  | nil => simp
  | left x _ ih =>
    intro y hy; simp at hy
    rcases hy with rfl | hy
    · simp
    · rcases ih y hy with h | h <;> simp [h]
  | right x _ ih =>
    intro y hy; simp at hy
    rcases hy with rfl | hy
    · simp
    · rcases ih y hy with h | h <;> simp [h]

theorem isSplit_mem : ∀ {u : AmendUnit} {ps : List AmendUnit}, isSplit u ps = true →
    ∀ x ∈ u, x ∈ ps.flatten
  | u, [], h => by simp [isSplit] at h; subst h; simp
  | u, p :: ps, h => by
    unfold isSplit at h
    split at h
    · rename_i w hw
      intro x hx
      rcases interleave_mem (removeSub_sound hw) x hx with hp | hw'
      · simp [hp]
      · simp [isSplit_mem h x hw']
    · simp at h

/-- **部分施行の正しさ。** `ps` が `u` の部分列への分割で、部分どうしが独立なら、並べた順（施行日の順）に当てた結果は
単位全体を一度に当てたのと同じ。`isSplit` / `indepParts` は実行できるので、実データでは `native_decide` で前提を確かめる -/
theorem applyParts_eq : ∀ (u : AmendUnit) (ps : List AmendUnit),
    isSplit u ps = true → indepParts ps = true → ∀ r, applyUnit r u = applyParts r ps
  | u, [], h, _, r => by simp [isSplit] at h; subst h; rfl
  | u, p :: ps, h, hi, r => by
    unfold isSplit at h
    split at h
    · rename_i w hw
      simp only [indepParts, Bool.and_eq_true] at hi
      have hpw : IndependentUnits p w := fun a ha b hb =>
        independentUnits_sound hi.1 a ha b (isSplit_mem h b hb)
      rw [applyUnit_interleave (removeSub_sound hw) hpw r, applyUnit_append, applyParts,
        List.foldlM_cons]
      cases applyUnit r p with
      | none => rfl
      | some r1 => simp [applyParts_eq w ps h hi.2 r1, applyParts]
    · simp at h

end Lawean.Ident
