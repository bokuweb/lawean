# 第3条（借地権の存続期間）

## 原文

> 借地権の存続期間は、三十年とする。ただし、契約でこれより長い期間を定めたときは、その期間とする。

XML: `art:3/para:1/sent:1 (Function=main)`, `art:3/para:1/sent:2 (Function=proviso)`

## 期待する Semantic IR

案 A: 本文 Rule + ただし書き Rule（Exception として関連づけ）

```
Rule R3-1
  subject:    Ref(D:借地権)
  condition:  True
  effect:     Set(存続期間, Duration(30, Year))
  provenance: main/art:3/para:1/sent:1 (high, human)

Rule R3-2
  subject:    Ref(D:借地権)
  condition:  Pred(契約で定めた期間 > Duration(30, Year))
  effect:     Set(存続期間, Var(契約で定めた期間))
  overrides:  [R3-1]                                     ← ただし書き = R3-1 の Exception
  provenance: main/art:3/para:1/sent:2 (high, human)
```

案 B: 1 つの Rule に default と override を畳み込む

```
Rule R3
  effect: Set(存続期間, Max(Duration(30, Year), Var(契約で定めた期間)))
```

## 論点

- **案 A を採る。** 案 B は意味的には正しいが、原文の「本文 / ただし書き」構造が消える。
  [ADR-0004](../adr/0004-exception-not-negation.md)（例外を潰さない）の最初の適用例。
  案 B のような正規化は Verification IR（Z3 向け）でやればよい
- `Set` という Effect が要る。義務・禁止・許可のどれでもなく、**法律関係の内容を定める**効果。
  03-ir-design の `Effect` 列挙には無かった → 04 で追加する（`Effect::Set` または `Effect::Determine`）
- `Duration(30, Year)` の計算規則は民法第143条（暦による計算）。Temporal モデルはこれを前提にする
- 「契約で定めた期間」は事実（Fact）であり、IR は変数として持つ。Z3 では `contract_period: Int` になる
- 第9条（強行規定）により「三十年より短い特約は無効」が導かれるが、それは第3条の Rule には書かない。
  第9条側のメタ制約と第3条の Rule から Resolved IR で導出する（[art-09.md](art-09.md)）
