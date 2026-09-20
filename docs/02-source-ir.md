# 02. Source IR — e-Gov 法令 XML の lossless な写像

状態: draft。写像表は `fixtures/403AC0000000090.xml`（借地借家法）に実際に出てくる要素から書いた。
他法令で出てくる要素（別表・様式・図・改正規定など）は出てきたときに追記する。

## 方針

- **意味解析はしない。** ここは XML の構造を Rust の型に写すだけ。「ただし書き」は XML が `Function="proviso"` と言っているから持つのであって、
  それが Exception であることを解釈するのは Semantic IR の仕事
- **lossless。** Source IR → XML の往復で、空白の正規化を除いて元に戻せること。往復テストを CI に置く
- **ID を最初から付ける。** すべての構造ノードに `stable_id` を振る（[ADR-0007](adr/0007-stable-id-and-version-id.md)）
- 法令の**バージョン**（`law_revision_id`）を LegalDocument に持つ。IR は「ある時点の法令」を表す

## 入力: e-Gov 法令 API v2

```
GET https://laws.e-gov.go.jp/api/2/law_data/{law_id}?response_format=xml
```

レスポンスは `law_data_response` でラップされている:

```
law_data_response
├── attached_files_info
├── law_info            law_id / law_num / law_num_era / promulgation_date …
├── revision_info       law_revision_id / law_title / amendment_law_id / amendment_enforcement_date / current_revision_status …
└── law_full_text
    └── Law             法令標準 XML スキーマの root
```

`law_revision_id` は `{law_id}_{施行日}_{改正法令ID}` の形（例: `403AC0000000090_20260521_504AC0000000048`）。
これをそのまま `version_id` に使う。

## 写像表

### 法令全体

| XML | Source IR | 備考 |
|---|---|---|
| `Law @Era @Year @Num @LawType @Lang @PromulgateMonth @PromulgateDay` | `LegalDocument { law_id, law_num, promulgated_on, law_type, … }` | `law_info` の値を優先し、`Law` 属性は検証用に保持 |
| `LawNum` | `LegalDocument.law_num_text` | 「平成三年法律第九十号」 |
| `LawBody / LawTitle @Kana @Abbrev @AbbrevKana` | `LegalDocument.title` | |
| `LawBody / TOC` | 保持するが IR 上は導出可能。往復のために raw で持つ | `TOCChapter / TOCSection / TOCArticle / TOCSupplProvision / ArticleRange` |
| `LawBody / EnactStatement` | `LegalDocument.enact_statement` | 借地借家法には無い |
| `LawBody / Preamble` | `LegalDocument.preamble` | 借地借家法には無い |
| `LawBody / MainProvision` | `LegalDocument.main_provision: Vec<Provision>` | 本則 |
| `LawBody / SupplProvision` (複数) | `LegalDocument.suppl_provisions: Vec<SupplProvision>` | 附則。**原始附則 1 本 + 改正法ごとの附則**が並ぶ |
| `LawBody / AppdxTable, AppdxNote, AppdxStyle, AppdxFormat, AppdxFig, Appdx` | `LegalDocument.appendices` | 別表・別記様式など。借地借家法には無い。**未着手** |

### 附則

| XML | Source IR | 備考 |
|---|---|---|
| `SupplProvision @AmendLawNum @Extract` | `SupplProvision { amend_law_num: Option<String>, extract: bool, label, body }` | `AmendLawNum` が無いものが原始附則。`Extract="true"` は「抄」（改正法の附則のうち本法に関係する部分のみ） |
| `SupplProvisionLabel` | `SupplProvision.label` | 「附　則」 |
| `SupplProvision / Article` | `SupplProvision.body = Articles(Vec<Article>)` | 原始附則は第1〜14条。**本則と条番号が衝突する**ので stable_id に附則の識別子が要る |
| `SupplProvision / Paragraph` (Article 無し) | `SupplProvision.body = Paragraphs(Vec<Paragraph>)` | 改正法附則の一部はこの形 |

改正法附則の `Article @Num` は改正法側の番号（第72条、第125条など）で、本法の番号体系ではない。

### 構造（本則・附則共通）

```
MainProvision
└── Part? / Chapter? / Section? / Subsection? / Division?     ← 借地借家法は Chapter / Section まで
    └── Article
        └── Paragraph
            ├── ParagraphSentence
            │   └── Sentence+
            └── Item*
                ├── ItemTitle
                ├── ItemSentence
                │   └── Sentence+ | Column+
                └── Subitem1* … Subitem10*
```

| XML | Source IR | 備考 |
|---|---|---|
| `Chapter @Num / ChapterTitle` | `Provision::Chapter { num, title, children }` | |
| `Section @Num / SectionTitle` | `Provision::Section { … }` | Part / Subsection / Division も同型。借地借家法には無いが型は用意する |
| `Article @Num @Delete? @Hide?` | `Article { stable_id, num: ArticleNum, caption, title, paragraphs }` | `Num` は `"3"`, `"22_2"`（第二十二条の二）など枝番あり → `ArticleNum` 型で持つ |
| `ArticleCaption` | `Article.caption` | 「（借地権の存続期間）」。括弧込みで保持 |
| `ArticleTitle` | `Article.title` | 「第三条」。`Num` から導出できるが保持 |
| `Paragraph @Num @OldStyle? @OldNum?` | `Paragraph { stable_id, num, caption?, sentences, items }` | |
| `ParagraphNum` | `Paragraph.num_text` | 第1項は空要素、第2項以降は「２」（全角） |
| `ParagraphSentence / Sentence` | `Paragraph.sentences: Vec<Sentence>` | |
| `Item @Num / ItemTitle / ItemSentence` | `Item { stable_id, num, title, sentences_or_columns, subitems }` | |
| `Subitem1..10` | `Item.subitems`（再帰） | 借地借家法には無い |

