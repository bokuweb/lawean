//! 逆変換（ADR-0012）: 構造化記述 → 人が読める形。
//! - `amend`: `Op` → 改め文（`lawean-amend::parse` と往復）
//! - `semantic`: Semantic IR → 日本語（原文と並べてレビューする）

pub mod amend;
pub mod numeral;
pub mod semantic;

pub use amend::{render_instruction, render_unit, render_units};
pub use numeral::to_kanji;
pub use semantic::{model as render_model, rule as render_rule};
