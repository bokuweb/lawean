//! 衆議院の制定法律の測定（`bench_apply`）で当てられなかった形を、小さな法令に当てて確かめる

use lawean_amend::*;
use lawean_source::*;

/// 甲法: 本則（`main`）と附則（`suppl`）の中身から裸の `<Law>` を組む
fn law(toc: &str, main: &str, suppl: &str) -> LegalDocument {
    let xml = format!(
        r#"<Law Era="Heisei" Lang="ja" LawType="Act" Num="1" Year="1"><LawNum>平成元年法律第一号</LawNum><LawBody><LawTitle>甲法</LawTitle>{toc}<MainProvision>{main}</MainProvision><SupplProvision><SupplProvisionLabel>附　則</SupplProvisionLabel>{suppl}</SupplProvision></LawBody></Law>"#
    );
    parse_law_xml(&xml).unwrap()
}

fn para(num: u32, caption: &str, body: &str, children: &str) -> String {
    let caption = if caption.is_empty() {
        String::new()
    } else {
        format!("<ParagraphCaption>{caption}</ParagraphCaption>")
    };
    let label = if num == 1 {
        String::new()
    } else {
        char::from_u32('０' as u32 + num).unwrap().to_string()
    };
    format!(
        r#"<Paragraph Num="{num}">{caption}<ParagraphNum>{label}</ParagraphNum><ParagraphSentence><Sentence Num="1">{body}</Sentence></ParagraphSentence>{children}</Paragraph>"#
    )
}

