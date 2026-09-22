//! identity patch（ADR-0013）を実データで: 番号で書かれた改め文を発射台に束縛して id ベースの操作にし、
//! Lean の `applyUnit` の写し（`ident::apply_unit`）で当てた結果が e-Gov のリビジョンと一致することを確かめる。
//! 同じデータを `lawean-lean` が Lean に出力し、`lean/Lawean/Consolidate.lean` が `native_decide` で同じことを検査する。

use lawean_amend::ident::*;
use lawean_amend::{parse_units, AmendUnit};
use lawean_source::*;

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn revision(id: &str) -> LegalDocument {
    parse_response(&fixture(&format!("revisions/{id}.xml"))).unwrap()
}

fn current() -> LegalDocument {
    parse_response(&fixture("403AC0000000090.xml")).unwrap()
}

fn unit35() -> AmendUnit {
    parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0)
}

fn units73_74() -> (AmendUnit, AmendUnit) {
    let mut u = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let u74 = u.remove(1);
    (u.remove(0), u74)
}

fn assert_same_render(got: &IdentRevision, want: &IdentRevision) {
    let (g, w) = (got.render(), want.render());
    for i in 0..g.len().max(w.len()) {
        assert_eq!(
            g.get(i),
            w.get(i),
            "node #{i}: {:?}",
            got.nodes.get(i).map(|n| &n.id)
        );
    }
}

/// 令和3年法律第37号 第35条: 繰り下げは束縛で消え、残るのは本文の Replace 2 つと項の挿入 4 つ
#[test]
fn reiwa3_act37_art35_binds_to_ident_ops() {
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let b = bind(&base, &unit35(), "503AC0000000037/art35").unwrap();
    let kinds: Vec<&str> = b
        .ops
        .iter()
        .map(|o| match o {
            IdentOp::Replace { .. } => "replace",
            IdentOp::InsertAfter { .. } => "insert",
            IdentOp::Delete { .. } => "delete",
            IdentOp::Resolve { .. } => "resolve",
            IdentOp::Renumber { .. } => "renumber",
        })
        .collect();
    assert_eq!(
        kinds,
        ["insert", "replace", "replace", "insert", "insert", "insert"]
    );
    // 「同条第三項中「前項」を「第三項」に改め」は、第38条第3項の id への本文全体の置換になる
    let IdentOp::Replace { id, expected, new } = &b.ops[1] else {
        panic!()
    };
    assert_eq!(id, "403AC0000000090/main/chap:3/sec:3/art:38/para:3");
    assert!(expected.contains("前項の規定による説明をしなかったとき"));
    assert!(new.contains("第三項の規定による説明をしなかったとき"));
    // 新しい id は改正法 ID と位置から
    let IdentOp::InsertAfter { anchor, new_id, .. } = &b.ops[3] else {
        panic!()
    };
    assert_eq!(anchor, "403AC0000000090/main/chap:3/sec:3/art:38/para:2");
    assert_eq!(new_id, "503AC0000000037/art35/art:38/new:1");
}

/// 束縛した操作を Lean と同じ apply で当てると e-Gov の改正後リビジョンと（id を捨てて）一致する
#[test]
fn reiwa3_act37_art35_consolidates_via_ident_apply() {
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let b = bind(&base, &unit35(), "503AC0000000037/art35").unwrap();
    let got = apply_unit(&from_document(&base), &b.ops).unwrap();
    assert!(got.wf());
    assert!(!got.has_conflict());
    assert_same_render(
        &got,
        &from_document(&revision("403AC0000000090_20220518_503AC0000000037")),
    );
    // 束縛の途中で作った Source IR とも一致する（Rust の 2 つの経路が同じ結果）
    assert_eq!(got, from_document(&b.doc));
    // 番号は描画時に数える: 旧第3項は第5項になっている
    assert_eq!(
        got.para_num("403AC0000000090/main/chap:3/sec:3/art:38/para:3"),
        Some(5)
    );
}

/// 令和4年法律第48号 第73条 → 第74条を、第73条が作った id を保ったまま連ねる
#[test]
fn reiwa4_act48_stages_chain_through_created_ids() {
    let base = revision("403AC0000000090_20220525_504AC0000000048");
    let (u73, u74) = units73_74();
    let b73 = bind(&base, &u73, "504AC0000000048/art73").unwrap();
    let after73 = apply_unit(&from_document(&base), &b73.ops).unwrap();
    assert_same_render(
        &after73,
        &from_document(&revision("403AC0000000090_20230220_504AC0000000048")),
    );
    // 第74条は第73条の作った id（第61条第1項）に anchor して新しい項を入れ、旧 id を削る
    let b74 = bind(&b73.doc, &u74, "504AC0000000048/art74").unwrap();
    let [IdentOp::InsertAfter {
        anchor,
        new_id,
        art,
        text,
    }, IdentOp::Delete { id }] = b74.ops.as_slice()
    else {
        panic!("{:?}", b74.ops)
    };
    assert_eq!(anchor, "504AC0000000048/art73/art:61/new:1");
    assert_eq!(new_id, "504AC0000000048/art74/art:61/new:1");
    assert_eq!(art, "61");
    assert!(text.contains("第百三十三条の二第五項及び第六項並びに第百三十三条の三第二項を除く"));
    assert_eq!(id, "504AC0000000048/art73/art:61/new:1");
    let after74 = apply_unit(&after73, &b74.ops).unwrap();
    assert_same_render(&after74, &from_document(&current()));

    // 依存と施行順序
    assert!(depends_on(&b74.ops, &b73.ops));
    assert!(!depends_on(&b73.ops, &b74.ops));
    assert!(schedule_ok(&[&b73.ops, &b74.ops]));
    assert!(!schedule_ok(&[&b74.ops, &b73.ops]));
    // 違反した順序で無理に当てると、依存先の id が無くて失敗する
    assert_eq!(apply_unit(&from_document(&base), &b74.ops), None);
}

