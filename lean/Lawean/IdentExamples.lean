import Lawean.Ident

/-!
# 実行例 — identity patch（ADR-0013）

借地借家法の実際の改正 3 件（令和3年法律第37号 第35条、令和4年法律第48号 第73・74条）の形を単純化し、
番号ベースでは扱えなかった「割り込み」「依存」「衝突」を id ベースで検査する。
-/

namespace Lawean.Ident

/-- 改正前（2021-05-19 版を単純化）。id は `{条}/{項}` を stable_id の代わりに使う -/
def rev0 : Revision :=
  { nodes :=
      [ { id := "22/1", art := 22, text := "書面によって契約をするとき" },
        { id := "38/1", art := 38, text := "定期建物賃貸借は書面によってする" },
        { id := "38/2", art := 38, text := "あらかじめ書面を交付して説明する" },
        { id := "38/3", art := 38, text := "前項の規定に違反したときは無効とする" } ] }

/-- 令和3年法律第37号 第35条: 第38条第1項の次に一項を加える -/
def r3_37 : AmendUnit :=
  [ .insertAfter "38/1" "r3-37/38/1a" 38 "電磁的記録によることができる" ]

/-- 令和4年法律第48号 第73条: 第61条を新設（1 段目） -/
def r4_48_73 : AmendUnit :=
  [ .insertAfter "38/3" "r4-48/61/1" 61 "民事訴訟法の特例（旧）" ]

/-- 同 第74条: 第61条を全部改正（3 段目）。第73条が作った id を触るので第73条に依存する -/
def r4_48_74 : AmendUnit :=
  [ .replace "r4-48/61/1" "民事訴訟法の特例（旧）" "民事訴訟法の特例（新）" ]

-- 1. 割り込み ------------------------------------------------------------

/-- 発射台 rev0 に対して書いた改正案: 旧第38条第3項を改める -/
def draft : AmendUnit := [ .replace "38/3" "前項の規定に違反したときは無効とする" "第二項の規定に違反したときは無効とする" ]

/-- 令和3年改正が先に割り込んで旧第3項が第4項に繰り下がっても、id で指しているので同じ項に当たる -/
example : (applyUnit rev0 (r3_37 ++ draft)).map (fun r => (r.nodes.find? (·.id = "38/3")).map (·.text)) =
    some (some "第二項の規定に違反したときは無効とする") := by native_decide

/-- 番号は描画時に計算する: 割り込み前は第3項、割り込み後は第4項 -/
example : paraNum rev0 "38/3" = some 3 := by native_decide
example : (applyUnit rev0 r3_37).bind (paraNum · "38/3") = some 4 := by native_decide

-- 2. 独立なら可換 ------------------------------------------------------------

/-- 令和3年 第35条と令和4年 第73条は触る id が交わらない -/
example : independentUnits r3_37 r4_48_73 = true := by native_decide

/-- 具体例: どちらの順でも同じ（native_decide） -/
example : applyUnit rev0 (r3_37 ++ r4_48_73) = applyUnit rev0 (r4_48_73 ++ r3_37) := by native_decide

/-- 同じことを定理から: 独立性は decide で判定でき、あとは applyUnit_comm が全ての発射台について言う -/
example (r : Revision) : applyUnit r (r3_37 ++ r4_48_73) = applyUnit r (r4_48_73 ++ r3_37) :=
  applyUnit_comm r r3_37 r4_48_73 (by decide)

-- 3. 依存と施行順序 ------------------------------------------------------------

example : dependsOn r4_48_74 r4_48_73 = true := by native_decide
example : dependsOn r4_48_73 r4_48_74 = false := by native_decide

/-- 第73条 → 第74条の順は依存の線形拡張。逆は違反 -/
example : scheduleOk [r4_48_73, r4_48_74] = true := by native_decide
example : scheduleOk [r4_48_74, r4_48_73] = false := by native_decide

/-- 違反した順序で無理に当てると、依存先の id が無くて失敗する -/
example : applyUnit rev0 (r4_48_74 ++ r4_48_73) = none := by native_decide
example : (applyUnit rev0 (r4_48_73 ++ r4_48_74)).map (·.wf) = some true := by native_decide

-- 4. 衝突と調整規定 ------------------------------------------------------------

/-- 2 つの改正法が同じ項を別々に改める -/
def amendA : AmendUnit := [ .replace "22/1" "書面によって契約をするとき" "電磁的記録によって契約をするとき" ]
def amendB : AmendUnit := [ .replace "22/1" "書面によって契約をするとき" "書面又は電子契約によって契約をするとき" ]

/-- 依存も独立もしていない（同じ id を触る）ので、順序に意味がある -/
example : independentUnits amendA amendB = false := by native_decide

/-- どちらの順でも溶け込みは成功するが、後から当てた方が衝突として残る。黙って片方を勝たせない -/
example : (applyUnit rev0 (amendA ++ amendB)).map (·.conflicts) =
    some [("22/1", "電磁的記録によって契約をするとき", ["書面又は電子契約によって契約をするとき"])] := by
  native_decide
example : (applyUnit rev0 (amendB ++ amendA)).map (·.hasConflict) = some true := by native_decide

/-- 調整規定 = 両方に依存する patch。衝突を解消して本文を確定する -/
def adjust : AmendUnit := [ .resolve "22/1" "書面、電磁的記録又は電子契約によって契約をするとき" ]

example : (applyUnit rev0 (amendA ++ amendB ++ adjust)).map (·.hasConflict) = some false := by native_decide
example : (applyUnit rev0 (amendB ++ amendA ++ adjust)).map (·.hasConflict) = some false := by native_decide

#eval (applyUnit rev0 (r3_37 ++ r4_48_73 ++ r4_48_74 ++ draft)).map fun r =>
  r.nodes.map fun n => (n.id, paraNum r n.id, n.text)

#print axioms applyOp_comm
#print axioms applyUnit_comm

end Lawean.Ident
