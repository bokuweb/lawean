import Lawean.Refs

/-!
# 条を丸ごと指す参照（「第六十一条」「前条」）と条ずれ

`renumber`（「第六十一条を第六十二条とし」）で条の番号が動くと、条を指す絶対参照の描画が変わる = ハネ。
相対形「前条」は参照元も同じだけ動けばそのまま、間に条が入れば絶対形になる（Rust `hane::article_hane_candidates` と同じ規則）。
-/

namespace Lawean.Ident

/-- 第60〜62条と、第70条（第61条を指す） -/
def artRev0 : Revision :=
  { nodes :=
      [ { id := "a60", art := "60", text := "甲" },
        { id := "a61", art := "61", text := "乙" },
        { id := "a62", art := "62", text := "前条の規定にかかわらず" },
        { id := "a70", art := "70", text := "第六十一条の規定による" } ] }

def artBodies : Bodies :=
  [ ("a62", [ .ref "a61" .prevArt, .text "の規定にかかわらず" ]),
    ("a70", [ .ref "a61" .art, .text "の規定による" ]) ]

/-- 改正前の描画は本文と一致する -/
example : artBodies.all (fun (id, body) =>
    (artRev0.nodes.find? (·.id = id)).map (·.text) == some (renderBody artRev0 id body)) = true := by
  native_decide

/-- 第六十条の次に一条を加え、第六十一条・第六十二条を一条ずつ繰り下げる -/
def shiftBefore : AmendUnit :=
  [ .renumber "a62" "63", .renumber "a61" "62", .insertAfter "a60" "new61" "61" "丙" ]

/-- 「第六十一条」は「第六十二条」にハネる。「前条」は参照元も動くのでそのまま -/
example : (applyUnit artRev0 shiftBefore).map (haneFixes artRev0 · artBodies) =
    some [("a70", "第六十一条の規定による", "第六十二条の規定による")] := by native_decide

/-- 第六十一条の次に一条を加え、第六十二条を繰り下げる: 間に条が入るので「前条」は絶対形になる -/
def shiftBetween : AmendUnit :=
  [ .renumber "a62" "63", .insertAfter "a61" "new62" "62" "丙" ]

example : (applyUnit artRev0 shiftBetween).map (haneFixes artRev0 · artBodies) =
    some [("a62", "前条の規定にかかわらず", "第六十一条の規定にかかわらず")] := by native_decide

/-- 条ずれの無い改正（本文だけ）ではハネは出ない -/
example : (applyUnit artRev0 [.replace "a60" "甲" "丁"]).map (haneFixes artRev0 · artBodies) =
    some [] := by native_decide

end Lawean.Ident
