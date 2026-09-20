//! 手書きで Semantic IR を組むための短い構築子。docs/03-examples の記法にできるだけ寄せる。

use crate::ir::*;
use lawean_source::StableId;

pub fn rule(id: &str) -> Rule {
    Rule {
        id: RuleId(id.into()),
        subject: None,
        condition: Expr::True,
        effect: Effect::Unknown(UnknownExpr {
            kind: UnknownKind::Unparsed,
            text: String::new(),
        }),
        overrides: Vec::new(),
        temporal: None,
        interpretations: Vec::new(),
        provenance: Provenance {
            source: StableId(String::new()),
            confidence: Confidence::Low,
            by: Author::Human(String::new()),
            note: None,
        },
    }
}

impl Rule {
    pub fn subject(mut self, e: EntityRef) -> Self {
        self.subject = Some(e);
        self
    }
    pub fn condition(mut self, c: Expr) -> Self {
        self.condition = c;
        self
    }
    pub fn effect(mut self, e: Effect) -> Self {
        self.effect = e;
        self
    }
    pub fn overrides(mut self, ids: &[&str]) -> Self {
        self.overrides
            .extend(ids.iter().map(|i| Override::Rule(RuleId((*i).into()))));
        self
    }
    pub fn overrides_contract(mut self) -> Self {
        self.overrides.push(Override::Contract);
        self
    }
    pub fn temporal(mut self, t: RuleTemporal) -> Self {
        self.temporal = Some(t);
        self
    }
    pub fn interpretation(
        mut self,
        authority: Authority,
        statement: &str,
        confidence: Confidence,
    ) -> Self {
        self.interpretations.push(Interpretation {
            authority,
            statement: statement.into(),
            confidence,
        });
        self
    }
    pub fn provenance(mut self, source: &str, confidence: Confidence, by: &str) -> Self {
        self.provenance = Provenance {
            source: StableId(source.into()),
            confidence,
            by: Author::Human(by.into()),
            note: None,
        };
        self
    }
    pub fn note(mut self, n: &str) -> Self {
        self.provenance.note = Some(n.into());
        self
    }
}

pub fn definition(id: &str, term: &str, scope: &[&str], body: DefinitionBody) -> Definition {
    Definition {
        id: DefinitionId(id.into()),
        term: term.into(),
        scope: scope.iter().map(|s| StableId((*s).into())).collect(),
        body,
        provenance: Provenance {
            source: StableId(String::new()),
            confidence: Confidence::Low,
            by: Author::Human(String::new()),
            note: None,
        },
    }
}

impl Definition {
    pub fn provenance(mut self, source: &str, confidence: Confidence, by: &str) -> Self {
        self.provenance = Provenance {
            source: StableId(source.into()),
            confidence,
            by: Author::Human(by.into()),
            note: None,
        };
        self
    }
}

// ---- Expr

pub fn and(xs: impl IntoIterator<Item = Expr>) -> Expr {
    Expr::And(xs.into_iter().collect())
}
pub fn or(xs: impl IntoIterator<Item = Expr>) -> Expr {
    Expr::Or(xs.into_iter().collect())
}
pub fn not(x: Expr) -> Expr {
    Expr::Not(Box::new(x))
}
pub fn pred(name: &str) -> Predicate {
    Predicate {
        name: name.into(),
        args: Vec::new(),
    }
}
impl Predicate {
    pub fn arg(mut self, key: &str, a: Arg) -> Self {
        self.args.push((key.into(), a));
        self
    }
    pub fn expr(self) -> Expr {
        Expr::Pred(self)
    }
    pub fn fact(self) -> Fact {
        Fact { pred: self }
    }
}
pub fn cmp(a: Value, op: CmpOp, b: Value) -> Expr {
    Expr::Cmp(a, op, b)
}
pub fn rule_ref(id: &str) -> Expr {
    Expr::Ref(RefTarget::Rule(RuleId(id.into())))
}
pub fn unknown(kind: UnknownKind, text: &str) -> UnknownExpr {
    UnknownExpr {
        kind,
        text: text.into(),
    }
}
pub fn intentional(text: &str) -> Expr {
    Expr::Unknown(unknown(UnknownKind::Intentional, text))
}

// ---- Arg / Entity / Value

pub fn def(id: &str) -> EntityRef {
    EntityRef::Definition(DefinitionId(id.into()))
}
pub fn named(name: &str) -> EntityRef {
    EntityRef::Named(name.into())
}
pub fn entity(e: EntityRef) -> Arg {
    Arg::Entity(e)
}
pub fn text(t: &str) -> Arg {
    Arg::Text(t.into())
}
pub fn value(v: Value) -> Arg {
    Arg::Value(v)
}
pub fn expr_arg(e: Expr) -> Arg {
    Arg::Expr(Box::new(e))
}
pub fn var(name: &str) -> Value {
    Value::Var(name.into())
}
pub fn rule_value(id: &str) -> Value {
    Value::RuleValue(RuleId(id.into()))
}
pub fn years(n: u32) -> Duration {
    Duration {
        length: n,
        unit: Unit::Year,
    }
}
pub fn months(n: u32) -> Duration {
    Duration {
        length: n,
        unit: Unit::Month,
    }
}
pub fn event(name: &str) -> Event {
    Event(name.into())
}
pub fn after(from: &str, length: Duration) -> Period {
    Period {
        from: event(from),
        length,
        direction: Direction::Forward,
    }
}
pub fn before(from: &str, length: Duration) -> Period {
    Period {
        from: event(from),
        length,
        direction: Direction::Backward,
    }
}
pub fn window(from: Period, to: Period) -> Window {
    Window { from, to }
}

// ---- Effect

pub fn action(verb: &str) -> Action {
    Action {
        verb: verb.into(),
        args: Vec::new(),
    }
}
impl Action {
    pub fn arg(mut self, key: &str, a: Arg) -> Self {
        self.args.push((key.into(), a));
        self
    }
}
pub fn set(attribute: &str, v: Value) -> Effect {
    Effect::Set {
        attribute: attribute.into(),
        value: v,
    }
}
pub fn exception(id: &str) -> Effect {
    Effect::Exception(RuleId(id.into()))
}
pub fn same_as(id: &str) -> Effect {
    Effect::SameAs(RuleId(id.into()))
}
pub fn power(a: Action) -> Effect {
    Effect::Power {
        action: a,
        exercise: None,
    }
}
pub fn deem_and_apply(from: Fact, to: Fact, apply: &str) -> Effect {
    Effect::DeemAndApply {
        from,
        to,
        apply: RuleId(apply.into()),
    }
}
