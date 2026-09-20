//! 改め文の往復: parse → render → parse が同じ操作列になる（実データ 2 件）。
//! Semantic IR の日本語化: 手書き 8 条分がすべて描画でき、代表例が意図した文になる。

use lawean_amend::parse_units;
use lawean_render::*;
use lawean_resolve::ResolvedModel;
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_source::parse_response;

fn fixture(rel: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../fixtures/{rel}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

#[test]
fn amendment_text_roundtrips_through_ops() {
    for f in [
        "amendments/503AC0000000037_art35.txt",
        "amendments/504AC0000000048_art73-74.txt",
    ] {
        let units = parse_units(&fixture(f)).unwrap();
        let text = render_units(&units);
        let again = parse_units(&text).unwrap_or_else(|e| panic!("{f}: {e}\n{text}"));
        // 文面は同条・同項を明示番号に正規化するので変わる。操作列と条文内容が一致すればよい
        let ops = |us: &[lawean_amend::AmendUnit]| -> Vec<Vec<Vec<lawean_amend::Op>>> {
            us.iter()
                .map(|u| u.instructions.iter().map(|i| i.ops.clone()).collect())
                .collect()
        };
        assert_eq!(ops(&units), ops(&again), "{f}\n{text}");
        assert_eq!(
            units
                .iter()
                .map(|u| (&u.article_of_amending_law, &u.target_title))
                .collect::<Vec<_>>(),
            again
                .iter()
                .map(|u| (&u.article_of_amending_law, &u.target_title))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn rendered_amendment_reads_like_the_original() {
    let units = parse_units(&fixture("amendments/503AC0000000037_art35.txt")).unwrap();
    let text = render_units(&units);
    assert!(
        text.starts_with("第三十五条\u{3000}借地借家法の一部を次のように改正する。\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "\u{3000}\u{3000}第二十二条に次の一項を加える。\n\u{3000}２\u{3000}前項前段の特約"
        ),
        "{text}"
    );
    // 複合文: 同条・同項は明示の番号に正規化される
    assert!(text.contains("第三十八条中第七項を第九項とし、第三十八条中第四項から第六項までを二項ずつ繰り下げ、第三十八条第三項中「前項」を「第三項」に改め"), "{text}");
}

#[test]
fn semantic_rules_render_to_japanese() {
    let m = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&m);
    let r3_2 = m.rules.iter().find(|r| r.id.0 == "R3-2").unwrap();
    assert_eq!(
        render_rule(r3_2, &rm),
        "【R3-2】借地権について、契約で定めた期間が三十年を超えるとき、存続期間を契約で定めた期間とする。（第3条第1項ただし書；〔R3-1〕に優先）〔人手:bokuweb、確度 High〕"
    );
    let r5_1 = m.rules.iter().find(|r| r.id.0 == "R5-1").unwrap();
    let s = render_rule(r5_1, &rm);
    assert!(s.starts_with("【R5-1】借地権者について、存続期間が満了する、かつ、更新を請求した（by=借地権者）、かつ、建物があるとき、契約を更新した（conditions=従前と同一）ものとみなす。（第5条第1項本文；〔R5-1-proviso〕が優先）"), "{s}");

    // 原文と並べた全体表示
    let doc = parse_response(&fixture("403AC0000000090.xml")).unwrap();
    let all = render_model(&m, Some(&doc));
    assert!(all.contains("原文（第3条第1項ただし書）: ただし、契約でこれより長い期間を定めたときは、その期間とする。"), "{all}");
    for r in &m.rules {
        assert!(all.contains(&format!("【{}】", r.id.0)));
    }
}
