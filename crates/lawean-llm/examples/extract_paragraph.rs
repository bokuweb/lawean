//! 1 項を実 API で構造化し、リクエスト・レスポンス・結果を fixtures/llm/ に保存する。
//!
//! ANTHROPIC_API_KEY=... cargo run -p lawean-llm --example extract_paragraph -- fixtures/403AC0000000090.xml main/chap:2/sec:1/art:5/para:1
//! `--dry-run` でリクエスト本体だけ表示して終わる（API は呼ばない）。

use lawean_extract::{extract, to_model};
use lawean_llm::*;
use lawean_source::parse_response;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry = args.iter().any(|a| a == "--dry-run");
    let args: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let path = args.first().expect("fixture path");
    let para = args
        .get(1)
        .expect("paragraph stable_id (without law id prefix)");

    let doc = parse_response(&std::fs::read_to_string(path).expect("read")).expect("parse");
    let law_id = doc.law_id.clone().unwrap_or_default();
    let para_id = format!("{law_id}/{para}");
    let sk = extract(&doc);
    let mut model = to_model(&doc, &sk);
    let defs = model.definitions.clone();
    let input = ParagraphInput::new(&doc, &sk, &defs, &para_id);
    if input.skeletons.is_empty() {
        eprintln!("no sentences under {para_id}");
        std::process::exit(2);
    }
    let body = request_body(SYSTEM, &input.user_message(), output_schema());
    if dry {
        println!("{}", serde_json::to_string_pretty(&body).unwrap());
        return;
    }
    let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY");
    let resp = call(&key, &body).expect("api");
    let slug = para.replace('/', "_").replace(':', "-");
    let dir = format!("{}/../../fixtures/llm", env!("CARGO_MANIFEST_DIR"));
    std::fs::write(
        format!("{dir}/{slug}.response.json"),
        serde_json::to_string_pretty(&resp).unwrap(),
    )
    .unwrap();
    eprintln!("cost ≈ ${:.3}  usage = {}", cost_usd(&resp), resp["usage"]);

    let text = extract_text(&resp).expect("text");
    let out: Output = serde_json::from_str(&text).expect("output json");
    let (accepted, rejected) = validate_output(&input, &out);
    for r in &rejected {
        eprintln!("rejected {}: {}", r.sentence, r.reason);
    }
    let rules = to_rules(&input, &accepted, MODEL);
    merge(&mut model, rules);
    let issues = lawean_semantic::validate(&model, &doc);
    for i in &issues {
        eprintln!("issue: {i:?}");
    }
    for r in model
        .rules
        .iter()
        .filter(|r| r.provenance.source.0.starts_with(&para_id))
    {
        println!("{:#?}", r);
    }
}
