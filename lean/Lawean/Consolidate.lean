import Lawean.Ident
import Lawean.Data.Rev_403AC0000000090_20210519
import Lawean.Data.Rev_403AC0000000090_20220518
import Lawean.Data.Rev_403AC0000000090_20230220
import Lawean.Data.Rev_403AC0000000090_20260521
import Lawean.Data.Rev_403AC0000000090_20280613
import Lawean.Data.Unit_503AC0000000037_art35
import Lawean.Data.Unit_504AC0000000048_art73
import Lawean.Data.Unit_504AC0000000048_art74
import Lawean.Data.Unit_505AC0000000053_art125

/-!
# 溶け込みの正 — 実データを `Ident.applyUnit` に通す（ADR-0011）

`Lawean/Data/` は Rust（`lawean-lean`）が e-Gov のリビジョンと改め文から出したもの。
ここでは、束縛した改正単位を発射台に当てた結果が e-Gov の改正後リビジョンと一致することを `native_decide` で検査する。
これで `applyOp_comm` / `applyUnit_comm` 等の定理は、玩具データではなく**この実データに載っている `applyUnit`** について言える。

一致は `Revision.render`（id を捨てた (条, 本文) の列）で見る。Lean が当てた結果の id は発射台の id と改正法が振った id のままで、
e-Gov のリビジョンは stable_id を振り直しているので、id まで含めて等しくはならない。
-/

namespace Lawean.Data
open Lawean.Ident

/-- 令和3年法律第37号 第35条: 2021-05-19 版に当てると 2022-05-18 版になる（本則 1 項ずつ、目次も） -/
theorem consolidates_503AC0000000037_art35 :
    (applyUnit rev_403AC0000000090_20210519 unit_503AC0000000037_art35).map Revision.render =
      some rev_403AC0000000090_20220518.render := by
  native_decide

/-- 結果は id が重複せず、衝突（期待した本文と違った項）も無い = 発射台が合っている -/
theorem wf_503AC0000000037_art35 :
    (applyUnit rev_403AC0000000090_20210519 unit_503AC0000000037_art35).map
      (fun r => (r.wf, r.hasConflict)) = some (true, false) := by
  native_decide

/-- 令和4年法律第48号 第73条（2 段目）: 2022-05-18 版（= 2022-05-25 版）に当てると 2023-02-20 版になる -/
theorem consolidates_504AC0000000048_art73 :
    (applyUnit rev_403AC0000000090_20220518 unit_504AC0000000048_art73).map Revision.render =
      some rev_403AC0000000090_20230220.render := by
  native_decide

/-- 同 第74条（3 段目）を第73条に続けて当てると現行（2026-05-21 版）になる -/
theorem consolidates_504AC0000000048_art73_74 :
    (applyUnit rev_403AC0000000090_20220518 (unit_504AC0000000048_art73 ++ unit_504AC0000000048_art74)).map
      Revision.render = some rev_403AC0000000090_20260521.render := by
  native_decide

theorem wf_504AC0000000048_art73_74 :
    (applyUnit rev_403AC0000000090_20220518 (unit_504AC0000000048_art73 ++ unit_504AC0000000048_art74)).map
      (fun r => (r.wf, r.hasConflict)) = some (true, false) := by
  native_decide

/-- 令和5年法律第53号 第125条（4 段目、未施行）: 現行に当てると 2028-06-13 版になる。
第47条〜第61条を第49条〜第64条にする条ずれ（`renumber`）と 3 条の新設を含む -/
theorem consolidates_505AC0000000053_art125 :
    (applyUnit rev_403AC0000000090_20260521 unit_505AC0000000053_art125).map Revision.render =
      some rev_403AC0000000090_20280613.render := by
  native_decide

theorem wf_505AC0000000053_art125 :
    (applyUnit rev_403AC0000000090_20260521 unit_505AC0000000053_art125).map
      (fun r => (r.wf, r.hasConflict)) = some (true, false) := by
  native_decide

-- 施行順序 ------------------------------------------------------------

/-- 第74条は第73条が作った id（第61条第1項）を触るので第73条に依存する。逆は無い -/
theorem art74_depends_on_art73 :
    dependsOn unit_504AC0000000048_art74 unit_504AC0000000048_art73 = true ∧
    dependsOn unit_504AC0000000048_art73 unit_504AC0000000048_art74 = false := by
  native_decide

/-- 第73条 → 第74条の順は依存の線形拡張。逆は違反 -/
theorem schedule_504AC0000000048 :
    scheduleOk [unit_504AC0000000048_art73, unit_504AC0000000048_art74] = true ∧
    scheduleOk [unit_504AC0000000048_art74, unit_504AC0000000048_art73] = false := by
  native_decide

/-- 違反した順序で無理に当てると、第61条が無くて失敗する（Rust の `stage_order_matters` と同じ） -/
theorem art74_before_art73_fails :
    applyUnit rev_403AC0000000090_20220518 (unit_504AC0000000048_art74 ++ unit_504AC0000000048_art73) = none := by
  native_decide

-- 独立と可換 ------------------------------------------------------------

/-- 令和3年 第35条（第22・38・39条）と令和4年 第73条（目次・第42・61条）は触る id が交わらない -/
theorem independent_art35_art73 :
    IndependentUnits unit_503AC0000000037_art35 unit_504AC0000000048_art73 := by
  native_decide

/-- したがって**どの発射台に対しても**順序を入れ替えられる。定理 `applyUnit_comm` から。
（第73条は第35条と同じ本文を触らないので、2021-05-19 版に束縛しても同じ操作列になる） -/
theorem art35_art73_commute (r : Revision) :
    applyUnit r (unit_503AC0000000037_art35 ++ unit_504AC0000000048_art73) =
      applyUnit r (unit_504AC0000000048_art73 ++ unit_503AC0000000037_art35) :=
  applyUnit_comm r _ _ independent_art35_art73

/-- 具体例: 2021-05-19 版から両方を当てると（どちらの順でも）2023-02-20 版になる -/
theorem both_orders_reach_20230220 :
    (applyUnit rev_403AC0000000090_20210519 (unit_504AC0000000048_art73 ++ unit_503AC0000000037_art35)).map
      Revision.render = some rev_403AC0000000090_20230220.render := by
  native_decide

-- 発射台のずれは衝突として残る ------------------------------------------------------------

/-- 改正済みの 2022-05-18 版に第35条をもう一度当てると、失敗ではなく成功し、
「前項」を手当てした 2 つの項が衝突（期待した本文と違う）として残る。黙って片方を勝たせない -/
theorem art35_on_wrong_base_conflicts :
    (applyUnit rev_403AC0000000090_20220518 unit_503AC0000000037_art35).map
      (fun r => r.conflicts.map (·.1)) =
      some ["403AC0000000090/main/chap:3/sec:3/art:38/para:2", "403AC0000000090/main/chap:3/sec:3/art:38/para:3"] := by
  native_decide

#print axioms consolidates_503AC0000000037_art35
#print axioms art35_art73_commute

end Lawean.Data
