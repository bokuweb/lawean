//! Semantic IR — 法令の意味構造（Rule / Definition / Effect / Temporal / Unknown / Provenance）。
//! 設計は `docs/04-semantic-ir.md`、根拠となる実例は `docs/03-examples/`。
//!
//! Source IR とは別のツリーで、`Provenance.source`（stable_id）でのみ結ばれる（ADR-0006）。
//! v0.1 は型と手書きデータの置き場。自然文からの抽出（semantic parser）はまだ無い。

pub mod build;
pub mod examples;
pub mod ir;
pub mod validate;

pub use ir::*;
pub use validate::{validate, Issue};
