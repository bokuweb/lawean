import Lawean.Refs

/-!
# 法令空間と他法令への波及（docs/09、ADR-0013）

`Refs.lean` の「参照を id で持つ本文」を法令をまたいで使う。他法令からの参照は絶対形（「借地借家法第三十八条第四項」）なので、
描画に要るのは参照先の法令のリビジョンにおける条番号と項番号だけ。

- 法令空間 `LawSpace` = 法令 id → その時点のリビジョン
- 他法令参照 `XPiece.xref law target form`。描画は参照先の法令で `paraNum` / `art` を引く。無ければ **参照切れ**
- **波及の分類** `impact`: A の改正の前後で、B の各参照を分類する。参照先が無くなれば `dangling`、番号が動けば `shifted`（新しい描画つき = 他法令側に要る手当て）、
  位置は同じで本文が変われば `semanticChange`
- **健全性**: `dangling` / `shifted` の報告は必ず参照先の (条, 項番号) の変化に対応する（`impact_sound`）。
  **完全性**: B の描画が変わるなら必ずどれかの参照先の (条, 項番号) が変わっている（`xrender_congr` の対偶 `ximpact_complete`）

Rust の `lawean-space::impact` と同じ分類。施行日の区間（`TimingGap`）はここでは扱わない（時間は Revision の外）。
-/

namespace Lawean.Ident

abbrev LawId := String

/-- 法令空間: 法令 id とその時点のリビジョン -/
abbrev LawSpace := List (LawId × Revision)

def LawSpace.get (s : LawSpace) (law : LawId) : Option Revision :=
  (s.find? (·.1 = law)).map (·.2)

/-- 他法令参照の書き方: 条だけ（「第二十八条」）か、条と項（「第三十八条第四項」） -/
inductive XForm
  | article
  | paragraph
deriving Repr, DecidableEq

/-- 他法令を含む本文の断片 -/
inductive XPiece
  | text (s : String)
  /-- 自法令内の参照（`Refs.lean` と同じ） -/
  | ref (target : NodeId) (form : RefForm)
  /-- 他法令 `law` の項 `target` への参照 -/
  | xref (law : LawId) (target : NodeId) (form : XForm)
deriving Repr, DecidableEq

abbrev XBody := List XPiece

/-- 条番号の文字列（"38"）を漢数字に。枝番（"42_2"）は「の」で繋ぐ -/
def artKanji (art : ArtNum) : String :=
  let k := fun (s : String) => match s.toNat? with | some n => kanji n | none => s
  match art.splitOn "_" with
  | [] => ""
  | a :: rest => rest.foldl (fun acc b => acc ++ "の" ++ k b) (k a)

/-- 他法令参照の描画に要る値: (参照先の条, 参照先の項番号)。参照先が無ければ none -/
def xrefNums (s : LawSpace) (law : LawId) (target : NodeId) : Option (ArtNum × Nat) :=
  match s.get law with
  | none => none
  | some r =>
    match r.nodes.find? (·.id = target), paraNum r target with
    | some n, some p => some (n.art, p)
    | _, _ => none

def renderXRef (form : XForm) : Option (ArtNum × Nat) → String
  | some (art, p) =>
    match form with
    | .article => "第" ++ artKanji art ++ "条"
    | .paragraph => "第" ++ artKanji art ++ "条第" ++ kanji p ++ "項"
  | none => "（削除された規定）"

/-- 自法令内の参照の描画に要る値（法令が空間に無ければ none） -/
def hereNums (s : LawSpace) (here : LawId) (src target : NodeId) : Option (Option Nat × Option Nat) :=
  (s.get here).map fun r => refNums r src target

/-- 自法令 `here` の項 `src` の本文を、空間 `s` の中で描画する -/
def renderXPiece (s : LawSpace) (here : LawId) (src : NodeId) : XPiece → String
  | .text t => t
  | .ref target form =>
    match hereNums s here src target with
    | some nums => renderRef form nums
    | none => "（削除された項）"
  | .xref law target form => renderXRef form (xrefNums s law target)

