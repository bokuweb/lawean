//! 無矛盾の検査（docs/07 §「効力の齟齬」）: 同じ対象に相反する効果を与える 2 つの Rule が、同時に適用される世界があるか。
//!
//! 例外・特則（`overrides`）が正しく張ってあれば「両方適用」は起きない（applies に ¬applies_例外 が入る）。
//! 張り忘れると、Z3 が両方適用される世界を反例として出す。効果の帰結は主張しない（主張すると unsat になって見えない）。
//!
//! 見る組: `Void t` と `Preserve t`（無効 vs 効力を妨げない）、同じ属性への `Set` で値が違う、
//! 同じ行為への `Obligation` と `Prohibition`。主体は述語名に畳まれているので、主体違いは別の対象として扱う（限界）。
//!
//! もう一つの問い（`vacuous`）: 例外を差し引いた後に、その Rule が適用される世界が**残っているか**。
//! 「罰則の対象を定めたが、例外規定を考慮すると対象が存在しない」は applies(R) = 条件 ∧ ¬applies(例外) が unsat、
//! ということ。条件どうしが同じ変数で書けていれば（数値・同じ述語）Z3 が答える。別の語で書かれた同じ事柄は層 2 の同一性判定が要る。

use crate::check::{run_z3, CheckError, Verdict};
use crate::smt::{applies, target_name, Compiler};
use lawean_resolve::ResolvedModel;
use lawean_semantic::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// 無効とする vs 効力を妨げない
    VoidVsPreserve { target: String },
    /// 同じ属性に違う値
    SetVsSet { attribute: String },
    /// 同じ行為に義務と禁止
    ObligationVsProhibition { verb: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub a: RuleId,
    pub b: RuleId,
    pub kind: ConflictKind,
    /// `Counterexample` = 両方が同時に適用される世界がある（矛盾）。`Proved` = 起きない
    pub verdict: Verdict,
}

fn candidates(model: &SemanticModel) -> Vec<(RuleId, RuleId, ConflictKind)> {
    let mut out = Vec::new();
    let rules = &model.rules;
    for (i, a) in rules.iter().enumerate() {
        for b in &rules[i + 1..] {
            let kind = match (&a.effect, &b.effect) {
                (Effect::Void(t1), Effect::Preserve(t2))
                | (Effect::Preserve(t2), Effect::Void(t1))
                    if target_name(t1) == target_name(t2) =>
                {
                    Some(ConflictKind::VoidVsPreserve {
                        target: target_name(t1),
                    })
                }
                (
                    Effect::Set {
                        attribute: x,
                        value: v1,
                    },
                    Effect::Set {
                        attribute: y,
                        value: v2,
                    },
                ) if x == y && v1 != v2 => Some(ConflictKind::SetVsSet {
                    attribute: x.clone(),
                }),
                (Effect::Obligation(p), Effect::Prohibition(q))
                | (Effect::Prohibition(q), Effect::Obligation(p))
                    if p.verb == q.verb =>
                {
                    Some(ConflictKind::ObligationVsProhibition {
                        verb: p.verb.clone(),
                    })
                }
                _ => None,
            };
            if let Some(k) = kind {
                out.push((a.id.clone(), b.id.clone(), k));
            }
        }
    }
    out
}

/// 1 組の SMT-LIB
pub fn conflict_script(
    rm: &ResolvedModel<'_>,
    a: &RuleId,
    b: &RuleId,
    kind: &ConflictKind,
) -> String {
    let mut c = Compiler::new(rm);
    c.compile_model_with(false);
    let mut extra = String::new();
    if let ConflictKind::SetVsSet { .. } = kind {
        // 値が違う世界だけを探す（同じ値なら矛盾ではない）
        let va = match rm.rule(a).map(|r| &r.effect) {
            Some(Effect::Set { value, .. }) => c.value(value),
            _ => "0".into(),
        };
        let vb = match rm.rule(b).map(|r| &r.effect) {
            Some(Effect::Set { value, .. }) => c.value(value),
            _ => "0".into(),
        };
        extra = format!(" (not (= {va} {vb}))");
    }
    let mut smt = c.smt;
    smt.assert(
        format!("both apply: {} and {}", a.0, b.0),
        format!("(and {} {}{extra})", applies(a), applies(b)),
    );
    let mut s = String::from("(set-option :produce-models true)\n(set-logic ALL)\n");
    s.push_str(&smt.render());
    s.push_str("(check-sat)\n(get-model)\n");
    s
}

/// 相反する効果を持つ全ての組について、同時に適用される世界があるかを z3 に問う
pub fn conflicts(rm: &ResolvedModel<'_>) -> Result<Vec<Conflict>, CheckError> {
    let mut out = Vec::new();
    for (a, b, kind) in candidates(rm.model) {
        let verdict = run_z3(&conflict_script(rm, &a, &b, &kind))?;
        out.push(Conflict {
            a,
            b,
            kind,
            verdict,
        });
    }
    Ok(out)
}

/// 適用される世界が残っていない Rule（例外に飲まれた、または条件自体が矛盾）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vacuity {
    pub rule: RuleId,
    /// この Rule に張られた例外（`overrides` でこの Rule を上書きするもの）
    pub exceptions: Vec<RuleId>,
    /// `Proved` = 適用される世界が無い（空振り）。`Counterexample` = 適用される世界がある（そのモデル）
    pub verdict: Verdict,
}

/// 1 つの Rule について「applies(R) が成り立つ世界があるか」の SMT-LIB
pub fn vacuity_script(rm: &ResolvedModel<'_>, r: &RuleId) -> String {
    let mut c = Compiler::new(rm);
    c.compile_model_with(false);
    let mut smt = c.smt;
    smt.assert(format!("applies: {}", r.0), applies(r));
    let mut s = String::from("(set-option :produce-models true)\n(set-logic ALL)\n");
    s.push_str(&smt.render());
    s.push_str("(check-sat)\n(get-model)\n");
    s
}

/// 全ての Rule について空振りを見る。`Proved` のものが空振り
pub fn vacuous(rm: &ResolvedModel<'_>) -> Result<Vec<Vacuity>, CheckError> {
    let mut out = Vec::new();
    for r in &rm.model.rules {
        let verdict = run_z3(&vacuity_script(rm, &r.id))?;
        out.push(Vacuity {
            rule: r.id.clone(),
            exceptions: rm.exceptions_of(&r.id).to_vec(),
            verdict,
        });
    }
    Ok(out)
}
