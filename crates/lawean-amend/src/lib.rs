//! 改正 — 改め文を操作に分解し、Source IR のリビジョンに適用し、発射台とハネを検査する（docs/08-amendment.md）。

pub mod apply;
pub mod body;
pub mod hane;
pub mod ident;
pub mod numbering;
pub mod op;
pub mod parse;
pub mod scenario;

pub use apply::{
    apply_unit, article_label, diff_snapshots, para_text, paragraph_mapping, snapshot_main,
    toc_text, ApplyError,
};
pub use hane::{article_mapping, hane_candidates, HaneCandidate};
pub use op::*;
pub use parse::{
    kana_index, kana_of, parse_instruction, parse_scope_locs, parse_units, ParseError,
};
pub use scenario::{explore, run_sequence, Enforcement, ScenarioReport, Stage};
