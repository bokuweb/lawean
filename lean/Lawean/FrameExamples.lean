import Lawean.Properties

/-!
# 改正 × 意味 — 実データ（docs/10 M3）

令和3年法律第37号 第35条（第22・38・39条を触る）の前後で、第3・4・9条の性質が**再証明なしに**成り立つことを frame 定理で示す。
性質は現行（`sem_403AC0000000090_hand`、施行後）について証明してあり（`Properties.lean`）、
改正前（`sem_403AC0000000090_20210519_hand`、第22条第2項が無い）へ移送する。逆向きも同じ定理。

「触らない」は 2 段で確かめる:
1. `modified_503AC0000000037_art35`（改正単位が本文を変えた・作った項の e-Gov id、Rust が計算）と S の Rule の項が交わらない
2. S が両 Model に同じレコードで入っている（`Sub`、`rfl`）— これが定理の前提。1 は人が読むための説明

`touched`（anchor を含む）だと第22条第1項が入る（第2項をその後ろに加えたので）が、anchor は位置を指すだけで本文は変わらない。
実際 S9 は `touched` とは交わるが `modified` とは交わらず、`Sub S9 M0` は成り立つ。

触る改正の例として、第3条ただし書きを書き換える架空の改正も置く。S3 の Rule のレコードが変わるので `Sub` が成り立たず、
性質は再検証になり、実際に破れる（反例）。
-/

namespace Lawean.Data
open Lawean.Sem

abbrev M0 := sem_403AC0000000090_20210519_hand

theorem M0_wf : M0.wf = true := by decide

/-- S の Rule の項は、改正単位が触った項に含まれない -/
def untouched (S : List Rule) (touched : List String) : Bool :=
  S.all fun r => !(decide (r.para ∈ touched))

-- 1. 令3-37 は第3・4・9条の Rule を触らない ------------------------------------------------------------

theorem S3_untouched : untouched S3 modified_503AC0000000037_art35 = true := by decide
theorem S4_untouched : untouched S4 modified_503AC0000000037_art35 = true := by decide
theorem S9_untouched : untouched S9 modified_503AC0000000037_art35 = true := by decide
/-- anchor まで含めると第22条第1項で交わる（本文は変わっていない） -/
theorem S9_anchor_touched : untouched S9 touched_503AC0000000037_art35 = false := by decide

-- 2. 同じレコードで改正前の Model に入っている ------------------------------------------------------------

theorem S3_sub0 : Sub S3 M0 := by sub S3
theorem S4_sub0 : Sub S4 M0 := by sub S4
theorem S9_sub0 : Sub S9 M0 := by sub S9

-- 3. 性質の移送（再証明なし） ------------------------------------------------------------

/-- 第3条: 改正前でも 存続期間 ≥ 30 年 -/
theorem art3_ge_30_before : ∀ w, consistent M0 w = true → 360 ≤ w.ints "a:存続期間" :=
  transfer M M0 M_wf M0_wf S3 S3_sub S3_sub0 S3_closed _ art3_ge_30_on

/-- 第4条: 改正前でも 更新後の期間 ≥ 10 年 / 最初の更新なら ≥ 20 年 -/
theorem art4_ge_10_before : ∀ w, consistent M0 w = true →
    w.bools "p:更新する" = true → 120 ≤ w.ints "a:更新後の期間" :=
  transfer M M0 M_wf M0_wf S4 S4_sub S4_sub0 S4_closed _ art4_ge_10_on

theorem art4_first_ge_20_before : ∀ w, consistent M0 w = true →
    w.bools "p:更新する" = true → w.bools "p:最初の更新" = true → 240 ≤ w.ints "a:更新後の期間" :=
  transfer M M0 M_wf M0_wf S4 S4_sub S4_sub0 S4_closed _ art4_first_ge_20_on

/-- 第9条 + 補題: 改正前でも -/
theorem art9_voids_short_before : ∀ w, consistent M0 w = true →
    (w.ints "a:契約で定めた期間" < 360 →
      w.bools "p:反する(target=Scope(StableId(\"403AC0000000090/main/chap:2/sec:1\")))" = true ∧
      w.bools "p:不利(to=D:借地権者,judgement=Unknown(UnknownExpr { kind: Intentional, text: \"不利\" }))" = true) →
    w.ints "a:契約で定めた期間" < 360 → w.bools "void:特約" = true :=
  transfer M M0 M_wf M0_wf S9 S9_sub S9_sub0 S9_closed _ art9_voids_short_on

-- 4. 触る改正: 第3条ただし書きを「二十年より長い期間」に書き換える架空の改正 ------------------------------------------------------------

/-- 改正後の第3条ただし書き。本文が変わったので層 2 が抽出し直した Rule（ここでは手書き） -/
def «R3-2@draft» : Rule :=
  { «R3-2» with cond := .cmp (.var "a:契約で定めた期間") .gt (.int 240) }

/-- 架空の改正後の Model: R3-2 だけ差し替え -/
def M_draft : Model :=
  { rules := M.rules.map fun r => if r.id = "R3-2" then «R3-2@draft» else r }

theorem M_draft_wf : M_draft.wf = true := by decide

/-- 架空の改正が触った項 = 第3条第1項。S3 の Rule の項と交わる → frame は使えない（再検証） -/
def touched_draft : List String := ["403AC0000000090/main/chap:2/sec:1/art:3/para:1"]
theorem S3_touched_by_draft : untouched S3 touched_draft = false := by decide

/-- 再検証すると破れる: 契約で 25 年と定めると存続期間は 25 年 < 30 年 -/
theorem art3_ge_30_fails_after_draft :
    ∃ w : World, consistent M_draft w = true ∧ w.ints "a:存続期間" < 360 :=
  ⟨{ ints := fun n => if n = "a:契約で定めた期間" then 300 else if n = "a:存続期間" then 300 else 0,
     bools := fun n => n = "eff:R6" },
   by native_decide, by decide⟩

/-- 第4条は第3条の改正に依存しないので、そのまま移送できる -/
theorem S4_sub_draft : Sub S4 M_draft := by sub S4
theorem art4_ge_10_after_draft : ∀ w, consistent M_draft w = true →
    w.bools "p:更新する" = true → 120 ≤ w.ints "a:更新後の期間" :=
  transfer M M_draft M_wf M_draft_wf S4 S4_sub S4_sub_draft S4_closed _ art4_ge_10_on

#print axioms art3_ge_30_before
#print axioms art4_ge_10_after_draft

end Lawean.Data
