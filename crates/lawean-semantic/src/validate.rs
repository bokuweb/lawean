//! Semantic IR の整合性検査。意味の正しさではなく、参照が解決できるかを見る。
//!
//! - Provenance / Definition.scope の stable_id が Source IR に存在する
//! - RuleId / DefinitionId の参照先がモデル内に存在する

use crate::ir::*;
use lawean_source::{LegalDocument, StableId};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    UnknownStableId {
        owner: String,
        id: StableId,
    },
    UnknownRule {
        owner: String,
        id: RuleId,
    },
    UnknownDefinition {
        owner: String,
        id: DefinitionId,
    },
    DuplicateRule(RuleId),
    DuplicateDefinition(DefinitionId),
    DocumentMismatch {
        model: String,
        source: Option<String>,
    },
}

pub fn validate(model: &SemanticModel, source: &LegalDocument) -> Vec<Issue> {
    let mut issues = Vec::new();
    if source.version_id.as_deref() != Some(model.document.as_str()) {
        issues.push(Issue::DocumentMismatch {
            model: model.document.clone(),
            source: source.version_id.clone(),
        });
    }

    let ids: HashSet<&StableId> = source.stable_ids().into_iter().collect();
    let mut rules = HashSet::new();
    for r in &model.rules {
        if !rules.insert(&r.id) {
            issues.push(Issue::DuplicateRule(r.id.clone()));
        }
    }
    let mut defs = HashSet::new();
    for d in &model.definitions {
        if !defs.insert(&d.id) {
            issues.push(Issue::DuplicateDefinition(d.id.clone()));
        }
    }

    let mut cx = Cx {
        ids: &ids,
        rules: &rules,
        defs: &defs,
        issues: &mut issues,
        owner: String::new(),
    };
    for r in &model.rules {
        cx.owner = r.id.0.clone();
        cx.stable(&r.provenance.source);
        if let Some(s) = &r.subject {
            cx.entity(s);
        }
        cx.expr(&r.condition);
        cx.effect(&r.effect);
        for o in &r.overrides {
            if let Override::Rule(id) = o {
                cx.rule(id);
            }
        }
    }
    for d in &model.definitions {
        cx.owner = d.id.0.clone();
        cx.stable(&d.provenance.source);
        for s in &d.scope {
            cx.stable(s);
        }
        if let DefinitionBody::Expr(e) = &d.body {
            cx.expr(e);
        }
    }
    for u in &model.unknowns {
        cx.owner = "unknown".into();
        cx.stable(&u.provenance.source);
    }
    issues
}

struct Cx<'a> {
    ids: &'a HashSet<&'a StableId>,
    rules: &'a HashSet<&'a RuleId>,
    defs: &'a HashSet<&'a DefinitionId>,
    issues: &'a mut Vec<Issue>,
    owner: String,
}

impl Cx<'_> {
    fn stable(&mut self, id: &StableId) {
        if !self.ids.contains(id) {
            self.issues.push(Issue::UnknownStableId {
                owner: self.owner.clone(),
                id: id.clone(),
            });
        }
    }
    fn rule(&mut self, id: &RuleId) {
        if !self.rules.contains(id) {
            self.issues.push(Issue::UnknownRule {
                owner: self.owner.clone(),
                id: id.clone(),
            });
        }
    }
    fn definition(&mut self, id: &DefinitionId) {
        if !self.defs.contains(id) {
            self.issues.push(Issue::UnknownDefinition {
                owner: self.owner.clone(),
                id: id.clone(),
            });
        }
    }
    fn entity(&mut self, e: &EntityRef) {
        if let EntityRef::Definition(d) = e {
            self.definition(d);
        }
    }
    fn reference(&mut self, r: &RefTarget) {
        match r {
            RefTarget::Rule(id) => self.rule(id),
            RefTarget::Definition(id) => self.definition(id),
            RefTarget::Scope(s) | RefTarget::Provision(s) => self.stable(s),
            RefTarget::External { .. } | RefTarget::Relative(_) => {}
        }
    }
    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::And(xs) | Expr::Or(xs) => xs.iter().for_each(|x| self.expr(x)),
            Expr::Not(x) => self.expr(x),
            Expr::Pred(p) => self.predicate(p),
            Expr::Cmp(a, _, b) => {
                self.value(a);
                self.value(b);
            }
            Expr::Ref(r) => self.reference(r),
            Expr::True | Expr::False | Expr::Time(_) | Expr::Unknown(_) => {}
        }
    }
    fn predicate(&mut self, p: &Predicate) {
        p.args.iter().for_each(|(_, a)| self.arg(a));
    }
    fn arg(&mut self, a: &Arg) {
        match a {
            Arg::Entity(e) => self.entity(e),
            Arg::Value(v) => self.value(v),
            Arg::Expr(e) => self.expr(e),
            Arg::Ref(r) => self.reference(r),
            Arg::List(xs) => xs.iter().for_each(|x| self.arg(x)),
            Arg::Text(_) => {}
        }
    }
    fn value(&mut self, v: &Value) {
        match v {
            Value::RuleValue(id) => self.rule(id),
            Value::Add(a, b) | Value::Sub(a, b) => {
                self.value(a);
                self.value(b);
            }
            Value::Interest {
                principal, rate, ..
            } => {
                self.value(principal);
                self.value(rate);
            }
            _ => {}
        }
    }
    fn action(&mut self, a: &Action) {
        a.args.iter().for_each(|(_, x)| self.arg(x));
    }
    fn target(&mut self, t: &Target) {
        match t {
            Target::RuleEffect(id) => self.rule(id),
            Target::Fact(f) => self.predicate(&f.pred),
            Target::Contract(_) => {}
        }
    }
    fn effect(&mut self, e: &Effect) {
        match e {
            Effect::Obligation(a) | Effect::Prohibition(a) | Effect::Permission(a) => {
                self.action(a)
            }
            Effect::Power { action, exercise } => {
                self.action(action);
                if let Some(x) = exercise {
                    self.effect(x);
                }
            }
            Effect::Set { value, .. } => self.value(value),
            Effect::Deem(f) | Effect::Presume(f) => self.predicate(&f.pred),
            Effect::Void(t) | Effect::Preserve(t) => self.target(t),
            Effect::Exception(id) | Effect::SameAs(id) => self.rule(id),
            Effect::DeemAndApply { from, to, apply } => {
                self.predicate(&from.pred);
                self.predicate(&to.pred);
                self.rule(apply);
            }
            Effect::Apply(r) => self.reference(r),
            Effect::ApplyMutatis { rules, substitute } => {
                rules.iter().for_each(|r| self.rule(r));
                for (a, b) in substitute {
                    self.entity(a);
                    self.entity(b);
                }
            }
            Effect::ApplyExternal { .. } | Effect::Unknown(_) => {}
        }
    }
}
