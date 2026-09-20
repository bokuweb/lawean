# 第4条（借地権の更新後の期間）

## 原文

> 当事者が借地契約を更新する場合においては、その期間は、更新の日から十年（借地権の設定後の最初の更新にあっては、二十年）とする。
> ただし、当事者がこれより長い期間を定めたときは、その期間とする。

## 期待する Semantic IR

```
Rule R4-1
  subject:    Ref(D:借地権)
  condition:  And(Pred(更新する), Not(Pred(最初の更新)))
  effect:     Set(存続期間, Duration(10, Year, from = Event(更新の日)))
  provenance: main/art:4/para:1/sent:1 (high, human)

Rule R4-1'
  subject:    Ref(D:借地権)
  condition:  And(Pred(更新する), Pred(最初の更新))
  effect:     Set(存続期間, Duration(20, Year, from = Event(更新の日)))
  provenance: main/art:4/para:1/sent:1 (high, human)     ← 同じ文から 2 Rule

Rule R4-2
  condition:  Pred(当事者が定めた期間 > <R4-1 or R4-1' が定める期間>)
  effect:     Set(存続期間, Var(当事者が定めた期間))
  overrides:  [R4-1, R4-1']
  provenance: main/art:4/para:1/sent:2 (high, human)
```

## 論点

- **括弧書きによる場合分け**「（〜にあっては、二十年）」。1 文から 2 Rule に分けるか、`If(最初の更新, 20年, 10年)` という式にするか。
  分けると Rule 数が増えるが、それぞれに Provenance と例外を付けやすい。**分ける**方向で進め、実例が増えたら再検討
- `Not(Pred(最初の更新))` と書いたが、これは原文に無い条件を補っている（本文は「更新する場合」全般で、括弧書きが最初の更新を上書きする）。
  忠実に書くなら R4-1 の condition は `Pred(更新する)` で、R4-1' が R4-1 を `overrides` する形。**こちらの方が原文に忠実**。
  → 括弧書きも「ただし書き」と同じ Exception 構造として扱えるか、が論点
- `from = Event(更新の日)`: 期間には**起算点**が要る。Temporal 型は `Duration` 単体でなく `(起算点, 長さ)` を持つ
- R4-2 の「これより長い」の「これ」は R4-1 / R4-1' が定める値への参照。**Rule の effect の値を参照する式**が必要
