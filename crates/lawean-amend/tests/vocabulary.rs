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
