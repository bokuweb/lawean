import Lawean.Basic

/-!
# apply — 溶け込み

`applyOp : Revision → Op → Option Revision`。対象の条・項・字句が無ければ `none`（発射台の不一致）。
-/

namespace Lawean

/-- 条の列の中で、番号 n の条だけを f で書き換える。無ければ none、f が失敗しても none -/
def updateArticle (n : Nat) (f : Article → Option Article) : List Article → Option (List Article)
  | [] => none
  | a :: rest =>
    if a.num = n then
      Option.map (· :: rest) (f a)
    else
      Option.map (a :: ·) (updateArticle n f rest)

/-- 文字列 s に from_ が含まれるか（`String.replace` は含まれなくても成功するので別に見る） -/
def contains (s from_ : String) : Bool := (s.splitOn from_).length > 1

/-- 項の列の中の全出現を置換。1 箇所も無ければ none -/
def replaceInParas (from_ to : String) (ps : List Para) : Option (List Para) :=
  if ps.any (fun p => contains p.text from_) then
    some (ps.map fun p => { p with text := p.text.replace from_ to })
  else
    none

/-- 番号 m の項だけを書き換える -/
def updatePara (m : Nat) (f : Para → Option Para) : List Para → Option (List Para)
  | [] => none
  | p :: rest =>
    if p.num = m then Option.map (· :: rest) (f p)
    else Option.map (p :: ·) (updatePara m f rest)

/-- k より後ろの項番号を d だけずらす -/
def shiftAfter (k : Nat) (d : Int) (ps : List Para) : List Para :=
  ps.map fun p => if p.num > k then { p with num := (Int.ofNat p.num + d).toNat } else p

/-- 第 after 項の直後に項を挿入し、後続を 1 つ繰り下げる -/
def insertAfter (after : Nat) (text : String) (ps : List Para) : Option (List Para) :=
  if ps.any (·.num = after) then
    let shifted := shiftAfter after 1 ps
    let before := shifted.filter (·.num ≤ after)
    let rest := shifted.filter (fun p => ¬ p.num ≤ after)
    some (before ++ [{ num := after + 1, text }] ++ rest)
  else
    none

/-- 条の項の列を書き換える操作を、条の書き換えに持ち上げる -/
def onParas (f : List Para → Option (List Para)) (art : Article) : Option Article :=
  Option.map (fun ps => { art with paras := ps }) (f art.paras)

def replaceOne (from_ to : String) (p : Para) : Option Para :=
  if contains p.text from_ then some { p with text := p.text.replace from_ to } else none

def deleteParaAt (m : Nat) (ps : List Para) : Option (List Para) :=
  if ps.any (·.num = m) then some (shiftAfter m (-1) (ps.filter (·.num ≠ m))) else none

def appendParaTo (text : String) (ps : List Para) : Option (List Para) :=
  some (ps ++ [{ num := ps.length + 1, text }])

/-- 操作を「条の書き換え」として読む -/
def Op.articleUpdate : Op → (Article → Option Article)
  | .replace _ none from_ to => onParas (replaceInParas from_ to)
  | .replace _ (some m) from_ to => onParas (updatePara m (replaceOne from_ to))
  | .insertParaAfter _ k text => onParas (insertAfter k text)
  | .appendPara _ text => onParas (appendParaTo text)
  | .deletePara _ m => onParas (deleteParaAt m)

def applyOp (r : Revision) (op : Op) : Option Revision :=
  Option.map Revision.mk (updateArticle op.article op.articleUpdate r.articles)

/-- 改正単位の適用。途中で失敗したら全体が失敗 -/
def applyUnit (r : Revision) (u : AmendUnit) : Option Revision :=
  u.foldlM applyOp r

end Lawean
