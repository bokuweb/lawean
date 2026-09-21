//! ブラウザ向けの薄い皮。`lawean-check::run_texts` を JSON で呼ぶ。
//! ビルド: `wasm-pack build crates/lawean-wasm --target web --out-dir ../../docs/playground/pkg --no-typescript`
//!
//! ADR-0015 の経路（Lean → C → WASM）とは別で、これは Rust の写し（`ident::apply_unit`）を wasm32 にしたもの。
//! Lean との一致は `Consolidate.lean` / `Cases.lean` の三者一致で担保する。

use wasm_bindgen::prelude::*;

/// 入力 JSON: { base, amendment, expected?, taisho?, other_laws?: [xml], enforced?, suppl?（起草中の附則）, promulgated?（公布予定日）, base_draft?（起草時の発射台）, other_laws_draft?: [xml] }。出力は `Report` の JSON
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
    let others_draft: Vec<String> = v["other_laws_draft"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let report = lawean_check::run_text_input(&lawean_check::TextInput {
        base_xml: s("base").unwrap_or(""),
        amendment: s("amendment").unwrap_or(""),
        expected_xml: s("expected"),
        taisho: s("taisho"),
        other_laws: &others,
        enforced: s("enforced"),
        suppl: s("suppl"),
        promulgated: s("promulgated"),
        base_draft_xml: s("base_draft"),
        other_laws_draft: &others_draft,
    });
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

/// Z3 に渡す SMT-LIB（z3 はブラウザ側の z3-solver の WASM で走らせる）。
/// 入力 JSON: { base, amendment, enforced?, suppl?, promulgated? }。
/// 出力: [{ kind, name, script, sat_means, unsat_means }]。
///
/// - `enforcement`: 単位ごとの施行日が附則の区間にあるか（Z3 の暦、民法第143条。Rust の暦と同じ答えになるはず）
/// - `vacuity`: 改正後の本文から層 1 が出した Rule のうち、条件に型のある部分（期間の比較・経過）を持つものについて、
///   例外を差し引いても適用される世界が残るか（unsat = 空振り）
/// - `conflict`: 相反する効果の組が同時に適用される世界があるか（sat = 齟齬）
#[wasm_bindgen]
pub fn smt_scripts(input_json: &str) -> String {
    use lawean_extract::calendar::parse;
    use lawean_extract::suppl::{admissible, spec_from_text};
    use lawean_semantic::Expr;
    let v: serde_json::Value = match serde_json::from_str(input_json) {
        Ok(v) => v,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let s = |k: &str| v[k].as_str();
    let mut out = Vec::new();
    // 施行期日
    if let (Some(suppl), Some(day), Some(p)) = (
        s("suppl"),
        s("enforced").and_then(parse),
        s("promulgated").and_then(parse),
    ) {
        let spec = spec_from_text(suppl, Some(p));
        if let Ok(units) = lawean_amend::parse_units(s("amendment").unwrap_or("")) {
            for u in &units {
                let arts: Vec<String> = u
                    .instructions
                    .iter()
                    .flat_map(|i| i.ops.iter())
                    .filter_map(|o| o.article().map(|a| a.to_num_string()))
                    .collect();
                let by_target = spec
                    .for_target_articles(&arts)
                    .filter(|(_, sc)| sc.is_some());
                let by_amending = u
                    .article_of_amending_law
                    .trim_start_matches('第')
                    .split('条')
                    .next()
                    .and_then(lawean_extract::suppl::kanji_num)
                    .and_then(|a| spec.for_article(a, None));
                let Some((clause, _)) = by_target.or(by_amending) else {
                    continue;
                };
                let Some(e) = &clause.enforcement else {
                    continue;
                };
                let t = lawean_verify::temporal::DateExpr::lit(day.0, day.1, day.2);
                let Some(adm) = lawean_verify::enforcement::admissible(p, e, &t) else {
                    continue;
                };
                let range = admissible(p, e)
                    .map(|(lo, hi)| {
                        format!(
                            "{}〜{}",
                            lawean_extract::calendar::fmt(lo),
                            lawean_extract::calendar::fmt(hi)
                        )
                    })
                    .unwrap_or_default();
                out.push(serde_json::json!({
                    "kind": "enforcement",
                    "name": format!("{}: 施行日 {} は附則「{}」の区間（Rust の暦では {range}）にあるか", u.article_of_amending_law, lawean_extract::calendar::fmt(day), clause.text.chars().take(40).collect::<String>()),
                    "script": lawean_verify::temporal::script(&[], &[adm]),
                    "sat_means": "範囲内（Z3 の暦でも許される）",
                    "unsat_means": "範囲外",
                }));
            }
        }
    }
    // 改正後の本文の Semantic IR（層 1 の骨組み）
    let doc = match lawean_check::consolidated_document(
        s("base").unwrap_or(""),
        s("amendment").unwrap_or(""),
    ) {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let model = lawean_extract::to_model(&doc, &lawean_extract::extract(&doc));
    let rm = lawean_resolve::ResolvedModel::new(&model);
    fn typed(e: &Expr) -> bool {
        match e {
            Expr::Cmp(..) | Expr::Time(_) => true,
            Expr::And(xs) | Expr::Or(xs) => xs.iter().any(typed),
            Expr::Not(x) => typed(x),
            _ => false,
        }
    }
    for r in &model.rules {
        // 自分の条件に型のある部分（期間の比較・経過）がある Rule だけ。条件が Unknown だけの Rule は自由変数なので
        // 常に世界が残り、例外側の条件が取れていない（True）と常に空振りになって、どちらも意味が無い
        if !typed(&r.condition) {
            continue;
        }
        let sent = r
            .provenance
            .source
            .0
            .rsplit("/main/")
            .next()
            .unwrap_or("")
            .to_string();
        out.push(serde_json::json!({
            "kind": "vacuity",
            "name": format!("{sent}: 例外を差し引いて適用される世界が残るか（{}）", model.rules.iter().find(|x| x.id == r.id).map(|_| format!("{:?}", r.condition)).unwrap_or_default().chars().take(120).collect::<String>()),
            "script": lawean_verify::consistency::vacuity_script(&rm, &r.id),
            "sat_means": "残る",
            "unsat_means": "空振り（例外に飲まれた、または条件が矛盾）",
        }));
    }
    for (a, b, kind, script) in lawean_verify::consistency::conflict_scripts(&rm) {
        let ra = rm
            .rule(&a)
            .map(|r| {
                r.provenance
                    .source
                    .0
                    .rsplit("/main/")
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();
        let rb = rm
            .rule(&b)
            .map(|r| {
                r.provenance
                    .source
                    .0
                    .rsplit("/main/")
                    .next()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();
        out.push(serde_json::json!({
            "kind": "conflict",
            "name": format!("{ra} と {rb}: {kind:?} が同時に適用される世界があるか"),
            "script": script,
            "sat_means": "ある（効力の齟齬。特則の overrides が要る）",
            "unsat_means": "無い",
        }));
    }
    serde_json::to_string(&out).unwrap()
}
