# 04. Semantic IR

状態: **骨組みのみ**。[03-examples/](03-examples/) で裏付けられた分だけここに書く。
構想段階の型スケッチ（`bokuweb/life` の `idea/legal-ir/03-ir-design.md`）は素材であって仕様ではない。

## トップレベル

```rust
struct SemanticModel {
    document: VersionId,            // どの Source IR に対する意味か
    definitions: Vec<Definition>,
    rules: Vec<Rule>,
    unknowns: Vec<UnknownNode>,     // どこにも当てはめられなかった文の置き場
}
```

Source IR（`LegalDocument`）とは**別のツリー**。両者は `Provenance.stable_id` でのみ結ばれる（[ADR-0006](adr/0006-source-and-semantic-separated.md)）。

## Rule

```rust
struct Rule {
    id: RuleId,                     // stable_id + version_id
    subject: Option<EntityRef>,
    condition: Expr,
    effect: Effect,
    overrides: Vec<RuleId>,         // この Rule が例外・特則として上書きする Rule
    temporal: Option<Temporal>,
    interpretations: Vec<Interpretation>,   // 空なら文理解釈のみ
    provenance: Provenance,
}
```

- `overrides` は**宣言する側**が持つ（ただし書き → 本文、特則 → 原則）。逆向きの `exceptions` は resolver が張る
- 1 文から複数 Rule が出てよい（[art-04.md](03-examples/art-04.md)）。Provenance は同じ stable_id を指す

## Effect（03 で必要になった分）

| Effect | 出典 | 例 |
|---|---|---|
| `Obligation(Action)` | 「しなければならない」 | 第22条後段: 書面によってしなければならない |
| `Prohibition(Action)` | 「してはならない」 | |
| `Permission(Action)` | 「できる」（許容） | [ADR-0003](adr/0003-permission-vs-power.md) |
| `Power(Action)` | 「できる」（法律関係を変える権限） | 第13条 建物買取請求 |
| `Set(Attribute, Value)` | 「〜は、〜とする」 | 第3条 存続期間 30 年。**03 で追加** |
| `Deem(Fact)` | 「みなす」 | 第5条 更新したものとみなす。[ADR-0002](adr/0002-deem-vs-presume.md) |
| `Presume(Fact)` | 「推定する」 | [ADR-0002](adr/0002-deem-vs-presume.md) |
| `Void(Target)` | 「無効とする」 | 第9条。**03 で追加** |
| `Exception(RuleId)` | 「この限りでない」「適用しない」 | 第5条ただし書き |
| `SameAs(RuleId)` | 「前項と同様とする」 | 第5条第2項。**03 で追加。要検討** |
| `Unknown(UnknownExpr)` | 上のどれにも当てはまらない | 第5条第3項の「みなして〜適用する」 |

TODO: `Deem(X AS Y) THEN Apply(R)`（読み替え + 適用）を合成 Effect にするか Unknown にするか（[art-05.md](03-examples/art-05.md)）。

## Expr

```rust
enum Expr {
    True, False,
    And(Vec<Expr>), Or(Vec<Expr>), Not(Box<Expr>),
    Pred(Predicate),                // 述語 + 引数。引数に EntityRef / Value / Var
    Cmp(Value, CmpOp, Value),       // 期間・金額・数量の比較
    Ref(RefTarget),                 // Rule / Definition / Scope / 外部法令
    Unknown(UnknownExpr),
}
```

- `Not` は**原文に「〜でない」「〜を除き」がある場合だけ**使う。例外は `overrides` で表す（[ADR-0004](adr/0004-exception-not-negation.md)）
- `Cmp` の `Value` に `RuleValue(RuleId)`（他 Rule の effect が定める値）が要る（[art-04.md](03-examples/art-04.md) の「これより長い」）

## Unknown

```rust
struct UnknownExpr {
    kind: UnknownKind,              // Intentional / Unparsed / Ambiguous / External
    text: String,                   // 該当箇所の原文
    provenance: Provenance,
}
```

[ADR-0001](adr/0001-unknown-not-opaque.md)。`Intentional` = 法律が意図的に開放的にしている（正当事由、遅滞なく、相当）、
`Unparsed` = 変換器が理解できなかった、`Ambiguous` = 複数解釈があり未選択、`External` = 他法令・政令・判例に委ねられている。

## Temporal

TODO。03 の art-03 / art-04 / 附則第4条・第6条 / 民法第140・143条 を書いてから。最低限:

- `Duration { length, unit, from: Event }` — 起算点付き
- `Instant` — 施行日・公布日・特定日
- `Range` — 適用期間
- 「施行前に生じた事項」「なお従前の例による」を表す経過措置ノード

## Reference

```rust
enum RefTarget {
    Rule(RuleId),
    Definition(DefinitionId),
    Scope(StableId),                // 「この節」「この法律」→ 構造パス
    Provision(StableId),            // 「前条」「第十六条」→ 構造パス（Resolved IR で Rule 集合に展開）
    External { law: LawId, path: Option<StableId> },   // 「民法第六百四条」
    Relative(RelativeRef),          // 「前項」「次条第一項」「前二項」→ resolver が Provision に解決
}
```

文字列にしない。`Relative` は Source IR の位置から機械的に解決できるので、Semantic IR の段階では未解決のままでよい。

## Definition

```rust
struct Definition {
    id: DefinitionId,
    term: String,
    scope: StableId,                // 「この法律において」→ law root、「この節において」→ section
    body: Expr,                     // v0.1 では Unknown(Unparsed) でよい
    provenance: Provenance,
}
```

## Provenance

```rust
struct Provenance {
    source: StableId,               // Source IR の構造パス。テキストオフセットではない
    confidence: Confidence,         // High / Medium / Low
    by: Author,                     // Human(name) / Llm(model) / Rule(parser_rule_id)
    note: Option<String>,
}
```

全ノード必須。

## 決めていないこと

- [ ] Entity / Predicate の語彙をどう管理するか（自由文字列か、法令ごとの語彙表か）
- [ ] `SameAs` を残すか、resolver で展開して消すか
- [ ] Interpretation の `authority`（文理 / 判例 / 行政解釈 / 学説）の粒度
- [ ] 手続（第41条以降の裁判手続）をどう扱うか。v0.1 では `Unknown(External)` で全部逃がす想定
