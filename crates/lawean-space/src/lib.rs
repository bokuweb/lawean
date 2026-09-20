//! 法令空間 — 複数の法令をある時点で並べ、法令をまたぐ参照と、改正が他法令に与える影響を計算する（docs/09）。

pub mod impact;
pub mod mapping;
pub mod override_graph;
pub mod space;
pub mod xref;

pub use impact::{impact, Impact, ImpactKind};
pub use mapping::{provision_mapping, ProvisionMapping};
pub use override_graph::{override_cycles, prefix_model};
pub use space::{enforced_on, LawSpace};
pub use xref::{cross_refs, locate, CrossRef};
