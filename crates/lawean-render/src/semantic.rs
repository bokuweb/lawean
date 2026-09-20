//! Semantic IR → 日本語。法律家が IR をレビューするための表示。原文（Source IR）があれば並べて出す。

use lawean_resolve::ResolvedModel;
use lawean_semantic::*;
use lawean_source::{LegalDocument, StableId};

pub fn duration(d: &Duration) -> String {
    let unit = match d.unit {
        Unit::Year => "年",
        Unit::Month => "月",
        Unit::Week => "週間",
        Unit::Day => "日",
        Unit::Hour => "時間",
    };
    format!("{}{unit}", crate::numeral::to_kanji(d.length))
}

fn entity(e: &EntityRef) -> String {
    match e {
        EntityRef::Definition(d) => d.0.trim_start_matches("D:").to_string(),
        EntityRef::Named(n) => n.clone(),
    }
}

fn reference(r: &RefTarget) -> String {
    match r {
        RefTarget::Rule(id) => format!("〔{}〕", id.0),
        RefTarget::Definition(d) => d.0.trim_start_matches("D:").to_string(),
        RefTarget::Scope(s) | RefTarget::Provision(s) => location(s),
        RefTarget::External { law, path } => format!("{law}{}", path.clone().unwrap_or_default()),
        RefTarget::Relative(rel) => rel.text.clone(),
    }
}

pub fn value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Duration(d) => duration(d),
        Value::Period(p) => format!("{}から{}", p.from.0, duration(&p.length)),
        Value::PeriodValue(PeriodValue::Definite(p)) => value(&Value::Period(p.clone())),
        Value::PeriodValue(PeriodValue::Indefinite) => "期間の定めなし".into(),
        Value::RatePerMille(r) => format!("千分の{r}"),
        Value::Var(x) => x.clone(),
        Value::RuleValue(id) => format!("〔{}〕が定める値", id.0),
        Value::Add(a, b) => format!("{}に{}を加えた額", value(a), value(b)),
        Value::Sub(a, b) => format!("{}から{}を控除した額", value(a), value(b)),
        Value::Interest {
            principal,
            rate,
            from,
        } => format!(
            "{}に{}の日からの{}の利息を付した額",
            value(principal),
            from.0,
            value(rate)
        ),
        Value::Unknown(u) => format!("《{}》", u.text),
    }
}

fn arg(a: &Arg) -> String {
    match a {
        Arg::Entity(e) => entity(e),
        Arg::Value(v) => value(v),
        Arg::Expr(e) => expr(e),
        Arg::Ref(r) => reference(r),
        Arg::Text(t) => t.clone(),
        Arg::List(xs) => xs.iter().map(arg).collect::<Vec<_>>().join("・"),
    }
}

fn predicate(p: &Predicate) -> String {
    if p.args.is_empty() {
        return p.name.clone();
    }
    let args: Vec<String> = p
        .args
        .iter()
        .map(|(k, a)| format!("{k}={}", arg(a)))
        .collect();
    format!("{}（{}）", p.name, args.join("、"))
}

fn cmp(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Lt => "未満",
        CmpOp::Le => "以下",
        CmpOp::Eq => "と等しい",
        CmpOp::Ge => "以上",
        CmpOp::Gt => "を超える",
    }
}

pub fn expr(e: &Expr) -> String {
    match e {
        Expr::True => "常に".into(),
        Expr::False => "決して".into(),
        Expr::And(xs) => xs.iter().map(expr).collect::<Vec<_>>().join("、かつ、"),
        Expr::Or(xs) => format!(
            "（{}）",
            xs.iter().map(expr).collect::<Vec<_>>().join("、又は、")
        ),
        Expr::Not(x) => format!("{}でない", expr(x)),
        Expr::Pred(p) => predicate(p),
        Expr::Cmp(a, op, b) => format!("{}が{}{}", value(a), value(b), cmp(*op)),
        Expr::Ref(r) => format!("{}が適用される", reference(r)),
        Expr::Time(t) => format!("{t:?}"),
        Expr::Unknown(u) => format!("《{}》", u.text),
    }
}

fn action(a: &Action) -> String {
    if a.args.is_empty() {
        return a.verb.clone();
    }
    let args: Vec<String> = a
        .args
        .iter()
        .map(|(k, x)| format!("{k}={}", arg(x)))
        .collect();
    format!("{}（{}）", a.verb, args.join("、"))
}

fn target(t: &Target) -> String {
    match t {
        Target::Contract(s) => s.clone(),
        Target::RuleEffect(id) => format!("〔{}〕の効果", id.0),
        Target::Fact(f) => predicate(&f.pred),
    }
}

pub fn effect(e: &Effect) -> String {
    match e {
        Effect::Obligation(a) => format!("{}しなければならない", action(a)),
        Effect::Prohibition(a) => format!("{}してはならない", action(a)),
        Effect::Permission(a) => format!("{}することができる", action(a)),
        Effect::Power {
            action: a,
            exercise,
        } => match exercise {
            Some(x) => format!("{}することができる（行使すると: {}）", action(a), effect(x)),
            None => format!("{}することができる（権限）", action(a)),
        },
        Effect::Set {
            attribute,
            value: v,
        } => format!("{attribute}を{}とする", value(v)),
        Effect::Deem(f) => format!("{}ものとみなす", predicate(&f.pred)),
        Effect::Presume(f) => format!("{}ものと推定する", predicate(&f.pred)),
        Effect::Void(t) => format!("{}は無効とする", target(t)),
        Effect::Exception(id) => format!("〔{}〕は適用しない", id.0),
        Effect::SameAs(id) => format!("〔{}〕と同様とする", id.0),
        Effect::DeemAndApply { from, to, apply } => {
            format!(
                "{}を{}とみなして〔{}〕を適用する",
                predicate(&from.pred),
                predicate(&to.pred),
                apply.0
            )
        }
        Effect::Apply(r) => format!("{}を適用する", reference(r)),
        Effect::ApplyExternal { law, topic } => format!("{topic}については{law}による"),
        Effect::ApplyMutatis { rules, substitute } => {
            let r: Vec<&str> = rules.iter().map(|x| x.0.as_str()).collect();
            let s: Vec<String> = substitute
                .iter()
                .map(|(a, b)| format!("{}を{}と", entity(a), entity(b)))
                .collect();
            format!("〔{}〕を{}読み替えて準用する", r.join("・"), s.join(""))
        }
        Effect::Preserve(t) => format!("{}の効力を妨げない", target(t)),
        Effect::Unknown(u) => format!("《{}》", u.text),
    }
}

