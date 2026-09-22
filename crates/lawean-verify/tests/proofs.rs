//! 手書き IR（借地借家法）に対する性質の証明・反証。z3 が無ければスキップ。

use lawean_resolve::ResolvedModel;
use lawean_semantic::build::*;
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_semantic::*;
use lawean_verify::*;

fn y(n: u32) -> Value {
    Value::Duration(years(n))
}
fn implies(a: Expr, b: Expr) -> Expr {
    or([not(a), b])
}
fn run(model: &SemanticModel, p: Property) -> Verdict {
    let rm = ResolvedModel::new(model);
    check(&rm, &p).unwrap()
}
macro_rules! need_z3 {
    () => {
        if !z3_available() {
            eprintln!("z3 not on PATH; skipping");
            return;
        }
    };
}

#[test]
fn art3_duration_is_at_least_30_years() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let p = Property::new(
        "第3条: 存続期間 ≥ 30年",
        cmp(var("存続期間"), CmpOp::Ge, y(30)),
    );
    assert_eq!(run(&m, p), Verdict::Proved);
}

#[test]
fn art3_duration_is_not_always_30_years() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let p = Property::new(
        "第3条: 存続期間 = 30年（偽）",
        cmp(var("存続期間"), CmpOp::Eq, y(30)),
    );
    match run(&m, p) {
        Verdict::Counterexample(model) => {
            // 反例: 契約で 30 年より長く定めた場合（ただし書き）
            let v: i64 = model_value(&model, "a:契約で定めた期間")
                .unwrap()
                .parse()
                .unwrap();
            assert!(v > 360, "{model}");
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn art4_renewed_duration_is_at_least_10_years() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let p = Property::new(
        "第4条: 更新後の期間 ≥ 10年",
        implies(
            pred("更新する").expr(),
            cmp(var("更新後の期間"), CmpOp::Ge, y(10)),
        ),
    );
    assert_eq!(run(&m, p), Verdict::Proved);
}

/// 検証が見つけた IR のバグ（docs/07-verification.md）。
/// ただし書き R4-2 の「これより長い」を「R4-1 の値（10 年）より長い」と書くと、
/// 最初の更新で 15 年と定めた場合に 20 年を下回る。
#[test]
fn art4_first_renewal_bug_in_original_encoding() {
    need_z3!();
    let mut m = shakuchi_shakuya::model();
    m.rules.retain(|r| !r.id.0.starts_with("R4-2"));
    m.rules.push(
        rule("R4-2")
            .subject(def("D:借地権"))
            .condition(cmp(
                var("当事者が定めた期間"),
                CmpOp::Gt,
                rule_value("R4-1"),
            ))
            .effect(set("更新後の期間", var("当事者が定めた期間")))
            .overrides(&["R4-1", "R4-1'"])
            .provenance(
                "403AC0000000090/main/chap:2/sec:1/art:4/para:1/sent:2",
                Confidence::High,
                "test",
            ),
    );
    let p = Property::new(
        "第4条: 最初の更新なら 更新後の期間 ≥ 20年",
        implies(
            and([pred("更新する").expr(), pred("最初の更新").expr()]),
            cmp(var("更新後の期間"), CmpOp::Ge, y(20)),
        ),
    );
    match run(&m, p) {
        Verdict::Counterexample(model) => {
            let v: i64 = model_value(&model, "a:当事者が定めた期間")
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                v > 120 && v < 240,
                "counterexample should be between 10 and 20 years: {model}"
            );
        }
        v => panic!("{v:?}"),
    }
}

#[test]
fn art4_first_renewal_is_at_least_20_years_after_fix() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let p = Property::new(
        "第4条: 最初の更新なら 更新後の期間 ≥ 20年",
        implies(
            and([pred("更新する").expr(), pred("最初の更新").expr()]),
            cmp(var("更新後の期間"), CmpOp::Ge, y(20)),
        ),
    );
    assert_eq!(run(&m, p), Verdict::Proved);
}

#[test]
fn art22_fixed_term_lease_is_not_voided_by_art9() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let p = Property::new(
        "第22条 × 第9条: 存続期間 ≥ 50年なら 第9条は適用されない",
        implies(cmp(var("存続期間"), CmpOp::Ge, y(50)), not(rule_ref("R9"))),
    );
    assert_eq!(run(&m, p), Verdict::Proved);
}

