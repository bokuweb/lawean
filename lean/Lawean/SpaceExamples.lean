import Lawean.Space
import Lawean.Data.Rev_403AC0000000090_20260521
import Lawean.Data.Rev_403AC0000000090_20220518
import Lawean.Data.Unit_draft_insert_38_4
import Lawean.Data.Unit_draft_delete_28

/-!
# 他法令への波及 — 実データ（docs/09 計画 3・5）

借地借家法（A）の改正案が、A を参照する施行令（C）と高齢者居住安定確保法（B）に与える影響を、Lean の `impact` で分類する。
Rust の `lawean-space` のテストと同じ結論:

- 第38条第3項の次に 1 項を挿入 → 施行令の「借地借家法第三十八条第四項」（2 箇所）は第五項に**ずれる**。手当ては「第三十八条第五項」
- 第28条を削る → 高齢者居住安定確保法第58条第2項の「借地借家法第二十八条」は**参照切れ**

B・C の本文は参照を id で持つ `XBody` として手書き（層 2 の参照解決が出す形）。
-/

namespace Lawean.Data
open Lawean.Ident

def A : LawId := "403AC0000000090"
def B : LawId := "413AC0000000026"
def C : LawId := "504CO0000000187"

def A38_4 : NodeId := "403AC0000000090/main/chap:3/sec:3/art:38/para:4"
def A28 : NodeId := "403AC0000000090/main/chap:3/sec:1/art:28/para:1"

/-- 施行令第1項・第2項（e-Gov の実物）。「借地借家法第三十八条第四項」を他法令参照で -/
def bodiesC : List (LawId × NodeId × XBody) :=
  [ (C, "504CO0000000187/main/para:1",
      [ .text "借地借家法", .xref A A38_4 .paragraph,
        .text "の規定による承諾は、建物の賃貸人が、法務省令で定めるところにより、あらかじめ、当該承諾に係る建物の賃借人に対し同項の規定による電磁的方法による提供に用いる電磁的方法の種類及び内容を示した上で、当該建物の賃借人から書面又は電子情報処理組織を使用する方法その他の情報通信の技術を利用する方法であって法務省令で定めるもの（次項において「書面等」という。）によって得るものとする。" ]),
    (C, "504CO0000000187/main/para:2",
      [ .text "建物の賃貸人は、前項の承諾を得た場合であっても、当該承諾に係る建物の賃借人から書面等により借地借家法", .xref A A38_4 .paragraph,
        .text "の規定による電磁的方法による提供を受けない旨の申出があったときは、当該電磁的方法による提供をしてはならない。" ]) ]

/-- 高齢者居住安定確保法第58条第2項（e-Gov の実物）。「借地借家法第二十八条」は条の参照 -/
def bodiesB : List (LawId × NodeId × XBody) :=
  [ (B, "413AC0000000026/main/chap:5/art:58/para:2",
      [ .text "借地借家法", .xref A A28 .article, .text "の規定は、前項の解約の申入れについては、適用しない。" ]) ]

-- 計画 3: 項の挿入で施行令の参照がずれる ------------------------------------------------------------

def spaceBefore3 : LawSpace := [(A, rev_403AC0000000090_20260521)]
def spaceAfter3 : Option LawSpace :=
  (applyUnit rev_403AC0000000090_20260521 unit_draft_insert_38_4).map fun r => [(A, r)]

/-- 改正前の描画は実物の本文と一致する（`XBody` の書き方が正しい） -/
example : renderXBody spaceBefore3 C "504CO0000000187/main/para:1" bodiesC[0]!.2.2 =
    "借地借家法第三十八条第四項の規定による承諾は、建物の賃貸人が、法務省令で定めるところにより、あらかじめ、当該承諾に係る建物の賃借人に対し同項の規定による電磁的方法による提供に用いる電磁的方法の種類及び内容を示した上で、当該建物の賃借人から書面又は電子情報処理組織を使用する方法その他の情報通信の技術を利用する方法であって法務省令で定めるもの（次項において「書面等」という。）によって得るものとする。" := by
  native_decide

/-- 2 箇所とも `shifted`、手当ては「第三十八条第五項」 -/
theorem insert_38_4_shifts_cabinet_order :
    spaceAfter3.map (fun s' => (impact spaceBefore3 s' bodiesC).map fun i => (i.node, i.kind)) =
      some [ ("504CO0000000187/main/para:1", .shifted "第三十八条第五項"),
             ("504CO0000000187/main/para:2", .shifted "第三十八条第五項") ] := by
  native_decide

/-- 描画し直すと施行令の本文がそのまま手当て後の形になる（= 他法令側のハネの手当ての生成） -/
theorem insert_38_4_rerenders_cabinet_order :
    spaceAfter3.map (fun s' => (renderXBody s' C "504CO0000000187/main/para:1" bodiesC[0]!.2.2).take 13) =
      some "借地借家法第三十八条第五項" := by
  native_decide

-- 計画 5: 条の削除で高齢者居住安定確保法の参照が切れる ------------------------------------------------------------

def spaceBefore5 : LawSpace := [(A, rev_403AC0000000090_20220518)]
def spaceAfter5 : Option LawSpace :=
  (applyUnit rev_403AC0000000090_20220518 unit_draft_delete_28).map fun r => [(A, r)]

theorem delete_28_leaves_b58_dangling :
    spaceAfter5.map (fun s' => (impact spaceBefore5 s' bodiesB).map fun i => (i.node, i.kind)) =
      some [ ("413AC0000000026/main/chap:5/art:58/para:2", .dangling) ] := by
  native_decide

/-- 逆に、令3-37 の実物（第22・38・39条への項の追加）は施行令の参照をずらさない（計画 1）。
第38条第4項そのものが令3-37 で作られた項なので 2021-05-19 版には無い — 施行令は改正後を前提に書かれている。
ここでは現行に対して「何も触らない改正」を当てた場合で代用する -/
theorem no_change_no_impact :
    impact spaceBefore3 spaceBefore3 bodiesC = [] := by
  native_decide

#print axioms ximpact_complete
#print axioms classify_shifted

end Lawean.Data
