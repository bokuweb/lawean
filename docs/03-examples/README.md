# 03. Examples — 条文と期待する Semantic IR

Semantic IR の**テストコーパス**。対象法（借地借家法）から条文を抜き、「この条文はこういう IR になってほしい」を手書きする。
[04-semantic-ir.md](../04-semantic-ir.md) の型はここから逆算して決める。**型が先ではない。**

## 書き方

- 1 ファイル 1 条（または 1 項）。ファイル名は `art-03.md` のように条番号
- 各ファイルに、原文（`fixtures/` の XML から）・Source IR の stable_id・期待する Semantic IR・論点・Unknown にした箇所を書く
- IR の記法は仮。Rust 風でも YAML 風でもよい。**型を決めるためのスケッチ**なので、書きながら記法が変わってよい
- 判断に迷った箇所は「論点」に残す。ADR にすべき判断が見つかったら `docs/adr/` に切り出す

## 選んだ条文と、それで検証したいこと

| 条 | 検証したいこと | 状態 |
|---|---|---|
| [第2条](art-02.md) | 定義規定。`Column` ペアからの Definition 抽出。定義語の参照 | ✅ |
| [第3条](art-03.md) | 最小の Rule。期間（30年）。ただし書きによる上書き（Exception か、それとも default 値か） | ✅ |
| [第4条](art-04.md) | 場合分けを含む期間（10年 / 最初の更新は 20年）。「更新の日から」という起算点 | ✅ |
| [第5条](art-05.md) | **みなす**（法定更新）。「前条の規定によるもののほか」の参照。「遅滞なく」の Unknown。第2項「前項と同様とする」 | ✅ |
| [第9条](art-09.md) | **強行規定**。「この節の規定に反する特約で〜に不利なものは無効」。Rule ではなくメタ制約 | ✅ |
| 第6条 | 正当事由。意図的に開放的な条件 → `Unknown(intentional)` | ⬜ |
| 第10条 | 対抗力。「登記がなくても」の権限（Power）。第三者 | ⬜ |
| 第13条 | 建物買取請求権。Power と、時価という金額の Unknown | ⬜ |
| 第22条 | 定期借地権。「第九条及び第十六条の規定にかかわらず」複数 Rule への Exception。「五十年以上」。書面要件。第2項の「みなして〜適用する」 | ⬜ |
| 第26条 | 建物賃貸借の法定更新。期間満了の 1 年前から 6 か月前まで。通知 | ⬜ |
| 第32条 | 借賃増減請求権。「不相当となったとき」の Unknown。「一定の期間」の特約 | ⬜ |
| 第38条 | 定期建物賃貸借。書面・電磁的記録。第3項の説明義務 → 無効 | ⬜ |
| 附則第4条 | 経過措置の原則。「この法律の施行前に生じた事項にも適用する」ただし書き付き。時間 | ⬜ |
| 附則第6条 | 「なお従前の例による」。廃止された旧法への参照 | ⬜ |
| 民法第140条・第143条 | 期間計算（初日不算入、暦による計算）。Temporal モデルの前提 | ⬜ |

## 記法（仮）

```
Rule <id>
  subject:    <Entity>
  condition:  <Expr>
  effect:     <Effect>
  exceptions: [<RuleId>…]         ← 他 Rule から「これの例外」と宣言されたもの。逆向きリンクは resolver が張る
  temporal:   <Temporal>
  provenance: <stable_id> (confidence: high|medium|low, by: human|llm)
```

`Expr` は普通の論理 AST（`And / Or / Not / Pred / Ref / Unknown`）。`Effect` は `Obligation / Prohibition / Permission / Power /
Deem / Presume / Set / Void / Exception / …` の直和。詳細は 04 で決める。
