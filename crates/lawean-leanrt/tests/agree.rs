//! Lean の `applyUnit`（C 経由）と Rust の写しが同じ答えを出す — 実データ 3 件 + 失敗例

use lawean_amend::ident::{self, bind, from_document};
use lawean_amend::parse_units;
use lawean_source::{parse_response, LegalDocument};

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}
fn revision(id: &str) -> LegalDocument {
    parse_response(&fixture(&format!("revisions/{id}.xml"))).unwrap()
}

macro_rules! need_lean {
    () => {
        if !lawean_leanrt::available() {
            eprintln!("lean not linked; skipping");
            return;
        }
    };
}

#[test]
fn lean_and_rust_agree_on_r3_37_art35() {
    need_lean!();
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let unit = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let b = bind(&base, &unit, "503AC0000000037/art35").unwrap();
    let r0 = from_document(&base);
    let rust = ident::apply_unit(&r0, &b.ops).unwrap();
    let lean = lawean_leanrt::apply_unit(&r0, &b.ops).unwrap().unwrap();
    assert_eq!(lean, rust);
    assert_eq!(lawean_leanrt::check_unit(&r0, &b.ops), Ok(true));
    // e-Gov とも一致（Lean の結果を直接）
    let egov = from_document(&revision("403AC0000000090_20220518_503AC0000000037"));
    assert_eq!(lean.render(), egov.render());
}

#[test]
fn lean_reports_missing_target_and_conflicts() {
    need_lean!();
    let base = revision("403AC0000000090_20220518_503AC0000000037");
    let mut units = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let u74 = units.remove(1);
    let u73 = units.remove(0);
    let b73 = bind(&base, &u73, "504AC0000000048/art73").unwrap();
    let b74 = bind(&b73.doc, &u74, "504AC0000000048/art74").unwrap();
    let r0 = from_document(&base);
    // 第74条を先に: 第61条が無い
    assert_eq!(lawean_leanrt::apply_unit(&r0, &b74.ops), Ok(None));
    assert_eq!(lawean_leanrt::check_unit(&r0, &b74.ops), Ok(false));
    assert_eq!(
        lawean_leanrt::relation(&b73.ops, &b74.ops),
        Ok(lawean_leanrt::Relation::BDependsOnA)
    );
    // 順に当てれば現行と一致
    let mut both = b73.ops.clone();
    both.extend(b74.ops.clone());
    let lean = lawean_leanrt::apply_unit(&r0, &both).unwrap().unwrap();
    let current = from_document(&parse_response(&fixture("403AC0000000090.xml")).unwrap());
    assert_eq!(lean.render(), current.render());
    // 衝突: 改正済みの版に第35条をもう一度
    let base0 = revision("403AC0000000090_20210519_503AC0000000037");
    let u35 = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let b35 = bind(&base0, &u35, "503AC0000000037/art35").unwrap();
    let lean = lawean_leanrt::apply_unit(&r0, &b35.ops).unwrap().unwrap();
    let rust = ident::apply_unit(&r0, &b35.ops).unwrap();
    assert_eq!(lean, rust);
    assert!(lean.has_conflict());
    assert_eq!(lawean_leanrt::check_unit(&r0, &b35.ops), Ok(false));
    assert_eq!(
        lawean_leanrt::relation(&b35.ops, &b73.ops),
        Ok(lawean_leanrt::Relation::Independent)
    );
}

/// 公職選挙法（本則 1165 項、2MB）でも Lean 側で溶け込む
#[test]
fn lean_consolidates_koshoku_senkyo() {
    need_lean!();
    let base = revision("325AC1000000100_20201212_502AC1000000045");
    let unit = parse_units(&fixture("amendments/503AC0000000051.txt"))
        .unwrap()
        .remove(0);
    let b = bind(&base, &unit, "503AC0000000051").unwrap();
    let r0 = from_document(&base);
    let t = std::time::Instant::now();
    let lean = lawean_leanrt::apply_unit(&r0, &b.ops).unwrap().unwrap();
    eprintln!("lean applyUnit on 1167 paragraphs: {:?}", t.elapsed());
    let egov = from_document(&revision("325AC1000000100_20210602_503AC0000000051"));
    assert_eq!(lean.render(), egov.render());
}
