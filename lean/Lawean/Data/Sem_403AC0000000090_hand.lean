import Lawean.Sem

/-!
自動生成: `cargo run -p lawean-lean --example gen`。手で編集しない。

借地借家法の手書き Semantic IR（`lawean-semantic/src/examples/shakuchi_shakuya.rs`、docs/03-examples の第2〜6・9・22・26条）。Rule は層化された順（例外 → 原則、参照先 → 参照元）。

- `sem_403AC0000000090_hand`: 現行（`403AC0000000090_20260521_504AC0000000048`）
- `sem_403AC0000000090_20210519_hand`: 令和3年法律第37号 第35条の発射台（2021-05-19 版）に存在する項の Rule だけ（第22条第2項が無い）
- `touched_503AC0000000037_art35`: 同条が触った項（anchor を含む。e-Gov の 2022-05-18 版の id）
- `modified_503AC0000000037_art35`: 同条が本文を変えた・作った項（anchor を除く）。frame 定理（`Frame.lean`）で「触らない Rule の性質は保たれる」を言うのに使う
-/

namespace Lawean.Data
open Lawean.Sem

def «R3-2» : Rule :=
  { id := "R3-2",
    cond := (.cmp (.var "a:契約で定めた期間") .gt (.int 360)),
    effect := (.set "a:存続期間" (.var "a:契約で定めた期間")),
    overrides := ["R3-1"],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:2",
    conf := 100 }

def «R3-1» : Rule :=
  { id := "R3-1",
    cond := .tt,
    effect := (.set "a:存続期間" (.int 360)),
    overrides := [],
    exceptions := ["R3-2"],
    source := "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:1",
    conf := 100 }

def «R4-2'» : Rule :=
  { id := "R4-2'",
    cond := (.and [(.pred "p:更新する"), (.pred "p:最初の更新"), (.cmp (.var "a:当事者が定めた期間") .gt (.ruleValue "R4-1'"))]),
    effect := (.set "a:更新後の期間" (.var "a:当事者が定めた期間")),
    overrides := ["R4-1'"],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:4/para:1/sent:2",
    conf := 100 }

def «R4-1'» : Rule :=
  { id := "R4-1'",
    cond := (.and [(.pred "p:更新する"), (.pred "p:最初の更新")]),
    effect := (.set "a:更新後の期間" (.int 240)),
    overrides := ["R4-1"],
    exceptions := ["R4-2'"],
    source := "403AC0000000090/main/chap:2/sec:1/art:4/para:1/sent:1",
    conf := 100 }

def «R4-2» : Rule :=
  { id := "R4-2",
    cond := (.and [(.pred "p:更新する"), (.cmp (.var "a:当事者が定めた期間") .gt (.ruleValue "R4-1"))]),
    effect := (.set "a:更新後の期間" (.var "a:当事者が定めた期間")),
    overrides := ["R4-1"],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:4/para:1/sent:2",
    conf := 100 }

def «R4-1» : Rule :=
  { id := "R4-1",
    cond := (.pred "p:更新する"),
    effect := (.set "a:更新後の期間" (.int 120)),
    overrides := [],
    exceptions := ["R4-1'", "R4-2"],
    source := "403AC0000000090/main/chap:2/sec:1/art:4/para:1/sent:1",
    conf := 100 }

def «R5-1-proviso» : Rule :=
  { id := "R5-1-proviso",
    cond := (.pred "p:異議を述べた(by=D:借地権設定者,timing=Unknown(UnknownExpr { kind: Intentional, text: \"遅滞なく\" }))"),
    effect := (.exception "R5-1"),
    overrides := ["R5-1"],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:5/para:1/sent:2",
    conf := 100 }

def «R5-1» : Rule :=
  { id := "R5-1",
    cond := (.and [(.pred "p:存続期間が満了する"), (.pred "p:更新を請求した(by=D:借地権者)"), (.pred "p:建物がある")]),
    effect := (.deem "p:契約を更新した(conditions=従前と同一)"),
    overrides := [],
    exceptions := ["R5-1-proviso"],
    source := "403AC0000000090/main/chap:2/sec:1/art:5/para:1/sent:1",
    conf := 100 }

