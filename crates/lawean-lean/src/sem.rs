//! Semantic IR → `Lawean.Sem.Model`（docs/10 §5、ADR-0015）。
//! 写像は `lawean-verify::smt` の SMT 写像と同じ（同じ名前・同じ値）。違いは Rule の並びだけで、
//! Lean の評価器は層化された順（例外 → 原則、参照先 → 参照元）を前提にするので、ここでトポロジカルソートする。

use crate::lean_string as ls;
use lawean_resolve::ResolvedModel;
use lawean_semantic::*;
use lawean_verify::smt::{months, pred_name, target_name};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SemError {
    /// 上書き・参照のグラフに循環があり、評価順が決められない
    #[error("上書き・参照に循環がある: {0:?}")]
    Cycle(Vec<String>),
}

fn value(v: &Value) -> String {
    match v {
        Value::Int(i) => format!("(.int {})", int(*i)),
        Value::Duration(d) => format!("(.int {})", int(months(d))),
        Value::Period(p) => format!("(.int {})", int(months(&p.length))),
        Value::PeriodValue(PeriodValue::Definite(p)) => {
            format!("(.int {})", int(months(&p.length)))
        }
        Value::PeriodValue(PeriodValue::Indefinite) => "(.int (-1))".into(),
        Value::RatePerMille(r) => format!("(.int {r})"),
        Value::Var(name) => format!("(.var {})", ls(&format!("a:{name}"))),
        Value::RuleValue(id) => format!("(.ruleValue {})", ls(&id.0)),
        Value::Add(a, b) => format!("(.add {} {})", value(a), value(b)),
        Value::Sub(a, b) => format!("(.sub {} {})", value(a), value(b)),
        Value::Interest { .. } => "(.var \"interest\")".into(),
        Value::Unknown(u) => format!("(.var {})", ls(&format!("unknown:{}", u.text))),
    }
}

fn int(i: i64) -> String {
    if i < 0 {
        format!("({i})")
    } else {
        i.to_string()
    }
}

fn expr(rm: &ResolvedModel<'_>, e: &Expr) -> String {
    match e {
        Expr::True => ".tt".into(),
        Expr::False => ".ff".into(),
        Expr::And(xs) => format!("(.and [{}])", list(rm, xs)),
        Expr::Or(xs) => format!("(.or [{}])", list(rm, xs)),
        Expr::Not(x) => format!("(.not {})", expr(rm, x)),
        Expr::Pred(p) => format!("(.pred {})", ls(&pred_name(p))),
        Expr::Cmp(a, op, b) => {
            let op = match op {
                CmpOp::Lt => "lt",
                CmpOp::Le => "le",
                CmpOp::Eq => "eq",
                CmpOp::Ge => "ge",
                CmpOp::Gt => "gt",
            };
            format!("(.cmp {} .{op} {})", value(a), value(b))
        }
        Expr::Ref(RefTarget::Rule(id)) => format!("(.ref {})", ls(&id.0)),
        Expr::Ref(other) => format!("(.unknown {})", ls(&format!("ref:{other:?}"))),
        Expr::Time(t) => format!("(.unknown {})", ls(&format!("time:{t:?}"))),
        Expr::Unknown(u) => format!("(.unknown {})", ls(&format!("unknown:{}", u.text))),
    }
}

fn list(rm: &ResolvedModel<'_>, xs: &[Expr]) -> String {
    xs.iter()
        .map(|x| expr(rm, x))
        .collect::<Vec<_>>()
        .join(", ")
}

fn effect(e: &Effect, owner: &RuleId) -> String {
    match e {
        Effect::Set {
            attribute,
            value: v,
        } => {
            format!("(.set {} {})", ls(&format!("a:{attribute}")), value(v))
        }
        Effect::Deem(f) | Effect::Presume(f) => format!("(.deem {})", ls(&pred_name(&f.pred))),
        Effect::Void(t) => format!("(.void {})", ls(&target_name(t))),
        Effect::Preserve(t) => format!("(.preserve {})", ls(&target_name(t))),
        Effect::Exception(r) => format!("(.exception {})", ls(&r.0)),
        Effect::SameAs(r) | Effect::DeemAndApply { apply: r, .. } => {
            format!("(.sameAs {})", ls(&r.0))
        }
        _ => format!("(.mark {})", ls(&owner.0)),
    }
}

fn refs(e: &Expr, out: &mut Vec<RuleId>) {
    match e {
        Expr::And(xs) | Expr::Or(xs) => xs.iter().for_each(|x| refs(x, out)),
        Expr::Not(x) => refs(x, out),
        Expr::Ref(RefTarget::Rule(id)) => out.push(id.clone()),
        _ => {}
    }
}