#[test]
fn art9_voids_short_term_agreement_given_lemma() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let r9_cond = m
        .rules
        .iter()
        .find(|r| r.id.0 == "R9")
        .unwrap()
        .condition
        .clone();
    let short = cmp(var("契約で定めた期間"), CmpOp::Lt, y(30));
    // 補題: 30 年未満の特約は「この節の規定に反し、借地権者に不利」（評価概念を人が埋める）
    let lemma = implies(short.clone(), r9_cond);
    let p = Property::new(
        "第9条: 契約期間 < 30年 ⇒ 特約は無効",
        implies(short, rule_ref("R9")),
    )
    .lemma(lemma);
    assert_eq!(run(&m, p), Verdict::Proved);
}

#[test]
fn script_is_generated_without_solver() {
    let m = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&m);
    let s = script(
        &rm,
        &Property::new("x", cmp(var("存続期間"), CmpOp::Ge, y(30))),
    );
    assert!(s.contains("(declare-const |applies:R3-1| Bool)"));
    assert!(s.contains("(assert (=> |applies:R3-1| (= |a:存続期間| 360)))"));
    assert!(s.ends_with("(check-sat)\n(get-model)\n"));
}

// ---------------------------------------------------------------- 無矛盾（効力の齟齬）

/// 手書き IR には、同じ対象に相反する効果を同時に与える Rule の組が無い。
/// 第3条・第4条の `Set` の組は overrides で片方しか適用されないので unsat（Proved）
#[test]
fn hand_written_model_has_no_conflicting_effects() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&m);
    let cs = conflicts(&rm).unwrap();
    assert!(
        !cs.is_empty(),
        "候補の組は挙がる（第3条の本文とただし書き等）"
    );
    for c in &cs {
        assert_eq!(
            c.verdict,
            Verdict::Proved,
            "{} vs {} {:?}",
            c.a.0,
            c.b.0,
            c.kind
        );
    }
}

/// 効力の齟齬: 第9条（借地権者に不利な特約は無効）に対し、「書面による特約は効力を妨げない」を
/// overrides 無しで足すと、両方が同時に適用される世界がある = 矛盾。overrides を張れば消える
#[test]
fn a_preserve_rule_without_override_conflicts_with_art9() {
    need_z3!();
    let mut m = shakuchi_shakuya::model();
    m.rules.push(
        rule("R-draft")
            .condition(pred("書面による特約").expr())
            .effect(Effect::Preserve(Target::Contract("特約".into())))
            .provenance(
                "403AC0000000090/main/chap:2/sec:1/art:9/para:2/sent:1",
                Confidence::High,
                "test",
            ),
    );
    let rm = ResolvedModel::new(&m);
    let cs = conflicts(&rm).unwrap();
    let c = cs
        .iter()
        .find(|c| c.b.0 == "R-draft")
        .expect("第9条との組が挙がる");
    assert_eq!(c.a.0, "R9");
    assert!(matches!(c.kind, ConflictKind::VoidVsPreserve { ref target } if target == "特約"));
    let Verdict::Counterexample(model) = &c.verdict else {
        panic!("{:?}", c.verdict)
    };
    // 反例: 書面による特約で、この節の規定に反し、借地権者に不利
    assert_eq!(
        model_value(model, "p:書面による特約").as_deref(),
        Some("true")
    );

    // 「第九条の規定にかかわらず」= overrides を張ると矛盾は消える
    m.rules.last_mut().unwrap().overrides = vec![Override::Rule(RuleId("R9".into()))];
    let rm = ResolvedModel::new(&m);
    let cs = conflicts(&rm).unwrap();
    let c = cs.iter().find(|c| c.b.0 == "R-draft").unwrap();
    assert_eq!(c.verdict, Verdict::Proved);
}

// ---------------------------------------------------------------- 空振り（例外を差し引くと対象が無い）

/// 手書き IR の Rule はすべて、例外を差し引いても適用される世界が残る
#[test]
fn hand_written_model_has_no_vacuous_rule() {
    need_z3!();
    let m = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&m);
    for v in vacuous(&rm).unwrap() {
        assert!(
            matches!(v.verdict, Verdict::Counterexample(_)),
            "{} は空振り（例外 {:?}）",
            v.rule.0,
            v.exceptions
        );
    }
}

