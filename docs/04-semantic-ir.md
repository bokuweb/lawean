# 04. Semantic IR

状態: **v0.1 の型を実装**（[crates/lawean-semantic/src/ir.rs](../crates/lawean-semantic/src/ir.rs) が正。この文書は設計意図と論点）。
[03-examples/](03-examples/) のうち第2・3・4・5・6・9・22・26条を手書き IR データ
（[examples/shakuchi_shakuya.rs](../crates/lawean-semantic/src/examples/shakuchi_shakuya.rs)）にして、
Source IR に対する参照整合性（stable_id / RuleId / DefinitionId）を `validate` で検査している。
semantic parser（自然文 → IR）はまだ無い。
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
    overrides: Vec<Override>,       // この Rule が例外・特則として上書きする Rule、または契約（「契約の条件にかかわらず」第32条）
    temporal: Option<Temporal>,
    interpretations: Vec<Interpretation>,   // 空なら文理解釈のみ
    provenance: Provenance,
}
```

- `Override::Rule(RuleId) | Override::Contract`。後者は法令が特約に優先することの宣言（第32条第1項）。強行規定（第9条）は逆に「特約を無効にする Rule」なので `Void`
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
| `DeemAndApply { deem: (Fact, Fact), apply: RuleId }` | 「A を B とみなして、〜の規定を適用する」 | 第5条第3項・第22条第2項・第26条第3項・第38条第2項。**4 箇所で同型なので合成 Effect にする** |
| `Apply(Scope)` | 「〜の規定は、〜にも適用する」 | 附則第4条 |
| `ApplyExternal { law, topic }` | 「なお従前の例による」 | 附則第6条。廃止法令の適用 |
| `Preserve(Target)` | 「〜の効力を妨げない」「なお同項の効力を有する」 | 附則第4条ただし書き・第10条第2項。Void の反対 |
| `ApplyMutatis { rules, substitute }` | 「〜について準用する」 | 第13条第3項。主体を置換して適用。`DeemAndApply`（事実の読み替え）との統合は未決 |
| `Unknown(UnknownExpr)` | 上のどれにも当てはまらない | |

`Obligation / Prohibition / Permission / Power` の `Action` は `{ verb, by, to, content, timing, form }` 程度の構造。
「対抗することができない」「述べることができない」は `Prohibition`（[art-06.md](03-examples/art-06.md)、[art-38.md](03-examples/art-38.md)）。

## Expr

```rust
enum Expr {
    True, False,
    And(Vec<Expr>), Or(Vec<Expr>), Not(Box<Expr>),
    Pred(Predicate),                // 述語 + 引数。引数に EntityRef / Value / Var
    Cmp(Value, CmpOp, Value),       // 期間・金額・数量の比較
    Time(TimeCond),                 // 時間述語
    Ref(RefTarget),                 // Rule / Definition / Scope / 外部法令
    Unknown(UnknownExpr),
}
```

- `Not` は**原文に「〜でない」「〜を除き」がある場合だけ**使う。例外は `overrides` で表す（[ADR-0004](adr/0004-exception-not-negation.md)）
- `Cmp` の `Value` に `RuleValue(RuleId)`（他 Rule の effect が定める値）が要る（[art-04.md](03-examples/art-04.md) の「これより長い」）
- `Value` には算術（`Add / Sub`）と `Interest { principal, rate, from }` が要る（[art-32.md](03-examples/art-32.md) 「不足額に年一割の利息を付して」）
- **譲歩表現（「〜ても」「〜にかかわらず」「〜がなくても」）は condition に入れない**（[art-10.md](03-examples/art-10.md) 「登記がなくても」）

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

03 の例から必要になった型。計算規則は民法第140〜143条（[minpo-140-143.md](03-examples/minpo-140-143.md)）。

```rust
enum Unit { Hour, Day, Week, Month, Year }
struct Duration { length: u32, unit: Unit }

