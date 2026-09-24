import Lawean.Stage
import Lawean.IdentExamples

/-!
# 実行例 — 部分施行（`Stage.lean`）

令3-49 第6条の形: 1 つの改正単位（改正法の条）のうち一部の改正規定だけを附則第一条の号で別の日に施行する。
`lawean-check::stage::parts_of` が命令を部分列に分け、施行日ごとに当てる。
-/

namespace Lawean.Ident

/-- 1 つの改正単位: 第22条と第38条第3項を改める -/
def op22 : Op := .replace "22/1" "書面によって契約をするとき" "書面又は電磁的記録によって契約をするとき"
def op38 : Op := .replace "38/3" "前項の規定に違反したときは無効とする" "第二項の規定に違反したときは無効とする"
def whole : AmendUnit := [op22, op38]

/-- 号で第38条の改正規定だけを先に施行（公布の日） -/
def early : AmendUnit := [op38]
/-- 残り（本文の日） -/
def late : AmendUnit := [op22]

-- 独立な分割: 部分列への分割で、部分どうしが独立
example : isSplit whole [early, late] = true := by native_decide
example : indepParts [early, late] = true := by native_decide

/-- 号の日 → 本文の日の順に当てても、単位全体を一度に当てたのと同じ（定理 `applyParts_eq` の実例） -/
example : applyParts rev0 [early, late] = applyUnit rev0 whole := by native_decide
/-- 施行日の順が逆（政令で本文の日が先に来た）でも同じ -/
example : applyParts rev0 [late, early] = applyUnit rev0 whole := by native_decide

-- 分割の誤り: 命令を落とす・二重に数える
example : isSplit whole [early] = false := by native_decide
example : isSplit whole [early, late, early] = false := by native_decide

-- 独立でない分割: 同じ項を 2 段で改める単位を、後の文だけ先に施行
def op38b : Op := .replace "38/3" "第二項の規定に違反したときは無効とする" "第二項又は第三項の規定に違反したときは無効とする"
def chain : AmendUnit := [op38, op38b]

example : isSplit chain [[op38b], [op38]] = true := by native_decide
/-- 部分列への分割ではあるが独立でない → `lawean-check::stage::overlaps` が「同じ箇所」を報告する場合 -/
example : indepParts [[op38b], [op38]] = false := by native_decide
/-- 実際に結果が変わる: 後の文を先に当てると期待した本文と違い、衝突が残る -/
example : applyParts rev0 [[op38b], [op38]] ≠ applyUnit rev0 chain := by native_decide
example : ((applyParts rev0 [[op38b], [op38]]).map Revision.hasConflict) = some true := by
  native_decide

end Lawean.Ident
