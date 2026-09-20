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
