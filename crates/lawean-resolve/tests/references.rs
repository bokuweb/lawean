//! 借地借家法の全文（本則 + 原始附則）の参照表現を認識・解決する。

use lawean_resolve::*;
use lawean_source::*;

fn doc() -> LegalDocument {
    let path = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(path).unwrap()).unwrap()
}

type Para = Vec<(StableId, String)>;

/// 本則と原始附則の全文。項ごと（号の文も同じ項に含める）
fn paragraphs(doc: &LegalDocument) -> Vec<Para> {
    let mut out = Vec::new();
    fn para(p: &Paragraph, out: &mut Vec<Para>) {
        let mut cur = Vec::new();
        for s in &p.sentences {
            cur.push((s.stable_id.clone(), s.plain_text()));
        }
        for c in &p.children {
            if let ParagraphChild::Item(i) = c {
                item(i, &mut cur);
            }
        }
        out.push(cur);
    }
    fn item(i: &Item, out: &mut Para) {
        match &i.body {
            ItemBody::Sentences(ss) => ss
                .iter()
                .for_each(|s| out.push((s.stable_id.clone(), s.plain_text()))),
            ItemBody::Columns(cs) => cs.iter().for_each(|c| {
                c.sentences
                    .iter()
                    .for_each(|s| out.push((s.stable_id.clone(), s.plain_text())))
            }),
            _ => {}
        }
        for c in &i.children {
            if let ItemChild::Subitem(s) = c {
                item(s, out);
            }
        }
    }
    fn prov(p: &Provision, out: &mut Vec<Para>) {
        match p {
            Provision::Container(c) => c.children.iter().for_each(|c| prov(c, out)),
            Provision::Article(a) => a.children.iter().for_each(|c| {
                if let ArticleChild::Paragraph(p) = c {
                    para(p, out)
                }
            }),
            Provision::Paragraph(p) => para(p, out),
            Provision::Raw(_) => {}
        }
    }
    doc.main_provision.iter().for_each(|p| prov(p, &mut out));
    for c in &doc.suppl_provisions[0].children {
        match c {
            SupplChild::Provision(p) => prov(p, &mut out),
            SupplChild::Paragraph(p) => para(p, &mut out),
            SupplChild::Raw(_) => {}
        }
    }
    out
}

#[test]
fn all_references_in_target_law_resolve() {
    let doc = doc();
    let index = Index::build(&doc);
    let mut total = 0;
    let mut external = 0;
    let mut inferred = 0;
    let mut failures = Vec::new();
    let known = doc.stable_ids();
    for para in paragraphs(&doc) {
        for r in resolve_paragraph(&index, &para) {
            total += 1;
            match &r.resolution {
                Resolution::Internal(ids) => {
                    assert!(!ids.is_empty(), "{}: empty", r.span.text);
                    for t in ids {
                        assert!(
                            known.contains(&t),
                            "{}: resolved to unknown {t}",
                            r.span.text
                        );
                    }
                }
                Resolution::External { inferred: true, .. } => {
                    external += 1;
                    inferred += 1;
                }
                Resolution::External { .. } => external += 1,
                Resolution::Unresolved(u) => failures.push(format!("「{}」 {u:?}", r.span.text)),
            }
        }
    }
    eprintln!(
        "references: {total} total, {external} external ({inferred} inferred), {} unresolved",
        failures.len()
    );
    for f in &failures {
        eprintln!("  {f}");
    }
    assert!(
        total > 200,
        "expected a few hundred references, got {total}"
    );
    assert!(failures.is_empty(), "{} unresolved", failures.len());
}

fn one(index: &Index, from: &str, text: &str) -> Vec<String> {
    let from = StableId(format!("403AC0000000090/{from}"));
    let v = resolve_sentence(index, &from, text);
    assert_eq!(v.len(), 1, "{text}: {v:?}");
    match &v[0].resolution {
        Resolution::Internal(ids) => ids
            .iter()
            .map(|i| i.0.trim_start_matches("403AC0000000090/").to_string())
            .collect(),
        other => panic!("{text}: {other:?}"),
    }
}

