//! ブラウザ向けの薄い皮。`lawean-check::run_texts` を JSON で呼ぶ。
//! ビルド: `wasm-pack build crates/lawean-wasm --target web --out-dir ../../docs/playground/pkg --no-typescript`
//!
//! ADR-0015 の経路（Lean → C → WASM）とは別で、これは Rust の写し（`ident::apply_unit`）を wasm32 にしたもの。
//! Lean との一致は `Consolidate.lean` / `Cases.lean` の三者一致で担保する。

use wasm_bindgen::prelude::*;

/// 入力 JSON: { base, amendment, expected?, taisho?, other_laws?: [xml], enforced? }。出力は `Report` の JSON
#[wasm_bindgen]
pub fn check(input_json: &str) -> String {
    let v: serde_json::Value = match serde_json::from_str(input_json) {
        Ok(v) => v,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let s = |k: &str| v[k].as_str();
    let others: Vec<String> = v["other_laws"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let report = lawean_check::run_texts(
        s("base").unwrap_or(""),
        s("amendment").unwrap_or(""),
        s("expected"),
        s("taisho"),
        &others,
        s("enforced"),
    );
    serde_json::to_string(&report).unwrap()
}