/// 令和3年 第35条と令和4年 第73条は独立（触る id が交わらない）なので、どちらの順でも同じ
#[test]
fn independent_units_commute_on_real_data() {
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let (u73, _) = units73_74();
    let b35 = bind(&base, &unit35(), "503AC0000000037/art35").unwrap();
    let b73 = bind(&base, &u73, "504AC0000000048/art73").unwrap();
    assert!(independent_units(&b35.ops, &b73.ops));
    let r0 = from_document(&base);
    let ab = apply_unit(&apply_unit(&r0, &b35.ops).unwrap(), &b73.ops);
    let ba = apply_unit(&apply_unit(&r0, &b73.ops).unwrap(), &b35.ops);
    assert_eq!(ab, ba);
    assert!(ab.is_some());
}

/// 発射台がずれても失敗ではなく衝突: 改正済みの版に同じ操作を当てると、挿入は同じ項に当たり、
/// 本文の置換は期待した本文と違うので `conflicts` に残る
#[test]
fn wrong_base_yields_conflicts_not_failure() {
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let b = bind(&base, &unit35(), "503AC0000000037/art35").unwrap();
    let already = from_document(&revision("403AC0000000090_20220518_503AC0000000037"));
    let got = apply_unit(&already, &b.ops).unwrap();
    assert!(got.has_conflict());
    let ids: Vec<&str> = got.conflicts().iter().map(|c| c.0).collect();
    assert_eq!(
        ids,
        [
            "403AC0000000090/main/chap:3/sec:3/art:38/para:2",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:3"
        ]
    );
}

/// 改正法が振った id と e-Gov の id の対応: 令3-37 第35条が触った項を e-Gov の id で言える
#[test]
fn touched_paragraphs_in_egov_ids() {
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let after = revision("403AC0000000090_20220518_503AC0000000037");
    let b = bind(&base, &unit35(), "503AC0000000037/art35").unwrap();
    let touched = touched_egov_ids(&from_document(&base), &b.ops, &from_document(&after)).unwrap();
    assert_eq!(
        touched,
        [
            "403AC0000000090/main/chap:2/sec:4/art:22/para:1",
            "403AC0000000090/main/chap:2/sec:4/art:22/para:2",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:5",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:3",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:4",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:1",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:2",
            "403AC0000000090/main/chap:3/sec:3/art:39/para:2",
            "403AC0000000090/main/chap:3/sec:3/art:39/para:3",
        ]
    );
    // anchor を除いた「本文が変わった・できた項」
    let modified =
        modified_egov_ids(&from_document(&base), &b.ops, &from_document(&after)).unwrap();
    assert_eq!(
        modified,
        [
            "403AC0000000090/main/chap:2/sec:4/art:22/para:2",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:5",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:3",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:4",
            "403AC0000000090/main/chap:3/sec:3/art:38/para:2",
            "403AC0000000090/main/chap:3/sec:3/art:39/para:3",
        ]
    );
}

/// 改め文の無い先行改正を、2 つのリビジョンの差から id の操作列にする（docs/13）。
/// 当てた結果が施行時の版と一致し、令4-68 第98条（古物営業法）と独立 = 可換
#[test]
fn derived_unit_reproduces_revision_and_is_independent_of_the_real_unit() {
    use lawean_amend::ident::{
        apply_unit as apply_ident, bind, derive_unit, from_document, independent_units,
    };
    let rev =
        |id: &str| lawean_source::parse_response(&fixture(&format!("revisions/{id}.xml"))).unwrap();
    let draft = rev("324AC0000000108_20220617_504AC0000000068");
    let enf = rev("324AC0000000108_20240401_505AC0000000063");
    let after = rev("324AC0000000108_20250601_504AC0000000068");
    let (rd, re, ra) = (
        from_document(&draft),
        from_document(&enf),
        from_document(&after),
    );
    let b = derive_unit(&rd, &re, "derived");
    assert!(!b.is_empty());
    assert_eq!(apply_ident(&rd, &b).unwrap().render(), re.render());
    // 令4-68 第98条を起草時の版に束縛したものと独立。どちらの順でも e-Gov の 2025-06-01 版になる
    let units =
        lawean_amend::parse_units(&fixture("amendments/504AC0000000068_5laws.txt")).unwrap();
    let u98 = units
        .iter()
        .find(|u| u.target_title == "古物営業法")
        .unwrap();
    let x = bind(&draft, u98, "504AC0000000068/art98").unwrap().ops;
    assert!(independent_units(&x, &b));
    let mut xb = x.clone();
    xb.extend(b.iter().cloned());
    let mut bx = b.clone();
    bx.extend(x.iter().cloned());
    assert_eq!(apply_ident(&rd, &xb).unwrap().render(), ra.render());
    assert_eq!(apply_ident(&rd, &bx).unwrap().render(), ra.render());
}
