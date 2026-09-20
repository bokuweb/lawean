//! 文ごとの層 1 抽出結果（骨組み）と、それを Semantic IR の Rule に落とす変換。

use crate::clause::{self, Concession, ConditionClause, OverrideSpan};
use crate::effect::{self, EffectKind};
use lawean_resolve::{resolve_sentence_with, Antecedent, Index, Resolution};
use lawean_semantic::build;
use lawean_semantic::*;
use lawean_source::{LegalDocument, SentenceFunction, StableId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideTarget {
    /// この法令内の条・項（参照解決済み）
    Provisions(Vec<StableId>),
    /// 他法令の規定
    External(String),
    Contract,
    /// 「〜の規定にかかわらず」だが参照が解決できなかった
    Unresolved(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    pub sentence: StableId,
    pub paragraph: StableId,
    pub function: SentenceFunction,
    pub text: String,
    pub effect: EffectKind,
    pub effect_tail: Option<String>,
    pub conditions: Vec<ConditionClause>,
    pub overrides: Vec<OverrideTarget>,
    pub concessions: Vec<Concession>,
    /// 文中の参照（解決済み）
    pub references: Vec<(String, Resolution)>,
    pub in_item: bool,
    pub in_column: bool,
}

/// 文書全体を項ごとに抽出する。参照の先行詞は項の中で引き継ぐ
pub fn extract(doc: &LegalDocument) -> Vec<Skeleton> {
    let index = Index::build(doc);
    let mut out = Vec::new();
    for g in doc.sentence_groups() {
        let mut ante = Antecedent::default();
        for s in &g.sentences {
            let text = s.sentence.plain_text();
            let refs = resolve_sentence_with(&index, &s.sentence.stable_id, &text, &mut ante);
            let (kind, tail) = effect::classify(&text);
            let overrides = clause::overrides(&text)
                .into_iter()
                .map(|o| match o {
                    OverrideSpan::Contract { .. } => OverrideTarget::Contract,
                    OverrideSpan::Rule {
                        start,
                        end,
                        text: t,
                    } => {
                        // X の範囲に入る参照の解決結果を集める
                        let mut ids = Vec::new();
                        let mut ext = None;
                        for r in &refs {
                            if r.span.start >= start && r.span.end <= end {
                                match &r.resolution {
                                    Resolution::Internal(v) => ids.extend(v.iter().cloned()),
                                    Resolution::External { law, .. } => ext = Some(law.clone()),
                                    Resolution::Unresolved(_) => {}
                                }
                            }
                        }
                        if !ids.is_empty() {
                            OverrideTarget::Provisions(ids)
                        } else if let Some(l) = ext {
                            OverrideTarget::External(l)
                        } else {
                            OverrideTarget::Unresolved(t)
                        }
                    }
                })
                .collect();
            out.push(Skeleton {
                sentence: s.sentence.stable_id.clone(),
                paragraph: g.paragraph.clone(),
                function: s.sentence.function,
                effect: kind,
                effect_tail: tail,
                conditions: clause::conditions(&text),
                overrides,
                concessions: clause::concessions(&text),
                references: refs
                    .into_iter()
                    .map(|r| (r.span.text, r.resolution))
                    .collect(),
                in_item: s.in_item,
                in_column: s.in_column,
                text,
            });
        }
    }
    out
}

/// 骨組み Rule の ID は文の stable_id から機械的に決める
pub fn rule_id(sentence: &StableId) -> RuleId {
    RuleId(format!("S:{}", sentence.0))
}

/// 骨組みを Semantic IR に落とす。条件・効果の中身は `Unknown(Unparsed)`、
/// 効果の種別・overrides・Provenance は層 1 の結果で埋める。層 2（LLM）はこの Rule を上書きしていく
pub fn to_model(doc: &LegalDocument, skeletons: &[Skeleton]) -> SemanticModel {
    let by_sentence: BTreeMap<&StableId, &Skeleton> =
        skeletons.iter().map(|s| (&s.sentence, s)).collect();
    // 条・項の stable_id → その配下で Rule になる文
    let rule_sentences: Vec<&StableId> = skeletons
        .iter()
        .filter(|s| is_rule(s))
        .map(|s| &s.sentence)
        .collect();
    let under = |scope: &StableId| -> Vec<RuleId> {
        rule_sentences
            .iter()
            .filter(|s| lawean_resolve::model::is_under(s, scope))
            .map(|s| rule_id(s))
            .collect()
    };

    let mut rules = Vec::new();
    for sk in skeletons.iter().filter(|s| is_rule(s)) {
        let cond_text: Vec<&str> = sk.conditions.iter().map(|c| c.text.as_str()).collect();
        let condition = if cond_text.is_empty() {
            Expr::True
        } else {
            Expr::Unknown(build::unknown(
                UnknownKind::Unparsed,
                &cond_text.join(" / "),
            ))
        };
        let mut r = build::rule(&rule_id(&sk.sentence).0)
            .condition(condition)
            .effect(effect_of(sk, &under))
            .provenance(&sk.sentence.0, Confidence::Medium, "lawean-extract");
        r.provenance.by = Author::Parser("lawean-extract/v0.1".into());
        // ただし書きは同じ項の本文を上書きする
        if sk.function == SentenceFunction::Proviso {
            let mains: Vec<&str> = by_sentence
                .values()
                .filter(|o| {
                    o.paragraph == sk.paragraph
                        && o.function != SentenceFunction::Proviso
                        && is_rule(o)
                })
                .map(|o| o.sentence.0.as_str())
                .collect();
            for m in mains {
                r.overrides
                    .push(Override::Rule(rule_id(&StableId(m.into()))));
            }
        }
        for o in &sk.overrides {
            match o {
                OverrideTarget::Provisions(ids) => {
                    for id in ids {
                        for rid in under(id) {
                            if rid != r.id {
                                r.overrides.push(Override::Rule(rid));
                            }
                        }
                    }
                }
                OverrideTarget::Contract => r.overrides.push(Override::Contract),
                OverrideTarget::External(_) | OverrideTarget::Unresolved(_) => {}
            }
        }
        if !sk.concessions.is_empty() {
            let n: Vec<&str> = sk.concessions.iter().map(|c| c.text.as_str()).collect();
            r.provenance.note = Some(format!("譲歩: {}", n.join(" / ")));
        }
        rules.push(r);
    }

    let unknowns = skeletons
        .iter()
        .filter(|s| !is_rule(s))
        .map(|s| UnknownNode {
            unknown: build::unknown(UnknownKind::Unparsed, &s.text),
            provenance: Provenance {
                source: s.sentence.clone(),
                confidence: Confidence::Low,
                by: Author::Parser("lawean-extract/v0.1".into()),
                note: Some(format!("{:?}", s.effect)),
            },
        })
        .collect();

    SemanticModel {
        document: doc.version_id.clone().unwrap_or_default(),
        definitions: Vec::new(),
        rules,
        unknowns,
    }
}

fn is_rule(s: &Skeleton) -> bool {
    !matches!(
        s.effect,
        EffectKind::Fragment | EffectKind::Definition | EffectKind::Enforce | EffectKind::Repeal
    )
}

fn effect_of(sk: &Skeleton, under: &dyn Fn(&StableId) -> Vec<RuleId>) -> Effect {
    let u = || {
        build::unknown(
            UnknownKind::Unparsed,
            sk.effect_tail.as_deref().unwrap_or(""),
        )
    };
    let act = || {
        build::action(sk.effect_tail.as_deref().unwrap_or(""))
            .arg("text", Arg::Text(sk.text.clone()))
    };
    let fact = || {
        build::pred(sk.effect_tail.as_deref().unwrap_or(""))
            .arg("text", Arg::Text(sk.text.clone()))
            .fact()
    };
    let first_ref = || {
        sk.references.iter().find_map(|(_, r)| match r {
            Resolution::Internal(ids) => ids.first().cloned(),
            _ => None,
        })
    };
    match sk.effect {
        EffectKind::Obligation => Effect::Obligation(act()),
        EffectKind::Prohibition | EffectKind::CanNot => Effect::Prohibition(act()),
        EffectKind::Can => Effect::Permission(act()),
        EffectKind::Deem => Effect::Deem(fact()),
        EffectKind::Presume => Effect::Presume(fact()),
        // 「この限りでない」「適用しない」「〜に限る」は本文・参照先への例外。
        // 上書き関係は overrides 側に張られるので、効果そのものは Unknown のまま層 2 に渡す
        EffectKind::Exception | EffectKind::NotApply | EffectKind::Limit => Effect::Unknown(u()),
        EffectKind::Apply => match first_ref() {
            Some(id) => Effect::Apply(RefTarget::Provision(id)),
            None => Effect::Unknown(u()),
        },
        EffectKind::ApplyMutatis => Effect::ApplyMutatis {
            rules: Vec::new(),
            substitute: Vec::new(),
        },
        // 読み替え元・先の事実は層 2 で埋める。適用先の Rule は参照から引く
        EffectKind::DeemAndApply => {
            match first_ref().and_then(|id| under(&id).into_iter().next()) {
                Some(apply) => Effect::DeemAndApply {
                    from: fact(),
                    to: fact(),
                    apply,
                },
                None => Effect::Unknown(u()),
            }
        }
        EffectKind::Void => Effect::Void(Target::Contract(sk.text.clone())),
        EffectKind::FormerExample => Effect::ApplyExternal {
            law: "従前の例".into(),
            topic: sk.text.clone(),
        },
        EffectKind::Preserve => Effect::Preserve(Target::Contract(sk.text.clone())),
        EffectKind::Set | EffectKind::Follow | EffectKind::Suffice | EffectKind::Lapse => {
            Effect::Set {
                attribute: sk.effect_tail.clone().unwrap_or_default(),
                value: Value::Unknown(u()),
            }
        }
        EffectKind::Fragment
        | EffectKind::Definition
        | EffectKind::Enforce
        | EffectKind::Repeal => Effect::Unknown(u()),
    }
}