def «R5-2» : Rule :=
  { id := "R5-2",
    cond := (.and [(.pred "p:存続期間が満了した"), (.pred "p:土地の使用を継続する(by=D:借地権者)"), (.pred "p:建物がある")]),
    effect := (.sameAs "R5-1"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:5/para:2/sent:1",
    conf := 60 }

def «R5-3» : Rule :=
  { id := "R5-3",
    cond := (.pred "p:転借地権が設定されている"),
    effect := (.sameAs "R5-2"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:5/para:3/sent:1",
    conf := 60 }

def «R6» : Rule :=
  { id := "R6",
    cond := (.not (.pred "p:正当の事由がある(factors=使用を必要とする事情+従前の経過+土地の利用状況+立退料の申出,judgement=Unknown(UnknownExpr { kind: Intentional, text: \"正当の事由があると認められる\" }))")),
    effect := (.mark "R6"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:1/art:6/para:1/sent:1",
    conf := 100 }

def «R22-1a» : Rule :=
  { id := "R22-1a",
    cond := (.cmp (.var "a:存続期間") .ge (.int 600)),
    effect := (.mark "R22-1a"),
    overrides := ["R9"],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:4/art:22/para:1/sent:1",
    conf := 100 }

def «R9» : Rule :=
  { id := "R9",
    cond := (.and [(.pred "p:反する(target=Scope(StableId(\"403AC0000000090/main/chap:2/sec:1\")))"), (.pred "p:不利(to=D:借地権者,judgement=Unknown(UnknownExpr { kind: Intentional, text: \"不利\" }))")]),
    effect := (.void "特約"),
    overrides := [],
    exceptions := ["R22-1a"],
    source := "403AC0000000090/main/chap:2/sec:1/art:9/para:1/sent:1",
    conf := 100 }

def «R22-1b» : Rule :=
  { id := "R22-1b",
    cond := (.ref "R22-1a"),
    effect := (.mark "R22-1b"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:4/art:22/para:1/sent:2",
    conf := 100 }

def «R22-2» : Rule :=
  { id := "R22-2",
    cond := (.pred "p:特約が電磁的記録によってされた(of=Rule(RuleId(\"R22-1a\")))"),
    effect := (.sameAs "R22-1b"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:2/sec:4/art:22/para:2/sent:1",
    conf := 100 }

def «R26-1» : Rule :=
  { id := "R26-1",
    cond := (.and [(.pred "p:期間の定めがある"), (.not (.pred "p:通知した(kind=更新しない+条件を変更しなければ更新しない,within=Time(Within(Window { from: Period { from: Event(\"期間の満了\"), length: Duration { length: 1, unit: Year }, direction: Backward }, to: Period { from: Event(\"期間の満了\"), length: Duration { length: 6, unit: Month }, direction: Backward } })))"))]),
    effect := (.deem "p:契約を更新した(conditions=従前と同一)"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:3/sec:1/art:26/para:1/sent:1",
    conf := 100 }

def «R26-1-proviso» : Rule :=
  { id := "R26-1-proviso",
    cond := (.ref "R26-1"),
    effect := (.set "a:期間" (.int (-1))),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:3/sec:1/art:26/para:1/sent:2",
    conf := 100 }

def «R26-2» : Rule :=
  { id := "R26-2",
    cond := (.and [(.pred "p:通知した"), (.pred "p:期間が満了した"), (.pred "p:使用を継続する(by=賃借人)"), (.not (.pred "p:異議を述べた(by=賃貸人,timing=Unknown(UnknownExpr { kind: Intentional, text: \"遅滞なく\" }))"))]),
    effect := (.sameAs "R26-1"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:3/sec:1/art:26/para:2/sent:1",
    conf := 100 }

def «R26-3» : Rule :=
  { id := "R26-3",
    cond := (.pred "p:転貸借がされている"),
    effect := (.sameAs "R26-2"),
    overrides := [],
    exceptions := [],
    source := "403AC0000000090/main/chap:3/sec:1/art:26/para:3/sent:1",
    conf := 100 }

def sem_403AC0000000090_hand : Model := { rules := [«R3-2», «R3-1», «R4-2'», «R4-1'», «R4-2», «R4-1», «R5-1-proviso», «R5-1», «R5-2», «R5-3», «R6», «R22-1a», «R9», «R22-1b», «R22-2», «R26-1», «R26-1-proviso», «R26-2», «R26-3»] }

def sem_403AC0000000090_20210519_hand : Model := { rules := [«R3-2», «R3-1», «R4-2'», «R4-1'», «R4-2», «R4-1», «R5-1-proviso», «R5-1», «R5-2», «R5-3», «R6», «R22-1a», «R9», «R22-1b», «R26-1», «R26-1-proviso», «R26-2», «R26-3»] }

def touched_503AC0000000037_art35 : List String := ["403AC0000000090/main/chap:2/sec:4/art:22/para:1", "403AC0000000090/main/chap:2/sec:4/art:22/para:2", "403AC0000000090/main/chap:3/sec:3/art:38/para:5", "403AC0000000090/main/chap:3/sec:3/art:38/para:3", "403AC0000000090/main/chap:3/sec:3/art:38/para:4", "403AC0000000090/main/chap:3/sec:3/art:38/para:1", "403AC0000000090/main/chap:3/sec:3/art:38/para:2", "403AC0000000090/main/chap:3/sec:3/art:39/para:2", "403AC0000000090/main/chap:3/sec:3/art:39/para:3"]

def modified_503AC0000000037_art35 : List String := ["403AC0000000090/main/chap:2/sec:4/art:22/para:2", "403AC0000000090/main/chap:3/sec:3/art:38/para:5", "403AC0000000090/main/chap:3/sec:3/art:38/para:3", "403AC0000000090/main/chap:3/sec:3/art:38/para:4", "403AC0000000090/main/chap:3/sec:3/art:38/para:2", "403AC0000000090/main/chap:3/sec:3/art:39/para:3"]

end Lawean.Data
