//! Source IR — e-Gov 法令標準 XML を lossless に保持する層。意味解析はしない。
//! 設計は `docs/02-source-ir.md`。

pub mod egov;
pub mod emit;
pub mod ir;
pub mod parse;
pub mod xml;

pub use egov::parse_response;
pub use emit::emit_law;
pub use ir::*;
pub use parse::{parse_law, ParseOptions};
