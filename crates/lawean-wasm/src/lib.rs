//! ブラウザ向けの薄い皮。`lawean-check::run_texts` を JSON で呼ぶ。
//! ビルド: `wasm-pack build crates/lawean-wasm --target web --out-dir ../../docs/playground/pkg --no-typescript`
//!
//! ADR-0015 の経路（Lean → C → WASM）とは別で、これは Rust の写し（`ident::apply_unit`）を wasm32 にしたもの。
//! Lean との一致は `Consolidate.lean` / `Cases.lean` の三者一致で担保する。

use wasm_bindgen::prelude::*;

/// 入力 JSON: { base, amendment, expected?, taisho?, other_laws?: [xml], enforced?, suppl?（起草中の附則）, promulgated?（公布予定日） }。出力は `Report` の JSON
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
        s("suppl"),
        s("promulgated"),
    );
    serde_json::to_string(&report).unwrap()
}

/// 法令 XML の全文から、層 1 の候補（時間表現・罰則・金額）を JSON Lines ではなく JSON 配列で。
/// 入力: e-Gov の XML。出力: `[{ field, category, raw, normalized, unit, role, source_label, evidence: { sentence, start, end, snippet, context }, confidence, reason }, ...]`
#[wasm_bindgen]
pub fn candidates(xml: &str) -> String {
    match lawean_source::parse_response(xml) {
        Ok(doc) => serde_json::to_string(&lawean_extract::candidates::candidates(&doc)).unwrap(),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

/// 法令の題名・法令番号と、本則の条・項の一覧（起草の画面で条文を見ながら改め文を書くため）。
/// 出力: { title, law_num, law_id, articles: [{ id, num, label, caption, paragraphs: [{ id, num, text }] }] }
#[wasm_bindgen]
pub fn outline(xml: &str) -> String {
    use lawean_source::ir::{inline_text, ArticleChild, Provision};
    let doc = match lawean_source::parse_response(xml) {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    fn walk(ps: &[Provision], out: &mut Vec<serde_json::Value>) {
        for p in ps {
            match p {
                Provision::Container(c) => walk(&c.children, out),
                Provision::Article(a) => {
                    let paragraphs: Vec<serde_json::Value> = a
                        .children
                        .iter()
                        .filter_map(|ch| match ch {
                            ArticleChild::Paragraph(p) => Some(serde_json::json!({
                                "id": p.stable_id.0,
                                "num": p.num,
                                "text": lawean_amend::apply::para_text(p),
                            })),
                            _ => None,
                        })
                        .collect();
                    out.push(serde_json::json!({
                        "id": a.stable_id.0,
                        "num": a.num.to_num_string(),
                        "label": a.title.as_ref().map(|t| inline_text(t)).unwrap_or_default(),
                        "caption": a.caption.as_ref().map(|t| inline_text(t)).unwrap_or_default(),
                        "paragraphs": paragraphs,
                    }));
                }
                _ => {}
            }
        }
    }
    let mut articles = Vec::new();
    walk(&doc.main_provision, &mut articles);
    serde_json::json!({
        "title": doc.title.as_ref().map(|t| inline_text(&t.text)).unwrap_or_default(),
        "law_num": doc.law_num.clone().unwrap_or_default(),
        "law_id": doc.law_id.clone().unwrap_or_default(),
        "articles": articles,
    })
    .to_string()
}