#[test]
fn specific_resolutions() {
    let doc = doc();
    let index = Index::build(&doc);
    // 第5条第2項「前項」→ 第5条第1項
    assert_eq!(
        one(
            &index,
            "main/chap:2/sec:1/art:5/para:2/sent:1",
            "前項と同様とする"
        ),
        ["main/chap:2/sec:1/art:5/para:1"]
    );
    // 第5条「前条」→ 第4条
    assert_eq!(
        one(
            &index,
            "main/chap:2/sec:1/art:5/para:1/sent:1",
            "前条の規定"
        ),
        ["main/chap:2/sec:1/art:4"]
    );
    // 第22条第2項「前項前段」→ 第22条第1項第1文
    assert_eq!(
        one(
            &index,
            "main/chap:2/sec:4/art:22/para:2/sent:1",
            "前項前段の特約"
        ),
        ["main/chap:2/sec:4/art:22/para:1/sent:1"]
    );
    // 第13条第3項「前二項」→ 第1項・第2項
    assert_eq!(
        one(
            &index,
            "main/chap:2/sec:2/art:13/para:3/sent:1",
            "前二項の規定は"
        ),
        [
            "main/chap:2/sec:2/art:13/para:1",
            "main/chap:2/sec:2/art:13/para:2"
        ]
    );
    // 第22条「次条第一項」→ 第23条第1項
    assert_eq!(
        one(
            &index,
            "main/chap:2/sec:4/art:22/para:1/sent:1",
            "次条第一項において同じ"
        ),
        ["main/chap:2/sec:4/art:23/para:1"]
    );
    // 第22条「第三十八条第二項」→ 別の章の条
    assert_eq!(
        one(
            &index,
            "main/chap:2/sec:4/art:22/para:2/sent:1",
            "第三十八条第二項"
        ),
        ["main/chap:3/sec:3/art:38/para:2"]
    );
    // 附則第4条「附則第二条」→ 原始附則第2条
    assert_eq!(
        one(
            &index,
            "suppl:0/art:4/para:1/sent:2",
            "附則第二条の規定による"
        ),
        ["suppl:0/art:2"]
    );
    // 附則から本則の「第三十八条」
    assert_eq!(
        one(&index, "suppl:0/art:12/para:1/sent:1", "第三十八条の規定"),
        ["main/chap:3/sec:3/art:38"]
    );
    // 第2条の号: 「第一号」は同条第1項の号
    assert_eq!(
        one(
            &index,
            "main/chap:1/art:2/para:1/sent:1",
            "第二条第一項第三号"
        ),
        ["main/chap:1/art:2/para:1/item:3"]
    );
}

#[test]
fn antecedent_tracking() {
    let doc = doc();
    let index = Index::build(&doc);
    let from = StableId("403AC0000000090/main/chap:2/sec:1/art:5/para:1/sent:1".into());
    let v = resolve_sentence(
        &index,
        &from,
        "第三十八条第一項の規定による場合において、同項の規定にかかわらず、同条第三項の",
    );
    let ids: Vec<Vec<String>> = v
        .iter()
        .map(|r| match &r.resolution {
            Resolution::Internal(ids) => ids
                .iter()
                .map(|i| i.0.rsplit("art:").next().unwrap().to_string())
                .collect(),
            o => panic!("{o:?}"),
        })
        .collect();
    assert_eq!(
        ids,
        [vec!["38/para:1"], vec!["38/para:1"], vec!["38/para:3"]]
    );

    // 先行詞なしの「同条」は Unresolved
    let v = resolve_sentence(&index, &from, "同条の規定");
    assert!(matches!(
        v[0].resolution,
        Resolution::Unresolved(Unresolved::NoAntecedent)
    ));
}

#[test]
fn external_list_and_same_law_antecedent() {
    let doc = doc();
    let index = Index::build(&doc);
    // 第42条: 他法令の条の列挙
    let from = StableId("403AC0000000090/main/chap:4/art:42/para:1/sent:1".into());
    let v = resolve_sentence(&index, &from, "非訟事件手続法（平成二十三年法律第五十一号）第二十七条、第四十条、第四十二条の二及び第六十三条第一項後段の規定は、適用しない。");
    assert_eq!(v.len(), 4);
    for r in &v {
        assert!(
            matches!(&r.resolution, Resolution::External { law, inferred: false, .. } if law == "非訟事件手続法"),
            "{r:?}"
        );
    }
    // 第20条第4項: 他法令の条を「同条」で受ける
    let from = StableId("403AC0000000090/main/chap:2/sec:3/art:20/para:4/sent:1".into());
    let v = resolve_sentence(&index, &from, "民事調停法（昭和二十六年法律第二百二十二号）第十九条の規定は、同条に規定する期間内に第一項の申立てをした場合に準用する。");
    assert!(
        matches!(&v[1].resolution, Resolution::External { law, .. } if law == "民事調停法"),
        "{:?}",
        v[1]
    );
    // 直後の「第一項」は他法令の列挙ではない（「同条に規定する期間内に」を挟む）ので、この条の第1項
    assert!(
        matches!(&v[2].resolution, Resolution::Internal(ids) if ids[0].0.ends_with("art:20/para:1")),
        "{:?}",
        v[2]
    );
    // 第7条第2項: 本文の「前項」をただし書きの「同項」が受ける
    let para = vec![
        (
            StableId("403AC0000000090/main/chap:2/sec:1/art:7/para:2/sent:1".into()),
            "前項の借地権設定者の承諾があったものとみなす。".to_string(),
        ),
        (
            StableId("403AC0000000090/main/chap:2/sec:1/art:7/para:2/sent:2".into()),
            "ただし、同項の規定により延長された場合".to_string(),
        ),
    ];
    let v = resolve_paragraph(&index, &para);
    assert!(
        matches!(&v[1].resolution, Resolution::Internal(ids) if ids[0].0.ends_with("art:7/para:1")),
        "{:?}",
        v[1]
    );
}

#[test]
fn external_references_are_marked() {
    let doc = doc();
    let index = Index::build(&doc);
    let from = StableId("403AC0000000090/main/chap:4/art:41/para:1/sent:1".into());
    let v = resolve_sentence(
        &index,
        &from,
        "民法（明治二十九年法律第八十九号）第六百四条の規定",
    );
    assert!(
        matches!(&v[0].resolution, Resolution::External { law, .. } if law == "民法"),
        "{v:?}"
    );
}
