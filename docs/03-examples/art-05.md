# 第5条（借地契約の更新請求等）— みなす

## 原文

> 1. 借地権の存続期間が満了する場合において、借地権者が契約の更新を請求したときは、建物がある場合に限り、前条の規定によるもののほか、
>    従前の契約と同一の条件で契約を更新したものとみなす。ただし、借地権設定者が遅滞なく異議を述べたときは、この限りでない。
> 2. 借地権の存続期間が満了した後、借地権者が土地の使用を継続するときも、建物がある場合に限り、前項と同様とする。
> 3. 転借地権が設定されている場合においては、転借地権者がする土地の使用の継続を借地権者がする土地の使用の継続とみなして、
>    借地権者と借地権設定者との間について前項の規定を適用する。

## 期待する Semantic IR

```
Rule R5-1
  subject:    Ref(D:借地権者)
  condition:  And(
                Pred(存続期間が満了する),
                Pred(更新を請求した, by = Ref(D:借地権者)),
                Pred(建物がある)
              )
  effect:     Deem(Fact(契約を更新した, conditions = 従前と同一), 
                   except_period = Ref(R4-*))            ← 「前条の規定によるもののほか」
  provenance: main/art:5/para:1/sent:1 (high, human)

Rule R5-1-proviso
  condition:  Pred(異議を述べた, by = Ref(D:借地権設定者), timing = Unknown(intentional, "遅滞なく"))
  effect:     Exception(R5-1)                              ← 「この限りでない」
  provenance: main/art:5/para:1/sent:2 (high, human)

Rule R5-2
  condition:  And(
                Pred(存続期間が満了した),
                Pred(土地の使用を継続する, by = Ref(D:借地権者)),
                Pred(建物がある)
              )
  effect:     SameAs(R5-1)                                 ← 「前項と同様とする」
  exceptions: [R5-1-proviso]                               ← 「同様」なので ただし書きも引き継ぐ（要確認）
  provenance: main/art:5/para:2/sent:1 (medium, human)

Rule R5-3
  condition:  Pred(転借地権が設定されている)
  effect:     Deem(Fact(使用継続, by = Ref(D:転借地権者)) AS Fact(使用継続, by = Ref(D:借地権者)))
              THEN Apply(R5-2, between = (Ref(D:借地権者), Ref(D:借地権設定者)))
  provenance: main/art:5/para:3/sent:1 (medium, human)
```

## 論点

- **`Deem`（みなす）は反証を許さない**。[ADR-0002](../adr/0002-deem-vs-presume.md)。ここでは「更新した」という法的事実が発生する
- 「前条の規定によるもののほか」: 更新後の期間は第4条で決まる、という参照。`Deem` の中に `except_period` を入れたが、
  むしろ「R5-1 は更新の事実だけ発生させ、期間は R4 が独立に決める」と読むほうが素直。**Rule 間の合成は Resolved IR の仕事**で、
  Semantic IR は参照を張るだけにする
- **「遅滞なく」は `Unknown(intentional)`**。意図的に開放的な時間条件。[ADR-0001](../adr/0001-unknown-not-opaque.md)
- **「前項と同様とする」を `SameAs(R5-1)` で表す。** effect をコピーするのではなく参照にする。ただし書きが引き継がれるかは
  解釈問題 → confidence を medium にして、複数解釈の余地を残す（[ADR-0005](../adr/0005-no-single-interpretation.md)）
- 第3項は「A を B とみなして、前項の規定を適用する」= **事実の読み替え + Rule の適用**。`Deem(X AS Y) THEN Apply(R)` という合成 Effect が要る。
  v0.1 では `Unknown(unparsed)` で止めて、Provenance と参照（R5-2）だけ残す選択肢もある
