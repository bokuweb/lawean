import Lawean.Ident
import Lawean.Data.Rev_324AC0000000108_20220617
import Lawean.Data.Rev_324AC0000000108_20250601
import Lawean.Data.Unit_504AC0000000068_art98
import Lawean.Data.Unit_derived_324AC0000000108_20240401

/-!
# 先行改正との可換性 — 公布順と施行順が逆になった実例（docs/13）

令和4年法律第68号（刑法等一部改正法整備法、公布 2022-06-17、施行 2025-06-01）第98条は古物営業法を改める。
その改め文は公布時の古物営業法（2022-06-17 版）に対して書かれたが、当たるのは施行日の直前の状態で、
その間に後から公布された令和5年法律第79号が先に施行されている（2024-04-01）。

先行改正の改め文は使わず、e-Gov の 2 つの版の差を id の操作列にしたもの（`unit_derived_…`、Rust `ident::derive_unit`）と
第98条の束縛が**触る id で交わらない**ことを `native_decide` で確かめると、`applyUnit_comm` から
**どの発射台に対しても順序を入れ替えられる**。つまり調整規定は要らない（実際にも置かれていない）。
交わるなら順序で結果が変わるので、調整規定か改め文の書き直し（令3-37 附則第63条の形）が要る — それは Rust の `Stale` 検査が報告する。
-/

namespace Lawean.Data
open Lawean.Ident

/-- 第98条と先行改正は触る id が交わらない -/
theorem independent_art98_intervening :
    IndependentUnits unit_504AC0000000068_art98 unit_derived_324AC0000000108_20240401 := by
  native_decide

/-- したがってどの発射台に対しても順序を入れ替えられる（調整規定は要らない） -/
theorem art98_intervening_commute (r : Revision) :
    applyUnit r (unit_504AC0000000068_art98 ++ unit_derived_324AC0000000108_20240401) =
      applyUnit r (unit_derived_324AC0000000108_20240401 ++ unit_504AC0000000068_art98) :=
  applyUnit_comm r _ _ independent_art98_intervening

/-- 実際の順（先行改正 → 第98条）で 2022-06-17 版から e-Gov の 2025-06-01 版になる -/
theorem intervening_then_art98_reaches_20250601 :
    (applyUnit rev_324AC0000000108_20220617
      (unit_derived_324AC0000000108_20240401 ++ unit_504AC0000000068_art98)).map Revision.render =
      some rev_324AC0000000108_20250601.render := by
  native_decide

/-- 起草した順（第98条 → 先行改正）でも同じ（可換性の具体例。上の定理からも従う） -/
theorem art98_then_intervening_reaches_20250601 :
    (applyUnit rev_324AC0000000108_20220617
      (unit_504AC0000000068_art98 ++ unit_derived_324AC0000000108_20240401)).map Revision.render =
      some rev_324AC0000000108_20250601.render := by
  rw [art98_intervening_commute]
  exact intervening_then_art98_reaches_20250601

end Lawean.Data