### 文

| XML | Source IR | 備考 |
|---|---|---|
| `Sentence @Num @Function @WritingMode` | `Sentence { stable_id, num, function: SentenceFunction, text: Vec<Inline> }` | |
| `@Function="main"` | `SentenceFunction::Main` | 本文 |
| `@Function="proviso"` | `SentenceFunction::Proviso` | **ただし書き**。XML が明示してくれる。Semantic IR の Exception 抽出の第一手掛かり |
| `@Function` 無し | `SentenceFunction::Unspecified` | 1 文だけの項はこれ。`Main` に潰さない（lossless） |
| `@WritingMode="vertical"` | 保持するだけ | |
| `Column @Num` | `ItemBody::Columns(Vec<Column { num, sentences }>)` | 第2条の定義規定が `Column1 = 用語 / Column2 = 定義` の形。**Definition 抽出の手掛かり**だが、それは Semantic IR の仕事 |
| `Ruby / Rt`, `Line`, `Sup`, `Sub` | `Inline::Ruby { base, rt }` など | 借地借家法には無い。型は用意する |
| `QuoteStruct`, `ArithFormula`, `Fig`, `Table`, `List`, `Note`, `Style`, `Format`, `Remarks` | `Inline::Raw(xml)` で逃がす | v0.1 では未着手。lossless のため raw 保持 |

### 改正規定

| XML | Source IR | 備考 |
|---|---|---|
| `AmendProvision / AmendProvisionSentence / NewProvision` | **v0.1 では扱わない** | 改め文。改正法そのものを読むときに必要。v0.2 の改正 patch で設計する |

## stable_id

`stable_id` は「法令内で位置を一意に指すパス」。改正で条番号が繰り下がっても追跡できるよう、
**位置ベースの ID と、改正 lineage を持つ ID の 2 層**にする。v0.1 では位置ベースのみ実装する。

```
403AC0000000090 / main / art:3 / para:1 / sent:2          第3条第1項ただし書き
403AC0000000090 / main / art:2 / para:1 / item:1 / col:1   第2条第1号 前段（用語）
403AC0000000090 / suppl:0 / art:4 / para:1 / sent:1        原始附則第4条
403AC0000000090 / suppl:令和三年五月一九日法律第三七号 / art:72 / para:1  改正法附則
```

- 附則は `suppl:{index}` または `suppl:{AmendLawNum}` で区別する。原始附則は `suppl:0`
- 枝番は `art:22_2` のように XML の `Num` をそのまま使う
- **Semantic IR の Provenance はこの stable_id を指す。** テキストオフセットではなく構造パスで指す

## 型（Rust 疑似コード）

```rust
struct LegalDocument {
    law_id: LawId,                // "403AC0000000090"
    version_id: VersionId,        // "403AC0000000090_20260521_504AC0000000048"
    law_num_text: String,
    title: LawTitle,
    promulgated_on: Date,
    enforced_on: Option<Date>,    // revision_info.amendment_enforcement_date
    toc_raw: Option<String>,
    main_provision: Vec<Provision>,
    suppl_provisions: Vec<SupplProvision>,
    appendices: Vec<Appendix>,    // v0.1: 空
}

enum Provision {
    Part(Container), Chapter(Container), Section(Container),
    Subsection(Container), Division(Container),
    Article(Article),
}

struct Container { stable_id: StableId, num: String, title: String, children: Vec<Provision> }

struct Article {
    stable_id: StableId,
    num: ArticleNum,              // 3 / 22_2
    caption: Option<String>,
    title: String,
    paragraphs: Vec<Paragraph>,
    deleted: bool,
}

struct Paragraph {
    stable_id: StableId,
    num: u32,
    num_text: String,
    sentences: Vec<Sentence>,
    items: Vec<Item>,
}

struct Item {
    stable_id: StableId,
    num: String,
    title: String,                // 「一」
    body: ItemBody,
    subitems: Vec<Item>,
}

enum ItemBody { Sentences(Vec<Sentence>), Columns(Vec<Column>) }

struct Sentence {
    stable_id: StableId,
    num: u32,
    function: SentenceFunction,   // Main / Proviso / Unspecified
    text: Vec<Inline>,
}

struct SupplProvision {
    stable_id: StableId,
    amend_law_num: Option<String>,
    extract: bool,
    label: String,
    body: SupplBody,              // Articles(Vec<Article>) | Paragraphs(Vec<Paragraph>)
}
```

## 未確認・TODO

- [ ] 法令標準 XML スキーマ（XSD）の一次情報を確認し、上の表に無い要素を洗い出す
- [ ] `Article @Num` の枝番表記（`22_2` か `22:2` か）を実 XML で確認する
- [ ] 往復テスト（XML → IR → XML）の正規化ルールを決める（空白・改行・属性順）
- [ ] 過去版の取得: `law_revisions` API で `law_revision_id` の一覧を取り、v0.2 の改正 patch に備える
