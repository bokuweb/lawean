//! 経過措置の場合分け（docs/10 §3 の 4）。借地借家法の附則第4条（遡及）・第6条（なお従前の例による）の形を小さく組み、
//! 「施行前の事実に新法と旧法の両方が適用される」「どちらも適用されない」を Z3 の反例で出す。z3 が無ければスキップ。

use lawean_resolve::ResolvedModel;
use lawean_semantic::build::*;
use lawean_semantic::*;
use lawean_verify::transitional::*;
use lawean_verify::*;

macro_rules! need_z3 {
    () => {
        if !z3_available() {
            eprintln!("z3 not on PATH; skipping");
            return;
        }
    };
}

fn renewal() -> Expr {
    pred("更新を請求した")
        .arg("by", entity(def("借地権者")))
        .expr()
}

/// 新法の更新規定（第5条相当）と旧借地法の更新規定。時間属性だけ差し替えて使う
fn model(new_t: RuleTemporal, old_t: RuleTemporal) -> SemanticModel {
    model_with(renewal(), new_t, renewal(), old_t)
}

fn model_with(new_c: Expr, new_t: RuleTemporal, old_c: Expr, old_t: RuleTemporal) -> SemanticModel {
    SemanticModel {
        document: "test".into(),
        definitions: vec![],
        rules: vec![
            rule("R5")
                .condition(new_c)
                .effect(Effect::Deem(pred("更新した").fact()))
                .temporal(new_t),
            rule("旧借地法4")
                .condition(old_c)
                .effect(Effect::Deem(pred("更新した").fact()))
                .temporal(old_t),
        ],
        unknowns: vec![],
    }
}

fn transition() -> Transition {
    Transition::new("契約の更新", "借地権の設定", &["R5"], &["旧借地法4"])
}

fn run(m: &SemanticModel, t: &Transition) -> Vec<TransitionFinding> {
    let rm = ResolvedModel::new(m);
    transitions(&rm, t).unwrap()
}

fn overlap(fs: &[TransitionFinding]) -> &Verdict {
    &fs.iter()
        .find(|f| matches!(f.kind, TransitionKind::Overlap { .. }))
        .unwrap()
        .verdict
}

fn gap(fs: &[TransitionFinding]) -> &Verdict {
    &fs.iter()
        .find(|f| f.kind == TransitionKind::Gap)
        .unwrap()
        .verdict
}

fn int(model: &str, name: &str) -> i64 {
    let v = model_value(model, name).unwrap_or_else(|| panic!("{name} not in {model}"));
    match v.strip_prefix("(- ").and_then(|s| s.strip_suffix(')')) {
        Some(n) => -n.trim().parse::<i64>().unwrap(),
        None => v.parse().unwrap(),
    }
}

#[test]
fn builders_express_both_kinds_of_facts_before() {
    let t = in_force_from("施行日").also_before("施行日");
    assert_eq!(t.effective_from, Some(event("施行日")));
    assert_eq!(t.facts_before, Some(FactsBefore::Also(event("施行日"))));
    let t = in_force_until("施行日").only_before("施行日");
    assert_eq!(t.effective_to, Some(event("施行日")));
    assert_eq!(t.facts_before, Some(FactsBefore::Only(event("施行日"))));
}

/// 附則第6条が正しく張ってある: 施行前に設定した借地権の更新は旧法だけ、施行後は新法だけ
#[test]
fn renewal_is_partitioned_by_the_date_of_setting() {
    need_z3!();
    let m = model(
        in_force_from("施行日"),
        in_force_until("施行日").only_before("施行日"),
    );
    let fs = run(&m, &transition());
    assert_eq!(overlap(&fs), &Verdict::Proved, "{fs:?}");
    assert_eq!(gap(&fs), &Verdict::Proved, "{fs:?}");
}