fn table(rows: &[&[&str]]) -> String {
    let rows: String = rows
        .iter()
        .map(|r| {
            let cols: String = r
                .iter()
                .map(|c| format!(r#"<TableColumn><Sentence Num="1">{c}</Sentence></TableColumn>"#))
                .collect();
            format!("<TableRow>{cols}</TableRow>")
        })
        .collect();
    format!("<TableStruct><Table>{rows}</Table></TableStruct>")
}

fn apply(doc: &LegalDocument, amend: &str) -> LegalDocument {
    let t = format!("第一条　甲法（平成元年法律第一号）の一部を次のように改正する。\n{amend}");
    let units = parse_units(&t).unwrap();
    apply_unit(doc, &units[0], "test").unwrap_or_else(|e| panic!("{e}"))
}

fn xml(doc: &LegalDocument) -> String {
    emit_law(doc).to_xml()
}

/// 「附則第二項及び第三項を次のように改める。」+「（…の特例）」: 内容の見出しは項の見出し（電波法 令元-6）。
/// 見出しの行を番号の無い項と読まない
#[test]
fn replacing_suppl_paragraphs_with_a_caption() {
    let doc = law(
        "",
        &format!(
            r#"<Article Num="1"><ArticleTitle>第一条</ArticleTitle>{}</Article>"#,
            para(1, "", "甲とする。", "")
        ),
        &[
            para(1, "", "この法律は、公布の日から施行する。", ""),
            para(2, "（旧特例）", "当分の間、甲とする。", ""),
            para(3, "", "前項の規定は、乙に適用する。", ""),
        ]
        .concat(),
    );
    let got = apply(
        &doc,
        "　　附則第二項及び第三項を次のように改める。
　　（電波利用料の特例）
　２　当分の間、丙とする。
　３　前項の規定は、丁に適用する。",
    );
    let x = xml(&got);
    assert!(x.contains("（電波利用料の特例）"), "{x}");
    assert!(!x.contains("（旧特例）"), "{x}");
    assert!(
        x.contains("当分の間、丙とする。") && x.contains("前項の規定は、丁に適用する。"),
        "{x}"
    );
    assert!(!x.contains("乙に適用"), "{x}");
}

/// 附則の条の項の表（「附則第四条第一項中…同項の表中「令和二年度五千億円」を削る」。特別会計に関する法律 令2-6）:
/// 附則の条で引き、字句が行の升目をつないだものなら、その行を削る
#[test]
fn deleting_a_row_of_a_suppl_table_by_its_text() {
    let rows = table(&[
        &["年度", "控除額"],
        &["令和二年度", "五千億円"],
        &["令和三年度", "六千億円"],
    ]);
    let doc = law(
        "",
        &format!(
            r#"<Article Num="4"><ArticleTitle>第四条</ArticleTitle>{}</Article>"#,
            para(1, "", "本則の甲とする。", "")
        ),
        &format!(
            r#"<Article Num="4"><ArticleTitle>第四条</ArticleTitle>{}</Article>"#,
            para(
                1,
                "",
                "令和元年度から令和二年度までの控除額は、次の表のとおりとする。",
                &rows
            )
        ),
    );
    let got = apply(
        &doc,
        "　　附則第四条第一項中「令和元年度」を「令和二年度」に改め、同項の表中「令和二年度五千億円」を削る。",
    );
    let x = xml(&got);
    assert!(!x.contains("五千億円"), "{x}");
    assert!(x.contains("六千億円"), "{x}");
    assert!(x.contains("令和二年度から令和二年度まで"), "{x}");
}

/// 定義の号の二つの欄をまたぐ字句（「平均給与等支給額　適用年の継続雇用者（当該」。租税特別措置法 平30-7）
#[test]
fn replacing_a_phrase_across_the_columns_of_a_definition_item() {
    let item = r#"<Item Num="1"><ItemTitle>一</ItemTitle><ItemSentence><Column Num="1"><Sentence Num="1">平均給与等支給額</Sentence></Column><Column Num="2"><Sentence Num="1">適用年の継続雇用者（当該適用年に給与等の支給を受けた者をいう。）に対する給与等の支給額をいう。</Sentence></Column></ItemSentence></Item>"#;
    let doc = law(
        "",
        &format!(
            r#"<Article Num="2"><ArticleTitle>第二条</ArticleTitle>{}</Article>"#,
            para(
                1,
                "",
                "この条において、次の各号に掲げる用語の意義は、当該各号に定めるところによる。",
                item
            )
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第二条第一号中「平均給与等支給額　適用年の継続雇用者（当該」を「継続雇用者給与等支給額　継続雇用者（当該」に改める。",
    );
    let x = xml(&got);
    assert!(
        x.contains(
            r#"<Column Num="1"><Sentence Num="1">継続雇用者給与等支給額</Sentence></Column>"#
        ),
        "{x}"
    );
    assert!(x.contains(">継続雇用者（当該適用年に"), "{x}");
}

/// 「同項第三号イを次のように改める。」+「イ　…」「(1)　…」: 号の下の細目を差し替える（租税特別措置法 平30-7）
#[test]
fn replacing_a_subitem_with_its_subitems() {
    let item = r#"<Item Num="3"><ItemTitle>三</ItemTitle><ItemSentence><Sentence Num="1">次に掲げる会社</Sentence></ItemSentence><Subitem1 Num="1"><Subitem1Title>イ</Subitem1Title><Subitem1Sentence><Sentence Num="1">旧のイ</Sentence></Subitem1Sentence></Subitem1><Subitem1 Num="2"><Subitem1Title>ロ</Subitem1Title><Subitem1Sentence><Sentence Num="1">ロのまま</Sentence></Subitem1Sentence></Subitem1></Item>"#;
    let doc = law(
        "",
        &format!(
            r#"<Article Num="3"><ArticleTitle>第三条</ArticleTitle>{}</Article>"#,
            para(1, "", "次に掲げるものとする。", item)
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第三条第一項第三号イを次のように改める。
　　　イ　次に掲げる外国関係会社
　　　　(1)　株式等の保有を主たる事業とするもの
　　　　(2)　航空機の貸付けを主たる事業とするもの",
    );
    let x = xml(&got);
    assert!(!x.contains("旧のイ"), "{x}");
    assert!(x.contains("次に掲げる外国関係会社"), "{x}");
    assert!(x.contains("株式等の保有を主たる事業とするもの"), "{x}");
    assert!(x.contains("航空機の貸付けを主たる事業とするもの"), "{x}");
    assert!(x.contains("ロのまま"), "{x}");
}

/// 「第十二条第一項の表道府県の項第二号中「A」を「B」に改め」: 項は上欄の空いた続きの行にわたり、
/// 号は「二　…」の行から次の号の前まで（地方交付税法 毎年）
#[test]
fn replacing_a_phrase_in_an_item_of_a_row_group() {
    let rows = table(&[
        &["地方団体の種類", "経費の種類", "測定単位"],
        &["道府県", "一　警察費", "令和二年度の職員数"],
        &[
            "",
            "二　補正予算債償還費",
            "平成十三年度から令和二年度までの地方債の額",
        ],
        &["", "", "令和二年度の元利償還金"],
        &["", "三　災害復旧費", "令和二年度の事業費"],
        &["市町村", "一　消防費", "令和二年度の人口"],
    ]);
    let doc = law(
        "",
        &format!(
            r#"<Article Num="12"><ArticleTitle>第十二条</ArticleTitle>{}</Article>"#,
            para(1, "", "測定単位は、次の表のとおりとする。", &rows)
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第十二条第一項の表道府県の項第二号中「令和二年度」を「令和三年度」に改める。",
    );
    let x = xml(&got);
    assert!(
        x.contains("平成十三年度から令和三年度までの地方債の額"),
        "{x}"
    );
    assert!(x.contains("令和三年度の元利償還金"), "{x}");
    assert!(x.contains("令和二年度の職員数"), "{x}");
    assert!(x.contains("令和二年度の事業費"), "{x}");
    assert!(x.contains("令和二年度の人口"), "{x}");
}

/// 位置を言わない「「第一章　総則」を「第一章　通則」に改める。」: 章名と目次も（科学技術・イノベーション創出の活性化に関する法律 平30-94）
#[test]
fn a_bare_replacement_reaches_chapter_titles_and_the_toc() {
    let toc = r#"<TOC><TOCLabel>目次</TOCLabel><TOCChapter Num="1"><ChapterTitle>第一章　総則</ChapterTitle><ArticleRange>（第一条）</ArticleRange></TOCChapter></TOC>"#;
    let doc = law(
        toc,
        &format!(
            r#"<Chapter Num="1"><ChapterTitle>第一章　総則</ChapterTitle><Article Num="1"><ArticleTitle>第一条</ArticleTitle>{}</Article></Chapter>"#,
            para(1, "", "この法律は、甲を目的とする。", "")
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(&doc, "　　「第一章　総則」を「第一章　通則」に改める。");
    let x = xml(&got);
    assert!(!x.contains("総則"), "{x}");
    assert_eq!(x.matches("第一章　通則").count(), 2, "{x}");
}

/// 目次の傍点（`<Ruby>さ<Rt>ヽ</Rt></Ruby>`）は字句に数えない（水産資源保護法「さく河魚類」）
#[test]
fn toc_text_drops_ruby_readings() {
    let toc = r#"<TOC><TOCLabel>目次</TOCLabel><TOCChapter Num="1"><ChapterTitle>第一章　<Ruby>さ<Rt>ヽ</Rt></Ruby><Ruby>く<Rt>ヽ</Rt></Ruby>河魚類の保護</ChapterTitle><ArticleRange>（第一条）</ArticleRange></TOCChapter></TOC>"#;
    let doc = law(
        toc,
        &format!(
            r#"<Chapter Num="1"><ChapterTitle>第一章　さく河魚類の保護</ChapterTitle><Article Num="1"><ArticleTitle>第一条</ArticleTitle>{}</Article></Chapter>"#,
            para(1, "", "甲とする。", "")
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let t = apply::toc_text(&doc).unwrap();
    assert!(t.contains("さく河魚類の保護"), "{t}");
    assert!(!t.contains('ヽ'), "{t}");
    let got = apply(&doc, "　　目次中「さく河魚類」を「遡河魚類」に改める。");
    assert!(apply::toc_text(&got).unwrap().contains("遡河魚類の保護"));
}

/// 「第二号イ(5)中」: 細目の下の括弧の細目（特定複合観光施設区域整備法 第41条。刑法等の一部を改正する法律の整理法 令4-68）
#[test]
fn a_phrase_in_a_nested_subitem() {
    let item = r#"<Item Num="2"><ItemTitle>二</ItemTitle><ItemSentence><Sentence Num="1">次に掲げる者</Sentence></ItemSentence><Subitem1 Num="1"><Subitem1Title>イ</Subitem1Title><Subitem1Sentence><Sentence Num="1">次に掲げる罪</Sentence></Subitem1Sentence><Subitem2 Num="1"><Subitem2Title>（１）</Subitem2Title><Subitem2Sentence><Sentence Num="1">禁錮以上の刑に処せられた者</Sentence></Subitem2Sentence></Subitem2><Subitem2 Num="2"><Subitem2Title>（２）</Subitem2Title><Subitem2Sentence><Sentence Num="1">禁錮の刑の執行を終わつた者</Sentence></Subitem2Sentence></Subitem2></Subitem1></Item>"#;
    let doc = law(
        "",
        &format!(
            r#"<Article Num="41"><ArticleTitle>第四十一条</ArticleTitle>{}</Article>"#,
            para(
                1,
                "",
                "次の各号のいずれかに該当する者は、免許を受けることができない。",
                item
            )
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第四十一条第一項第二号イ(2)中「禁錮」を「拘禁刑」に改める。",
    );
    let x = xml(&got);
    assert!(x.contains("拘禁刑の刑の執行を終わつた者"), "{x}");
    assert!(x.contains("禁錮以上の刑に処せられた者"), "{x}");
}

/// ただし書が proviso と印されていない（「ただし、」で始まる文が main。水道法 第15条第2項）、
/// 文の中にある（「…ヲ有ス但シ…」。手形法 第20条）
#[test]
fn a_proviso_that_is_not_marked_as_such() {
    let p2 = r#"<Paragraph Num="2"><ParagraphNum>２</ParagraphNum><ParagraphSentence><Sentence Function="main" Num="1">常時水を供給しなければならない。</Sentence><Sentence Function="main" Num="2">ただし、命令を受けたため、給水を停止することができる。</Sentence></ParagraphSentence></Paragraph>"#;
    let doc = law(
        "",
        &format!(
            r#"<Article Num="15"><ArticleTitle>第十五条</ArticleTitle>{}{p2}</Article><Article Num="20"><ArticleTitle>第二十条</ArticleTitle>{}</Article>"#,
            para(1, "", "拒んではならない。", ""),
            para(1, "", "裏書ハ指名債権ノ譲渡ノ効力ヲ有ス但シ期間経過後ノ裏書ハ指名債権ノ譲渡ノ効力ノミヲ有ス", "")
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第十五条第二項ただし書中「受けたため、」を「受けた場合」に改める。
　　第二十条第一項ただし書中「指名債権」を「債権」に改める。",
    );
    let x = xml(&got);
    assert!(x.contains("ただし、命令を受けた場合給水を停止"), "{x}");
    assert!(
        x.contains("裏書ハ指名債権ノ譲渡ノ効力ヲ有ス但シ期間経過後ノ裏書ハ債権ノ譲渡"),
        "{x}"
    );
}

/// 号を表の行で書いた項（「一　選挙長｜一日につき｜一万六百円」）の「第一号中」、項の列記（List）の中の字句
#[test]
fn items_written_as_table_rows_and_lists() {
    let rows = table(&[
        &["一　選挙長", "一日につき", "一万六百円"],
        &["二　投票管理者", "一日につき", "一万六百円"],
    ]);
    let list = r#"<List><ListSentence><Sentence Num="1">刑事局</Sentence></ListSentence></List><List><ListSentence><Sentence Num="1">情報通信局</Sentence></ListSentence></List>"#;
    let doc = law(
        "",
        &format!(
            r#"<Article Num="14"><ArticleTitle>第十四条</ArticleTitle>{}</Article><Article Num="19"><ArticleTitle>第十九条</ArticleTitle>{}</Article>"#,
            para(1, "", "費用の額は、次に掲げるとおりとする。", &rows),
            para(1, "", "警察庁に、次の局を置く。", list)
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第十四条第一項第一号中「一万六百円」を「一万八百円」に改める。
　　第十九条第一項中「情報通信局」を「サイバー警察局」に改める。",
    );
    let x = xml(&got);
    assert_eq!(x.matches("一万八百円").count(), 1, "{x}");
    assert_eq!(x.matches("一万六百円").count(), 1, "{x}");
    assert!(x.contains("サイバー警察局"), "{x}");
}

/// 「「都道府県公安委員会（以下「」及び「」という。）」を削り」: 定義の語を挟む二つの字句を削り、語を残す
#[test]
fn deleting_the_phrases_around_a_defined_term() {
    let doc = law(
        "",
        &format!(
            r#"<Article Num="25"><ArticleTitle>第二十五条</ArticleTitle>{}</Article>"#,
            para(
                1,
                "",
                "都道府県公安委員会（以下「公安委員会」という。）は、公安委員会規則で定める。",
                ""
            )
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    );
    let got = apply(
        &doc,
        "　　第二十五条中「都道府県公安委員会（以下「」及び「」という。）」を削る。",
    );
    let x = xml(&got);
    assert!(x.contains("公安委員会は、公安委員会規則で定める。"), "{x}");
}

/// 「附則第二項から第三項までを削る。」: 範囲の項も附則で引く（本則の仮の条を見ていた）
#[test]
fn deleting_a_range_of_suppl_paragraphs() {
    let doc = law(
        "",
        &format!(
            r#"<Article Num="1"><ArticleTitle>第一条</ArticleTitle>{}</Article>"#,
            para(1, "", "甲とする。", "")
        ),
        &[
            para(1, "", "この法律は、公布の日から施行する。", ""),
            para(2, "", "乙の特例。", ""),
            para(3, "", "丙の特例。", ""),
            para(4, "", "丁の特例。", ""),
        ]
        .concat(),
    );
    let got = apply(&doc, "　　附則第二項から第三項までを削る。");
    let x = xml(&got);
    assert!(!x.contains("乙の特例") && !x.contains("丙の特例"), "{x}");
    assert!(x.contains("丁の特例"), "{x}");
}

fn subitems_law() -> LegalDocument {
    let item = r#"<Item Num="1"><ItemTitle>一</ItemTitle><ItemSentence><Sentence Num="1">次に掲げる業務</Sentence></ItemSentence><Subitem1 Num="1"><Subitem1Title>イ</Subitem1Title><Subitem1Sentence><Sentence Num="1">イの業務</Sentence></Subitem1Sentence></Subitem1><Subitem1 Num="2"><Subitem1Title>ロ</Subitem1Title><Subitem1Sentence><Sentence Num="1">次に掲げる者</Sentence></Subitem1Sentence><Subitem2 Num="1"><Subitem2Title>（１）</Subitem2Title><Subitem2Sentence><Sentence Num="1">旧の(1)</Sentence></Subitem2Sentence></Subitem2><Subitem2 Num="2"><Subitem2Title>（２）</Subitem2Title><Subitem2Sentence><Sentence Num="1">旧の(2)</Sentence></Subitem2Sentence></Subitem2></Subitem1><Subitem1 Num="3"><Subitem1Title>ハ</Subitem1Title><Subitem1Sentence><Sentence Num="1">ハの業務</Sentence></Subitem1Sentence></Subitem1></Item><Item Num="2"><ItemTitle>二</ItemTitle><ItemSentence><Sentence Num="1">第二号の業務</Sentence></ItemSentence></Item>"#;
    law(
        "",
        &format!(
            r#"<Article Num="10"><ArticleTitle>第十条</ArticleTitle>{}</Article>"#,
            para(1, "", "機構は、次の業務を行う。", item)
        ),
        &para(1, "", "この法律は、公布の日から施行する。", ""),
    )
}

/// 「第一号ハを削る。」: 号の下の細目だけを削る（号の全部を削っていた）
#[test]
fn deleting_a_subitem_keeps_the_item() {
    let got = apply(&subitems_law(), "　　第十条第一号ハを削る。");
    let x = xml(&got);
    assert!(!x.contains("ハの業務"), "{x}");
    assert!(
        x.contains("イの業務") && x.contains("次に掲げる業務"),
        "{x}"
    );
}

/// 「同号ロ(1)及び(2)を次のように改める。」「同号ロ(2)を次のように改める。」「同号ロに次のように加える。」+「(3)　…」
#[test]
fn editing_parenthesized_subitems() {
    let got = apply(
        &subitems_law(),
        "　　第十条第一号ロ(1)及び(2)を次のように改める。
　　　　(1)　新の第一
　　　　(2)　新の第二",
    );
    let x = xml(&got);
    assert!(
        x.contains("新の第一") && x.contains("新の第二") && !x.contains("旧の"),
        "{x}"
    );
    let got = apply(
        &subitems_law(),
        "　　第十条第一号ロ(2)を次のように改める。
　　　　(2)　新の第二
　　第十条第一号ロに次のように加える。
　　　　(3)　加えた第三",
    );
    let x = xml(&got);
    assert!(
        x.contains("旧の(1)") && x.contains("新の第二") && !x.contains("旧の(2)"),
        "{x}"
    );
    assert!(x.contains("加えた第三"), "{x}");
    assert!(x.contains(r#"<Subitem2 Num="3">"#), "{x}");
}
