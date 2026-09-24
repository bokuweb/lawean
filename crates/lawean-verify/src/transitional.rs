//! 経過措置の場合分け（docs/10 §3 の 4、§4「経過措置の衝突」）。
//!
//! 同じ事柄（topic）について新法の Rule と旧法の Rule が競合するとき、事実の生じた日（契約日・借地権の設定日）と
//! 施行日の前後で、どちらが効くかは `RuleTemporal` で決まる。日付を Int の定数にして Z3 に問う:
//!
//! - **両方適用**（Overlap）: 新法 n と旧法 o が同じ事実に同時に及ぶ世界があるか。`applies(n) ∧ reach(n) ∧ applies(o) ∧ reach(o)`
//! - **空白**（Gap）: すべての施行日の後に、どの Rule も及ばない事実の日があるか。`∧ ¬reach(R)`。
//!   条件（applies）は見ない。新法で要件が変わるのは改正の中身であって空白ではないから、時間の覆いだけを問う
//!
//! `reach(R)`（評価時点 now、事実の日 tf、`tf ≤ now`）:
//!
//! | 属性 | 式 |
//! |---|---|
//! | `effective_from = B` | `now ≥ B`。事実は `tf ≥ B`（不遡及）。`Also(b)` なら `tf ≥ B ∨ tf < b`（施行前の事項にも適用） |
//! | `effective_to = E` | `now < E`。`Only(b)` なら `∨ tf < b`（効力を失った後も施行前の事実には及ぶ = なお従前の例による） |
//! | `effective_to` 無しの `Only(b)` | `tf < b` |
//! | `temporal` 無し | `true`（経過措置の外。常に効いている） |
//!
//! 競合する組（どの新法の規定がどの旧法の規定の代わりか）は人が与える（docs/03-examples/suppl-06 の `instead_of`）。
//! 附則の Rule（`ApplyExternal` + `overrides`）から組と `FactsBefore` を導くのは未。

use crate::check::{run_z3, CheckError, Verdict};
use crate::smt::{applies, sym, Compiler};
use lawean_resolve::ResolvedModel;
use lawean_semantic::*;
use std::collections::BTreeSet;

/// 同じ事柄について競合する新法・旧法の Rule の組
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    /// 「契約の更新」（「〜に関しては」）
    pub topic: String,
    /// 施行日と比べる事実の日（「借地権の設定」）
    pub fact: Event,
    pub new: Vec<RuleId>,
    pub old: Vec<RuleId>,
    /// 日付どうしの既知の関係（政令で同日に定めた、等）
    pub assumptions: Vec<(Event, CmpOp, Event)>,
}

