//! Resolved IR — 参照・定義語 scope・overrides の解決。
//!
//! - `reference` / `resolve`: 条文中の「前項」「第三十八条第二項」「同条」などを stable_id に落とす
//! - `index`: Source IR の位置索引
//! - `model`: Semantic IR の上で overrides の逆引き、scope に属する Rule の列挙、定義語の有効 scope を計算する

pub mod index;
pub mod model;
pub mod numeral;
pub mod reference;
pub mod resolve;

pub use index::Index;
pub use model::ResolvedModel;
pub use reference::{find_references, RefKind};
pub use resolve::{
    resolve, resolve_paragraph, resolve_sentence, resolve_sentence_with, Antecedent, Resolution,
    Unresolved,
};
