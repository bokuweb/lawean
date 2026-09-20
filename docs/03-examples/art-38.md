# 第38条（定期建物賃貸借）— 特則・書面・説明義務・無効・面積要件

## 原文（第1・2・3・5・6・8項を抜粋）

> 1. 期間の定めがある建物の賃貸借をする場合においては、公正証書による等書面によって契約をするときに限り、第三十条の規定にかかわらず、
>    契約の更新がないこととする旨を定めることができる。この場合には、第二十九条第一項の規定を適用しない。
> 2. 前項の規定による建物の賃貸借の契約がその内容を記録した電磁的記録によってされたときは、その契約は、書面によってされたものとみなして、同項の規定を適用する。
> 3. 第一項の規定による建物の賃貸借をしようとするときは、建物の賃貸人は、あらかじめ、建物の賃借人に対し、〜その旨を記載した書面を交付して説明しなければならない。
> 5. 建物の賃貸人が第三項の規定による説明をしなかったときは、契約の更新がないこととする旨の定めは、無効とする。
> 6. 第一項の規定による建物の賃貸借において、期間が一年以上である場合には、建物の賃貸人は、期間の満了の一年前から六月前までの間
>    （以下この項において「通知期間」という。）に建物の賃借人に対し期間の満了により建物の賃貸借が終了する旨の通知をしなければ、
>    その終了を建物の賃借人に対抗することができない。ただし、建物の賃貸人が通知期間の経過後建物の賃借人に対しその旨の通知をした場合においては、
>    その通知の日から六月を経過した後は、この限りでない。
> 8. 前二項の規定に反する特約で建物の賃借人に不利なものは、無効とする。

## 期待する Semantic IR（抜粋）

```
Rule R38-1a
  condition:  And(Pred(期間の定めがある), Pred(書面によって契約する, form = 公正証書等))   ← 「ときに限り」
  effect:     Power(Action(特約を定める, content = Not(契約の更新)))
  overrides:  [R30]                                        ← 「第三十条の規定にかかわらず」（R30 = 第3章第1節の強行規定）
  provenance: main/art:38/para:1/sent:1

Rule R38-1b
  condition:  Pred(R38-1a の特約をした)                       ← 「この場合には」
  effect:     Exception(R29-1)                              ← 「第二十九条第一項の規定を適用しない」
  overrides:  [R29-1]
  provenance: main/art:38/para:1/sent:2

Rule R38-3
  condition:  Pred(R38-1a の賃貸借をしようとする)
  subject:    賃貸人
  effect:     Obligation(Action(書面を交付して説明する, to = 賃借人, timing = あらかじめ, content = 更新がなく満了で終了))
  provenance: main/art:38/para:3/sent:1

Rule R38-5
  condition:  Not(Pred(R38-3 の説明をした))
  effect:     Void(特約(更新がない))
  provenance: main/art:38/para:5/sent:1

Rule R38-6
  condition:  And(
                Pred(R38-1a の賃貸借),
                Cmp(期間, >=, Duration(1, Year)),
                Not(Pred(通知した, by = 賃貸人, within = Window(Event(満了) - 1Y, Event(満了) - 6M)))
              )
  effect:     Prohibition(Action(終了を対抗する, to = 賃借人))    ← 「対抗することができない」
  provenance: main/art:38/para:6/sent:1

Rule R38-6-proviso
  condition:  And(
                Pred(通知した, by = 賃貸人, after = Window.to),      ← 「通知期間の経過後」
                Cmp(Now, >, Event(その通知の日) + Duration(6, Month))
              )
  effect:     Exception(R38-6)
  overrides:  [R38-6]
  provenance: main/art:38/para:6/sent:2

Rule R38-8
  condition:  And(Pred(反する, target = [R38-6, R38-7]), Pred(不利, to = 賃借人))
  effect:     Void(特約)
  provenance: main/art:38/para:8/sent:1

Definition D:通知期間
  term:  "通知期間"
  scope: main/art:38/para:6                                ← 「以下この項において」
  body:  Window(Event(満了) - 1Y, Event(満了) - 6M)
```

## 論点

- **「ときに限り」は condition**（必要条件）。「かかわらず」は overrides。「適用しない」は Exception。1 項に 3 種類の関係が同居する
- **項限りの定義**「以下この項において「通知期間」という」。Definition の scope が項になる。しかも body が Temporal（Window）。
  Definition の body は Expr だけでなく Temporal 値も取れる必要がある
- R38-5 と R38-8 は `Void` の 2 パターン: **特定の特約が無効**（説明義務違反）と**メタ制約による無効**（第9条型）。
  effect の型は同じ `Void` でよいが、対象の指定方法が違う（具体的な特約 vs 「反する特約」という述語）
- 「対抗することができない」は Permission の否定ではなく **Prohibition（主張の禁止）**。ここでも「できない」= Not(Permission) にしない
- R38-6-proviso の「その通知の日から六月を経過した後」は **通知という事象からの経過期間**。民法第140条で初日不算入
- 第7項（200 ㎡未満・やむを得ない事情 → 解約申入れ → 1 月で終了）は面積の `Cmp` と `Unknown(Intentional, やむを得ない事情)` と
  `Power` と `Event + Duration` が全部入り。時間があれば別途書く
