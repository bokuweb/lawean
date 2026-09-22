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
                    // 定義の号は 2 欄（「借地権」「建物の所有を目的とする…」）
                    ItemBody::Columns(cs) => cs
                        .iter()
                        .flat_map(|c| c.sentences.iter())
                        .map(|s| s.plain_text())
                        .collect(),
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
    assert!(
        matches!(&u.instructions[1].ops[0], Op::AppendContainers { text, .. } if text.len() == 2)
    );
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

/// 令5-53・令3-37 の形: 「第一章第五節中第十七条の前に次の三条を加える」「第二編第二章に次の一節を加える」
/// 「第七条から第九条までを次のように改める」「第四章第四節を削る」「第百八十五条（見出しを含む。）中」
/// 「第九条の見出し中「A」の下に「B」を加え」— 借地借家法（第一章 総則、第二章 借地 第一節…）に当てる
#[test]
fn batch_a_forms_on_shakuchi_shakka() {
    let base = revision("403AC0000000090_20220518_503AC0000000037");
    let t = "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。
　　第二章第一節中第三条の前に次の一条を加える。
　第二条の二　甲は、乙とする。
　　第一章に次の一節を加える。
　　　　第三節　雑則
　第二条の三　丙は、丁とする。
　　第四条（見出しを含む。）中「更新」を「更改」に改める。
　　第五条の見出し中「契約」の下に「等」を加え、同条の見出し中「借地」を削る。";
    let units = parse_units(t).unwrap();
    let u = &units[0];
    assert!(
        matches!(&u.instructions[0].ops[0], Op::InsertArticleBefore { before, .. } if before.to_num_string() == "3")
    );
    assert!(
        matches!(&u.instructions[1].ops[0], Op::AppendContainers { path, .. } if path.len() == 1)
    );
    assert!(matches!(
        &u.instructions[2].ops[0],
        Op::ReplaceCaption { .. }
    ));
    assert!(matches!(&u.instructions[2].ops[1], Op::Replace { .. }));
    let got = apply_unit(&base, u, "test").unwrap();
    let snap = snapshot_main(&got);
    assert!(snap["2_2"][0].1.contains("甲は、乙とする"));
    assert!(snap["2_3"][0].1.contains("丙は、丁とする"));
    // 第4条の見出しと本文の「更新」→「更改」、第5条の見出し
    let cap = |n: &str| -> String {
        fn find<'a>(ps: &'a [Provision], n: &str) -> Option<&'a Article> {
            ps.iter().find_map(|p| match p {
                Provision::Article(a) if a.num.to_num_string() == n => Some(a),
                Provision::Container(c) => find(&c.children, n),
                _ => None,
            })
        }
        find(&got.main_provision, n)
            .and_then(|a| a.caption.as_ref())
            .map(|c| inline_text(c))
            .unwrap_or_default()
    };
    assert_eq!(cap("4"), "（借地権の更改後の期間）");
    assert!(snap["4"][0].1.contains("更改"), "{:?}", snap["4"]);
    assert_eq!(cap("5"), "（契約等の更新請求等）");
    // 第2条の2 は第二章第一節の第3条の前、第2条の3 は第一章の新しい第三節の中
    fn path_of(ps: &[Provision], n: &str, acc: &mut Vec<String>) -> bool {
        for p in ps {
            match p {
                Provision::Article(a) if a.num.to_num_string() == n => return true,
                Provision::Container(c) => {
                    acc.push(c.title.as_ref().map(|t| inline_text(t)).unwrap_or_default());
                    if path_of(&c.children, n, acc) {
                        return true;
                    }
                    acc.pop();
                }
                _ => {}
            }
        }
        false
    }
    let mut p = vec![];
    assert!(path_of(&got.main_provision, "2_2", &mut p));
    assert_eq!(p, vec!["第二章　借地", "第一節　借地権の存続期間等"]);
    let mut p = vec![];
    assert!(path_of(&got.main_provision, "2_3", &mut p));
    assert_eq!(p, vec!["第一章　総則", "第三節　雑則"]);
    // id の世界でも同じ本文。往復
    let b = ident::bind(&base, u, "t").unwrap();
    assert_eq!(snapshot_main(&b.doc), snap);
    let text = lawean_render::amend::render_unit(u);
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
            .collect::<Vec<_>>(),
        "{text}"
    );
}

