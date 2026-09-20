# 第26条（建物賃貸借契約の更新等）— 期間窓と法定更新

## 原文

> 1. 建物の賃貸借について期間の定めがある場合において、当事者が期間の満了の一年前から六月前までの間に相手方に対して
>    更新をしない旨の通知又は条件を変更しなければ更新をしない旨の通知をしなかったときは、従前の契約と同一の条件で契約を更新したものとみなす。
>    ただし、その期間は、定めがないものとする。
> 2. 前項の通知をした場合であっても、建物の賃貸借の期間が満了した後建物の賃借人が使用を継続する場合において、
>    建物の賃貸人が遅滞なく異議を述べなかったときも、同項と同様とする。
> 3. 建物の転貸借がされている場合においては、建物の転借人がする建物の使用の継続を建物の賃借人がする建物の使用の継続とみなして、
>    建物の賃借人と賃貸人との間について前項の規定を適用する。

## 期待する Semantic IR

```
Rule R26-1
  subject:    Parties(建物賃貸借)
  condition:  And(
                Pred(期間の定めがある),
                Not(Pred(通知した,
                         kind = Or(更新しない, 条件変更なければ更新しない),
                         within = Window(
                           from = Event(期間の満了) - Duration(1, Year),
                           to   = Event(期間の満了) - Duration(6, Month)
                         )))
              )
  effect:     Deem(Fact(契約を更新した, conditions = 従前と同一))
  provenance: main/art:26/para:1/sent:1 (high, human)

Rule R26-1-proviso
  condition:  Pred(R26-1 により更新された)
  effect:     Set(期間, Indefinite)                          ← 「定めがないものとする」
  overrides:  []                                            ← 例外ではない（下の考察）
  provenance: main/art:26/para:1/sent:2 (high, human)

Rule R26-2
  condition:  And(
                Pred(通知した),                               ← 「前項の通知をした場合であっても」
                Pred(期間が満了した),
                Pred(使用を継続する, by = 賃借人),
                Not(Pred(異議を述べた, by = 賃貸人, timing = Unknown(Intentional, "遅滞なく")))
              )
  effect:     SameAs(R26-1)
  provenance: main/art:26/para:2/sent:1 (high, human)

Rule R26-3
  condition:  Pred(転貸借がされている)
  effect:     Deem(Fact(使用継続, by = 転借人) AS Fact(使用継続, by = 賃借人)) THEN Apply(R26-2)
  provenance: main/art:26/para:3/sent:1 (high, human)
```

## 論点

- **時間窓 `Window(from, to)`** が Temporal に要る。起算点は「期間の満了」という**将来の事象**で、そこから遡る。
  `Event(満了) - 1年` から `Event(満了) - 6月` まで。民法第140条（初日不算入）が「前」の計算にどう効くかは別途（[minpo-140-143.md](minpo-140-143.md)）
- **この条のただし書きは例外ではなく部分的な上書き**。「同一の条件で更新」のうち期間だけ「定めなし」にする。
  `overrides` で表せるが、上書きの粒度が「Rule 全体」ではなく「effect の一部（期間）」になる。
  → `overrides` は Rule 単位のままにし、部分上書きは Resolved IR で `Set(期間)` が `Deem(同一条件)` より優先されることで表す。
  **`overrides: [R26-1]` と書いてはいけない**: 07 の意味論では applies(R26-1) = cond ∧ ¬applies(proviso) かつ applies(proviso) = applies(R26-1) となり、
  cond が真の世界が存在しなくなる（第26条第1項が成り立つ模型が無い）。手書き IR の当初版はそう書いており、Lean の層化（[10](../10-lean-semantics.md)、上書き・参照の循環）が検出した。
  Z3 は第26条を触る性質が無かったので気づかなかった
- 第5条と構造が並行（法定更新 → 使用継続 → 転貸借の読み替え）。**パターン化できる**。semantic parser のルール候補
- 「期間の定めがない」は借地借家法で特別な意味を持つ状態（第27条 解約申入れ → 6 月で終了）。`Indefinite` を Temporal の値にする
