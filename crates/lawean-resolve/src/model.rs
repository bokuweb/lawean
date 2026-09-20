//! Semantic IR の上での解決: overrides の逆引き、scope → Rule 集合、定義語の有効 scope。

use lawean_semantic::*;
use lawean_source::StableId;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ResolvedModel<'a> {
    pub model: &'a SemanticModel,
    /// Rule → それを上書きする Rule 群（`overrides` の逆向き）
    pub exceptions: BTreeMap<RuleId, Vec<RuleId>>,
    /// Rule → その Rule が契約に優先するか
    pub overrides_contract: BTreeMap<RuleId, bool>,
}

impl<'a> ResolvedModel<'a> {
    pub fn new(model: &'a SemanticModel) -> Self {
        let mut exceptions: BTreeMap<RuleId, Vec<RuleId>> = BTreeMap::new();
        let mut overrides_contract = BTreeMap::new();
        for r in &model.rules {
            let mut contract = false;
            for o in &r.overrides {
                match o {
                    Override::Rule(id) => {
                        exceptions.entry(id.clone()).or_default().push(r.id.clone())
                    }
                    Override::Contract => contract = true,
                }
            }
            overrides_contract.insert(r.id.clone(), contract);
        }
        ResolvedModel {
            model,
            exceptions,
            overrides_contract,
        }
    }

    pub fn rule(&self, id: &RuleId) -> Option<&'a Rule> {
        self.model.rules.iter().find(|r| &r.id == id)
    }

    /// この Rule を上書きする Rule 群（例外の例外は含まない。1 段だけ）
    pub fn exceptions_of(&self, id: &RuleId) -> &[RuleId] {
        self.exceptions.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 構造パス（節・条・項）配下に Provenance を持つ Rule 群。「この節の規定」の展開に使う
    pub fn rules_under(&self, scope: &StableId) -> Vec<&'a Rule> {
        self.model
            .rules
            .iter()
            .filter(|r| is_under(&r.provenance.source, scope))
            .collect()
    }

    /// 位置 `at` で用語 `term` が指す Definition。scope が最も内側のものを選ぶ
    pub fn definition_at(&self, term: &str, at: &StableId) -> Option<&'a Definition> {
        self.model
            .definitions
            .iter()
            .filter(|d| d.term == term)
            .filter_map(|d| {
                d.scope
                    .iter()
                    .filter(|s| is_under(at, s))
                    .map(|s| (s.0.len(), d))
                    .max_by_key(|(l, _)| *l)
            })
            .max_by_key(|(l, _)| *l)
            .map(|(_, d)| d)
    }

    /// `overrides` を辿って、Rule の優先順位を「上書きする側が先」で返す（サイクルは打ち切り）
    pub fn override_chain(&self, id: &RuleId) -> Vec<RuleId> {
        let mut out = vec![id.clone()];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i].clone();
            for e in self.exceptions_of(&cur) {
                if !out.contains(e) {
                    out.push(e.clone());
                }
            }
            i += 1;
        }
        out.reverse();
        out
    }
}

/// `id` が `scope` と同じか、その配下か
pub fn is_under(id: &StableId, scope: &StableId) -> bool {
    id.0 == scope.0 || id.0.starts_with(&format!("{}/", scope.0))
}