/// 令5-53 の形: 条の中の読替え表の行「第六十四条の表第百三十三条第一項の項中「A」を「B」に改め」、
/// 「同項に次の表を加える」（欄の行）、「別表を削る」。借地借家法（2028 版、第64条に読替え表）と宅建業法（別表）に当てる
#[test]
fn table_rows_append_table_and_delete_appdx() {
    let base = revision("403AC0000000090_20280613_505AC0000000053");
    let t = "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。
　　第六十四条の表第百三十三条第一項の項中「当事者」を「関係人」に改め、同条の表第百三十三条第三項の項中「借地借家法第四十一条の事件の記録」を削る。
　　第三条に次の表を加える。
第一条
甲
乙
第二条
丙
丁";
    let units = parse_units(t).unwrap();
    let u = &units[0];
    assert_eq!(
        u.instructions[0].ops.len(),
        2,
        "{:?}",
        u.instructions[0].ops
    );
    assert!(
        matches!(&u.instructions[0].ops[0], Op::ReplaceTableRow { row, from, to, .. } if row == "第百三十三条第一項" && from == "当事者" && to == "関係人")
    );
    assert!(matches!(&u.instructions[1].ops[0], Op::AppendTable { .. }));
    let got = apply_unit(&base, u, "test").unwrap();
    // 表の中身（Raw の TableStruct）
    let tables = |d: &LegalDocument, n: &str| -> String {
        fn find<'a>(ps: &'a [Provision], n: &str) -> Option<&'a Article> {
            ps.iter().find_map(|p| match p {
                Provision::Article(a) if a.num.to_num_string() == n => Some(a),
                Provision::Container(c) => find(&c.children, n),
                _ => None,
            })
        }
        find(&d.main_provision, n)
            .unwrap()
            .children
            .iter()
            .filter_map(|c| match c {
                ArticleChild::Paragraph(p) => Some(p),
                _ => None,
            })
            .flat_map(|p| p.children.iter())
            .filter_map(|c| match c {
                ParagraphChild::Raw(e) if e.name == "TableStruct" => Some(e.text()),
                _ => None,
            })
            .collect::<String>()
            .split_whitespace()
            .collect()
    };
    let t64 = tables(&got, "64");
    // 第百三十三条第一項の項の「当事者」→「関係人」、第百三十三条第三項の項の下欄「借地借家法第四十一条の事件の記録」が消えた
    assert!(
        t64.starts_with("第百三十三条第一項関係人関係人又は"),
        "{t64}"
    );
    assert!(
        !t64.contains("以下この章において同じ。）借地借家法第四十一条の事件の記録"),
        "{t64}"
    );
    assert_eq!(tables(&got, "3"), "第一条甲乙第二条丙丁");
    let b = ident::bind(&base, u, "t").unwrap();
    assert_eq!(tables(&b.doc, "64"), t64);
    assert_eq!(tables(&b.doc, "3"), "第一条甲乙第二条丙丁");
    let text = lawean_render::amend::render_unit(u);
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
            .collect::<Vec<_>>(),
        "{text}"
    );

    let takken = revision("327AC1000000176_20240401_505AC0000000079");
    assert!(!takken.appendices.is_empty());
    let t = "第一条　宅地建物取引業法（昭和二十七年法律第百七十六号）の一部を次のように改正する。
　　別表を削る。";
    let u = parse_units(t).unwrap().remove(0);
    assert!(
        matches!(&u.instructions[0].ops[0], Op::DeleteAppdx { tables } if tables == &["別表".to_string()])
    );
    let got = apply_unit(&takken, &u, "test").unwrap();
    assert!(got.appendices.is_empty());
}

/// 令5-53 の形（1 項だけの条の前に項を置く）: 「第八条中「手数料」を「前項の手数料以外の手数料」に改め、同条を同条第二項とし、
/// 同条に第一項として次の一項を加える」— 旅館業法 第8条（1 項）に当てる
#[test]
fn make_the_sole_paragraph_the_second_and_prepend_the_first() {
    let base = revision("323AC0000000138_20231213_505AC0000000052");
    let before = snapshot_main(&base);
    assert_eq!(before["8"].len(), 1);
    let t = "第一条　旅館業法（昭和二十三年法律第百三十八号）の一部を次のように改正する。
　　第八条中「営業者」を「前項の営業者」に改め、同条を同条第二項とし、同条に第一項として次の一項を加える。
　　　甲は、乙とする。";
    let units = parse_units(t).unwrap();
    let u = &units[0];
    assert!(matches!(
        &u.instructions[0].ops[1],
        Op::RenumberParagraph {
            from: ParaRef::Num(1),
            to: 2,
            ..
        }
    ));
    assert!(matches!(
        &u.instructions[0].ops[2],
        Op::InsertParagraphFirst { .. }
    ));
    let got = apply_unit(&base, u, "test").unwrap();
    let snap = snapshot_main(&got);
    assert_eq!(snap["8"].len(), 2, "{:?}", snap["8"]);
    assert_eq!(snap["8"][0], (1, "甲は、乙とする。".to_string()));
    assert_eq!(snap["8"][1].0, 2);
    assert!(snap["8"][1].1.contains("前項の営業者"));
    let b = ident::bind(&base, u, "t").unwrap();
    assert_eq!(snapshot_main(&b.doc), snap);
    // id の世界: 新しい項は前の条（第7条の2）の最後の項の後ろに insertAfter
    assert!(b.ops.iter().any(|o| matches!(o, ident::IdentOp::InsertAfter { anchor, .. } if anchor.contains("/art:7_2/"))), "{:?}", b.ops);
    let text = lawean_render::amend::render_unit(u);
    let again = parse_units(&text).unwrap();
    assert_eq!(
        again[0].instructions[0].ops, u.instructions[0].ops,
        "{text}"
    );
}