/// 評価順: 各 Rule の前に、それを上書きする Rule と、条件が参照する Rule が来る。
/// 元の順序をなるべく保つ（安定なトポロジカルソート）。循環があればエラー
pub fn stratify<'a>(rm: &ResolvedModel<'a>) -> Result<Vec<&'a Rule>, SemError> {
    let rules = &rm.model.rules;
    let mut deps: BTreeMap<&RuleId, Vec<RuleId>> = BTreeMap::new();
    for r in rules {
        let mut d: Vec<RuleId> = rm.exceptions_of(&r.id).to_vec();
        refs(&r.condition, &mut d);
        d.retain(|x| rules.iter().any(|r| &r.id == x));
        deps.insert(&r.id, d);
    }
    let mut done: BTreeSet<&RuleId> = BTreeSet::new();
    let mut out = Vec::new();
    fn visit<'a>(
        r: &'a Rule,
        rules: &'a [Rule],
        deps: &BTreeMap<&RuleId, Vec<RuleId>>,
        done: &mut BTreeSet<&'a RuleId>,
        path: &mut Vec<&'a RuleId>,
        out: &mut Vec<&'a Rule>,
    ) -> Result<(), SemError> {
        if done.contains(&r.id) {
            return Ok(());
        }
        if path.contains(&&r.id) {
            let mut cyc: Vec<String> = path.iter().map(|x| x.0.clone()).collect();
            cyc.push(r.id.0.clone());
            return Err(SemError::Cycle(cyc));
        }
        path.push(&r.id);
        for d in &deps[&r.id] {
            let dr = rules.iter().find(|x| &x.id == d).unwrap();
            visit(dr, rules, deps, done, path, out)?;
        }
        path.pop();
        done.insert(&r.id);
        out.push(r);
        Ok(())
    }
    for r in rules {
        visit(r, rules, &deps, &mut done, &mut Vec::new(), &mut out)?;
    }
    Ok(out)
}

fn conf(c: Confidence) -> u32 {
    match c {
        Confidence::High => 100,
        Confidence::Medium => 60,
        Confidence::Low => 30,
    }
}

/// Lean の識別子。Rule id をそのまま `«…»` で包む
fn ident(id: &str) -> String {
    format!("«{}»", id.replace('»', "_"))
}

/// Rule ごとに `def «id» : Rule`、最後に `def <name> : Model`。Rule は層化された順。
/// `exceptions` は層化した並びの中で計算する（Lean の `Model.exceptionsOf` と同じ順）
pub fn emit_model(doc: &str, name: &str, model: &SemanticModel) -> Result<String, SemError> {
    let rm = ResolvedModel::new(model);
    let rules = stratify(&rm)?;
    let mut s = format!(
        "import Lawean.Sem\n\n/-!\n自動生成: `cargo run -p lawean-lean --example gen`。手で編集しない。\n\n{doc}\n-/\n\nnamespace Lawean.Data\nopen Lawean.Sem\n\n"
    );
    for r in &rules {
        let overrides: Vec<String> = r
            .overrides
            .iter()
            .filter_map(|o| match o {
                Override::Rule(id) => Some(ls(&id.0)),
                Override::Contract => None,
            })
            .collect();
        let exceptions: Vec<String> = rules
            .iter()
            .filter(|x| x.overrides.contains(&Override::Rule(r.id.clone())))
            .map(|x| ls(&x.id.0))
            .collect();
        writeln!(s, "def {} : Rule :=", ident(&r.id.0)).unwrap();
        writeln!(s, "  {{ id := {},", ls(&r.id.0)).unwrap();
        writeln!(s, "    cond := {},", expr(&rm, &r.condition)).unwrap();
        writeln!(s, "    effect := {},", effect(&r.effect, &r.id)).unwrap();
        writeln!(s, "    overrides := [{}],", overrides.join(", ")).unwrap();
        writeln!(s, "    exceptions := [{}],", exceptions.join(", ")).unwrap();
        writeln!(s, "    source := {},", ls(&r.provenance.source.0)).unwrap();
        writeln!(s, "    conf := {} }}\n", conf(r.provenance.confidence)).unwrap();
    }
    let ids: Vec<String> = rules.iter().map(|r| ident(&r.id.0)).collect();
    writeln!(
        s,
        "def {name} : Model := {{ rules := [{}] }}",
        ids.join(", ")
    )
    .unwrap();
    s.push_str("\nend Lawean.Data\n");
    Ok(s)
}
