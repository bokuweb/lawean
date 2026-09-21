//! 実際の附則から施行期日を取り、e-Gov のリビジョンの施行日が許容区間に入ることを見る（借地借家法）

use lawean_extract::suppl::*;
use lawean_extract::temporal::{Dur, Enforcement, Unit};
use lawean_source::parse_response;

fn shakuchi() -> lawean_source::LegalDocument {
    let p = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn r3_37_art35_is_within_one_year_of_promulgation() {
    let doc = shakuchi();
    let spec = spec_for_law_id(&doc, "503AC0000000037").unwrap();
    assert_eq!(spec.promulgated, Some((2021, 5, 19)));
    // 本文は令和三年九月一日だが、第三十五条（借地借家法の改正）は第四号の「一年を超えない範囲内」
    assert_eq!(
        spec.main.as_ref().unwrap().enforcement,
        Some(Enforcement::Date {
            era: "令和".into(),
            y: 3,
            m: 9,
            d: 1
        })
    );
    let (clause, scope) = spec.for_article(35, Some(false)).unwrap();
    assert!(scope.unwrap().contains("第三十五条"));
    assert_eq!(
        clause.enforcement,
        Some(Enforcement::ByCabinetOrderWithin(Dur {
            n: 1,
            unit: Unit::Year
        }))
    );
    let (lo, hi) = admissible(
        spec.promulgated.unwrap(),
        clause.enforcement.as_ref().unwrap(),
    )
    .unwrap();
    // e-Gov 403AC0000000090_20220518: 上限の日ちょうどに施行された
    assert_eq!((lo, hi), ((2021, 5, 19), (2022, 5, 18)));
}

#[test]
fn r4_48_suppl_art73_is_within_nine_months_and_art74_within_four_years() {
    let doc = shakuchi();
    let spec = spec_for_law_id(&doc, "504AC0000000048").unwrap();
    assert_eq!(spec.promulgated, Some((2022, 5, 25)));
    // 借地借家法を改める第七十三条は本則に無く、附則第七十三条として第二号に挙がる
    assert!(spec.for_article(73, Some(false)).unwrap().1.is_none());
    let (clause, scope) = spec.for_article(73, None).unwrap();
    assert!(scope.unwrap().contains("附則第七十三条"));
    let (lo, hi) = admissible(
        spec.promulgated.unwrap(),
        clause.enforcement.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!((lo, hi), ((2022, 5, 25), (2023, 2, 24)));
    // e-Gov 403AC0000000090_20230220 は区間内
    assert!(lo <= (2023, 2, 20) && (2023, 2, 20) <= hi);
    // 第七十四条は号に無いので本文（四年以内）。e-Gov の現行 _20260521 は区間内
    let (clause, scope) = spec.for_article(74, None).unwrap();
    assert!(scope.is_none());
    let (lo, hi) = admissible(
        spec.promulgated.unwrap(),
        clause.enforcement.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!((lo, hi), ((2022, 5, 25), (2026, 5, 24)));
    assert!(lo <= (2026, 5, 21) && (2026, 5, 21) <= hi);
}

#[test]
fn every_suppl_provision_yields_a_spec() {
    let doc = shakuchi();
    let specs = enforcement_specs(&doc);
    assert!(specs.len() >= 5);
    // 原始附則は法令自身の公布日
    assert_eq!(specs[0].amend_law_num, None);
    assert_eq!(specs[0].promulgated, Some((1991, 10, 4)));
    let unreadable: Vec<&str> = specs
        .iter()
        .filter_map(|s| s.main.as_ref())
        .filter(|m| m.enforcement.is_none())
        .map(|m| m.text.as_str())
        .collect();
    // 読めないのは他法令の施行日に依るものだけ
    for u in &unreadable {
        assert!(u.contains("の施行の日から施行する"), "{u}");
    }
}

/// 単独法の改正法（宅建業法 平成28年法律第56号）の附則: ただし書きが被改正法の条（「第三十四条の二第一項の改正規定」）で
/// 範囲を書く。本文は一年以内（e-Gov 2017-04-01）、ただし書きの条は二年以内（e-Gov 2018-04-01）
#[test]
fn target_side_scopes_in_a_single_law_amendment() {
    let p = format!(
        "{}/../../fixtures/laws/327AC1000000176.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    let doc = parse_response(&std::fs::read_to_string(p).unwrap()).unwrap();
    let spec = spec_for_law_id(&doc, "428AC0000000056").unwrap();
    let p = spec.promulgated.unwrap();
    assert_eq!(p, (2016, 6, 3));
    // 第三十四条の二を触る単位 → ただし書き（二年以内）
    let (clause, scope) = spec.for_target_articles(&["34_2".to_string()]).unwrap();
    assert!(scope.unwrap().contains("第三十四条の二第一項の改正規定"));
    let (lo, hi) = admissible(p, clause.enforcement.as_ref().unwrap()).unwrap();
    assert_eq!((lo, hi), ((2016, 6, 3), (2018, 6, 2)));
    assert!(lo <= (2018, 4, 1) && (2018, 4, 1) <= hi);
    // 第三条を触る単位 → 本文（一年以内）
    let (clause, scope) = spec.for_target_articles(&["3".to_string()]).unwrap();
    assert!(scope.is_none());
    let (lo, hi) = admissible(p, clause.enforcement.as_ref().unwrap()).unwrap();
    assert_eq!((lo, hi), ((2016, 6, 3), (2017, 6, 2)));
    assert!(lo <= (2017, 4, 1) && (2017, 4, 1) <= hi);
}
