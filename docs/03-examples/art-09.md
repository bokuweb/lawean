# 第9条（強行規定）— Rule ではなくメタ制約

## 原文

> この節の規定に反する特約で借地権者に不利なものは、無効とする。

## 期待する Semantic IR

```
Rule R9
  subject:    Contract(特約)
  condition:  And(
                Pred(反する, target = Scope(main/chap:2/sec:1)),   ← 「この節の規定」= 第2章第1節（第3〜9条）
                Pred(不利, to = Ref(D:借地権者))
              )
  effect:     Void(Contract(特約))
  provenance: main/art:9/para:1/sent:1 (high, human)
```

## 論点

- 形の上では普通の Rule（条件 → 無効という効果）として書ける。**専用ノードは要らない**
- ただし condition の「この節の規定に反する」は、**他の Rule 群への参照を条件に含む**。`Scope(...)` は構造パス（節）を指し、
  Resolved IR でその節に属する Rule 集合に展開される
- 「借地権者に不利」は評価不能 → 実際には `Unknown(intentional)` に近い。しかし「30 年未満の期間を定める特約」のような
  **典型例は判定できる**（第3条の 30 年より短い = 不利）。v0.1 では条件を Unknown にし、Z3 向けには
  「第3条の期間 < 30 年の特約は無効」のような**手書きの補題**として与える
- `Void` は Effect の一種。03-ir-design の `Effect` 列挙には無かった → 04 で追加
- 第16条・第21条・第30条・第37条も同じ形。**パターンとして抽出できる**なら semantic parser のルールにする
- 第22条「第九条及び第十六条の規定にかかわらず」は、この R9 に対する Exception。強行規定に対する例外なので、
  Exception のグラフが `R22 → overrides → R9 → constrains → {R3..R8}` のように 2 段になる
