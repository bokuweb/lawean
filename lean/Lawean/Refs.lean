import Lawean.Ident

/-!
# 参照を id で持つ本文と、ハネの完全性（ADR-0012 / ADR-0013、docs/08 §5）

`Ident.Node.text` は描画済みの本文で、「前項」「第三項」という**番号**を含む。番号は改正で動くので、
本文そのものは変わらないのに描画が変わる項が出る = ハネ改正。

ここでは本文を「平文の断片」と「id で指す参照」の列（`Body`）で持ち、番号は描画時に `paraNum` から計算する（`renderBody`）。
そうすると:

- 参照は id なので、項の挿入・削除で**壊れない**（`Body` は `applyUnit` で変わらない。定義から明らか）
- **ハネの完全性**: 本文（`Body`）が同じなのに描画が違うなら、必ずどれかの参照の番号か距離が動いている（`renderBody_congr` の対偶）
- **ハネの手当ては生成できる**: 改正後の描画と改正前の描画の差がそのまま手当て（`haneFixes`）。
  Rust の `hane::render_fix` は同じ規則（相対形は距離が同じならそのまま、1 なら「前項」「次項」、それ以外は絶対形）
-/

namespace Lawean.Ident

/-- 参照の書き方。相対形は元の距離を持つ（「前二項」= `prev 2`）。`absoluteArt` は他の条の項（「第百四十二条の四第六項」） -/
inductive RefForm
  | absolute
  | absoluteArt
  | prev (k : Nat)
  | next
deriving Repr, DecidableEq

/-- 本文の断片 -/
inductive Piece
  | text (s : String)
  | ref (target : NodeId) (form : RefForm)
deriving Repr, DecidableEq

abbrev Body := List Piece

/-- 漢数字（Rust の `to_kanji` と同じ規則。9999 まで） -/
def kanji (n : Nat) : String :=
  let d := #["", "一", "二", "三", "四", "五", "六", "七", "八", "九"]
  if n = 0 then "〇"
  else
    let th := n / 1000
    let hu := (n % 1000) / 100
    let te := (n % 100) / 10
    let on := n % 10
    (if th > 0 then (if th > 1 then d[th]! else "") ++ "千" else "") ++
    (if hu > 0 then (if hu > 1 then d[hu]! else "") ++ "百" else "") ++
    (if te > 0 then (if te > 1 then d[te]! else "") ++ "十" else "") ++
    d[on]!

example : kanji 142 = "百四十二" := by native_decide
example : kanji 1001 = "千一" := by native_decide
example : kanji 20 = "二十" := by native_decide

/-- 2 つの項が同じ条にあるか -/
def sameArt (r : Revision) (a b : NodeId) : Bool :=
  match r.nodes.find? (·.id = a), r.nodes.find? (·.id = b) with
  | some x, some y => x.art == y.art
  | _, _ => false

/-- 条番号の文字列（"38"）を「第三十八条」に。枝番（"142_4"）は「第百四十二条の四」 -/
def artLabel (art : ArtNum) : String :=
  let k := fun (s : String) => match s.toNat? with | some n => kanji n | none => s
  match art.splitOn "_" with
  | [] => ""
  | a :: rest => rest.foldl (fun acc b => acc ++ "の" ++ k b) ("第" ++ k a ++ "条")

example : artLabel "142_4" = "第百四十二条の四" := by native_decide

/-- 参照の描画に要る数: (参照先の番号, 参照元の番号)。同じ条でなければ参照元は none -/
def refNums (r : Revision) (src target : NodeId) : Option Nat × Option Nat :=
  (paraNum r target, if sameArt r src target then paraNum r src else none)

/-- 参照先の条（`absoluteArt` の描画用） -/
def targetArt (r : Revision) (target : NodeId) : Option ArtNum :=
  (r.nodes.find? (·.id = target)).map (·.art)

/-- 番号から参照の字句へ。相対形は距離が元と同じならそのまま、1 なら「前項」「次項」、それ以外は絶対形（法制執務の慣行） -/
def renderRef (form : RefForm) : Option Nat × Option Nat → String
  | (some t, sp) =>
    match form, sp with
    | .prev k, some s =>
      if s = t + k then (if k = 1 then "前項" else "前" ++ kanji k ++ "項")
      else if s = t + 1 then "前項"
      else "第" ++ kanji t ++ "項"
    | .next, some s => if t = s + 1 then "次項" else "第" ++ kanji t ++ "項"
    | _, _ => "第" ++ kanji t ++ "項"
  | (none, _) => "（削除された項）"

