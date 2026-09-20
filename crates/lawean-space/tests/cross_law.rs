//! docs/09 §5 のテスト計画: 借地借家法（A）の改正が、高齢者居住安定確保法（B）と借地借家法施行令（C）に与える影響。

use lawean_amend::parse_units;
use lawean_semantic::build::*;
use lawean_semantic::*;
use lawean_source::*;
use lawean_space::*;

const A: &str = "403AC0000000090";
const B: &str = "413AC0000000026";
const C: &str = "504CO0000000187";

fn fixture(rel: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../fixtures/{rel}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
fn law(rel: &str) -> LegalDocument {
    parse_response(&fixture(rel)).unwrap()
}

/// A は改正前リビジョン、B・C は 2022-05-18 施行の版
fn space(a_rel: &str) -> LawSpace {
    let mut s = LawSpace::new();
    s.add(law(a_rel));
    s.add(law("laws/413AC0000000026_20220518_503AC0000000037.xml"));
    s.add(law("laws/504CO0000000187.xml"));
    s
}

#[test]
fn cross_references_from_b_and_c_to_a_resolve() {
    let s = space("revisions/403AC0000000090_20220518_503AC0000000037.xml");
    let from_b = cross_refs(&s, B, A);
    let arts: Vec<String> = from_b.iter().map(|r| r.article.to_num_string()).collect();
    for a in ["30", "38", "28", "32"] {
        assert!(arts.contains(&a.to_string()), "B → A 第{a}条: {arts:?}");
    }
    assert!(from_b.iter().all(|r| r.target.is_some()), "{from_b:?}");
    let from_c = cross_refs(&s, C, A);
    assert_eq!(from_c.len(), 2);
    assert!(from_c.iter().all(
        |r| r.paragraph == Some(4) && r.target.as_ref().unwrap().0.ends_with("/art:38/para:4")
    ));
    assert_eq!(
        enforced_on(s.get(C).unwrap()).as_deref(),
        Some("2022-05-18")
    );
}

/// 計画 1: 令和3年改正（実物、2022-05-18 施行）+ 施行令（同日施行）→ 波及なし
#[test]
fn plan1_real_amendment_same_day_has_no_impact() {
    let s = space("revisions/403AC0000000090_20210519_503AC0000000037.xml");
    let unit = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let imp = impact(&s, A, &unit, "2022-05-18").unwrap();
    assert!(imp.is_empty(), "{imp:#?}");
}

/// 計画 2: 同じ改正の施行が 2023-01-01 に遅れたら、施行令（2022-05-18）の「第三十八条第四項」は
/// その間、旧第4項（通知義務）を指す。参照ずれ + 時期の区間
#[test]
fn plan2_delayed_enforcement_creates_a_timing_gap_for_the_cabinet_order() {
    let s = space("revisions/403AC0000000090_20210519_503AC0000000037.xml");
    let unit = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let imp = impact(&s, A, &unit, "2023-01-01").unwrap();
    let on_c: Vec<&Impact> = imp.iter().filter(|i| i.reference.from_law == C).collect();
    assert!(!on_c.is_empty(), "{imp:#?}");
    assert!(on_c.iter().any(|i| matches!(&i.kind, ImpactKind::Shifted { moved_to, now_points_to } if moved_to.0.ends_with("/art:38/para:6") && now_points_to.as_ref().unwrap().0.ends_with("/art:38/para:4"))), "{on_c:#?}");
    let gap = on_c
        .iter()
        .find_map(|i| match &i.kind {
            ImpactKind::TimingGap {
                from,
                to,
                before,
                after,
            } => Some((
                from.clone(),
                to.clone(),
                before.clone().unwrap(),
                after.clone().unwrap(),
            )),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (gap.0.as_str(), gap.1.as_str()),
        ("2022-05-18", "2023-01-01")
    );
    assert!(
        gap.2.contains("期間の満了の一年前から六月前までの間"),
        "旧第4項は通知義務: {}",
        gap.2
    );
    assert!(
        gap.3.contains("電磁的方法"),
        "新第4項は電磁的方法: {}",
        gap.3
    );
    // B は影響を受けない
    assert!(imp.iter().all(|i| i.reference.from_law != B), "{imp:#?}");
}

/// 計画 3: 第38条第3項の次に 1 項を挿入する改正案（施行令は改正しない）→ 施行令の「第四項」が別の項を指す
#[test]
fn plan3_inserting_a_paragraph_shifts_the_cabinet_orders_reference() {
    let s = space("revisions/403AC0000000090_20220518_503AC0000000037.xml");
    let unit = parse_units(&fixture("amendments/drafts/insert-38-4.txt"))
        .unwrap()
        .remove(0);
    let imp = impact(&s, A, &unit, "2030-01-01").unwrap();
    let shifted: Vec<&Impact> = imp
        .iter()
        .filter(|i| i.reference.from_law == C && matches!(i.kind, ImpactKind::Shifted { .. }))
        .collect();
    assert_eq!(shifted.len(), 2, "{imp:#?}");
    if let ImpactKind::Shifted {
        moved_to,
        now_points_to,
    } = &shifted[0].kind
    {
        assert!(moved_to.0.ends_with("/art:38/para:5"));
        assert!(now_points_to
            .as_ref()
            .unwrap()
            .0
            .ends_with("/art:38/para:4"));
    }
    let gap = imp
        .iter()
        .find_map(|i| match &i.kind {
            ImpactKind::TimingGap { after, .. } => after.clone(),
            _ => None,
        })
        .unwrap();
    assert!(
        gap.contains("書面を保存しなければならない"),
        "改正後の第4項は挿入された項: {gap}"
    );
}

/// 計画 4: 第30条の強行規定を高齢者居住安定確保法第52条第1項の特約にも及ぼす改正案 →
/// B の第52条（第30条にかかわらず特約できる）と相互に「かかわらず」= 上書きの循環。
/// Z3: 改正前は「書面で契約すれば特約を定められる」が証明できるが、改正後は反例が出る
#[test]
fn plan4_widening_art30_contradicts_b52() {
    let s = space("revisions/403AC0000000090_20220518_503AC0000000037.xml");
    let unit = parse_units(&fixture("amendments/drafts/widen-30.txt"))
        .unwrap()
        .remove(0);
    let imp = impact(&s, A, &unit, "2030-01-01").unwrap();
    assert!(
        imp.iter().any(|i| i.reference.from_law == B
            && i.reference.article.to_num_string() == "30"
            && matches!(i.kind, ImpactKind::SemanticChange { .. })),
        "{imp:#?}"
    );

    // 意味層: A の R30（強行規定）と B の R52 を 1 つのモデルに
    let a_r30 = rule("R30")
        .subject(named("特約"))
        .condition(and([
            pred("反する")
                .arg(
                    "target",
                    Arg::Ref(RefTarget::Scope(StableId(format!("{A}/main/chap:3/sec:1")))),
                )
                .expr(),
            pred("不利").arg("to", entity(named("賃借人"))).expr(),
        ]))
        .effect(Effect::Void(Target::Contract("特約".into())))
        .provenance(
            &format!("{A}/main/chap:3/sec:1/art:30/para:1/sent:1"),
            Confidence::High,
            "test",
        );
    let b_r52 = rule("R52")
        .subject(named("終身賃貸事業者"))
        .condition(pred("書面によって契約する").expr())
        .effect(power(
            action("特約を定める").arg("content", text("賃借人が死亡した時に終了する")),
        ))
        .overrides(&["A:R30"])
        .provenance(
            &format!("{B}/main/chap:4/sec:1/art:52/para:1/sent:1"),
            Confidence::High,
            "test",
        );
    let before = SemanticModel {
        document: "space".into(),
        definitions: vec![],
        rules: vec![
            prefix_model(
                "A",
                &SemanticModel {
                    document: "".into(),
                    definitions: vec![],
                    rules: vec![a_r30.clone()],
                    unknowns: vec![],
                },
                &["A", "B"],
            )
            .rules
            .remove(0),
            prefix_model(
                "B",
                &SemanticModel {
                    document: "".into(),
                    definitions: vec![],
                    rules: vec![b_r52.clone()],
                    unknowns: vec![],
                },
                &["A", "B"],
            )
            .rules
            .remove(0),
        ],
        unknowns: vec![],
    };
    assert!(override_cycles(&before).is_empty());

    // 改正後: 第30条に「同項の特約についても、同様とする」（B52 にかかわらず無効）が加わる。
    // B の「第三十条の規定にかかわらず」は第30条の全 Rule に及ぶので、新 Rule も上書きする
    let a_r30b = rule("R30b")
        .subject(named("特約"))
        .effect(Effect::Void(Target::Contract("特約".into())))
        .overrides(&["B:R52"])
        .provenance(
            &format!("{A}/main/chap:3/sec:1/art:30/para:1/sent:2"),
            Confidence::High,
            "test",
        );
    let mut after = before.clone();
    after.rules.push(
        prefix_model(
            "A",
            &SemanticModel {
                document: "".into(),
                definitions: vec![],
                rules: vec![a_r30b],
                unknowns: vec![],
            },
            &["A", "B"],
        )
        .rules
        .remove(0),
    );
    after
        .rules
        .iter_mut()
        .find(|r| r.id.0 == "B:R52")
        .unwrap()
        .overrides
        .push(Override::Rule(RuleId("A:R30b".into())));
    let cycles = override_cycles(&after);
    let dbg: Vec<(String, Vec<String>)> = after
        .rules
        .iter()
        .map(|r| {
            (
                r.id.0.clone(),
                r.overrides.iter().map(|o| format!("{o:?}")).collect(),
            )
        })
        .collect();
    assert_eq!(cycles.len(), 1, "{cycles:?}\n{dbg:#?}");
    assert!(
        cycles[0].contains(&RuleId("A:R30b".into())) && cycles[0].contains(&RuleId("B:R52".into()))
    );

    if lawean_verify::z3_available() {
        use lawean_resolve::ResolvedModel;
        use lawean_verify::{check, Property, Verdict};
        let claim = || or([not(pred("書面によって契約する").expr()), rule_ref("B:R52")]);
        let p = Property::new("書面で契約すれば B52 の特約を定められる", claim());
        assert_eq!(
            check(&ResolvedModel::new(&before), &p).unwrap(),
            Verdict::Proved
        );
        match check(
            &ResolvedModel::new(&after),
            &Property::new("同上（改正後）", claim()),
        )
        .unwrap()
        {
            Verdict::Counterexample(m) => assert!(m.contains("applies:A:R30b"), "{m}"),
            v => panic!("{v:?}"),
        }
    }
}

/// 計画 5: 第28条を削る改正案 → B 第58条の「借地借家法第二十八条の規定は…適用しない」が参照切れ
#[test]
fn plan5_deleting_art28_leaves_b57_dangling() {
    let s = space("revisions/403AC0000000090_20220518_503AC0000000037.xml");
    let unit = parse_units(&fixture("amendments/drafts/delete-28.txt"))
        .unwrap()
        .remove(0);
    let imp = impact(&s, A, &unit, "2030-01-01").unwrap();
    let dangling: Vec<&Impact> = imp
        .iter()
        .filter(|i| matches!(i.kind, ImpactKind::Dangling))
        .collect();
    assert_eq!(dangling.len(), 1, "{imp:#?}");
    assert_eq!(dangling[0].reference.from_law, B);
    assert_eq!(dangling[0].reference.article.to_num_string(), "28");
    assert!(
        dangling[0].reference.sentence.0.contains("/art:58/"),
        "{}",
        dangling[0].reference.sentence.0
    );
}
