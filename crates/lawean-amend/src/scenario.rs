//! 改正シナリオ: 改正単位の適用順。施行日が確定していれば一意、未確定なら順序のパターンごとに適用して合流するかを見る。
//! （デジタル庁 2024 の「改正単位 / 改正シナリオ / 法令リビジョン」に対応）

use crate::apply::{apply_unit, snapshot_main, ApplyError};
use crate::op::AmendUnit;
use lawean_source::LegalDocument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enforcement {
    /// 施行日が確定している（YYYY-MM-DD）
    Fixed(String),
    /// 「公布の日から起算して…政令で定める日」など。同じ改正法の中の順序制約だけが分かる
    Undetermined { note: String },
}

#[derive(Debug, Clone)]
pub struct Stage {
    pub label: String,
    pub unit: AmendUnit,
    pub enforcement: Enforcement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    pub label: String,
    pub outcome: Result<(), ApplyError>,
}

/// 与えられた順に適用する。失敗した段で止まる
pub fn run_sequence(
    base: &LegalDocument,
    stages: &[Stage],
) -> (Option<LegalDocument>, Vec<StepResult>) {
    let mut cur = base.clone();
    let mut log = Vec::new();
    for s in stages {
        match apply_unit(&cur, &s.unit, &s.label) {
            Ok(next) => {
                log.push(StepResult {
                    label: s.label.clone(),
                    outcome: Ok(()),
                });
                cur = next;
            }
            Err(e) => {
                log.push(StepResult {
                    label: s.label.clone(),
                    outcome: Err(e),
                });
                return (None, log);
            }
        }
    }
    (Some(cur), log)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderOutcome {
    pub order: Vec<String>,
    /// 全段成功したか。失敗なら止まった段のエラー
    pub result: Result<(), (String, ApplyError)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioReport {
    pub outcomes: Vec<OrderOutcome>,
    /// 成功した順序の結果（本則スナップショット）がすべて一致するか
    pub confluent: bool,
    /// 成功した順序の数
    pub successes: usize,
}

/// 施行日が確定している段は日付順に固定し、未確定の段は残りの位置の全順列を試す（段数は小さい前提）
pub fn explore(base: &LegalDocument, stages: &[Stage]) -> ScenarioReport {
    let mut fixed: Vec<&Stage> = stages
        .iter()
        .filter(|s| matches!(s.enforcement, Enforcement::Fixed(_)))
        .collect();
    fixed.sort_by(|a, b| match (&a.enforcement, &b.enforcement) {
        (Enforcement::Fixed(x), Enforcement::Fixed(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    });
    let undetermined: Vec<&Stage> = stages
        .iter()
        .filter(|s| matches!(s.enforcement, Enforcement::Undetermined { .. }))
        .collect();

    // 未確定の段を、確定した列のどの隙間に入れるかの全パターン（順列 × 挿入位置）
    let mut orders: Vec<Vec<&Stage>> = vec![fixed.clone()];
    for u in permutations(&undetermined) {
        let mut seqs = vec![fixed.clone()];
        for s in u {
            let mut next = Vec::new();
            for seq in &seqs {
                for pos in 0..=seq.len() {
                    let mut v = seq.clone();
                    v.insert(pos, s);
                    next.push(v);
                }
            }
            seqs = next;
        }
        orders.extend(seqs);
    }
    if !undetermined.is_empty() {
        orders.remove(0);
    }
    // 同じ順序を 1 回だけ
    let mut seen = std::collections::BTreeSet::new();
    orders.retain(|o| seen.insert(o.iter().map(|s| s.label.clone()).collect::<Vec<_>>()));

    let mut outcomes = Vec::new();
    let mut snapshots = Vec::new();
    for order in &orders {
        let owned: Vec<Stage> = order.iter().map(|s| (*s).clone()).collect();
        let (doc, log) = run_sequence(base, &owned);
        let labels = order.iter().map(|s| s.label.clone()).collect();
        match doc {
            Some(d) => {
                snapshots.push(snapshot_main(&d));
                outcomes.push(OrderOutcome {
                    order: labels,
                    result: Ok(()),
                });
            }
            None => {
                let last = log.last().unwrap();
                outcomes.push(OrderOutcome {
                    order: labels,
                    result: Err((last.label.clone(), last.outcome.clone().unwrap_err())),
                });
            }
        }
    }
    let confluent = snapshots.windows(2).all(|w| w[0] == w[1]);
    ScenarioReport {
        successes: snapshots.len(),
        confluent,
        outcomes,
    }
}

fn permutations<'a>(xs: &[&'a Stage]) -> Vec<Vec<&'a Stage>> {
    if xs.is_empty() {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for i in 0..xs.len() {
        let mut rest = xs.to_vec();
        let x = rest.remove(i);
        for mut p in permutations(&rest) {
            p.insert(0, x);
            out.push(p);
        }
    }
    out
}