/// 個人情報保護法・資金決済法の形（相対の容器と号の先頭への追加）:
/// 「第二章第一節の節名中「A」を「B」に改め、同節を同章第二節とし、同節の前に次の一節を加える」+ 節名、
/// 「同節第一款の次に次の一款を加える」、「同項第一号を同項第二号とし、同項に第一号として次の一号を加える」、
/// 「第N条の前の見出し中「A」を「B」に改める」
#[test]
fn relative_containers_and_first_item() {
    let base = revision("403AC0000000090_20220518_503AC0000000037");
    let t = "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。
　　第二章第一節の節名中「存続期間」を「期間」に改め、同節を同章第二節とし、同節の前に次の一節を加える。
　　　　第一節　通則
　第二条の二　甲は、乙とする。
　　第二条第五号を同条第六号とし、同条第四号を同条第五号とし、同条第三号を同条第四号とし、同条第二号を同条第三号とし、同条第一号を同条第二号とし、同条に第一号として次の一号を加える。
　　一　甲　乙をいう。
　　第三条の前の見出し中「存続期間」を「期間」に改める。";
    let units = parse_units(t).unwrap();
    let u = &units[0];
    let ops = &u.instructions[0].ops;
    assert!(matches!(&ops[0], Op::ReplaceContainerTitle { path, .. } if path.len() == 2));
    assert!(matches!(&ops[1], Op::RenumberContainer { path, to } if path.len() == 2 && to == "2"));
    // 「同節の前に」は番号を変えた後の節（第二章第二節）の前
    assert!(
        matches!(&ops[2], Op::InsertContainersBefore { path, .. } if path == &vec![
            (lawean_source::ContainerKind::Chapter, "2".to_string()),
            (lawean_source::ContainerKind::Section, "2".to_string()),
        ])
    );
    assert!(matches!(
        u.instructions[1].ops.last(),
        Some(Op::InsertItemFirst { .. })
    ));
    assert!(
        matches!(&u.instructions[2].ops[0], Op::ReplaceCaption { article, .. } if article.to_num_string() == "3")
    );
    let got = apply_unit(&base, u, "test").unwrap();
    // 新しい第一節（通則）に第2条の2、元の第一節は第二節に
    let sections: Vec<String> = {
        fn walk(ps: &[Provision], out: &mut Vec<String>) {
            for p in ps {
                if let Provision::Container(c) = p {
                    out.push(c.title.as_ref().map(|t| inline_text(t)).unwrap_or_default());
                    walk(&c.children, out);
                }
            }
        }
        let mut v = vec![];
        walk(&got.main_provision, &mut v);
        v
    };
    assert!(
        sections.contains(&"第一節　通則".to_string()),
        "{sections:?}"
    );
    assert!(
        sections.contains(&"第二節　借地権の期間等".to_string()),
        "{sections:?}"
    );
    let snap = snapshot_main(&got);
    assert!(snap["2_2"][0].1.contains("甲は、乙とする"));
    // 第2条の号: 新しい第一号が先頭、元の第一号は第二号
    let items: Vec<(String, String)> = items_of(&got, "2", 0)
        .iter()
        .map(|(n, t)| (n.clone(), t.chars().take(4).collect::<String>()))
        .collect();
    assert_eq!(
        items,
        vec![
            ("1".to_string(), "甲乙をい".to_string()),
            ("2".to_string(), "借地権建".to_string()),
            ("3".to_string(), "借地権者".to_string()),
            ("4".to_string(), "借地権設".to_string()),
            ("5".to_string(), "転借地権".to_string()),
            ("6".to_string(), "転借地権".to_string()),
        ],
        "{items:?}"
    );
    let b = ident::bind(&base, u, "t").unwrap();
    assert_eq!(snapshot_main(&b.doc), snap);
    let text = lawean_render::amend::render_unit(u);
    let again = parse_units(&text).unwrap();
    assert_eq!(
        again[0]
            .instructions
            .iter()
            .flat_map(|i| i.ops.clone())
            .collect::<Vec<_>>(),
        u.instructions
            .iter()
            .flat_map(|i| i.ops.clone())
            .collect::<Vec<_>>(),
        "{text}"
    );
}