enum Event {                                    // 起算点となる事象。値は Fact として外から与える
    Symbol(String),                             // 「契約の日」「更新の日」「通知の日」「期間の満了」「施行日」
}

struct Period {                                 // 「〜の日から十年」「満了の一年前」
    from: Event,
    length: Duration,
    direction: Direction,                       // Forward | Backward
}

struct Window { from: Period, to: Period }      // 「一年前から六月前までの間」（第26条・第38条第6項）

enum PeriodValue { Definite(Period), Indefinite }   // 「期間の定めがない」（第26条ただし書き）

enum TimeCond {                                 // condition に出てくる時間述語
    Before(Event), After(Event),                // 「施行前に生じた」
    Within(Window),                             // 「〜までの間に」
    Elapsed(Period),                            // 「通知の日から六月を経過した後」
}

struct RuleTemporal {                           // Rule 自体の時間属性
    effective: Option<(Event, Option<Event>)>,  // 施行日〜（廃止日）
    facts_before_boundary: Option<Event>,       // 経過措置: 施行前の事実にも適用（附則第4条）/ 施行前の事実にだけ適用（附則第6条）
}
```

- 施行日は法令内で確定しない（政令委任）。`Event::Symbol("施行日")` として持ち、値は `LegalDocument.enforced_on` から束縛する
- Backward（遡り）の計算規則は民法に無い → `Unknown(Ambiguous)` 扱い。v0.1 の実装は Forward の 140・141・143 のみ
- **Fact の発生時点で適用 Rule 集合が変わる**（附則第6条: 施行前設定の借地権は旧法）。Temporal resolver の仕事

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
    scope: Vec<StableId>,           // 「この法律において」→ [law root]、「この条において」→ [art:6]、
                                    // 「第三十八条第二項及び第三十九条第三項において同じ」→ 飛び飛び（第22条）
    body: DefinitionBody,           // Expr | Temporal(Window など。第38条第6項「通知期間」) | Unknown
    provenance: Provenance,
}
```

内側の scope が外側を上書きする（第6条「借地権者（転借地権者を含む。以下この条において同じ。）」が第2条を上書き）。

## Provenance

```rust
struct Provenance {
    source: StableId,               // Source IR の構造パス。テキストオフセットではない
    confidence: Confidence,         // High / Medium / Low
    by: Author,                     // Human(name) / Llm(model) / Parser(rule_id) / Precedent(case_id)
    note: Option<String>,           // 「登記がなくても」のような、IR に写さなかった譲歩表現などを残す
}
```

全ノード必須。

## 決めていないこと

- [ ] Entity / Predicate の語彙をどう管理するか（自由文字列か、法令ごとの語彙表か）
- [ ] `SameAs` を残すか、resolver で展開して消すか
- [ ] `overrides` の粒度。第26条ただし書きは Rule 全体ではなく effect の一部（期間）だけ上書きする
- [ ] 強行規定（R9）に対する特則（R22）の展開順序。Exception グラフが 2 段になる
- [ ] 「ときに限り」（必要条件）と「場合において」（条件）を Expr 上で区別するか。「〜場合に限る」を `OnlyIf(cond)` という Effect 修飾にする案（[art-10.md](03-examples/art-10.md)）
- [ ] `ApplyMutatis`（準用 = 主体の置換）と `DeemAndApply`（みなして適用 = 事実の読み替え）を 1 つの `Substitute` にまとめるか
- [ ] 増額と減額のように、1 文が対称な 2 Rule を生むとき、分けるか `direction` パラメータにするか（[art-32.md](03-examples/art-32.md)）
- [ ] 裁判所が主体の Rule（第13条第2項、第4章）を v0.1 で扱うか `Unknown(External)` に逃がすか
- [ ] Interpretation の `authority`（文理 / 判例 / 行政解釈 / 学説）の粒度
- [ ] 手続（第41条以降の裁判手続）をどう扱うか。v0.1 では `Unknown(External)` で全部逃がす想定