impl Transition {
    pub fn new(topic: &str, fact: &str, new: &[&str], old: &[&str]) -> Self {
        let ids = |xs: &[&str]| xs.iter().map(|x| RuleId((*x).into())).collect();
        Transition {
            topic: topic.into(),
            fact: Event(fact.into()),
            new: ids(new),
            old: ids(old),
            assumptions: Vec::new(),
        }
    }
    pub fn assume(mut self, a: Event, op: CmpOp, b: Event) -> Self {
        self.assumptions.push((a, op, b));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionKind {
    /// 同じ事実に新法と旧法の両方が及ぶ
    Overlap { new: RuleId, old: RuleId },
    /// 施行後に、どちらも及ばない事実がある
    Gap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionFinding {
    pub topic: String,
    pub kind: TransitionKind,
    /// `Counterexample` = 起きる世界がある（`t:now`・`t:<事実>`・`t:<施行日>` の値）。`Proved` = 起きない
    pub verdict: Verdict,
}

pub fn time(e: &Event) -> String {
    sym(&format!("t:{}", e.0))
}

const NOW: &str = "|t:now|";

fn op(o: CmpOp) -> &'static str {
    match o {
        CmpOp::Lt => "<",
        CmpOp::Le => "<=",
        CmpOp::Eq => "=",
        CmpOp::Ge => ">=",
        CmpOp::Gt => ">",
    }
}

/// Rule の時間属性に出てくる日
fn events(t: &RuleTemporal) -> Vec<&Event> {
    let fb = t.facts_before.as_ref().map(|f| match f {
        FactsBefore::Also(e) | FactsBefore::Only(e) => e,
    });
    [t.effective_from.as_ref(), t.effective_to.as_ref(), fb]
        .into_iter()
        .flatten()
        .collect()
}

/// `reach(R)`: 評価時点 now に、事実の日 tf の事実へ R が及ぶか
fn reach(t: Option<&RuleTemporal>, tf: &str) -> String {
    let Some(t) = t else {
        return "true".into();
    };
    let mut parts = Vec::new();
    if let Some(b) = &t.effective_from {
        let b = time(b);
        parts.push(format!("(>= {NOW} {b})"));
        parts.push(match &t.facts_before {
            Some(FactsBefore::Also(x)) => format!("(or (>= {tf} {b}) (< {tf} {}))", time(x)),
            _ => format!("(>= {tf} {b})"),
        });
    }
    match (&t.effective_to, &t.facts_before) {
        (Some(e), Some(FactsBefore::Only(x))) => {
            parts.push(format!("(or (< {NOW} {}) (< {tf} {}))", time(e), time(x)))
        }
        (Some(e), _) => parts.push(format!("(< {NOW} {})", time(e))),
        (None, Some(FactsBefore::Only(x))) => parts.push(format!("(< {tf} {})", time(x))),
        (None, _) => {}
    }
    match parts.len() {
        0 => "true".into(),
        1 => parts.pop().unwrap(),
        _ => format!("(and {})", parts.join(" ")),
    }
}

/// 日付の宣言・`tf ≤ now`・既知の関係と、Rule の applies の定義。`extra` を足して script にする
fn script(rm: &ResolvedModel<'_>, t: &Transition, comment: String, extra: Vec<String>) -> String {
    let mut c = Compiler::new(rm);
    c.compile_model_with(false);
    let mut smt = c.smt;
    let tf = time(&t.fact);
    let mut days: BTreeSet<String> = [NOW.to_string(), tf.clone()].into();
    for id in t.new.iter().chain(&t.old) {
        if let Some(tm) = rm.rule(id).and_then(|r| r.temporal.as_ref()) {
            days.extend(events(tm).into_iter().map(time));
        }
    }
    for (a, _, b) in &t.assumptions {
        days.insert(time(a));
        days.insert(time(b));
    }
    for d in &days {
        smt.declare("Int", d);
    }
    smt.assert(
        format!("{} は評価時点より前に生じた", t.fact.0),
        format!("(<= {tf} {NOW})"),
    );
    for (a, o, b) in &t.assumptions {
        smt.assert(
            format!("{} {o:?} {}", a.0, b.0),
            format!("({} {} {})", op(*o), time(a), time(b)),
        );
    }
    let n = extra.len();
    for (i, e) in extra.into_iter().enumerate() {
        let c = if i + 1 == n {
            comment.clone()
        } else {
            format!("{comment} ({i})")
        };
        smt.assert(c, e);
    }
    let mut s = String::from("(set-option :produce-models true)\n(set-logic ALL)\n");
    s.push_str(&smt.render());
    s.push_str("(check-sat)\n(get-model)\n");
    s
}

fn temporal<'a>(rm: &ResolvedModel<'a>, id: &RuleId) -> Option<&'a RuleTemporal> {
    rm.rule(id).and_then(|r| r.temporal.as_ref())
}

fn overlap_script(rm: &ResolvedModel<'_>, t: &Transition, n: &RuleId, o: &RuleId) -> String {
    let tf = time(&t.fact);
    let both = format!(
        "(and {} {} {} {})",
        applies(n),
        reach(temporal(rm, n), &tf),
        applies(o),
        reach(temporal(rm, o), &tf)
    );
    script(
        rm,
        t,
        format!("{}: {} と {} が同じ事実に及ぶ", t.topic, n.0, o.0),
        vec![both],
    )
}

fn gap_script(rm: &ResolvedModel<'_>, t: &Transition) -> String {
    let tf = time(&t.fact);
    let mut extra = Vec::new();
    // すべての施行日・境の日の後に評価する
    let mut days = BTreeSet::new();
    for id in t.new.iter().chain(&t.old) {
        if let Some(tm) = temporal(rm, id) {
            days.extend(events(tm).into_iter().map(time));
        }
    }
    for d in days {
        extra.push(format!("(>= {NOW} {d})"));
    }
    let none: Vec<String> = t
        .new
        .iter()
        .chain(&t.old)
        .map(|id| format!("(not {})", reach(temporal(rm, id), &tf)))
        .collect();
    extra.push(format!("(and true {})", none.join(" ")));
    script(
        rm,
        t,
        format!("{}: どの規定も及ばない事実がある", t.topic),
        extra,
    )
}

/// 両方適用（新法 × 旧法の各組）と空白の SMT-LIB（z3 は呼ばない。ブラウザ側の z3 に渡す用）
pub fn transition_scripts(rm: &ResolvedModel<'_>, t: &Transition) -> Vec<(TransitionKind, String)> {
    let mut out = Vec::new();
    for n in &t.new {
        for o in &t.old {
            out.push((
                TransitionKind::Overlap {
                    new: n.clone(),
                    old: o.clone(),
                },
                overlap_script(rm, t, n, o),
            ));
        }
    }
    out.push((TransitionKind::Gap, gap_script(rm, t)));
    out
}

/// 経過措置の両方適用と空白を z3 に問う
pub fn transitions(
    rm: &ResolvedModel<'_>,
    t: &Transition,
) -> Result<Vec<TransitionFinding>, CheckError> {
    transition_scripts(rm, t)
        .into_iter()
        .map(|(kind, s)| {
            Ok(TransitionFinding {
                topic: t.topic.clone(),
                kind,
                verdict: run_z3(&s)?,
            })
        })
        .collect()
}
