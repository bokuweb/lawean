# 第2条（定義）

## 原文

> この法律において、次の各号に掲げる用語の意義は、当該各号に定めるところによる。
> 一　借地権　建物の所有を目的とする地上権又は土地の賃借権をいう。
> 二　借地権者　借地権を有する者をいう。
> 三　借地権設定者　借地権者に対して借地権を設定している者をいう。
> 四　転借地権　建物の所有を目的とする土地の賃借権で借地権者が設定しているものをいう。
> 五　転借地権者　転借地権を有する者をいう。

XML: `Item / ItemSentence / Column[1] = 用語, Column[2] = 定義`（[02-source-ir.md](../02-source-ir.md)）

## 期待する Semantic IR

```
Definition D:借地権
  term:       "借地権"
  scope:      Law(403AC0000000090)              ← 「この法律において」
  body:       Or(
                Pred(地上権, purpose = 建物の所有),
                Pred(土地の賃借権, purpose = 建物の所有)
              )
  provenance: main/art:2/para:1/item:1 (high, human)

Definition D:借地権者
  term:  "借地権者"
  scope: Law(403AC0000000090)
  body:  Entity(Person, has = Ref(D:借地権))

Definition D:借地権設定者
  body:  Entity(Person, sets(Ref(D:借地権), against = Ref(D:借地権者)))

Definition D:転借地権
  body:  And(Pred(土地の賃借権, purpose = 建物の所有), set_by = Ref(D:借地権者))

Definition D:転借地権者
  body:  Entity(Person, has = Ref(D:転借地権))
```

## 論点

- **定義の本体をどこまで構造化するか。** 「建物の所有を目的とする地上権又は土地の賃借権」を `Or(Pred, Pred)` にするか、
  `Unknown(text)` で止めるか。v0.1 では **term と scope と provenance だけ確定**し、body は Unknown でよい。
  Definition の価値の大半は「この語はこの法律で定義語である」と分かることにある（参照解決に効く）
- 地上権・賃借権は民法で定義される概念。外部法令への参照 `Ref(Law(民法), art:265)` を張るのは Resolved IR 以降
- `scope` は「この法律において」= 法令全体。「この節において」「この条において」もあるので scope は構造パスを取る
- 第2条は Rule を生まない。**Definition は Rule と別のトップレベルノード**（03-ir-design §12 の通り）
