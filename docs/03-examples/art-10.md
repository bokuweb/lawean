# 第10条（借地権の対抗力）— Power（対抗）、第三者、期間付きの例外の例外

## 原文

> 1. 借地権は、その登記がなくても、土地の上に借地権者が登記されている建物を所有するときは、これをもって第三者に対抗することができる。
> 2. 前項の場合において、建物の滅失があっても、借地権者が、その建物を特定するために必要な事項、その滅失があった日及び建物を新たに築造する旨を
>    土地の上の見やすい場所に掲示するときは、借地権は、なお同項の効力を有する。ただし、建物の滅失があった日から二年を経過した後にあっては、
>    その前に建物を新たに築造し、かつ、その建物につき登記した場合に限る。

## 期待する Semantic IR

```
Rule R10-1
  subject:    Ref(D:借地権者)
  condition:  And(
                Not(Pred(登記がある, of = 借地権)),                ← 「登記がなくても」は譲歩。条件ではない → 論点
                Pred(所有する, by = 借地権者, object = 建物(登記済み, on = 土地))
              )
  effect:     Power(Action(対抗する, to = Entity(第三者), with = 借地権))
  provenance: main/art:10/para:1/sent:1 (high, human)

Rule R10-2
  condition:  And(
                Pred(R10-1 の場合),
                Pred(建物が滅失した),
                Pred(掲示する, by = 借地権者, where = 土地の上の見やすい場所,
                     content = [建物を特定する事項, 滅失の日, 新たに築造する旨])
              )
  effect:     Preserve(Ref(R10-1).effect)                      ← 「なお同項の効力を有する」
  provenance: main/art:10/para:2/sent:1 (high, human)

Rule R10-2-proviso
  condition:  And(
                TimeCond::Elapsed(Period { from: Event(滅失の日), length: 2Y }),
                Not(And(Pred(築造した, before = 2年経過), Pred(登記した, of = その建物)))
              )
  effect:     Exception(R10-2)
  overrides:  [R10-2]
  provenance: main/art:10/para:2/sent:2 (high, human)
```

## 論点

- **「登記がなくても」は条件ではなく譲歩**。R10-1 の condition に `Not(登記がある)` を入れると「登記があるときは対抗できない」と読めてしまう。
  正しくは「登記の有無にかかわらず、建物登記があれば対抗できる」。→ condition から外し、注記（`note`）として Provenance に残す。
  **譲歩表現（〜ても、〜にかかわらず）を condition に入れない**というルールが semantic parser に要る
- `Power(対抗する)` は Hohfeld の power ではなく immunity に近い（第三者の権利主張を退ける力）。v0.1 では Power で妥協し、
  [ADR-0003](../adr/0003-permission-vs-power.md) の「結果」に「対抗力は Power の一種として扱う」を追記する候補
- 「第三者」は定義されていない Entity。民法の一般概念。`Entity(第三者)` を自由語彙で持つ
- ただし書きの「〜場合に限る」は **Exception の条件を否定形で書いた**もの。「2 年経過後は、築造かつ登記済みの場合に限り R10-2 が効く」=
  「2 年経過後で築造・登記が無ければ R10-2 は効かない」。上では後者で書いた。`Not(And(...))` は原文の「限る」の裏返しなので
  ADR-0004 違反ではないが、読みにくい。**「〜に限る」を `OnlyIf(cond)` という Effect の修飾として持つ**案を 04 の未決事項に追加
- 「滅失があった日から二年」は Forward の Period。民法140条で初日不算入
