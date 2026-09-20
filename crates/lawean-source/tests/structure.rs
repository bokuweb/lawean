//! docs/02-source-ir.md・docs/01-target-law.md に書いた事実を型付き IR から検証する。
//! 往復テストが「全部 Raw で逃げている」せいで通っていないことの確認も兼ねる。

use lawean_source::xml::Element;
use lawean_source::*;

fn load(id: &str) -> LegalDocument {
    let path = format!("{}/../../fixtures/{id}.xml", env!("CARGO_MANIFEST_DIR"));
    parse_response(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn articles<'a>(ps: &'a [Provision], out: &mut Vec<&'a Article>) {
    for p in ps {
        match p {
            Provision::Article(a) => out.push(a),
            Provision::Container(c) => articles(&c.children, out),
            Provision::Paragraph(_) | Provision::Raw(_) => {}
        }
    }
}

fn find_article(doc: &LegalDocument, base: u32) -> &Article {
    let mut v = Vec::new();
    articles(&doc.main_provision, &mut v);
    v.into_iter()
        .find(|a| matches!(&a.num, ArticleNum::Single { base: b, branch } if *b == base && branch.is_empty()))
        .unwrap_or_else(|| panic!("art {base}"))
}

fn paragraphs(a: &Article) -> Vec<&Paragraph> {
    a.children
        .iter()
        .filter_map(|c| match c {
            ArticleChild::Paragraph(p) => Some(p),
            _ => None,
        })
        .collect()
}

#[test]
fn shakuchi_shakuya_structure() {
    let doc = load("403AC0000000090");
    assert_eq!(doc.law_id.as_deref(), Some("403AC0000000090"));
    assert_eq!(
        doc.version_id.as_deref(),
        Some("403AC0000000090_20260521_504AC0000000048")
    );
    assert_eq!(doc.law_num.as_deref(), Some("平成三年法律第九十号"));
    assert_eq!(inline_text(&doc.title.as_ref().unwrap().text), "借地借家法");

    let mut arts = Vec::new();
    articles(&doc.main_provision, &mut arts);
    assert_eq!(arts.len(), 61, "本則の条数");
    assert_eq!(doc.suppl_provisions.len(), 9, "附則の本数");
    assert!(
        doc.suppl_provisions[0].amend_law_num.is_none(),
        "先頭は原始附則"
    );
    assert!(doc.suppl_provisions[0].extract);
    assert_eq!(
        doc.suppl_provisions[6].amend_law_num.as_deref(),
        Some("令和三年五月一九日法律第三七号")
    );

    // 第3条: 本文 + ただし書き
    let a3 = find_article(&doc, 3);
    assert_eq!(a3.stable_id.0, "403AC0000000090/main/chap:2/sec:1/art:3");
    assert_eq!(
        inline_text(a3.caption.as_ref().unwrap()),
        "（借地権の存続期間）"
    );
    let p = paragraphs(a3)[0];
    assert_eq!(p.sentences.len(), 2);
    assert_eq!(p.sentences[0].function, SentenceFunction::Main);
    assert_eq!(p.sentences[1].function, SentenceFunction::Proviso);
    assert_eq!(
        p.sentences[1].stable_id.0,
        "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:2"
    );
    assert!(p.sentences[1].plain_text().starts_with("ただし、"));
    assert_eq!(
        p.num_text.as_deref(),
        Some(&[][..]),
        "第1項の ParagraphNum は空要素"
    );

    // 第5条第2項: Function 属性なし
    let a5 = find_article(&doc, 5);
    let p2 = paragraphs(a5)[1];
    assert_eq!(p2.sentences[0].function, SentenceFunction::Unspecified);
    assert_eq!(inline_text(p2.num_text.as_ref().unwrap()), "２");

    // 第2条: 定義は Column ペア
    let a2 = find_article(&doc, 2);
    let items: Vec<&Item> = paragraphs(a2)[0]
        .children
        .iter()
        .filter_map(|c| match c {
            ParagraphChild::Item(i) => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(items.len(), 5);
    match &items[0].body {
        ItemBody::Columns(cols) => {
            assert_eq!(cols.len(), 2);
            assert_eq!(cols[0].sentences[0].plain_text(), "借地権");
            assert_eq!(
                cols[0].stable_id.0,
                "403AC0000000090/main/chap:1/art:2/para:1/item:1/col:1"
            );
        }
        other => panic!("expected Columns, got {other:?}"),
    }

    // 原始附則第4条: 本則と条番号が衝突するが stable_id は別
    let suppl = &doc.suppl_provisions[0];
    let s4 = suppl
        .children
        .iter()
        .find_map(|c| match c {
            SupplChild::Provision(Provision::Article(a))
                if a.num
                    == ArticleNum::Single {
                        base: 4,
                        branch: vec![],
                    } =>
            {
                Some(a)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(s4.stable_id.0, "403AC0000000090/suppl:0/art:4");
    assert_eq!(
        paragraphs(s4)[0].sentences[1].function,
        SentenceFunction::Proviso
    );

    // 改正法附則で Article を持たないものは Paragraph 直下
    assert!(
        doc.suppl_provisions[4]
            .children
            .iter()
            .any(|c| matches!(c, SupplChild::Paragraph(_))),
        "平成23年改正法附則"
    );
}

#[test]
fn minpo_covers_elements_absent_in_target_law() {
    let doc = load("129AC0000000089");
    let mut arts = Vec::new();
    articles(&doc.main_provision, &mut arts);

    assert!(
        arts.iter().any(|a| a.num
            == ArticleNum::Single {
                base: 121,
                branch: vec![2]
            }),
        "枝番 121_2"
    );
    assert!(
        arts.iter()
            .any(|a| matches!(&a.num, ArticleNum::Range { .. })),
        "削除範囲 155:157"
    );
    assert!(
        !arts.iter().any(|a| matches!(&a.num, ArticleNum::Other(_))),
        "未解釈の Num が無い"
    );
    assert!(
        find_article(&doc, 140).caption.is_none(),
        "第140条は見出しなし"
    );
    assert!(
        matches!(
            doc.main_provision[0],
            Provision::Container(Container {
                kind: ContainerKind::Part,
                ..
            })
        ),
        "編がある"
    );
    assert!(!doc.enact_statements.is_empty());

    let has_subitem = arts.iter().any(|a| {
        paragraphs(a).iter().any(|p| {
            p.children.iter().any(|c| matches!(c, ParagraphChild::Item(i) if i.children.iter().any(|s| matches!(s, ItemChild::Subitem(_)))))
        })
    });
    assert!(has_subitem, "Subitem1 がある");
    // ParagraphCaption は附則の Article 無し Paragraph に付く（「（施行期日）」）
    let has_para_caption = doc
        .suppl_provisions
        .iter()
        .flat_map(|s| s.children.iter())
        .any(|c| matches!(c, SupplChild::Paragraph(p) if p.caption.is_some()));
    assert!(has_para_caption, "ParagraphCaption がある");
}

/// 型付けされずに Raw で保持された要素を数える。増減を見るための指標。
#[test]
fn raw_coverage_report() {
    for id in ["403AC0000000090", "129AC0000000089"] {
        let doc = load(id);
        let mut raw: std::collections::BTreeMap<String, usize> = Default::default();
        let mut inline_raw: std::collections::BTreeMap<String, usize> = Default::default();
        collect_raw(&doc, &mut raw, &mut inline_raw);
        eprintln!("{id}: raw elements = {raw:?}");
        eprintln!("{id}: raw inlines  = {inline_raw:?}");
        // TOC / EnactStatement は意図的に raw。それ以外の raw は本則・附則の構造には無いはず
        for k in raw.keys() {
            assert!(
                matches!(k.as_str(), "TOC" | "EnactStatement"),
                "{id}: unexpected raw element <{k}>"
            );
        }
        for k in inline_raw.keys() {
            assert!(
                matches!(k.as_str(), "Ruby"),
                "{id}: unexpected raw inline <{k}>"
            );
        }
    }
}

fn collect_raw(
    doc: &LegalDocument,
    raw: &mut std::collections::BTreeMap<String, usize>,
    inline_raw: &mut std::collections::BTreeMap<String, usize>,
) {
    fn bump(m: &mut std::collections::BTreeMap<String, usize>, e: &Element) {
        *m.entry(e.name.clone()).or_default() += 1;
    }
    fn inl(v: &[Inline], m: &mut std::collections::BTreeMap<String, usize>) {
        for i in v {
            if let Inline::Raw(e) = i {
                bump(m, e);
            }
        }
    }
    fn sent(ss: &[Sentence], m: &mut std::collections::BTreeMap<String, usize>) {
        for s in ss {
            inl(&s.text, m);
        }
    }
    fn item(
        i: &Item,
        raw: &mut std::collections::BTreeMap<String, usize>,
        im: &mut std::collections::BTreeMap<String, usize>,
    ) {
        if let Some(t) = &i.title {
            inl(t, im);
        }
        match &i.body {
            ItemBody::Sentences(s) => sent(s, im),
            ItemBody::Columns(cs) => cs.iter().for_each(|c| sent(&c.sentences, im)),
            ItemBody::Mixed(e) => bump(raw, e),
            ItemBody::None => {}
        }
        for c in &i.children {
            match c {
                ItemChild::Subitem(s) => item(s, raw, im),
                ItemChild::Raw(e) => bump(raw, e),
            }
        }
    }
    fn para(
        p: &Paragraph,
        raw: &mut std::collections::BTreeMap<String, usize>,
        im: &mut std::collections::BTreeMap<String, usize>,
    ) {
        if let Some(c) = &p.caption {
            inl(c, im);
        }
        sent(&p.sentences, im);
        for c in &p.children {
            match c {
                ParagraphChild::Item(i) => item(i, raw, im),
                ParagraphChild::Raw(e) => bump(raw, e),
            }
        }
    }
    fn prov(
        p: &Provision,
        raw: &mut std::collections::BTreeMap<String, usize>,
        im: &mut std::collections::BTreeMap<String, usize>,
    ) {
        match p {
            Provision::Container(c) => {
                if let Some(t) = &c.title {
                    inl(t, im);
                }
                c.children.iter().for_each(|c| prov(c, raw, im));
            }
            Provision::Article(a) => {
                if let Some(c) = &a.caption {
                    inl(c, im);
                }
                for c in &a.children {
                    match c {
                        ArticleChild::Paragraph(p) => para(p, raw, im),
                        ArticleChild::Raw(e) => bump(raw, e),
                    }
                }
            }
            Provision::Paragraph(p) => para(p, raw, im),
            Provision::Raw(e) => bump(raw, e),
        }
    }
    if let Some(t) = &doc.toc {
        bump(raw, t);
    }
    if let Some(p) = &doc.preamble {
        bump(raw, p);
    }
    doc.enact_statements.iter().for_each(|e| bump(raw, e));
    doc.appendices.iter().for_each(|e| bump(raw, e));
    doc.law_extra.iter().for_each(|e| bump(raw, e));
    doc.main_provision
        .iter()
        .for_each(|p| prov(p, raw, inline_raw));
    for s in &doc.suppl_provisions {
        for c in &s.children {
            match c {
                SupplChild::Provision(p) => prov(p, raw, inline_raw),
                SupplChild::Paragraph(p) => para(p, raw, inline_raw),
                SupplChild::Raw(e) => bump(raw, e),
            }
        }
    }
}