/// stable_id → 「第3条第1項」のような位置表示（条があれば章・節は省く）
pub fn location(id: &StableId) -> String {
    let mut s = String::new();
    let has_article = id.0.contains("/art:");
    for seg in id.0.split('/') {
        if has_article
            && (seg.starts_with("chap:") || seg.starts_with("sec:") || seg.starts_with("part:"))
        {
            continue;
        }
        if let Some(n) = seg.strip_prefix("art:") {
            s.push_str(&format!("第{}条", n.replace('_', "の")));
        } else if let Some(n) = seg.strip_prefix("para:") {
            s.push_str(&format!("第{n}項"));
        } else if let Some(n) = seg.strip_prefix("item:") {
            s.push_str(&format!("第{n}号"));
        } else if let Some(n) = seg.strip_prefix("sent:") {
            s.push_str(if n == "1" { "本文" } else { "ただし書" });
        } else if let Some(n) = seg.strip_prefix("suppl:") {
            s.push_str(&format!(
                "附則{}",
                if n == "0" {
                    String::new()
                } else {
                    format!("({n})")
                }
            ));
        } else if let Some(n) = seg.strip_prefix("sec:") {
            s.push_str(&format!("第{n}節"));
        } else if let Some(n) = seg.strip_prefix("chap:") {
            s.push_str(&format!("第{n}章"));
        }
    }
    s
}

fn provenance(p: &Provenance) -> String {
    let by = match &p.by {
        Author::Human(n) => format!("人手:{n}"),
        Author::Llm(m) => format!("LLM:{m}"),
        Author::Parser(v) => format!("規則:{v}"),
        Author::Precedent(c) => format!("判例:{c}"),
    };
    format!("{}、確度 {:?}", by, p.confidence)
}

/// 1 つの Rule を「条件のとき、効果。（位置；優先関係）〔出所〕」の形に
pub fn rule(r: &Rule, rm: &ResolvedModel<'_>) -> String {
    let mut s = format!("【{}】", r.id.0);
    if let Some(sub) = &r.subject {
        s.push_str(&format!("{}について、", entity(sub)));
    }
    if r.condition != Expr::True {
        s.push_str(&format!("{}とき、", expr(&r.condition)));
    }
    s.push_str(&effect(&r.effect));
    s.push_str("。（");
    s.push_str(&location(&r.provenance.source));
    let over: Vec<String> = r
        .overrides
        .iter()
        .map(|o| match o {
            Override::Rule(id) => format!("〔{}〕", id.0),
            Override::Contract => "契約の定め".into(),
        })
        .collect();
    if !over.is_empty() {
        s.push_str(&format!("；{}に優先", over.join("・")));
    }
    let ex = rm.exceptions_of(&r.id);
    if !ex.is_empty() {
        s.push_str(&format!(
            "；{}が優先",
            ex.iter()
                .map(|e| format!("〔{}〕", e.0))
                .collect::<Vec<_>>()
                .join("・")
        ));
    }
    s.push_str("）〔");
    s.push_str(&provenance(&r.provenance));
    if let Some(n) = &r.provenance.note {
        s.push_str(&format!("；{n}"));
    }
    s.push('〕');
    s
}

/// 原文の文を stable_id で引く
pub fn source_text(doc: &LegalDocument, id: &StableId) -> Option<String> {
    doc.sentence_groups()
        .iter()
        .flat_map(|g| g.sentences.iter())
        .find(|s| s.sentence.stable_id == *id)
        .map(|s| s.sentence.plain_text())
}

/// モデル全体。原文があれば各 Rule の上に原文を置く
pub fn model(m: &SemanticModel, doc: Option<&LegalDocument>) -> String {
    let rm = ResolvedModel::new(m);
    let mut out = String::new();
    for d in &m.definitions {
        out.push_str(&format!(
            "【{}】「{}」（{}において）: {}\n",
            d.id.0,
            d.term,
            d.scope
                .iter()
                .map(location)
                .map(|s| if s.is_empty() {
                    "この法律".into()
                } else {
                    s
                })
                .collect::<Vec<_>>()
                .join("・"),
            match &d.body {
                DefinitionBody::Expr(e) => expr(e),
                DefinitionBody::Window(w) => format!("{w:?}"),
                DefinitionBody::Unknown(u) => format!("《{}》", u.text),
            }
        ));
    }
    let mut last_src: Option<StableId> = None;
    for r in &m.rules {
        if let Some(doc) = doc {
            if last_src.as_ref() != Some(&r.provenance.source) {
                if let Some(t) = source_text(doc, &r.provenance.source) {
                    out.push_str(&format!(
                        "\n原文（{}）: {t}\n",
                        location(&r.provenance.source)
                    ));
                }
                last_src = Some(r.provenance.source.clone());
            }
        }
        out.push_str(&rule(r, &rm));
        out.push('\n');
    }
    out
}
