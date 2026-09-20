# 附則第6条（借地契約の更新に関する経過措置）— 「なお従前の例による」

## 原文

> この法律の施行前に設定された借地権に係る契約の更新に関しては、なお従前の例による。

## 期待する Semantic IR

```
Rule RS6
  subject:    Ref(D:借地権)
  condition:  Pred(設定された, when = Before(Event(施行日)))
  effect:     ApplyExternal(
                law   = 旧借地法（廃止前）,
                topic = 契約の更新,                            ← 「〜に関しては」
                instead_of = [R4, R5, R6, R7?]                 ← 新法の更新規定
              )
  overrides:  [RS4]                                         ← 附則第4条の「特別の定め」
  temporal:   { facts_before: Event(施行日) }
  provenance: suppl:0/art:6/para:1/sent:1 (high, human)
```

## 論点

- **「なお従前の例による」は法制執務の定型**で、「廃止・改正前の規定を、その法令が存在するかのように適用する」という意味。
  `ApplyExternal` は「別の法令の Rule 群をこの topic について適用する」効果。準用（第○条の規定は〜について準用する）と近いが、
  準用は同じ法令内の Rule を読み替えて適用するのに対し、こちらは**存在しない法令**を適用する
- **「〜に関しては」= topic による適用範囲の限定**。「契約の更新に関する」Rule とは新法のどれか、を機械的に決めるのは難しい。
  v0.1 では `instead_of` は人手で書き、confidence を medium にする
- この Rule があるため、**1992-08-01 より前に設定された借地権の更新には第4〜7条が適用されない**。
  Resolved IR では「借地権の設定日」という Fact によって適用される Rule 集合が変わる。
  → Temporal resolver は「Rule の有効期間」だけでなく「**Fact の発生時点による Rule の選択**」を扱う必要がある
- 今も現役の経過措置（旧法借地権は 2026 年現在も多数存在する）。検証の実用上、附則は本則と同じ重さで扱う
