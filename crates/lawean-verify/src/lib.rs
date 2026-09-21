//! Verification IR — Semantic IR を SMT-LIB に落とし、z3 で性質を証明・反証する（docs/07-verification.md）。

pub mod check;
pub mod consistency;
pub mod enforcement;
pub mod smt;
pub mod temporal;
pub mod validity;

pub use check::{check, model_value, run_z3, script, z3_available, CheckError, Property, Verdict};
pub use consistency::{conflicts, vacuous, Conflict, ConflictKind, Vacuity};
pub use smt::{months, pred_name, Compiler, Smt};
