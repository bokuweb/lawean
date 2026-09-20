//! Semantic IR → SMT-LIB2。意味論は docs/07-verification.md。

use lawean_resolve::ResolvedModel;
use lawean_semantic::*;
use std::collections::BTreeSet;
use std::fmt::Write;

#[derive(Debug, Clone, Default)]
pub struct Smt {
    /// 宣言（重複なし、順序安定）
    pub decls: BTreeSet<String>,
    pub asserts: Vec<String>,
    /// 各 assert に付けるコメント（同じ index）
    pub comments: Vec<String>,
}

impl Smt {
    pub fn declare(&mut self, sort: &str, name: &str) {
        self.decls.insert(format!("(declare-const {name} {sort})"));
    }
    pub fn assert(&mut self, comment: impl Into<String>, s: String) {
        self.comments.push(comment.into());
        self.asserts.push(s);
    }
    pub fn render(&self) -> String {
        let mut out = String::new();
        for d in &self.decls {
            out.push_str(d);
            out.push('\n');
        }
        for (c, a) in self.comments.iter().zip(&self.asserts) {
            let _ = writeln!(out, "; {c}\n(assert {a})");
        }
        out
    }
}

/// SMT-LIB の quoted symbol。`|` と `\` 以外は何でも入る
pub fn sym(s: &str) -> String {
    format!("|{}|", s.replace(['|', '\\'], "_"))
}

pub fn applies(id: &RuleId) -> String {
    sym(&format!("applies:{}", id.0))
}

pub struct Compiler<'a> {
    pub rm: &'a ResolvedModel<'a>,
    pub smt: Smt,
}

impl<'a> Compiler<'a> {
    pub fn new(rm: &'a ResolvedModel<'a>) -> Self {
        Compiler {
            rm,
            smt: Smt::default(),
        }
    }

    /// モデル全体（applies の定義と効果）を出力する
    pub fn compile_model(&mut self) {
        for r in &self.rm.model.rules {
            let a = applies(&r.id);
            self.smt.declare("Bool", &a);
            let cond = self.expr(&r.condition);
            let mut parts = vec![cond];
            for x in self.rm.exceptions_of(&r.id) {
                let ax = applies(x);
                self.smt.declare("Bool", &ax);
                parts.push(format!("(not {ax})"));
            }
            let body = if parts.len() == 1 {
                parts.pop().unwrap()
            } else {
                format!("(and {})", parts.join(" "))
            };
            self.smt
                .assert(format!("{} applies", r.id.0), format!("(= {a} {body})"));
            let eff = self.effect(&r.effect, &r.id);
            if let Some(e) = eff {
                self.smt
                    .assert(format!("{} effect", r.id.0), format!("(=> {a} {e})"));
            }
        }
    }

    fn bool_const(&mut self, name: &str) -> String {
        let s = sym(name);
        self.smt.declare("Bool", &s);
        s
    }

    fn int_const(&mut self, name: &str) -> String {
        let s = sym(name);
        self.smt.declare("Int", &s);
        s
    }

