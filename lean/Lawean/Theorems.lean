import Lawean.Apply

/-!
# メタ定理（docs/08-amendment.md §5）

個々の改正の検査は Rust 側の実行で済む。ここで示すのは、全ての法令・全ての改正について成り立つこと。
-/

namespace Lawean

/-- 条の書き換えが条番号を変えない -/
def KeepsNum (f : Article → Option Article) : Prop :=
  ∀ a a', f a = some a' → a'.num = a.num

theorem onParas_keepsNum (f : List Para → Option (List Para)) : KeepsNum (onParas f) := by
  intro a a' h
  unfold onParas at h
  cases hf : f a.paras with
  | none => simp [hf] at h
  | some ps =>
    simp [hf] at h
    subst h
    rfl

/-- 改め文の操作はどれも条番号を変えない -/
theorem articleUpdate_keepsNum (op : Op) : KeepsNum op.articleUpdate := by
  cases op with
  | replace a p fr t =>
    cases p <;> simp only [Op.articleUpdate] <;> exact onParas_keepsNum _
  | insertParaAfter a k t => simp only [Op.articleUpdate]; exact onParas_keepsNum _
  | appendPara a t => simp only [Op.articleUpdate]; exact onParas_keepsNum _
  | deletePara a m => simp only [Op.articleUpdate]; exact onParas_keepsNum _

theorem updateArticle_cons (n : Nat) (f : Article → Option Article) (a : Article) (rest : List Article) :
    updateArticle n f (a :: rest) =
      if a.num = n then Option.map (· :: rest) (f a) else Option.map (a :: ·) (updateArticle n f rest) := by
  rfl

/-- 番号の違う条への 2 つの書き換えは、条番号を変えない限り可換 -/
theorem updateArticle_comm (n₁ n₂ : Nat) (h : n₁ ≠ n₂)
    (f g : Article → Option Article) (hf : KeepsNum f) (hg : KeepsNum g) :
    ∀ (as_ : List Article),
      (updateArticle n₁ f as_).bind (updateArticle n₂ g) =
      (updateArticle n₂ g as_).bind (updateArticle n₁ f) := by
  intro as_
  induction as_ with
  | nil => simp [updateArticle]
  | cons a rest ih =>
    by_cases h1 : a.num = n₁
    · have h2 : ¬ a.num = n₂ := by rw [h1]; exact h
      rw [updateArticle_cons, updateArticle_cons, if_pos h1, if_neg h2]
      cases hfa : f a with
      | none =>
        cases updateArticle n₂ g rest with
        | none => simp
        | some r2 => simp [updateArticle_cons, h1, hfa]
      | some a' =>
        have ha' : ¬ a'.num = n₂ := by rw [hf a a' hfa, h1]; exact h
        cases hr : updateArticle n₂ g rest with
        | none => simp [updateArticle_cons, ha', hr]
        | some r2 => simp [updateArticle_cons, ha', hr, h1, hfa]
    · by_cases h2 : a.num = n₂
      · rw [updateArticle_cons, updateArticle_cons, if_neg h1, if_pos h2]
        cases hga : g a with
        | none =>
          cases updateArticle n₁ f rest with
          | none => simp
          | some r1 => simp [updateArticle_cons, h2, hga]
        | some a' =>
          have ha' : ¬ a'.num = n₁ := by rw [hg a a' hga, h2]; exact fun e => h e.symm
          cases hr : updateArticle n₁ f rest with
          | none => simp [updateArticle_cons, ha', hr]
          | some r1 => simp [updateArticle_cons, ha', hr, h2, hga]
      · rw [updateArticle_cons, updateArticle_cons, if_neg h1, if_neg h2]
        cases hr1 : updateArticle n₁ f rest with
        | none =>
          cases hr2 : updateArticle n₂ g rest with
          | none => simp
          | some r2 =>
            have e := ih
            rw [hr1, hr2] at e
            simp at e
            simp [updateArticle_cons, h1, ← e]
        | some r1 =>
          cases hr2 : updateArticle n₂ g rest with
          | none =>
            have e := ih
            rw [hr1, hr2] at e
            simp at e
            simp [updateArticle_cons, h2, e]
          | some r2 =>
            have e := ih
            rw [hr1, hr2] at e
            simp at e
            simp [updateArticle_cons, h1, h2, e]

/-- 2 操作の逐次適用を、条の列の上の逐次書き換えに戻す -/
theorem applyOp_bind (r : Revision) (o₁ o₂ : Op) :
    (applyOp r o₁).bind (applyOp · o₂) =
      Option.map Revision.mk
        ((updateArticle o₁.article o₁.articleUpdate r.articles).bind (updateArticle o₂.article o₂.articleUpdate)) := by
  simp only [applyOp]
  cases updateArticle o₁.article o₁.articleUpdate r.articles <;> simp

/-- 改め文の 2 つの操作は、触る条が違えば可換（改正単位の可換性の十分条件） -/
theorem applyOp_comm (r : Revision) (op₁ op₂ : Op) (h : op₁.article ≠ op₂.article) :
    (applyOp r op₁).bind (applyOp · op₂) = (applyOp r op₂).bind (applyOp · op₁) := by
  rw [applyOp_bind, applyOp_bind,
    updateArticle_comm op₁.article op₂.article h op₁.articleUpdate op₂.articleUpdate
      (articleUpdate_keepsNum op₁) (articleUpdate_keepsNum op₂) r.articles]

/-- 項の挿入は、挿入点より後ろの項番号を必ず 1 つ繰り下げる（ハネの完全性の基礎） -/
theorem shiftAfter_gt (k : Nat) (ps : List Para) (p : Para) (hp : p ∈ ps) (hk : p.num > k) :
    ({ p with num := p.num + 1 } : Para) ∈ shiftAfter k 1 ps := by
  unfold shiftAfter
  refine List.mem_map.mpr ⟨p, hp, ?_⟩
  simp [hk]

/-- 挿入点以前の項は動かない -/
theorem shiftAfter_le (k : Nat) (ps : List Para) (p : Para) (hp : p ∈ ps) (hk : ¬ p.num > k) :
    p ∈ shiftAfter k 1 ps := by
  unfold shiftAfter
  exact List.mem_map.mpr ⟨p, hp, by simp [hk]⟩

end Lawean
