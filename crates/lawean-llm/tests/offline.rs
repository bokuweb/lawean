//! ネットワークなしで層 2 のパイプラインを通す: 入力生成 → （記録済みレスポンス）→ 検証 → IR 化 → 骨組みへのマージ → validate。

use lawean_extract::{extract, to_model};
use lawean_llm::*;
use lawean_semantic::*;
use lawean_source::*;

const PARA: &str = "403AC0000000090/main/chap:2/sec:1/art:5/para:1";

fn doc() -> LegalDocument {
    let path = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn sample_response() -> serde_json::Value {
    let path = format!(
        "{}/../../fixtures/llm/art5-para1.sample-response.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn user_message_carries_layer1_facts() {
    let doc = doc();
    let sk = extract(&doc);
    let model = to_model(&doc, &sk);
    let input = ParagraphInput::new(&doc, &sk, &model.definitions, PARA);
    let m = input.user_message();
    assert!(m.contains("[sent:1] (Main, deem)"), "{m}");
    assert!(m.contains("[sent:2] (Proviso, exception)"), "{m}");
    assert!(
        m.contains(
            "条件節: 借地権の存続期間が満了する / 借地権者が契約の更新を請求した / 建物がある"
        ),
        "{m}"
    );
    assert!(
        m.contains("前条 → 403AC0000000090/main/chap:2/sec:1/art:4"),
        "{m}"
    );
    assert!(m.contains("D:借地権者 借地権者"), "{m}");
    // 第6条限りの定義語はここでは見えない
    assert!(!m.contains("@art6"));
}

#[test]
fn request_body_shape() {
    let body = request_body(SYSTEM, "x", output_schema());
    assert_eq!(body["model"], MODEL);
    assert_eq!(body["thinking"]["type"], "adaptive");
    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    assert_eq!(body["fallbacks"], "default");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn sample_response_is_accepted_and_merges_into_skeleton() {
    let doc = doc();
    let sk = extract(&doc);
    let mut model = to_model(&doc, &sk);
    let defs = model.definitions.clone();
    let input = ParagraphInput::new(&doc, &sk, &defs, PARA);

    let text = extract_text(&sample_response()).unwrap();
    let out: Output = serde_json::from_str(&text).unwrap();
    let (accepted, rejected) = validate_output(&input, &out);
    assert!(rejected.is_empty(), "{rejected:?}");
    assert_eq!(accepted.len(), 2);

    let rules = to_rules(&input, &accepted, MODEL);
    let before = model.rules.len();
    merge(&mut model, rules);
    assert_eq!(model.rules.len(), before, "置き換えなので数は変わらない");

    let r1 = model
        .rules
        .iter()
        .find(|r| r.id.0 == format!("S:{PARA}/sent:1"))
        .unwrap();
    assert!(matches!(r1.provenance.by, Author::Llm(_)));
    assert_eq!(r1.provenance.confidence, Confidence::Medium);
    assert!(
        matches!(&r1.condition, Expr::And(v) if v.len() == 3),
        "{:?}",
        r1.condition
    );
    assert!(matches!(&r1.effect, Effect::Deem(f) if f.pred.name == "契約を更新した"));
    assert_eq!(
        r1.subject,
        Some(EntityRef::Definition(DefinitionId("D:借地権者".into())))
    );

    let r2 = model
        .rules
        .iter()
        .find(|r| r.id.0 == format!("S:{PARA}/sent:2"))
        .unwrap();
    // 骨組みの overrides（ただし書き → 本文）は LLM 出力で上書きされない
    assert!(r2
        .overrides
        .iter()
        .any(|o| matches!(o, Override::Rule(id) if id.0.ends_with("sent:1"))));

    assert!(validate_model(&model, &doc).is_empty());
    assert!(cost_usd(&sample_response()) > 0.0);
}

fn validate_model(m: &SemanticModel, d: &LegalDocument) -> Vec<Issue> {
    lawean_semantic::validate(m, d)
}

#[test]
fn bad_outputs_are_rejected_per_sentence() {
    let doc = doc();
    let sk = extract(&doc);
    let model = to_model(&doc, &sk);
    let input = ParagraphInput::new(&doc, &sk, &model.definitions, PARA);
    let text = extract_text(&sample_response()).unwrap();
    let mut out: Output = serde_json::from_str(&text).unwrap();

    // 効果種別の矛盾
    out.sentences[0].rules[0].effect.kind = "obligation".into();
    // 層 1 に無い参照
    out.sentences[1].rules[0].condition.all_of[0].any_of[0]
        .args
        .push(lawean_llm::response::ArgOut {
            key: "ref".into(),
            kind: "ref".into(),
            value: "403AC0000000090/main/chap:9/art:999".into(),
        });
    let (accepted, rejected) = validate_output(&input, &out);
    assert!(accepted.is_empty());
    assert_eq!(rejected.len(), 2);
    assert!(rejected[0].reason.contains("矛盾"), "{:?}", rejected[0]);
    assert!(
        rejected[1].reason.contains("層 1 に無い"),
        "{:?}",
        rejected[1]
    );

    // refusal は content を読む前に弾く
    let mut refused = sample_response();
    refused["stop_reason"] = serde_json::json!("refusal");
    assert!(matches!(extract_text(&refused), Err(LlmError::Refusal(_))));
}
