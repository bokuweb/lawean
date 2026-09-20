//! 借地借家法の全文に対する層 1 のカバレッジと、既知の文の分類。

use lawean_extract::*;
use lawean_semantic::*;
use lawean_source::*;
use std::collections::BTreeMap;

fn doc() -> LegalDocument {
    let path = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn find<'a>(sk: &'a [Skeleton], suffix: &str) -> &'a Skeleton {
    sk.iter()
        .find(|s| s.sentence.0.ends_with(suffix))
        .unwrap_or_else(|| panic!("{suffix}"))
}

#[test]
fn effect_kinds_cover_the_law() {
    let doc = doc();
    let sk = extract(&doc);
    let mut counts: BTreeMap<EffectKind, usize> = BTreeMap::new();
    for s in &sk {
        *counts.entry(s.effect).or_default() += 1;
    }
    let total = sk.len();
    let columns = sk.iter().filter(|s| s.in_column).count();
    eprintln!("sentences: {total}, columns: {columns}, effect kinds: {counts:?}");
    for s in sk
        .iter()
        .filter(|s| s.effect == EffectKind::Fragment && !s.in_column)
    {
        eprintln!("  fragment: {} {}", s.sentence, s.text);
    }
    // 定義欄以外の断片は、号に列挙された名詞句（「〜した者」「借地法（〜）」）だけのはず
    let unexplained = sk
        .iter()
        .filter(|s| s.effect == EffectKind::Fragment && !s.in_column)
        .count();
    assert!(unexplained <= 8, "{unexplained} non-column fragments");
    let classified = sk
        .iter()
        .filter(|s| !s.in_column && s.effect != EffectKind::Fragment)
        .count();
    let prose = total - columns;
    assert!(
        classified * 100 / prose >= 95,
        "classified {}% of prose sentences",
        classified * 100 / prose
    );
}

#[test]
fn known_sentences() {
    let doc = doc();
    let sk = extract(&doc);
    let s = find(&sk, "art:3/para:1/sent:1");
    assert_eq!(s.effect, EffectKind::Set);
    assert!(s.conditions.is_empty());
    let s = find(&sk, "art:3/para:1/sent:2");
    assert_eq!(s.function, SentenceFunction::Proviso);
    assert_eq!(s.conditions[0].text, "契約でこれより長い期間を定めた");

    let s = find(&sk, "art:5/para:1/sent:1");
    assert_eq!(s.effect, EffectKind::Deem);
    assert_eq!(s.conditions.len(), 3);
    assert!(
        matches!(&s.references[0].1, lawean_resolve::Resolution::Internal(ids) if ids[0].0.ends_with("art:4"))
    );

    // 第22条第1項前段: 「第九条及び第十六条の規定にかかわらず」
    let s = find(&sk, "art:22/para:1/sent:1");
    assert_eq!(s.effect, EffectKind::Can);
    match &s.overrides[0] {
        OverrideTarget::Provisions(ids) => {
            let v: Vec<&str> = ids
                .iter()
                .map(|i| i.0.rsplit('/').next().unwrap())
                .collect();
            assert_eq!(v, ["art:9", "art:16"]);
        }
        o => panic!("{o:?}"),
    }
    // 第32条: 契約への優先
    let s = find(&sk, "art:32/para:1/sent:1");
    assert!(matches!(s.overrides[0], OverrideTarget::Contract));
    assert_eq!(s.effect, EffectKind::Can);
    // 第10条: 「登記がなくても」は譲歩
    let s = find(&sk, "art:10/para:1/sent:1");
    assert_eq!(s.concessions[0].text, "その登記がなくても");
    assert_eq!(s.effect, EffectKind::Can);
    // 第9条: 無効
    assert_eq!(find(&sk, "art:9/para:1/sent:1").effect, EffectKind::Void);
    // 附則第6条: 従前の例
    assert_eq!(
        find(&sk, "suppl:0/art:6/para:1/sent:1").effect,
        EffectKind::FormerExample
    );
    // 第2条の定義欄は Fragment / Definition
    assert_eq!(
        find(&sk, "art:2/para:1/item:1/col:1/sent:1").effect,
        EffectKind::Fragment
    );
    assert_eq!(
        find(&sk, "art:2/para:1/item:1/col:2/sent:1").effect,
        EffectKind::Definition
    );
}

#[test]
fn skeleton_model_validates_against_source() {
    let doc = doc();
    let sk = extract(&doc);
    let model = to_model(&doc, &sk);
    let issues = validate(&model, &doc);
    assert!(issues.is_empty(), "{issues:#?}");
    assert!(model.rules.len() > 150, "{} rules", model.rules.len());

    // ただし書きは本文を上書きする
    let proviso = model
        .rules
        .iter()
        .find(|r| r.id.0.ends_with("art:3/para:1/sent:2"))
        .unwrap();
    assert!(proviso
        .overrides
        .iter()
        .any(|o| matches!(o, Override::Rule(id) if id.0.ends_with("art:3/para:1/sent:1"))));
    // 第22条前段は第9条・第16条の Rule を上書きする
    let r22 = model
        .rules
        .iter()
        .find(|r| r.id.0.ends_with("art:22/para:1/sent:1"))
        .unwrap();
    assert!(r22
        .overrides
        .iter()
        .any(|o| matches!(o, Override::Rule(id) if id.0.contains("art:9/"))));
    assert!(r22
        .overrides
        .iter()
        .any(|o| matches!(o, Override::Rule(id) if id.0.contains("art:16/"))));
    // 第32条は契約に優先
    let r32 = model
        .rules
        .iter()
        .find(|r| r.id.0.ends_with("art:32/para:1/sent:1"))
        .unwrap();
    assert!(r32.overrides.contains(&Override::Contract));
    // 第2条の定義が Column ペアから取れる。scope は「この法律において」= 法令全体
    let d = model
        .definitions
        .iter()
        .find(|d| d.term == "借地権者")
        .unwrap();
    assert_eq!(d.scope[0].0, "403AC0000000090");
    assert!(model.definitions.len() >= 5, "{}", model.definitions.len());
    // 骨組みはすべて Parser 由来・Medium 以下
    assert!(model
        .rules
        .iter()
        .all(|r| matches!(r.provenance.by, Author::Parser(_))
            && r.provenance.confidence <= Confidence::Medium));
}