/// 附則第4条（施行前の事項にも適用）だけで、更新について「特別の定め」を新法側から外し忘れた:
/// 施行前に設定した借地権の更新に新法と旧法の両方が適用される
#[test]
fn retroactive_new_law_and_grandfathered_old_law_overlap() {
    need_z3!();
    let m = model(
        in_force_from("施行日").also_before("施行日"),
        in_force_until("施行日").only_before("施行日"),
    );
    let fs = run(&m, &transition());
    match overlap(&fs) {
        Verdict::Counterexample(cex) => {
            let set = int(cex, "t:借地権の設定");
            let b = int(cex, "t:施行日");
            let now = int(cex, "t:now");
            assert!(set < b, "施行前に設定した借地権で起きる: {cex}");
            assert!(now >= b, "施行後に更新を請求したとき: {cex}");
        }
        v => panic!("{v:?}"),
    }
    assert_eq!(gap(&fs), &Verdict::Proved);
}

/// 附則第6条（なお従前の例による）を書き忘れた: 施行前に設定した借地権の更新にどちらも適用されない
#[test]
fn missing_grandfather_clause_leaves_a_gap() {
    need_z3!();
    let m = model(in_force_from("施行日"), in_force_until("施行日"));
    let fs = run(&m, &transition());
    assert_eq!(overlap(&fs), &Verdict::Proved);
    match gap(&fs) {
        Verdict::Counterexample(cex) => {
            let set = int(cex, "t:借地権の設定");
            let b = int(cex, "t:施行日");
            assert!(set < b, "{cex}");
            assert!(int(cex, "t:now") >= b, "{cex}");
        }
        v => panic!("{v:?}"),
    }
}

/// 経過措置の基準日が新法の施行日と違う（段階施行の別の号の日を指した）: 日の前後でずれ、両方か空白のどちらかが起きる
#[test]
fn boundary_of_a_different_stage_is_caught() {
    need_z3!();
    let m = model(
        in_force_from("施行日A"),
        in_force_until("施行日B").only_before("施行日B"),
    );
    let fs = run(&m, &transition());
    assert!(matches!(overlap(&fs), Verdict::Counterexample(_)), "{fs:?}");
    assert!(matches!(gap(&fs), Verdict::Counterexample(_)), "{fs:?}");

    // 2 つの日が同じだと分かっていれば（政令で同日に定めた）矛盾は消える
    let t = transition().assume(event("施行日A"), CmpOp::Eq, event("施行日B"));
    let fs = run(&m, &t);
    assert_eq!(overlap(&fs), &Verdict::Proved, "{fs:?}");
    assert_eq!(gap(&fs), &Verdict::Proved, "{fs:?}");
}

/// 遡及していても、新法と旧法の条件が排他なら両方適用は起きない（Z3 が条件を見ている）
#[test]
fn disjoint_conditions_do_not_overlap_even_when_retroactive() {
    need_z3!();
    let building = pred("建物がある").expr();
    let m = model_with(
        and([renewal(), building.clone()]),
        in_force_from("施行日").also_before("施行日"),
        and([renewal(), not(building)]),
        in_force_until("施行日").only_before("施行日"),
    );
    let fs = run(&m, &transition());
    assert_eq!(overlap(&fs), &Verdict::Proved, "{fs:?}");
}

/// 時間属性の無い Rule は常に効いている（経過措置の外）。空白は起きない
#[test]
fn rule_without_temporal_always_reaches() {
    need_z3!();
    let mut m = model(in_force_from("施行日"), in_force_until("施行日"));
    m.rules[1].temporal = None;
    let fs = run(&m, &transition());
    assert_eq!(gap(&fs), &Verdict::Proved, "{fs:?}");
}

/// ブラウザの z3 に渡す用に SMT-LIB だけ作れる
#[test]
fn scripts_are_available_without_z3() {
    let m = model(
        in_force_from("施行日"),
        in_force_until("施行日").only_before("施行日"),
    );
    let rm = ResolvedModel::new(&m);
    let ss = transition_scripts(&rm, &transition());
    assert_eq!(ss.len(), 2, "overlap 1 組 + gap");
    for (_, s) in &ss {
        assert!(s.contains("|t:施行日|"), "{s}");
        assert!(s.contains("|t:借地権の設定|"), "{s}");
        assert!(s.contains("(check-sat)"), "{s}");
    }
}
