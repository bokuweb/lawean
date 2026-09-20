# 11. 層 2 — 条件の中身と効果の引数を構造化する（計画）

状態: 計画（2026-09-20）。方針は [ADR-0008](adr/0008-extraction-strategy.md)・[ADR-0009](adr/0009-layer2-decisions-via-grande.md) のまま。ここは着手順と完了条件。
出力先は [10](10-lean-semantics.md) の Lean データ経路（`lawean-lean`）。

## 1. いま何があるか

層 1（`lawean-extract`、[ADR-0008](adr/0008-extraction-strategy.md)）は文ごとに次を出す:

```
403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:2 [Proviso] Set とする
    if   契約でこれより長い期間を定めた (ときは)
```

効果種別（20 種）、条件節の**文字列**、参照（解決済み）、「かかわらず」の 3 分類、ただし書き → overrides。
条件の中身は `Unknown(Unparsed)`。第3条なら手書き IR（[03-examples/art-03.md](03-examples/art-03.md)）は
`Cmp(Var(契約で定めた期間), Gt, RuleValue(R3-1))` と `Set(存続期間, Var(契約で定めた期間))` で、この差を埋めるのが層 2。

## 2. 出力の形

層 2 は Semantic IR の値を出す（Lean のソースではない。[ADR-0015](adr/0015-lean-as-semantic-backend.md)）。
各ノードに `Provenance { stable_id, by: Parser | Model("grande/e4b") | Human, confidence }`。
判定できなかった選択肢は捨てず `Unknown(Ambiguous)` に両方残す（[ADR-0005](adr/0005-no-single-interpretation.md)）。

## 3. 段階

### L1. 規則で候補を出す（`lawean-extract` に足す）

| 対象 | 手段 | 出力 |
|---|---|---|
| 節末動詞と格 | 形態素解析（vibrato、Rust）で条件節を分かち書き、節末の動詞（原形）を述語名、直前の「が／を／に／から／まで／と／で」句を引数候補に | `Pred(name, args)`。引数は `EntityRef`（定義語に当たれば）か `Var` |
| 期間・金額・数量 | 正規表現（「三十年」「六月」「年一割」「二百平方メートル」）。漢数字は `lawean-resolve::numeral` | `Value::Duration` / `Money` / `Int`。Lean へは月数・円の Int |
| 比較 | 「以上／以下／未満／を超える／より長い／より短い」 | `Cmp(a, op, b)`。「これ」「その期間」は照応（L2 へ） |
| 参照 | 層 1 の解決結果をそのまま | `Ref(Rule)` / `RuleValue(Rule)` |
| 定義語 | 定義規定（`Column`）と「以下「X」という。」の有効 scope（`lawean-resolve`） | `EntityRef::Definition` |
| 節の結合 | 「かつ」「又は」「若しくは」「、」の並列、「場合において」「ときは」の入れ子 | `And` / `Or` の木 |
| 効果の引数 | 「〜は、〜とする」の主語 → `Set(attr, value)`。「〜とみなす」の目的語 → `Deem(fact)`。「〜を無効とする」 → `Void(target)` | `Effect` の中身 |

完了条件: 手書き 8 条（[lawean-semantic/src/examples](../crates/lawean-semantic/src/examples/shakuchi_shakuya.rs)）に対して、
**構造**（`And`/`Or` の形、`Cmp` の演算子、`Effect` の種類と属性名）の一致率を出し、テストに固定する。述語名は正規化前なので一致を求めない。

### L2. grande で判定する（`lawean-decide` crate、[ADR-0009](adr/0009-layer2-decisions-via-grande.md) の表）

| 判定 | 型 | L1 だけのときの扱い |
|---|---|---|
| 「できる」は Permission か Power か | `choice` | `Unknown(Ambiguous)` に両方 |
| 2 つの述語は同じ事実か（「更新を請求した」= 「契約の更新を請求する」） | `noul` | 別の述語のまま（性質は弱くなるが偽にはならない） |
| 引数の主体はどの定義語か | `choice`（有効な定義語 + その他） | 格助詞の直前の名詞そのまま |
| 評価概念か（正当の事由・相当・不利） | `noul` | 名詞リスト（05-glossary）で暫定判定 |
| 照応の先（「これ」「その期間」「同項の」）はどの Rule / 値か | `choice`（同じ条の Rule + 上書き先） | 直前の Rule。[07](07-verification.md) の第4条のバグはここが原因 |
| 括弧書きは場合分けか説明か | `choice` | 説明として無視 |

grande の確率を `confidence` に。閾値未満は `Ambiguous` のまま人に回す。

完了条件: 借地借家法の「できる」44 文を人がラベルし、ゼロショットの精度を出す。足りなければ pointer head を学習（`tools/train_head.py`）。

### L3. 人の確認と昇格

`Author::Human` に置き換えた Rule だけ `High` にする。レビュー画面は `lawean-render::semantic`（原文と並べた日本語）を使う。
手書き IR と層 2 の出力の差分を出す `diff` を作り、レビューはその差分だけ見る。

### L4. Lean へ

`lawean-lean::emit_model` で層 2 の出力を出し、[10](10-lean-semantics.md) の性質を通す。
性質の前提に「確度 ≥ c」「Unknown k を仮定」が出るのが正常。層 2 が育つほど前提が減る。

## 4. 評価（継続的に測る）

| 指標 | 母集団 | 固定先 |
|---|---|---|
| 構造一致率（L1） | 手書き 8 条 | `lawean-extract/tests` |
| 判定精度（L2） | 「できる」44 文、照応 20 件程度 | `lawean-decide/tests`（ラベルは fixtures） |
| Unknown 率 | 借地借家法本則 211 文 | `lawean-extract/tests` |
| 性質の前提の数（L4） | [07](07-verification.md) の 6 性質 | `lean/Lawean/Properties.lean` |

## 5. 順序と依存

L1 は今すぐ始められる（形態素解析器の導入が最初の作業）。L2 は grande の環境（Gemma 4 E4B、ローカル）が要る。
[10](10-lean-semantics.md) の M1〜M3 は手書き IR で進むので L1/L2 と並行できる。合流点は M5 = L4。
