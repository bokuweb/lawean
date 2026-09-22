//! 層 1 の規則ベース抽出（ADR-0008）。
//! 文ごとに、効果の種別・条件節・上書き（〜の規定にかかわらず）・譲歩・参照を認識し、
//! 条件の中身を `Unknown(Unparsed)` にした骨組み Rule を作る。層 2（LLM）はこの骨組みを埋める。

pub mod amount;
pub mod calendar;
pub mod candidate;
pub mod candidates;
pub mod clause;
pub mod effect;
pub mod eval;
pub mod penalty;
pub mod skeleton;
pub mod suppl;
pub mod temporal;

pub use effect::EffectKind;
pub use skeleton::{definitions, extract, rule_id, to_model, OverrideTarget, Skeleton};