def renderXBody (s : LawSpace) (here : LawId) (src : NodeId) : XBody → String
  | [] => ""
  | p :: ps => renderXPiece s here src p ++ renderXBody s here src ps

/-- **描画は参照先の番号にしか依存しない**（自法令は `hereNums`、他法令は `xrefNums`） -/
theorem renderXBody_congr (s s' : LawSpace) (here : LawId) (src : NodeId) :
    ∀ body : XBody,
      (∀ t f, .ref t f ∈ body → hereNums s here src t = hereNums s' here src t) →
      (∀ l t f, .xref l t f ∈ body → xrefNums s l t = xrefNums s' l t) →
      renderXBody s here src body = renderXBody s' here src body
  | [], _, _ => rfl
  | .text t :: ps, h1, h2 => by
    simp only [renderXBody, renderXPiece]
    rw [renderXBody_congr s s' here src ps (fun t f ht => h1 t f (List.mem_cons_of_mem _ ht))
      (fun l t f ht => h2 l t f (List.mem_cons_of_mem _ ht))]
  | .ref t f :: ps, h1, h2 => by
    simp only [renderXBody, renderXPiece]
    rw [h1 t f (List.mem_cons_self _ _),
      renderXBody_congr s s' here src ps (fun t f ht => h1 t f (List.mem_cons_of_mem _ ht))
        (fun l t f ht => h2 l t f (List.mem_cons_of_mem _ ht))]
  | .xref l t f :: ps, h1, h2 => by
    simp only [renderXBody, renderXPiece]
    rw [h2 l t f (List.mem_cons_self _ _),
      renderXBody_congr s s' here src ps (fun t f ht => h1 t f (List.mem_cons_of_mem _ ht))
        (fun l t f ht => h2 l t f (List.mem_cons_of_mem _ ht))]

/-- **他法令への波及の完全性**: B の描画が変わるなら、B 自身の参照先か他法令の参照先の番号が動いている -/
theorem ximpact_complete (s s' : LawSpace) (here : LawId) (src : NodeId) (body : XBody)
    (h : renderXBody s here src body ≠ renderXBody s' here src body) :
    (∃ t f, .ref t f ∈ body ∧ hereNums s here src t ≠ hereNums s' here src t) ∨
    (∃ l t f, .xref l t f ∈ body ∧ xrefNums s l t ≠ xrefNums s' l t) :=
  Classical.byContradiction fun hc =>
    h (renderXBody_congr s s' here src body
      (fun t f ht => Classical.byContradiction fun hne => hc (Or.inl ⟨t, f, ht, hne⟩))
      (fun l t f ht => Classical.byContradiction fun hne => hc (Or.inr ⟨l, t, f, ht, hne⟩)))

-- 波及の分類 ------------------------------------------------------------

inductive ImpactKind
  /-- 参照先が改正で無くなる -/
  | dangling
  /-- 参照先の番号が動いた。`fix` は新しい描画（= 他法令側に要るハネの手当て） -/
  | shifted (fix : String)
  /-- 位置は同じだが本文が変わった -/
  | semanticChange (before after : String)
deriving Repr, DecidableEq

structure Impact where
  fromLaw : LawId
  node    : NodeId
  toLaw   : LawId
  target  : NodeId
  kind    : ImpactKind
deriving Repr, DecidableEq

def nodeText (s : LawSpace) (law : LawId) (id : NodeId) : Option String :=
  (s.get law).bind fun r => (r.nodes.find? (·.id = id)).map (·.text)

/-- 1 つの他法令参照の分類 -/
def classify (before after : LawSpace) (law : LawId) (target : NodeId) (form : XForm) : Option ImpactKind :=
  match xrefNums after law target with
  | none => some .dangling
  | some a =>
    if xrefNums before law target = some a then
      match nodeText before law target, nodeText after law target with
      | some tb, some ta => if tb = ta then none else some (.semanticChange tb ta)
      | _, _ => none
    else some (.shifted (renderXRef form (some a)))

/-- 空間の中の全部の他法令参照について、改正の前後（`before` / `after` は改正した法令だけが違う空間）を比べる -/
def impact (before after : LawSpace) (bodies : List (LawId × NodeId × XBody)) : List Impact :=
  bodies.flatMap fun (here, src, body) =>
    body.filterMap fun
      | .xref law target form =>
        (classify before after law target form).map fun kind =>
          { fromLaw := here, node := src, toLaw := law, target, kind }
      | _ => none

/-- 本文の変化の判定だけを取り出したもの -/
theorem semantic_part (before after : LawSpace) (law : LawId) (target : NodeId) (k : ImpactKind)
    (hc : (match nodeText before law target, nodeText after law target with
      | some tb, some ta => if tb = ta then none else some (ImpactKind.semanticChange tb ta)
      | _, _ => none) = some k) :
    ∃ tb ta, nodeText before law target = some tb ∧ nodeText after law target = some ta ∧ tb ≠ ta ∧
      k = .semanticChange tb ta := by
  cases hb : nodeText before law target <;> cases hb' : nodeText after law target <;> simp_all

/-- **健全性 (1)**: `dangling` は改正後に参照先が無い -/
theorem classify_dangling (before after : LawSpace) (law : LawId) (target : NodeId) (form : XForm)
    (hc : classify before after law target form = some .dangling) :
    xrefNums after law target = none := by
  unfold classify at hc
  cases ha : xrefNums after law target with
  | none => rfl
  | some a =>
    rw [ha] at hc
    by_cases hb : xrefNums before law target = some a
    · simp only [hb, ite_true] at hc
      obtain ⟨_, _, _, _, _, hk⟩ := semantic_part before after law target _ hc
      cases hk
    · simp [hb] at hc

/-- **健全性 (2)**: `shifted f` は参照先の (条, 項番号) が動いていて、`f` は改正後の描画（= 手当て） -/
theorem classify_shifted (before after : LawSpace) (law : LawId) (target : NodeId) (form : XForm) (f : String)
    (hc : classify before after law target form = some (.shifted f)) :
    xrefNums before law target ≠ xrefNums after law target ∧ f = renderXRef form (xrefNums after law target) := by
  unfold classify at hc
  cases ha : xrefNums after law target with
  | none => rw [ha] at hc; simp at hc
  | some a =>
    rw [ha] at hc
    by_cases hb : xrefNums before law target = some a
    · simp only [hb, ite_true] at hc
      obtain ⟨_, _, _, _, _, hk⟩ := semantic_part before after law target _ hc
      cases hk
    · simp only [hb, ite_false, Option.some.injEq, ImpactKind.shifted.injEq] at hc
      exact ⟨hb, hc.symm⟩

/-- **健全性 (3)**: `semanticChange` は位置が同じで本文が違う -/
theorem classify_semantic (before after : LawSpace) (law : LawId) (target : NodeId) (form : XForm) (tb ta : String)
    (hc : classify before after law target form = some (.semanticChange tb ta)) :
    xrefNums before law target = xrefNums after law target ∧ nodeText before law target = some tb ∧
      nodeText after law target = some ta ∧ tb ≠ ta := by
  unfold classify at hc
  cases ha : xrefNums after law target with
  | none => rw [ha] at hc; simp at hc
  | some a =>
    rw [ha] at hc
    by_cases hb : xrefNums before law target = some a
    · simp only [hb, ite_true] at hc
      obtain ⟨tb', ta', h1, h2, h3, hk⟩ := semantic_part before after law target _ hc
      cases hk
      exact ⟨hb, h1, h2, h3⟩
    · simp [hb] at hc

end Lawean.Ident
