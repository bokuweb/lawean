# 06. 層 2 — LLM による要件・効果の構造化

状態: 設計 + 実装（`crates/lawean-llm`）。オフラインテストは記録済みレスポンスで通る。**実 API での実行は未**（要 `ANTHROPIC_API_KEY`）。
方針は [ADR-0008](adr/0008-extraction-strategy.md)。

## 何をさせるか、させないか

層 1（`lawean-extract`）が出した骨組み Rule は、効果の種別・参照・overrides・条件節の**範囲**まで決まっていて、
条件節の**中身**が `Unknown(Unparsed, "借地権者が契約の更新を請求した")` のまま。層 2 はこの中身を述語と引数に分解し、
効果の引数（主体・行為・対象・値）を埋める。

| させる | させない |
|---|---|
| 条件節 → 述語の And/Or（否定は原文に否定がある場合のみ） | 効果の種別の変更（層 1 が正。違ったら棄却） |
| 主体・相手方・対象の同定（定義語なら定義 ID で） | 参照の解決（層 1 が正。層 1 に無い参照を出したら棄却） |
| 期間・金額・数量の値の抽出（`Duration` / `Money` / `Var`） | 評価概念の判定（「正当の事由」は `Unknown(Intentional)` として返させる） |
| 1 文を複数 Rule に割る（括弧書きの場合分け、増額/減額の対称） | 解釈の選択（判例・学説は人が `Interpretation` に書く） |
| `Unknown` の種類の判定（Intentional / Unparsed / External） | confidence を High にすること（出力は常に Medium 以下。人が上げる） |

## 入出力

**単位は項。** 1 リクエスト = 1 項（本文 + ただし書き + 号）。「この場合において」「前項と同様」のような文間の依存は項の中で閉じることが多いため。

入力（user message）:

```
## 位置
403AC0000000090/main/chap:2/sec:1/art:5/para:1  借地借家法 第五条（借地契約の更新請求等）第1項

## 文
[sent:1] (Main, Deem) 借地権の存続期間が満了する場合において、借地権者が契約の更新を請求したときは、建物がある場合に限り、前条の規定によるもののほか、従前の契約と同一の条件で契約を更新したものとみなす。
  条件節: 借地権の存続期間が満了する / 借地権者が契約の更新を請求した / 建物がある
  参照: 前条 → main/chap:2/sec:1/art:4
[sent:2] (Proviso, Exception) ただし、借地権設定者が遅滞なく異議を述べたときは、この限りでない。
  条件節: 借地権設定者が遅滞なく異議を述べた
  上書き: sent:1

## この位置で有効な定義語
D:借地権 借地権 / D:借地権者 借地権者 / D:借地権設定者 借地権設定者 / D:転借地権 転借地権 / D:転借地権者 転借地権者
```

出力は `output_config.format`（JSON Schema）で強制する。**再帰スキーマは使えない**ので、条件式は
「述語の選言の連言」（`all_of: [ { any_of: [Pred…] } ]`）の 2 段に固定する。v0.1 の対象（人物・日付・期間・金額・条件・義務・禁止・許可・例外・参照）はこれで足りる。

```jsonc
{
  "sentences": [
    {
      "sentence": "sent:1",
      "rules": [
        {
          "suffix": "",                       // 1 文から複数 Rule を出すとき "a" "b" …
          "subject": { "kind": "definition", "id": "D:借地権者" },
          "condition": { "all_of": [
            { "any_of": [ { "name": "存続期間が満了する", "args": [], "negated": false } ] },
            { "any_of": [ { "name": "更新を請求した", "args": [ { "key": "by", "kind": "definition", "value": "D:借地権者" } ], "negated": false } ] },
            { "any_of": [ { "name": "建物がある", "args": [], "negated": false } ] }
          ] },
          "effect": { "kind": "deem", "fact": "契約を更新した", "args": [ { "key": "conditions", "kind": "text", "value": "従前と同一" } ] },
          "temporal": [],
          "unknowns": [],
          "confidence": "medium",
          "note": "「前条の規定によるもののほか」— 更新後の期間は第4条が独立に定める"
        }
      ]
    },
    {
      "sentence": "sent:2",
      "rules": [ { "...": "effect.kind = exception", "unknowns": [ { "kind": "intentional", "text": "遅滞なく" } ] } ]
    }
  ]
}
```