def renderPiece (r : Revision) (src : NodeId) : Piece → String
  | .text s => s
  | .ref target .absoluteArt =>
    match targetArt r target, paraNum r target with
    | some art, some p => artLabel art ++ "第" ++ kanji p ++ "項"
    | _, _ => "（削除された規定）"
  | .ref target form => renderRef form (refNums r src target)

def renderBody (r : Revision) (src : NodeId) : Body → String
  | [] => ""
  | p :: ps => renderPiece r src p ++ renderBody r src ps

/-- 参照の描画が依存するもの: (参照先の番号, 参照元の番号, 参照先の条) -/
def refKey (r : Revision) (src target : NodeId) : (Option Nat × Option Nat) × Option ArtNum :=
  (refNums r src target, targetArt r target)

theorem paraNum_of_refKey {r r' : Revision} {src t : NodeId} (h : refKey r src t = refKey r' src t) :
    paraNum r t = paraNum r' t := by
  simp only [refKey, refNums, Prod.mk.injEq] at h
  exact h.1.1

/-- **描画は参照の番号にしか依存しない**: 各参照の (番号, 距離, 条) が同じなら描画は同じ -/
theorem renderBody_congr (r r' : Revision) (src : NodeId) :
    ∀ body : Body, (∀ t f, .ref t f ∈ body → refKey r src t = refKey r' src t) →
      renderBody r src body = renderBody r' src body
  | [], _ => rfl
  | .text s :: ps, h => by
    simp only [renderBody, renderPiece]
    rw [renderBody_congr r r' src ps (fun t f ht => h t f (List.mem_cons_of_mem _ ht))]
  | .ref t f :: ps, h => by
    have hk := h t f (List.mem_cons_self _ _)
    have h1 : refNums r src t = refNums r' src t := by simp only [refKey, Prod.mk.injEq] at hk; exact hk.1
    have h2 : targetArt r t = targetArt r' t := by simp only [refKey, Prod.mk.injEq] at hk; exact hk.2
    have h3 := paraNum_of_refKey hk
    have ih := renderBody_congr r r' src ps (fun t' f' ht => h t' f' (List.mem_cons_of_mem _ ht))
    cases f <;> simp only [renderBody, renderPiece] <;> rw [ih] <;> first | rw [h1] | rw [h2, h3]

/-- **ハネの完全性**（対偶）: 本文が同じなのに描画が違えば、どれかの参照の番号・距離・条が動いている -/
theorem hane_complete (r r' : Revision) (src : NodeId) (body : Body)
    (h : renderBody r src body ≠ renderBody r' src body) :
    ∃ t f, .ref t f ∈ body ∧ refKey r src t ≠ refKey r' src t :=
  Classical.byContradiction fun hc =>
    h (renderBody_congr r r' src body fun t f ht =>
      Classical.byContradiction fun hne => hc ⟨t, f, ht, hne⟩)

/-- 本文つきの項の列（描画済みの `Revision` と並行して持つ） -/
abbrev Bodies := List (NodeId × Body)

/-- **手当ての生成**: 改正の前後で描画が変わる項について (id, 改正前の描画, 改正後の描画) -/
def haneFixes (r r' : Revision) (bodies : Bodies) : List (NodeId × String × String) :=
  bodies.filterMap fun (id, body) =>
    let before := renderBody r id body
    let after := renderBody r' id body
    if before = after then none else some (id, before, after)

/-- 生成した手当ては健全: 挙がった項は必ず参照の番号が動いている -/
theorem haneFixes_sound (r r' : Revision) (bodies : Bodies) (id : NodeId) (b a : String)
    (h : (id, b, a) ∈ haneFixes r r' bodies) :
    ∃ body, (id, body) ∈ bodies ∧ ∃ t f, .ref t f ∈ body ∧ refKey r id t ≠ refKey r' id t := by
  simp only [haneFixes, List.mem_filterMap] at h
  obtain ⟨⟨id', body⟩, hmem, hf⟩ := h
  simp only at hf
  split at hf
  · exact absurd hf (by simp)
  · rename_i hne
    simp only [Option.some.injEq, Prod.mk.injEq] at hf
    obtain ⟨rfl, rfl, rfl⟩ := hf
    exact ⟨body, hmem, hane_complete r r' id' body hne⟩

end Lawean.Ident
