//! 層 2 — LLM による要件・効果の構造化（docs/06-llm-extraction.md、ADR-0008）。
//!
//! 流れ: `ParagraphInput`（層 1 の骨組み + 定義語）→ `request_body` → API → `Output` → `validate`（層 1 と突き合わせ）→ `to_rules`。
//! 棄却された文は骨組みのまま残る。

pub mod client;
pub mod prompt;
pub mod response;
pub mod schema;

pub use client::{call, cost_usd, extract_text, request_body, LlmError, MODEL};
pub use prompt::{ParagraphInput, SYSTEM};
pub use response::{to_rules, validate as validate_output, Output, Rejection};
pub use schema::output_schema;

/// 1 項分の骨組みモデルに、採用された LLM 出力を重ねる（同じ ID の骨組み Rule を置き換え、suffix 付きは追加）
pub fn merge(model: &mut lawean_semantic::SemanticModel, rules: Vec<lawean_semantic::Rule>) {
    for r in rules {
        if let Some(existing) = model.rules.iter_mut().find(|e| e.id == r.id) {
            let overrides = std::mem::take(&mut existing.overrides);
            *existing = r;
            existing.overrides = overrides;
        } else {
            // suffix 付き: 元の骨組み Rule の overrides を引き継ぐ
            let base =
                r.id.0
                    .rsplit_once('-')
                    .map(|(b, _)| b.to_string())
                    .unwrap_or_default();
            let overrides = model
                .rules
                .iter()
                .find(|e| e.id.0 == base)
                .map(|e| e.overrides.clone())
                .unwrap_or_default();
            let mut r = r;
            r.overrides = overrides;
            model.rules.push(r);
        }
    }
}