/// 罰則の型: 「三十年以上の期間を定めた者は…に処する」に、「二十年以上の期間を定めた場合はこの限りでない」と
/// いう例外を張ると、対象が存在しない（30 以上は必ず 20 以上）。Z3 は applies が unsat になることで見つける。
/// 例外を「五十年以上」に直せば、30〜50 年の世界が残る
#[test]
fn a_penalty_swallowed_by_its_exception_is_vacuous() {
    need_z3!();
    let mut m = shakuchi_shakuya::model();
    m.rules.push(
        rule("R-pen")
            .condition(cmp(var("契約で定めた期間"), CmpOp::Ge, y(30)))
            .effect(Effect::Prohibition(action("期間を定める")))
            .provenance(
                "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:1",
                Confidence::High,
                "test",
            ),
    );
    m.rules.push(
        rule("R-pen-ex")
            .condition(cmp(var("契約で定めた期間"), CmpOp::Ge, y(20)))
            .effect(Effect::Exception(RuleId("R-pen".into())))
            .overrides(&["R-pen"])
            .provenance(
                "403AC0000000090/main/chap:2/sec:1/art:3/para:1/sent:2",
                Confidence::High,
                "test",
            ),
    );
    let rm = ResolvedModel::new(&m);
    let vs = vacuous(&rm).unwrap();
    let v = vs.iter().find(|v| v.rule.0 == "R-pen").unwrap();
    assert_eq!(v.exceptions, vec![RuleId("R-pen-ex".into())]);
    assert_eq!(v.verdict, Verdict::Proved, "例外に飲まれて対象が無い");
    // 他の Rule は空振りしない
    assert!(vs
        .iter()
        .filter(|v| v.rule.0 != "R-pen")
        .all(|v| matches!(v.verdict, Verdict::Counterexample(_))));

    // 例外を五十年以上にすると 30〜50 年の対象が残る
    m.rules.last_mut().unwrap().condition = cmp(var("契約で定めた期間"), CmpOp::Ge, y(50));
    let rm = ResolvedModel::new(&m);
    let vs = vacuous(&rm).unwrap();
    let v = vs.iter().find(|v| v.rule.0 == "R-pen").unwrap();
    let Verdict::Counterexample(model) = &v.verdict else {
        panic!("{:?}", v.verdict)
    };
    let n: i64 = model_value(model, "a:契約で定めた期間")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!((360..600).contains(&n), "反例の期間（月）: {n}");
}

/// 「できる」とされる行為が禁止される: 「借地権者は建物を再築することができる」と
/// 「借地権者は建物を再築してはならない」を overrides 無しで並べると、両方が適用される世界がある
#[test]
fn a_permission_and_a_prohibition_of_the_same_act_conflict() {
    need_z3!();
    let mut m = shakuchi_shakuya::model();
    m.rules.push(
        rule("R-can")
            .condition(pred("借地権者である").expr())
            .effect(Effect::Permission(action("再築する")))
            .provenance(
                "403AC0000000090/main/chap:2/sec:1/art:7/para:1/sent:1",
                Confidence::High,
                "test",
            ),
    );
    m.rules.push(
        rule("R-must-not")
            .condition(pred("承諾がない").expr())
            .effect(Effect::Prohibition(action("再築する")))
            .provenance(
                "403AC0000000090/main/chap:2/sec:1/art:7/para:1/sent:2",
                Confidence::High,
                "test",
            ),
    );
    let rm = ResolvedModel::new(&m);
    let cs = conflicts(&rm).unwrap();
    let c = cs.iter().find(|c| matches!(c.kind, ConflictKind::PermissionVsProhibition { ref verb } if verb == "再築する")).expect("組が挙がる");
    assert!(
        matches!(c.verdict, Verdict::Counterexample(_)),
        "{:?}",
        c.verdict
    );
    // 禁止の側に「第七条の規定にかかわらず」（overrides）を張れば、同時適用は消える
    m.rules.last_mut().unwrap().overrides = vec![Override::Rule(RuleId("R-can".into()))];
    let rm = ResolvedModel::new(&m);
    let c2 = conflicts(&rm)
        .unwrap()
        .into_iter()
        .find(|c| c.b.0 == "R-must-not")
        .unwrap();
    assert_eq!(c2.verdict, Verdict::Proved);
}
