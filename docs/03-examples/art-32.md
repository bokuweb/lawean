# 第32条（借賃増減請求権）— 金額・利率・「不相当」の Unknown・「契約の条件にかかわらず」

## 原文

> 1. 建物の借賃が、土地若しくは建物に対する租税その他の負担の増減により、土地若しくは建物の価格の上昇若しくは低下その他の経済事情の変動により、
>    又は近傍同種の建物の借賃に比較して不相当となったときは、契約の条件にかかわらず、当事者は、将来に向かって建物の借賃の額の増減を請求することができる。
>    ただし、一定の期間建物の借賃を増額しない旨の特約がある場合には、その定めに従う。
> 2. 建物の借賃の増額について当事者間に協議が調わないときは、その請求を受けた者は、増額を正当とする裁判が確定するまでは、
>    相当と認める額の建物の借賃を支払うことをもって足りる。ただし、その裁判が確定した場合において、既に支払った額に不足があるときは、
>    その不足額に年一割の割合による支払期後の利息を付してこれを支払わなければならない。
> 3. （減額について第2項と対称）

## 期待する Semantic IR

```
Rule R32-1
  subject:    Parties(建物賃貸借)
  condition:  Pred(不相当となった, of = 借賃,
                causes = Or(租税等の負担の増減, 価格の変動その他経済事情の変動, 近傍同種の借賃との比較),
                = Unknown(Intentional, "不相当"))
  effect:     Power(Action(請求する, content = 借賃の増減, direction = Future))     ← 「将来に向かって」
  overrides:  [Contract(条件)]                                  ← 「契約の条件にかかわらず」= 特約より優先
  provenance: main/art:32/para:1/sent:1 (high, human)

Rule R32-1-proviso
  condition:  Pred(特約がある, content = 増額しない, period = Var(一定の期間))
  effect:     Apply(Contract(特約))                              ← 「その定めに従う」
  overrides:  [R32-1]                                           ← 増額側だけ。減額請求は特約があっても可（判例）→ Interpretation
  provenance: main/art:32/para:1/sent:2 (high, human)

Rule R32-2
  subject:    請求を受けた者
  condition:  And(Pred(増額請求があった), Not(Pred(協議が調った)), Not(Pred(裁判が確定した, content = 増額を正当とする)))
  effect:     Permission(Action(支払う, amount = Unknown(Intentional, "相当と認める額")))   ← 「もって足りる」= 債務不履行にならない
  provenance: main/art:32/para:2/sent:1 (high, human)

Rule R32-2-proviso
  subject:    請求を受けた者
  condition:  And(Pred(裁判が確定した), Cmp(Var(既払額), <, Var(確定額)))
  effect:     Obligation(Action(支払う,
                amount = Var(確定額) - Var(既払額)
                       + Interest(principal = 不足額, rate = 10%/year, from = Event(各支払期))))
  provenance: main/art:32/para:2/sent:2 (high, human)

Rule R32-3, R32-3-proviso: 第2項と対称（減額 / 超過額 / 受領の時から）
```

## 論点

- **「契約の条件にかかわらず」の overrides 先が Rule ではなく契約**。`overrides: [Contract(...)]` は法令 Rule 間の関係とは別種。
  「強行規定」と同じく、法令 vs 契約の優先関係を表す。04 に `overrides_contract: bool` のような別フィールドを置く案
- **ただし書きが増額だけを縛る**ことは文理（「増額しない旨の特約」）から読めるが、「減額しない特約は無効（減額請求は常に可）」は判例。
  → R32-1-proviso の `overrides` は R32-1 の**増額側だけ**。Rule を増額と減額の 2 つに分けるべきか（第4条の括弧書きと同じ論点）
- **金額の算術**: `Var(確定額) - Var(既払額)`、`Interest(rate = 年1割)`。`Expr` に算術（`Add / Sub / Mul`）と `Interest` が要る。
  Z3 の本領（数量・日付）が発揮される箇所。「支払期後」の起算は各支払期ごとなので、Interest の from が**複数の Event** になる
- **「もって足りる」は Permission**（相当額を払えば債務不履行にならない）。「できる」ではないが Permission と同じ効果。
  語彙表（05-glossary）に追加
- 「不相当」「相当と認める額」は `Unknown(Intentional)`。ただし考慮要素（租税・価格・近傍相場）は列挙されている → 第6条と同じ形
