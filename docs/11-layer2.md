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

層 2 は Semantic IR の値を出す（Lean のソースではない。[ADR-0016](adr/0016-lean-as-semantic-backend.md)）。
各ノードに `Provenance { stable_id, by: Parser | Model("grande/e4b") | Human, confidence }`。
判定できなかった選択肢は捨てず `Unknown(Ambiguous)` に両方残す（[ADR-0005](adr/0005-no-single-interpretation.md)）。

### 候補の契約（2026-09-22）

層 1・層 1.5・層 2 の抽出はすべて **原文根拠付きの候補** `lawean-extract::candidate::Candidate` を返す
（項目は jlsi/elsa の variable-extractor、RFC0050 に揃えた。フィールドは法令用）:
`field`（用途別）/ `category`（表示用の粗い分類）/ `value_kind` / `raw` / `normalized`（月数・日数・円・ISO 日付・stable_id）/
`unit` / `role`（格・「対象規定」「行為」）/ `source_label`（事象の句・見出し）/ `evidence`（文の stable_id + UTF-8 バイト範囲 + 断片 + 前後の文脈）/
`confidence`（high / medium / low）/ `reason`（規則名、固定）。
候補は値を確定しない。採用は起草者か Semantic IR への写し（L3）が決める。`evidence.snippet == text[start..end]` はテストで確かめる。
時間表現（`temporal`）と罰則（`penalty`）は変換済み、主体・客体・行為は `lawean-nlp`（下）が出す。

### L1.5. 主体・客体・行為を係り受けで（`lawean-nlp`、2026-09-22）

格助詞の正規表現ではなく GiNZA の係り受け（UD の `nsubj` / `obj` / `obl`）で取る。モデルは jewel（Python 不要の spaCy ランタイム、
`bokuweb/jewel`）で `ja_ginza` 5.2.0 の bundle を読む（`JEWEL_GINZA_BUNDLE`。無ければ層 1 だけで動く。WASM には入れない）。
法令の文に合わせた扱い:

- **括弧書きを落として解析**し、位置は元の本文に戻す（法令の括弧書きは深く、係り受けを壊す）
- **主題の引き継ぎ**: 主述語に主語が無く、従属節に「〜は」の主語があればそれを主述語の主語に（「建物の賃貸人は、…に代えて、…提供することができる」）。parser が「裁判所は、」を独立した ROOT にする遊離にも対応
- **号の断片**「〜した者」: ROOT が名詞なら、それに係る連体節の述語を主述語に
- 「戸別訪問をした」= サ変名詞 + する は名詞の側を述語に、複合語（「戸別」+「訪問」）は一語に
- 空文・長すぎる文（4000 バイト超）・括弧だけの文は落とさず空を返す。決定的

借地借家法の義務・禁止・可能・不能の文 73 のうち主述語の主語が取れたのは 48（66%）。残りは主語の無い文（ただし書、「〜することができる」の主体が文脈にあるもの）が大半。
GiNZA の parser は文節の主辞に `_bunsetu` を付けた関係（`nsubj_bunsetu`）を出すが、jewel の抽出プロファイルには Python 側の `bunsetu_recognizer` が無いので、
`lawean-nlp` で落としている。jewel 側にも `process_bunsetu` を足す PR を出した（bokuweb/jewel#23。README の export コマンドの numpy / Python の固定も）。

## 3. 段階

### L1. 規則で候補を出す（`lawean-extract` に足す）

| 対象 | 手段 | 出力 |
|---|---|---|
| 節末動詞と格 | 形態素解析（vibrato、Rust）で条件節を分かち書き、節末の動詞（原形）を述語名、直前の「が／を／に／から／まで／と／で」句を引数候補に | `Pred(name, args)`。引数は `EntityRef`（定義語に当たれば）か `Var` |
| 期間・時点・施行 | **済**: `lawean-extract::temporal::time_exprs`（正規表現。「〜の日から起算して六月を経過した日」「一年前から六月前までの間」「公布の日から起算して一年を超えない範囲内において政令で定める日から施行する」。利率・法令番号・読替えの中の字句は除く）。被覆 借地借家法 100%、公職選挙法 87%、民法 89%（[07](07-verification.md) 時間表現の抽出）。附則 → 施行日は `suppl` | `TimeExpr`（`Period` / `Elapsed` / `Within` / `Window` / `Before` / `Compare` / `Enforcement` …）。Z3 の暦へは `lawean-verify::enforcement`。残りは先行詞が文をまたぐもの（「同項の期間」）で L2 |
| 金額・数量 | 正規表現（「年一割」「二百平方メートル」）。漢数字は `lawean-resolve::numeral` | `Value::Money` / `Int` |
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
| **適合率・再現率（L1、候補）** | `fixtures/gold`: 規則作成に使った法令（dev: 高齢者居住安定確保法 60 文）と使っていない法令（eval: 大規模災害借地借家特別措置法・借地借家法施行令・同施行規則 20 文）。文書単位で分け、(field, raw) の完全一致。負例（数字はあるが候補が無い文）を含む | `lawean-extract/tests/gold.rs`（dev は完全一致、eval は下限）。`cargo run -p lawean-extract --example eval`。**2026-09-22: eval P 0.88 / R 0.88**（誤り 2 件はどちらも事象の句の境界。「地上権の放棄又は土地の賃貸借の解約の申入れがあった日」を最後の「又は」で切った、「〜の日から起算して一年以内」の事象を落とした）。eval の文を見て規則を直したらその文は dev に移す |
| 構造一致率（L1） | 手書き 8 条 | `lawean-extract/tests` |
| 判定精度（L2） | 「できる」44 文、照応 20 件程度 | `lawean-decide/tests`（ラベルは fixtures） |
| Unknown 率 | 借地借家法本則 211 文 | `lawean-extract/tests` |
| 性質の前提の数（L4） | [07](07-verification.md) の 6 性質 | `lean/Lawean/Properties.lean` |

## 5. 順序と依存

L1 は今すぐ始められる（形態素解析器の導入が最初の作業）。L2 は grande の環境（Gemma 4 E4B、ローカル）が要る。
[10](10-lean-semantics.md) の M1〜M3 は手書き IR で進むので L1/L2 と並行できる。合流点は M5 = L4。
