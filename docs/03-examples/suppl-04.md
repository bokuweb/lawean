# 附則第4条（経過措置の原則）— 施行日を境にした時間条件と旧法への参照

## 原文

> この法律の規定は、この附則に特別の定めがある場合を除き、この法律の施行前に生じた事項にも適用する。
> ただし、附則第二条の規定による廃止前の建物保護に関する法律、借地法及び借家法の規定により生じた効力を妨げない。

stable_id: `suppl:0/art:4/para:1/sent:1`（main）, `sent:2`（proviso）

## 期待する Semantic IR

```
Rule RS4
  subject:    Scope(main)                                   ← 「この法律の規定」= 本則の全 Rule
  condition:  And(
                Pred(事項が生じた, when = Before(Event(施行日))),
                Not(Ref(Scope(suppl:0), kind = 特別の定め))    ← 「この附則に特別の定めがある場合を除き」
              )
  effect:     Apply(Scope(main))                            ← 遡及適用
  temporal:   Retroactive { boundary: Event(施行日) }
  provenance: suppl:0/art:4/para:1/sent:1 (high, human)

Rule RS4-proviso
  condition:  Pred(効力が生じた, by = External { law: 旧建物保護法 | 旧借地法 | 旧借家法, repealed_by = suppl:0/art:2 })
  effect:     Preserve(効力)                                ← 「妨げない」
  overrides:  [RS4]
  provenance: suppl:0/art:4/para:1/sent:2 (high, human)
```

## 論点

- **「施行日」は法令内では確定していない**。附則第1条「公布の日から起算して一年を超えない範囲内において政令で定める日」。
  実際の施行日（1992-08-01）は政令で決まる。IR では `Event(施行日)` を**シンボルとして持ち**、値は外から与える
  （`LegalDocument.enforced_on`）。Z3 では定数として束縛する
- **廃止された法律への参照**。`External { law }` の law_id は e-Gov に旧借地法（大正10年法律第49号）が残っているか要確認。
  無ければ `Unknown(External)` で「廃止法令: 借地法」とだけ記録する
- 「この附則に特別の定めがある場合を除き」= **附則内の他の Rule が優先**。附則第5条以降が RS4 を `overrides` する。
  これは各附則条文の側に `overrides: [RS4]` を書くべきで、RS4 側の condition に `Not(...)` を書くのは ADR-0004 に反する。
  → 上の `Not(Ref(...))` は取り下げ。RS4 の condition は `Pred(施行前に生じた)` だけにし、附則第5・6条側が overrides を持つ
- **`Retroactive` という Temporal の種類**。通常の Rule は `effective: Range(施行日, ∞)` だが、この Rule は
  「施行前の事項にも及ぶ」。Temporal に `applies_to_facts_before: bool` 相当が要る
- `Preserve(効力)` は新しい Effect。「妨げない」= 旧法下で発生した法律効果は消えない。Void の反対
