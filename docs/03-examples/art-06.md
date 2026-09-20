# 第6条（借地契約の更新拒絶の要件）— 正当事由 = 意図的な Unknown

## 原文

> 前条の異議は、借地権設定者及び借地権者（転借地権者を含む。以下この条において同じ。）が土地の使用を必要とする事情のほか、
> 借地に関する従前の経過及び土地の利用状況並びに借地権設定者が土地の明渡しの条件として又は土地の明渡しと引換えに借地権者に対して
> 財産上の給付をする旨の申出をした場合におけるその申出を考慮して、正当の事由があると認められる場合でなければ、述べることができない。

## 期待する Semantic IR

```
Rule R6
  subject:    Ref(D:借地権設定者)
  condition:  Not(Pred(正当の事由がある,
                factors = [使用を必要とする事情, 従前の経過, 土地の利用状況, 立退料の申出],
                = Unknown(Intentional, "正当の事由があると認められる")))
  effect:     Prohibition(Action(異議を述べる, ref = Ref(R5-1-proviso)))
  provenance: main/art:6/para:1/sent:1 (high, human)

Definition D:借地権者@art6                       ← 「以下この条において同じ」
  term:  "借地権者"
  scope: main/art:6
  body:  Or(Ref(D:借地権者), Ref(D:転借地権者))
```

## 論点

- **「〜でなければ、述べることができない」は Prohibition + 条件**（許可の否定ではなく、要件を満たさない異議の禁止）。
  `Not(...)` を使っているが、これは原文に「でなければ」という否定があるので [ADR-0004](../adr/0004-exception-not-negation.md) に反しない
- 正当事由は判断要素（考慮事項）が列挙されているだけで、判定基準は無い。**`Unknown(Intentional)` に考慮要素のリストを添える**。
  Z3 には `justified: Bool` の自由変数として渡す。Lean で「正当事由が無ければ法定更新される」のような性質は証明できる
- **条限りの定義**「以下この条において同じ」。Definition の `scope` が条になる例。第2条の定義（法令全体）を条内で上書きしている。
  Resolved IR の definition resolver は内側の scope を優先する
- 第5条ただし書き（異議）の**要件**をこの条が定めている。R5-1-proviso の condition と R6 の condition は結合されるべきだが、
  それは Resolved IR で行う。Semantic IR は「R6 は R5-1-proviso の異議を制約する」という参照だけ持つ
