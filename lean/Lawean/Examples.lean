import Lawean.Theorems

/-!
# 実行例 — 借地借家法第38条（令和3年法律第37号 第35条）を単純化したもの

Rust 側（`lawean-amend`）と同じ操作を Lean の `applyUnit` で実行し、結果を確かめる。
-/

namespace Lawean

/-- 改正前の第38条（項の本文は番号だけの略記） -/
def art38 : Article :=
  { num := 38, paras := (List.range 7).map fun i => { num := i + 1, text := s!"p{i + 1}" } }

def art22 : Article := { num := 22, paras := [{ num := 1, text := "特約" }] }

def rev : Revision := { articles := [art22, art38] }

/-- 第38条の元の第2項の次と第1項の次に項を加える（実際の改め文は繰り下げを明示するが、ここでは挿入で表す）。
番号は各操作の直前の状態で解釈するので、後ろ側から挿入する -/
def unit38 : AmendUnit :=
  [ .insertParaAfter 38 2 "new4",
    .insertParaAfter 38 1 "new2",
    .replace 38 (some 3) "p2" "p2'" ]  -- 元の第2項（今は第3項）の「前項」を手当てするのに相当

#eval (applyUnit rev unit38).map fun r => (r.articles.map fun a => (a.num, a.paras.map fun p => (p.num, p.text)))

/-- 結果の項番号は 1..9 で、元の第2項は第3項に、第3項は第5項になっている -/
example :
    (applyUnit rev unit38).map (fun r => (r.articles.get! 1).paras.map fun p => (p.num, p.text)) =
      some [(1, "p1"), (2, "new2"), (3, "p2'"), (4, "new4"), (5, "p3"), (6, "p4"), (7, "p5"), (8, "p6"), (9, "p7")] := by
  native_decide

/-- 触る条が違う操作は順序を入れ替えても同じ（applyOp_comm の具体例） -/
example :
    (applyOp rev (.replace 22 none "特約" "特約A")).bind (applyOp · (.appendPara 38 "p8")) =
    (applyOp rev (.appendPara 38 "p8")).bind (applyOp · (.replace 22 none "特約" "特約A")) :=
  applyOp_comm rev _ _ (by decide)

/-- 発射台の不一致: 無い字句を改めると失敗する -/
example : applyOp rev (.replace 38 none "無い字句" "x") = none := by native_decide

#print axioms applyOp_comm
#print axioms updateArticle_comm

end Lawean
