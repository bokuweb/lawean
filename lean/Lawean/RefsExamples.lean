import Lawean.Refs
import Lawean.Data.Rev_403AC0000000090_20210519
import Lawean.Data.Unit_503AC0000000037_art35

/-!
# ハネの手当ての生成 — 実データ（令和3年法律第37号 第35条、第38条）

第38条第2項・第3項の本文を、参照を id で持つ `Body` として書く（層 2 の参照解決が出す形。ここでは手書き）。
令3-37 を当てた後の描画との差が、実際の改め文の「前項」→「第一項」「第三項」の置換と**一字も違わず**一致する。
-/

namespace Lawean.Data
open Lawean.Ident

abbrev P (n : Nat) : NodeId := "403AC0000000090/main/chap:3/sec:3/art:38/para:" ++ toString n

/-- 改正前の第38条第2項・第3項。「前項」は id 参照（相対形、距離 1）。「同項」は先行詞に追随するので平文のまま -/
def bodies38 : Bodies :=
  [ (P 2, [ .ref (P 1) (.prev 1),
            .text "の規定による建物の賃貸借をしようとするときは、建物の賃貸人は、あらかじめ、建物の賃借人に対し、同項の規定による建物の賃貸借は契約の更新がなく、期間の満了により当該建物の賃貸借は終了することについて、その旨を記載した書面を交付して説明しなければならない。" ]),
    (P 3, [ .text "建物の賃貸人が",
            .ref (P 2) (.prev 1),
            .text "の規定による説明をしなかったときは、契約の更新がないこととする旨の定めは、無効とする。" ]) ]

abbrev R0 := rev_403AC0000000090_20210519
abbrev U := unit_503AC0000000037_art35

/-- 改正前の描画は発射台の本文と一致する（Body の書き方が正しい） -/
example : bodies38.all (fun (id, body) =>
    (R0.nodes.find? (·.id = id)).map (·.text) == some (renderBody R0 id body)) = true := by
  native_decide

/-- 生成した手当て: 第2項の「前項」→「第一項」、第3項の「前項」→「第三項」（相対形の距離が 2 になるので絶対形） -/
theorem hane_fixes_38 :
    (applyUnit R0 U).map (fun r' => haneFixes R0 r' bodies38 |>.map fun (id, b, a) => (id, b.take 4, a.take 4)) =
      some [(P 2, "前項の規", "第一項の"), (P 3, "建物の賃", "建物の賃")] := by
  native_decide

/-- **生成した手当ては、実際の改め文の置換そのもの**: 期待本文と新本文が一字違わず `replace` として入っている -/
theorem hane_fixes_are_in_the_real_amendment :
    (applyUnit R0 U).map (fun r' =>
      (haneFixes R0 r' bodies38).all fun (id, b, a) => U.contains (.replace id b a)) = some true := by
  native_decide

/-- 逆に、改め文の中の `replace` はこの 2 つだけ（= 手当て以外の本文の書き換えは無い） -/
theorem real_replaces_are_exactly_the_fixes :
    (applyUnit R0 U).map (fun r' =>
      decide ((U.filter fun op => match op with | .replace .. => true | _ => false).length =
        (haneFixes R0 r' bodies38).length)) = some true := by
  native_decide

#print axioms hane_complete
#print axioms haneFixes_sound

end Lawean.Data
