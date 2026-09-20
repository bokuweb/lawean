//! 改正 — 改め文を操作に分解し、Source IR のリビジョンに適用し、発射台とハネを検査する（docs/08-amendment.md）。

pub mod apply;
pub mod hane;
pub mod op;
pub mod parse;
pub mod scenario;

pub use apply::{apply_unit, diff_snapshots, paragraph_mapping, snapshot_main, ApplyError};
pub use hane::{hane_candidates, HaneCandidate};
pub use op::*;
pub use parse::{parse_instruction, parse_units, ParseError};
pub use scenario::{explore, run_sequence, Enforcement, ScenarioReport, Stage};
