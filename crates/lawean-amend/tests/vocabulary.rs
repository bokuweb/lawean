//! 実際の改正法の語彙のうち、fixture の法令に当てて確かめる小さな形（自作の改め文を実際の法令に当てる）

use lawean_amend::*;
use lawean_source::*;

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn revision(id: &str) -> LegalDocument {
    parse_response(&fixture(&format!("revisions/{id}.xml"))).unwrap()
}

fn items_of(doc: &LegalDocument, art: &str, para: usize) -> Vec<(String, String)> {
    fn find<'a>(ps: &'a [Provision], n: &str) -> Option<&'a Article> {
        ps.iter().find_map(|p| match p {
            Provision::Article(a) if a.num.to_num_string() == n => Some(a),
            Provision::Container(c) => find(&c.children, n),
            _ => None,
        })
    }
    let a = find(&doc.main_provision, art).unwrap();
    let p = a
        .children
        .iter()
        .filter_map(|c| match c {
            ArticleChild::Paragraph(p) => Some(p),
            _ => None,
        })
        .nth(para)
        .unwrap();
    p.children
        .iter()
        .filter_map(|c| match c {
            ParagraphChild::Item(i) => Some((
                i.num.clone().unwrap_or_default(),
                match &i.body {
                    ItemBody::Sentences(ss) => ss.iter().map(|s| s.plain_text()).collect(),
                    _ => String::new(),
                },
            )),
            _ => None,
        })
        .collect()
}

/// 「第N条第M項第A号及び第B号を次のように改める」「第A号から第C号までを次のように改める」（令5-79 に 6 回）:
/// 挙げた号だけを差し替え、他の号はそのまま
#[test]
fn replace_a_set_of_items() {
    let base = revision("323AC0000000138_20231213_505AC0000000052");
    let before = items_of(&base, "5", 0);
    assert_eq!(before.len(), 4, "{before:?}");
    let t = "第一条　旅館業法（昭和二十三年法律第百三十八号）の一部を次のように改正する。
　　第五条第一項第一号及び第三号を次のように改める。
　　一　甲であるとき。
　　三　乙であるとき。
　　第五条第一項第二号から第四号までを次のように改める。
　　二　丙であるとき。
　　三　丁であるとき。
　　四　戊であるとき。";
    let units = parse_units(t).unwrap();
    let ops = &units[0].instructions[0].ops;
    assert!(
        matches!(&ops[0], Op::ReplaceItemSet { items, range: false, .. } if items == &["1".to_string(), "3".to_string()])
    );
    let ops = &units[0].instructions[1].ops;
    assert!(
        matches!(&ops[0], Op::ReplaceItemSet { items, range: true, .. } if items == &["2".to_string(), "3".to_string(), "4".to_string()])
    );
    let got = apply_unit(&base, &units[0], "test").unwrap();
    let after = items_of(&got, "5", 0);
    assert_eq!(
        after,
        vec![
            ("1".into(), "甲であるとき。".into()),
            ("2".into(), "丙であるとき。".into()),
            ("3".into(), "丁であるとき。".into()),
            ("4".into(), "戊であるとき。".into()),
        ]
    );
    // 往復
    let text = lawean_render::amend::render_unit(&units[0]);
    assert!(
        text.contains("第五条第一項第一号及び第三号を次のように改める。"),
        "{text}"
    );
    assert!(
        text.contains("第五条第一項第二号から第四号までを次のように改める。"),
        "{text}"
    );
    // id の世界でも同じ本文
    let b = ident::bind(&base, &units[0], "t").unwrap();
    assert_eq!(items_of(&b.doc, "5", 0), after);
}

/// 「附則に次の見出し及び二条を加える」（令3-49 第1条・第2条・第14条）: 原始附則の末尾に条を足す（文書の側だけ）。
/// 「本則に次の一章を加える」（令3-49）: 本則の末尾に章を足す。「同条第二項及び第三項を削り」（令5-79）: 位置の列挙の削除
#[test]
fn append_to_suppl_and_main_and_listed_delete() {
    let base = revision("323AC0000000138_20231213_505AC0000000052");
    let t = "第一条　旅館業法（昭和二十三年法律第百三十八号）の一部を次のように改正する。
　　第三条第五項及び第六項を削る。
　　本則に次の一章を加える。
　　　　第六章　雑則
　第十四条　甲は、乙とする。
　　附則に次の見出し及び二条を加える。
　　（罰則）
　第二十条　丙は、丁とする。
　第二十一条　戊は、己とする。";
    let units = parse_units(t).unwrap();
    let u = &units[0];
    assert_eq!(
        u.instructions[0].ops.len(),
        2,
        "{:?}",
        u.instructions[0].ops
    );
    assert!(matches!(&u.instructions[1].ops[0], Op::AppendContainers { text } if text.len() == 2));
    assert!(
        matches!(&u.instructions[2].ops[0], Op::AppendSupplArticles { text } if text.len() == 3)
    );
    let got = apply_unit(&base, u, "test").unwrap();
    // 本則: 第3条は第4項までになり、第六章 第14条が末尾に
    let snap = snapshot_main(&got);
    assert_eq!(snap["3"].len(), 4, "{:?}", snap["3"]);
    assert!(
        snap["14"][0].1.contains("甲は、乙とする"),
        "{:?}",
        snap.get("14")
    );
    assert!(
        matches!(got.main_provision.last(), Some(Provision::Container(c)) if c.children.len() == 1)
    );
    // 附則: 最後の 2 条が第20条・第21条、第20条に見出し
    let sp = got
        .suppl_provisions
        .iter()
        .find(|s| s.amend_law_num.is_none())
        .unwrap();
    let arts: Vec<&Article> = sp
        .children
        .iter()
        .filter_map(|c| match c {
            SupplChild::Provision(Provision::Article(a)) => Some(a),
            _ => None,
        })
        .collect();
    let n = arts.len();
    assert_eq!(arts[n - 2].num.to_num_string(), "20");
    assert_eq!(
        arts[n - 2]
            .caption
            .as_ref()
            .map(|c| inline_text(c))
            .as_deref(),
        Some("（罰則）")
    );
    assert_eq!(arts[n - 1].num.to_num_string(), "21");
    // id の世界でも本則は同じ、往復も
    let b = ident::bind(&base, u, "t").unwrap();
    assert_eq!(snapshot_main(&b.doc), snap);
    let text = lawean_render::amend::render_unit(u);
    assert!(
        text.contains("附則に次の見出し及び二条を加える。")
            && text.contains("本則に次の一章を加える。"),
        "{text}"
    );
    let again = parse_units(&text).unwrap();
    assert_eq!(
        again[0]
            .instructions
            .iter()
            .map(|i| i.ops.clone())
            .collect::<Vec<_>>(),
        u.instructions
            .iter()
            .map(|i| i.ops.clone())
            .collect::<Vec<_>>()
    );
}
