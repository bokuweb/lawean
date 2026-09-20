//! LLM 出力の型、層 1 との突き合わせ（棄却条件）、Semantic IR への適用。

use crate::prompt::{allowed_kinds, ParagraphInput};
use lawean_extract::{rule_id, Skeleton};
use lawean_resolve::Resolution;
use lawean_semantic::build;
use lawean_semantic::*;
use lawean_source::StableId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Output {
    pub sentences: Vec<SentenceOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SentenceOut {
    pub sentence: String,
    pub rules: Vec<RuleOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleOut {
    pub suffix: String,
    pub subject: String,
    pub condition: ConditionOut,
    pub effect: EffectOut,
    pub temporal: Vec<TemporalOut>,
    pub unknowns: Vec<UnknownOut>,
    pub confidence: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConditionOut {
    pub all_of: Vec<AnyOfOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnyOfOut {
    pub any_of: Vec<PredOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PredOut {
    pub name: String,
    pub args: Vec<ArgOut>,
    pub negated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArgOut {
    pub key: String,
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectOut {
    pub kind: String,
    pub head: String,
    pub args: Vec<ArgOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TemporalOut {
    pub kind: String,
    pub from: String,
    pub length: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnknownOut {
    pub kind: String,
    pub text: String,
}

/// 文単位の棄却理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    pub sentence: String,
    pub reason: String,
}

/// 層 1 と突き合わせ、採用できる文と棄却理由を返す
pub fn validate(
    input: &ParagraphInput<'_>,
    out: &Output,
) -> (Vec<(String, Vec<RuleOut>)>, Vec<Rejection>) {
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let labels: Vec<String> = input.skeletons.iter().map(|s| input.label(s)).collect();

    for label in &labels {
        let Some(so) = out.sentences.iter().find(|s| &s.sentence == label) else {
            rejected.push(Rejection {
                sentence: label.clone(),
                reason: "出力に無い".into(),
            });
            continue;
        };
        let sk = input.skeletons[labels.iter().position(|l| l == label).unwrap()];
        match check_sentence(input, sk, so) {
            Ok(()) => accepted.push((label.clone(), so.rules.clone())),
            Err(reason) => rejected.push(Rejection {
                sentence: label.clone(),
                reason,
            }),
        }
    }
    for so in &out.sentences {
        if !labels.contains(&so.sentence) {
            rejected.push(Rejection {
                sentence: so.sentence.clone(),
                reason: "入力に無い文".into(),
            });
        }
    }
    (accepted, rejected)
}

fn check_sentence(
    input: &ParagraphInput<'_>,
    sk: &Skeleton,
    so: &SentenceOut,
) -> Result<(), String> {
    if so.rules.is_empty() {
        return Err("Rule が無い".into());
    }
    let allowed = allowed_kinds(sk.effect);
    let known_refs: Vec<&str> = sk
        .references
        .iter()
        .flat_map(|(_, r)| match r {
            Resolution::Internal(ids) => ids.iter().map(|i| i.0.as_str()).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect();
    let defs: Vec<&str> = input.definitions.iter().map(|d| d.id.0.as_str()).collect();
    for r in &so.rules {
        if !allowed.contains(&r.effect.kind.as_str()) {
            return Err(format!(
                "effect.kind {} は層 1 の {:?} と矛盾",
                r.effect.kind, sk.effect
            ));
        }
        if r.confidence == "high" {
            return Err("confidence=high は不可".into());
        }
        if r.condition.all_of.is_empty()
            && !sk.conditions.is_empty()
            && !r.unknowns.iter().any(|u| u.kind == "unparsed")
        {
            return Err("層 1 に条件節があるのに条件が空で unparsed も無い".into());
        }
        let args = r
            .condition
            .all_of
            .iter()
            .flat_map(|a| a.any_of.iter())
            .flat_map(|p| p.args.iter())
            .chain(r.effect.args.iter());
        for a in args {
            match a.kind.as_str() {
                "ref"
                    if !known_refs
                        .iter()
                        .any(|k| *k == a.value || k.ends_with(&a.value)) =>
                {
                    return Err(format!("参照 {} は層 1 に無い", a.value));
                }
                "definition" if !defs.contains(&a.value.as_str()) => {
                    return Err(format!("定義語 {} はこの位置で有効でない", a.value));
                }
                _ => {}
            }
        }
        if r.subject.starts_with("D:") && !defs.contains(&r.subject.as_str()) {
            return Err(format!("主体の定義語 {} はこの位置で有効でない", r.subject));
        }
    }
    Ok(())
}

/// 採用された文の Rule を Semantic IR に変換する。ID は骨組みと同じ（`S:<stable_id>` + suffix）
pub fn to_rules(
    input: &ParagraphInput<'_>,
    accepted: &[(String, Vec<RuleOut>)],
    model_name: &str,
) -> Vec<Rule> {
    let mut out = Vec::new();
    for (label, rules) in accepted {
        let sk = input
            .skeletons
            .iter()
            .find(|s| input.label(s) == *label)
            .unwrap();
        for r in rules {
            let id = format!(
                "{}{}",
                rule_id(&sk.sentence).0,
                if r.suffix.is_empty() {
                    String::new()
                } else {
                    format!("-{}", r.suffix)
                }
            );
            let mut rule = build::rule(&id)
                .condition(condition(&r.condition))
                .effect(effect(&r.effect))
                .provenance(&sk.sentence.0, confidence(&r.confidence), "");
            rule.provenance.by = Author::Llm(model_name.into());
            if !r.note.is_empty() {
                rule.provenance.note = Some(r.note.clone());
            }
            if !r.subject.is_empty() {
                rule.subject = Some(entity(&r.subject));
            }
            if let Some(t) = r.temporal.first() {
                rule.temporal = temporal(t);
            }
            // 骨組みが持っていた overrides（ただし書き → 本文、〜にかかわらず）はそのまま引き継ぐ
            out.push(rule);
        }
    }
    out
}

fn confidence(s: &str) -> Confidence {
    match s {
        "medium" => Confidence::Medium,
        _ => Confidence::Low,
    }
}

fn entity(s: &str) -> EntityRef {
    if let Some(id) = s.strip_prefix("D:") {
        EntityRef::Definition(DefinitionId(format!("D:{id}")))
    } else {
        EntityRef::Named(s.to_string())
    }
}

fn arg(a: &ArgOut) -> Arg {
    match a.kind.as_str() {
        "definition" => Arg::Entity(entity(&a.value)),
        "entity" => Arg::Entity(EntityRef::Named(a.value.clone())),
        "ref" => Arg::Ref(RefTarget::Provision(StableId(a.value.clone()))),
        "duration" => match duration(&a.value) {
            Some(d) => Arg::Value(Value::Duration(d)),
            None => Arg::Value(Value::Unknown(build::unknown(
                UnknownKind::Unparsed,
                &a.value,
            ))),
        },
        "var" => Arg::Value(Value::Var(a.value.clone())),
        "money" => Arg::Value(Value::Var(a.value.clone())),
        "unknown" => Arg::Expr(Box::new(Expr::Unknown(build::unknown(
            UnknownKind::Intentional,
            &a.value,
        )))),
        _ => Arg::Text(a.value.clone()),
    }
}

pub fn duration(s: &str) -> Option<Duration> {
    let (n, unit) = s.trim().split_once(' ')?;
    let length: u32 = n.parse().ok()?;
    let unit = match unit.trim_end_matches('s') {
        "year" => Unit::Year,
        "month" => Unit::Month,
        "week" => Unit::Week,
        "day" => Unit::Day,
        "hour" => Unit::Hour,
        _ => return None,
    };
    Some(Duration { length, unit })
}

fn pred(p: &PredOut) -> Expr {
    let mut q = build::pred(&p.name);
    for a in &p.args {
        q = q.arg(&a.key, arg(a));
    }
    if p.negated {
        build::not(q.expr())
    } else {
        q.expr()
    }
}

fn condition(c: &ConditionOut) -> Expr {
    let mut conj: Vec<Expr> = Vec::new();
    for a in &c.all_of {
        match a.any_of.len() {
            0 => {}
            1 => conj.push(pred(&a.any_of[0])),
            _ => conj.push(build::or(a.any_of.iter().map(pred))),
        }
    }
    match conj.len() {
        0 => Expr::True,
        1 => conj.pop().unwrap(),
        _ => build::and(conj),
    }
}

fn action(e: &EffectOut) -> Action {
    let mut a = build::action(&e.head);
    for x in &e.args {
        a = a.arg(&x.key, arg(x));
    }
    a
}

fn fact(e: &EffectOut) -> Fact {
    let mut p = build::pred(&e.head);
    for x in &e.args {
        p = p.arg(&x.key, arg(x));
    }
    p.fact()
}

fn effect(e: &EffectOut) -> Effect {
    match e.kind.as_str() {
        "obligation" => Effect::Obligation(action(e)),
        "prohibition" => Effect::Prohibition(action(e)),
        "permission" => Effect::Permission(action(e)),
        "power" => Effect::Power {
            action: action(e),
            exercise: None,
        },
        "deem" => Effect::Deem(fact(e)),
        "presume" => Effect::Presume(fact(e)),
        "void" => Effect::Void(Target::Contract(e.head.clone())),
        "preserve" => Effect::Preserve(Target::Contract(e.head.clone())),
        "former_example" => Effect::ApplyExternal {
            law: "従前の例".into(),
            topic: e.head.clone(),
        },
        "set" | "follow" | "suffice" | "lapse" => {
            let value = e
                .args
                .iter()
                .find(|a| a.key == "value")
                .map(|a| match arg(a) {
                    Arg::Value(v) => v,
                    Arg::Text(t) => Value::Var(t),
                    _ => Value::Unknown(build::unknown(UnknownKind::Unparsed, &a.value)),
                })
                .unwrap_or_else(|| Value::Unknown(build::unknown(UnknownKind::Unparsed, "")));
            Effect::Set {
                attribute: e.head.clone(),
                value,
            }
        }
        // 上書き関係は層 1 が持つ。効果そのものは Unknown のまま
        _ => Effect::Unknown(build::unknown(UnknownKind::Unparsed, &e.head)),
    }
}

fn temporal(t: &TemporalOut) -> Option<RuleTemporal> {
    // v0.1: Rule の時間属性としては起算点だけ拾う。期間の値は condition / effect の args に入っている
    if t.from.is_empty() {
        return None;
    }
    Some(RuleTemporal {
        effective_from: Some(Event(t.from.clone())),
        effective_to: None,
        facts_before: None,
    })
}
