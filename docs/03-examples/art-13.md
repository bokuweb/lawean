# 第13条（建物買取請求権）— 形成権（Power）、時価という金額の Unknown、準用

## 原文

> 1. 借地権の存続期間が満了した場合において、契約の更新がないときは、借地権者は、借地権設定者に対し、建物その他借地権者が権原により
>    土地に附属させた物を時価で買い取るべきことを請求することができる。
> 2. 前項の場合において、建物が借地権の存続期間が満了する前に借地権設定者の承諾を得ないで残存期間を超えて存続すべきものとして
>    新たに築造されたものであるときは、裁判所は、借地権設定者の請求により、代金の全部又は一部の支払につき相当の期限を許与することができる。
> 3. 前二項の規定は、借地権の存続期間が満了した場合における転借地権者と借地権設定者との間について準用する。

## 期待する Semantic IR

```
Rule R13-1
  subject:    Ref(D:借地権者)
  condition:  And(Pred(存続期間が満了した), Not(Pred(契約の更新がある)))
  effect:     Power(Action(請求する, to = Ref(D:借地権設定者),
                content = 買取(object = [建物, 権原により附属させた物], price = Unknown(Intentional, "時価"))),
              exercise_effect = Deem(Fact(売買契約が成立した, price = 時価)))   ← 判例。原文には無い → Interpretation
  interpretations: [
    { authority: 判例(最判昭和35年…), expression: "請求により売買契約が成立する（形成権）", confidence: high }
  ]
  provenance: main/art:13/para:1/sent:1 (high, human)

Rule R13-2
  subject:    Entity(裁判所)
  condition:  And(
                Pred(R13-1 の場合),
                Pred(築造された, object = 建物, when = Before(Event(満了)),
                     without = 借地権設定者の承諾, as = 残存期間を超えて存続すべきもの),
                Pred(請求した, by = 借地権設定者)
              )
  effect:     Power(Action(期限を許与する, target = 代金の支払(全部 | 一部), length = Unknown(Intentional, "相当の期限")))
  provenance: main/art:13/para:2/sent:1 (high, human)

Rule R13-3
  condition:  Pred(存続期間が満了した)
  effect:     ApplyMutatis(rules = [R13-1, R13-2],
                substitute = { 借地権者 → 転借地権者 },
                between = (転借地権者, 借地権設定者))
  provenance: main/art:13/para:3/sent:1 (medium, human)
```

## 論点

- **Power の「行使したときの効果」が原文に書かれていない。** 建物買取請求権が形成権（請求だけで売買成立）であることは判例・通説。
  IR には `exercise_effect` を持たせつつ、それが文理ではなく判例由来であることを `interpretations` で示す。
  [ADR-0005](../adr/0005-no-single-interpretation.md) の最初の実例。**Provenance の `Author::Precedent`** が要る
- **「時価」は金額の `Unknown(Intentional)`**。Z3 には `price: Real, price > 0` の自由変数として渡す。
  「代金の全部又は一部」も同様に `paid: Real, 0 <= paid <= price`
- **裁判所が主体の Rule**。第4章の裁判手続と同じで、v0.1 では主体が裁判所の Rule は `Unknown(External)` に逃がす選択肢もある。
  ここは effect が明確（期限の許与）なので Rule として書けるが、`Entity(裁判所)` を法令上の当事者と同列に扱うかは要検討
- **準用 = `ApplyMutatis`**（読み替えて適用）。「〜について準用する」は主体の置換を伴う。第5条第3項の `DeemAndApply`（事実の読み替え）とは
  置換対象が違う（主体 vs 事実）。同じ型にできるか → 04 の未決事項へ。confidence は medium（置換規則を人が補っている）
- 第2項の条件は 4 つの述語が絡んでいて、自然文からの機械抽出は当面無理。`Unknown(Unparsed)` にして Provenance だけ残すのが v0.1 の現実解
