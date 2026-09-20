# 第22条（定期借地権）— 複数 Rule への特則、書面要件、電磁的記録のみなし

## 原文

> 1. 存続期間を五十年以上として借地権を設定する場合においては、第九条及び第十六条の規定にかかわらず、契約の更新
>    （更新の請求及び土地の使用の継続によるものを含む。次条第一項において同じ。）及び建物の築造による存続期間の延長がなく、
>    並びに第十三条の規定による買取りの請求をしないこととする旨を定めることができる。
>    この場合においては、その特約は、公正証書による等書面によってしなければならない。
> 2. 前項前段の特約がその内容を記録した電磁的記録（〜をいう。第三十八条第二項及び第三十九条第三項において同じ。）によってされたときは、
>    その特約は、書面によってされたものとみなして、前項後段の規定を適用する。

XML: 第1項は `Sentence Num=1`（前段）と `Num=2`（後段）、`Function` 属性なし。第2項が「前項前段」「前項後段」で参照する。

## 期待する Semantic IR

```
Rule R22-1a                                              ← 前段
  subject:    Parties(借地契約)
  condition:  Cmp(存続期間, >=, Duration(50, Year))
  effect:     Power(Action(特約を定める, content = [
                Not(契約の更新),                           ← 「更新の請求及び使用の継続によるものを含む」= R5 全体
                Not(建物の築造による存続期間の延長),        ← 第7条
                Not(Ref(R13) の買取請求)
              ]))
  overrides:  [R9, R16]                                    ← 「第九条及び第十六条の規定にかかわらず」
  provenance: main/art:22/para:1/sent:1 (high, human)

Rule R22-1b                                              ← 後段
  subject:    Parties(借地契約)
  condition:  Pred(R22-1a の特約をする)                     ← 「この場合においては」
  effect:     Obligation(Action(書面による, form = 公正証書等))
  provenance: main/art:22/para:1/sent:2 (high, human)

Rule R22-2
  condition:  Pred(R22-1a の特約が電磁的記録によってされた)
  effect:     Deem(Fact(書面によってされた)) THEN Apply(R22-1b)
  provenance: main/art:22/para:2/sent:1 (high, human)

Definition D:電磁的記録
  term:  "電磁的記録"
  scope: [main/art:22/para:2, main/art:38/para:2, main/art:39/para:3]   ← 「第三十八条第二項及び第三十九条第三項において同じ」
  body:  Unknown(Unparsed)
```

## 論点

- **`overrides` が強行規定（R9）を指す。** R9 は「第2章第1節に反する特約は無効」なので、R22-1a は R9 → {R3..R8} という
  メタ制約に穴を開けている。Exception グラフが 2 段になる（[art-09.md](art-09.md)）。Resolved IR での展開順序を決める必要がある
- **前段 / 後段は `Function` 属性で区別されない。** `Sentence Num` の順序だけ。「前項前段」の参照を解決するには
  `Relative { para: prev, sentence: 1 }` のような細かい RelativeRef が要る（[04-semantic-ir.md](../04-semantic-ir.md) の RefTarget）
- 「この場合においては」は前段の効果を条件にする定型。**前段の Rule への参照を condition に持つ**
- 第2項の「みなして〜適用する」は第5条第3項と同じ **読み替え + 適用**。ここは「電磁的記録 = 書面」の読み替えで、
  第38条第2項も全く同じ構造 → `Deem(X AS Y) THEN Apply(R)` を合成 Effect として持つ価値がある（Unknown に逃がさない）
- **Definition の scope が飛び飛び**（22条2項・38条2項・39条3項）。`scope: Vec<StableId>` にする
- `Power(特約を定める)` の content に「更新しない」「延長しない」を Not で書いたが、これは原文に「〜がなく」「〜しないこととする」があるので
  ADR-0004 に反しない。実質的には R5 / R7 / R13 の適用除外を**契約で選べる**という Power