    pub fn expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::True => "true".into(),
            Expr::False => "false".into(),
            Expr::And(xs) => {
                let v: Vec<String> = xs.iter().map(|x| self.expr(x)).collect();
                format!("(and {})", v.join(" "))
            }
            Expr::Or(xs) => {
                let v: Vec<String> = xs.iter().map(|x| self.expr(x)).collect();
                format!("(or {})", v.join(" "))
            }
            Expr::Not(x) => format!("(not {})", self.expr(x)),
            Expr::Pred(p) => {
                let name = pred_name(p);
                self.bool_const(&name)
            }
            Expr::Cmp(a, op, b) => {
                let (a, b) = (self.value(a), self.value(b));
                let op = match op {
                    CmpOp::Lt => "<",
                    CmpOp::Le => "<=",
                    CmpOp::Eq => "=",
                    CmpOp::Ge => ">=",
                    CmpOp::Gt => ">",
                };
                format!("({op} {a} {b})")
            }
            Expr::Ref(RefTarget::Rule(id)) => {
                let a = applies(id);
                self.smt.declare("Bool", &a);
                a
            }
            Expr::Ref(other) => self.bool_const(&format!("ref:{other:?}")),
            Expr::Time(t) => self.bool_const(&format!("time:{t:?}")),
            Expr::Unknown(u) => self.bool_const(&format!("unknown:{}", u.text)),
        }
    }

    pub fn value(&mut self, v: &Value) -> String {
        match v {
            Value::Int(i) => int_lit(*i),
            Value::Duration(d) => int_lit(months(d)),
            Value::Period(p) => int_lit(months(&p.length)),
            Value::PeriodValue(PeriodValue::Definite(p)) => int_lit(months(&p.length)),
            // 「期間の定めがない」は -1 で表す
            Value::PeriodValue(PeriodValue::Indefinite) => int_lit(-1),
            Value::RatePerMille(r) => int_lit(*r as i64),
            Value::Var(name) => self.int_const(&format!("a:{name}")),
            Value::RuleValue(id) => match self.rm.rule(id).map(|r| &r.effect) {
                Some(Effect::Set { value, .. }) => self.value(value),
                _ => self.int_const(&format!("rulevalue:{}", id.0)),
            },
            Value::Add(a, b) => format!("(+ {} {})", self.value(a), self.value(b)),
            Value::Sub(a, b) => format!("(- {} {})", self.value(a), self.value(b)),
            Value::Interest { .. } => self.int_const("interest"),
            Value::Unknown(u) => self.int_const(&format!("unknown:{}", u.text)),
        }
    }

    /// 効果を「applies を前件にした帰結」の形で返す。None なら追加の主張なし
    fn effect(&mut self, e: &Effect, owner: &RuleId) -> Option<String> {
        Some(match e {
            Effect::Set { attribute, value } => {
                let a = self.int_const(&format!("a:{attribute}"));
                let v = self.value(value);
                format!("(= {a} {v})")
            }
            Effect::Deem(f) | Effect::Presume(f) => {
                let name = pred_name(&f.pred);
                self.bool_const(&name)
            }
            Effect::Void(t) => self.bool_const(&format!("void:{}", target_name(t))),
            Effect::Preserve(t) => self.bool_const(&format!("preserve:{}", target_name(t))),
            Effect::Exception(_) => return None,
            Effect::SameAs(other) | Effect::DeemAndApply { apply: other, .. } => {
                let ef = self.rm.rule(other).map(|r| r.effect.clone())?;
                return self.effect(&ef, owner);
            }
            _ => self.bool_const(&format!("eff:{}", owner.0)),
        })
    }
}

fn int_lit(i: i64) -> String {
    if i < 0 {
        format!("(- {})", -i)
    } else {
        i.to_string()
    }
}

pub fn months(d: &Duration) -> i64 {
    match d.unit {
        Unit::Year => d.length as i64 * 12,
        Unit::Month => d.length as i64,
        // 日・週・時は v0.1 では月に丸めない。0 にせず目立つ値にする
        Unit::Week | Unit::Day | Unit::Hour => -1000 - d.length as i64,
    }
}

fn target_name(t: &Target) -> String {
    match t {
        Target::Contract(s) => s.clone(),
        Target::RuleEffect(id) => id.0.clone(),
        Target::Fact(f) => pred_name(&f.pred),
    }
}

/// 述語 + 引数 → 変数名。同じ名前・同じ引数なら同じ変数
pub fn pred_name(p: &Predicate) -> String {
    let mut s = format!("p:{}", p.name);
    if !p.args.is_empty() {
        s.push('(');
        let args: Vec<String> = p
            .args
            .iter()
            .map(|(k, a)| format!("{k}={}", arg_name(a)))
            .collect();
        s.push_str(&args.join(","));
        s.push(')');
    }
    s
}

fn arg_name(a: &Arg) -> String {
    match a {
        Arg::Entity(EntityRef::Definition(d)) => d.0.clone(),
        Arg::Entity(EntityRef::Named(n)) => n.clone(),
        Arg::Text(t) => t.clone(),
        Arg::Value(v) => format!("{v:?}"),
        Arg::Expr(e) => format!("{e:?}"),
        Arg::Ref(r) => format!("{r:?}"),
        Arg::List(xs) => xs.iter().map(arg_name).collect::<Vec<_>>().join("+"),
    }
}