`args[].kind` は `definition | entity | text | duration | money | var | ref | unknown`。`duration` は `"30 year"` のような文字列で受けて Rust 側でパースする。

## 検証（棄却条件）

LLM の出力は `lawean-llm::validate_response` で層 1 と突き合わせ、1 つでも引っかかった**文**は骨組みのまま残す（Rule 単位ではなく文単位で棄却。部分採用は confidence の意味を壊す）。

1. `sentence` が入力に無い / 入力の文が出力に無い
2. `effect.kind` が層 1 の `EffectKind` と一致しない（`can` は Permission / Power のどちらでも可。層 1 の `Unknown` 系は何でも可）
3. `args[].kind = ref` の値が、層 1 が解決した参照の stable_id に無い
4. `args[].kind = definition` の ID が、その位置で有効な定義語に無い
5. `confidence` が `high`
6. `condition.all_of` が空なのに層 1 の条件節が非空（逆は可: 条件節を Unknown に落とすのは許す）

棄却された文は `issues` に理由付きで残し、人がプロンプトか層 1 を直す材料にする。

## API

- モデル: `claude-opus-5`。adaptive thinking（デフォルト）、`output_config.effort: "high"`
- `output_config.format: { type: "json_schema", schema }`。スキーマは `lawean-llm::schema::output_schema()`
- `fallbacks: "default"` + `anthropic-beta: server-side-fallback-2026-07-01`。`stop_reason == "refusal"` は content を読む前に見る
- `max_tokens: 16000`、非ストリーミング（1 項の出力は数 KB）
- system prompt に `cache_control: {type: "ephemeral"}`。schema 説明・語彙・禁止事項は system に置き、項ごとの内容は user に置く
- 認証: `ANTHROPIC_API_KEY`。Rust には公式 SDK が無いので `POST https://api.anthropic.com/v1/messages` を直接叩く（`reqwest`）
- 全条文の一括処理は Batches API（50% 引き）を後で足す。まずは 1 項ずつ

## コスト見積（借地借家法 1 本）

項 ≈ 166。system ≈ 3k tok（キャッシュ）、user ≈ 1k、出力 ≈ 1.5k、thinking 込みで出力 ≈ 4k と見積もる。
入力 166 × 1k × $5/M ≈ $0.8、キャッシュ読み 166 × 3k × $0.5/M ≈ $0.25、出力 166 × 4k × $25/M ≈ $17。**1 回 ≈ $20**。
Batches なら半額。プロンプトを詰める段階では第 3・5・22・26・38 条だけ（≈ 15 項、≈ $2）で回す。

## 実行

```sh
# 記録済みレスポンスでのオフラインテスト（CI）
cargo test -p lawean-llm

# 実 API。1 項だけ。出力とレスポンス JSON を fixtures/llm/ に保存する
ANTHROPIC_API_KEY=... cargo run -p lawean-llm --example extract_paragraph -- fixtures/403AC0000000090.xml main/chap:2/sec:1/art:5/para:1
```

## 未決

- [ ] 号（Item）を親の項と一緒に送るか、号ごとに分けるか。定義規定（第2条）は号ごとが自然
- [ ] 1 項が長すぎる場合（第61条の読み替え規定）の扱い。v0.1 では `Unknown(Unparsed)` のまま送らない
- [ ] 層 2 の出力を人がレビューする UI / 形式。まずは差分が読める JSON をリポジトリに置く
- [ ] eval: 手書き例（`lawean-semantic/examples`）8 条分を正解として一致率を測る。これが層 2 の品質指標になる
