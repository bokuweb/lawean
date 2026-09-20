import Lawean.Refs
import Lawean.Data.Rev_325AC1000000100_20180620
import Lawean.Data.Rev_325AC1000000100_20201212
import Lawean.Data.Rev_325AC1000000100_20210602
import Lawean.Data.Unit_430AC0100000075
import Lawean.Data.Unit_503AC0000000051

/-!
# 実際に起きた改正漏れ — 公職選挙法（docs/12 §6）

平成30年法律第75号（参議院の特定枠）は第142条の4に第4項を挿入して第6項を第7項に繰り下げたが、
罰則の第244条第1項第2号の2「第百四十二条の四第六項の規定に違反して…」を改めなかった。
表示義務違反の罰則が消えた状態が施行（2018-10-25）から令和3年法律第51号（2021-06-02）まで続いた
（衆議院 第204回国会 質問第120号・答弁書）。

ここでは:
1. 令和3年法律第51号（訂正）を実際の発射台に当てると e-Gov の改正後リビジョン（本則 1167 項）と一致する
2. 平成30年法律第75号を発射台に当て、第244条第1項の本文を id 参照で描画し直すと、**Lean が生成する手当てが、
   3 年後に国会が成立させた訂正の `replace` と一字違わず同じ**になる
3. 平成30年法律第75号自身にはその置換が無い（= 見落とし）
-/

namespace Lawean.Data
open Lawean.Ident

abbrev K0 := rev_325AC1000000100_20180620
abbrev U75 := unit_430AC0100000075
abbrev U51 := unit_503AC0000000051

def A244 : NodeId := "325AC1000000100/main/chap:16/art:244/para:1"
def A142_4_6 : NodeId := "325AC1000000100/main/chap:13/art:142_4/para:6"

/-- 1. 訂正法は e-Gov と一致する -/
theorem consolidates_503AC0000000051 :
    (applyUnit rev_325AC1000000100_20201212 U51).map Revision.render =
      some rev_325AC1000000100_20210602.render := by
  native_decide

/-- 第244条第1項の本文を id 参照で: 「第百四十二条の四第六項」の 1 箇所を、旧第6項（`A142_4_6`）への他条参照にする -/
def body244 : Body :=
  match (K0.nodes.find? (·.id = A244)).map (·.text) with
  | some t =>
    match t.splitOn "第百四十二条の四第六項" with
    | [pre, post] => [ .text pre, .ref A142_4_6 .absoluteArt, .text post ]
    | _ => []
  | none => []

/-- 描画は発射台の本文そのもの（Body の作り方が正しい） -/
example : (K0.nodes.find? (·.id = A244)).map (·.text) = some (renderBody K0 A244 body244) := by
  native_decide

/-- 2. 平成30年法律第75号を当てた後、第244条第1項の描画が変わる = 手当てが 1 つ生成される。
その (id, 旧本文, 新本文) は、令和3年法律第51号の `replace` そのもの -/
theorem generated_fix_is_the_2021_correction :
    (applyUnit K0 U75).map (fun r' =>
      let fixes := haneFixes K0 r' [(A244, body244)]
      fixes.length = 1 && fixes.all fun (id, b, a) => U51.contains (.replace id b a)) = some true := by
  native_decide

/-- 生成した新本文には「第百四十二条の四第七項」が入っている -/
theorem generated_fix_says_seventh :
    (applyUnit K0 U75).map (fun r' =>
      (haneFixes K0 r' [(A244, body244)]).all fun (_, _, a) =>
        (a.splitOn "第百四十二条の四第七項").length = 2) = some true := by
  native_decide

/-- 3. 平成30年法律第75号には第244条への置換が無い（見落とし） -/
theorem h30_75_does_not_touch_244 :
    U75.all (fun op => match op with
      | .replace id _ _ => id != A244
      | _ => true) = true := by
  native_decide

#print axioms generated_fix_is_the_2021_correction

end Lawean.Data
