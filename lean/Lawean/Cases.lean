import Lawean.Ident
import Lawean.Data.Rev_403AC0000000090_20210519
import Lawean.Data.Rev_403AC0000000090_20220518
import Lawean.Data.Unit_case_hane_missing
import Lawean.Data.Unit_case_conflict_22_A
import Lawean.Data.Unit_case_conflict_22_B

/-!
# 失敗例（fixtures/cases/cases.json、docs/12）を Lean で

成功例（実際の改正）は `Consolidate.lean`。ここは失敗例のうち、束縛（Rust）は通るが溶け込みの結果が違うもの。
Rust の `lawean-check` と同じ結論を `native_decide` で確かめる（三者一致: Rust の写し / Lean / e-Gov）。

束縛の段階で止まる失敗（発射台の取り違え・引用ミス）は Lean に届かない。Lean で見えるのは id 操作になった後だけ。
`wrong-base`（発射台違い）は `Consolidate.lean` の `art35_on_wrong_base_conflicts`、
`r4-48-reversed`（施行順序）は同 `art74_before_art73_fails`。
-/

namespace Lawean.Data
open Lawean.Ident

/-- `hane-missing`: 溶け込みは成功し、id の重複も衝突も無い… -/
theorem hane_missing_consolidates :
    (applyUnit rev_403AC0000000090_20210519 unit_case_hane_missing).map (fun r => (r.wf, r.hasConflict)) =
      some (true, false) := by
  native_decide

/-- …が、e-Gov の改正後リビジョンと一致しない（旧第3項 = 新第5項の「前項」がそのまま） -/
theorem hane_missing_differs :
    (applyUnit rev_403AC0000000090_20210519 unit_case_hane_missing).map Revision.render ≠
      some rev_403AC0000000090_20220518.render := by
  native_decide

/-- 違うのはその 1 項だけ: 第38条の新第5項 -/
theorem hane_missing_differs_only_at_38_5 :
    (applyUnit rev_403AC0000000090_20210519 unit_case_hane_missing).map
      (fun r => (r.render.zip rev_403AC0000000090_20220518.render).filter (fun (a, b) => a != b) |>.map (·.1.1)) =
      some ["38"] := by
  native_decide

/-- `conflict-22`: どちらも発射台には当たる -/
theorem conflict_22_each_applies :
    (applyUnit rev_403AC0000000090_20210519 unit_case_conflict_22_A).map (·.hasConflict) = some false ∧
    (applyUnit rev_403AC0000000090_20210519 unit_case_conflict_22_B).map (·.hasConflict) = some false := by
  native_decide

/-- 同じ項を触るので独立ではない -/
theorem conflict_22_not_independent :
    independentUnits unit_case_conflict_22_A unit_case_conflict_22_B = false := by
  native_decide

/-- 続けて当てると、後から当てた方が衝突として残る（どちらの順でも）。黙って片方を勝たせない -/
theorem conflict_22_has_conflict :
    (applyUnit rev_403AC0000000090_20210519 (unit_case_conflict_22_A ++ unit_case_conflict_22_B)).map
      (fun r => r.conflicts.map (·.1)) = some ["403AC0000000090/main/chap:2/sec:4/art:22/para:1"] ∧
    (applyUnit rev_403AC0000000090_20210519 (unit_case_conflict_22_B ++ unit_case_conflict_22_A)).map
      (fun r => r.conflicts.map (·.1)) = some ["403AC0000000090/main/chap:2/sec:4/art:22/para:1"] := by
  native_decide

/-- 調整規定（resolve）で解消できる -/
theorem conflict_22_resolved :
    (applyUnit rev_403AC0000000090_20210519
      (unit_case_conflict_22_A ++ unit_case_conflict_22_B ++
        [.resolve "403AC0000000090/main/chap:2/sec:4/art:22/para:1"
          "存続期間を五十年以上として借地権を設定する場合においては、第九条及び第十六条の規定にかかわらず、契約の更新（更新の請求及び土地の使用の継続によるものを含む。次条第一項において同じ。）及び建物の築造による存続期間の延長がなく、並びに第十三条の規定による買取りの請求をしないこととする旨を定めることができる。この場合においては、その特約は、公正証書による等書面又は電磁的記録によってしなければならない。"])).map
      (·.hasConflict) = some false := by
  native_decide

end Lawean.Data
