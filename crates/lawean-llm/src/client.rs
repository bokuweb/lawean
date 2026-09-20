//! `POST /v1/messages` を直接叩く（Rust には公式 SDK が無い）。
//! 設定は docs/06-llm-extraction.md の「API」節。

use serde_json::{json, Value};

pub const MODEL: &str = "claude-opus-5";
pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
/// `fallbacks: "default"` 用
pub const BETA_FALLBACK: &str = "server-side-fallback-2026-07-01";

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("api {status}: {body}")]
    Api { status: u16, body: String },
    #[error("refusal: {0}")]
    Refusal(String),
    #[error("stop_reason={0} (output may be truncated)")]
    Truncated(String),
    #[error("no text block in response")]
    NoText,
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// リクエスト本体。system はキャッシュ、user は項ごと
pub fn request_body(system: &str, user: &str, schema: Value) -> Value {
    json!({
        "model": MODEL,
        "max_tokens": 16000,
        "thinking": { "type": "adaptive" },
        "output_config": {
            "effort": "high",
            "format": { "type": "json_schema", "schema": schema }
        },
        "fallbacks": "default",
        "system": [
            { "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }
        ],
        "messages": [ { "role": "user", "content": user } ]
    })
}

/// レスポンスからテキストブロックを取り出す。refusal / max_tokens は先に見る
pub fn extract_text(resp: &Value) -> Result<String, LlmError> {
    match resp.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => {
            let cat = resp
                .pointer("/stop_details/category")
                .and_then(Value::as_str)
                .unwrap_or("?");
            return Err(LlmError::Refusal(cat.into()));
        }
        Some("max_tokens") => return Err(LlmError::Truncated("max_tokens".into())),
        _ => {}
    }
    resp.get("content")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        })
        .and_then(|b| b.get("text").and_then(Value::as_str))
        .map(str::to_string)
        .ok_or(LlmError::NoText)
}

pub fn call(api_key: &str, body: &Value) -> Result<Value, LlmError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;
    let resp = client
        .post(API_URL)
        .header("content-type", "application/json")
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("anthropic-beta", BETA_FALLBACK)
        .json(body)
        .send()?;
    let status = resp.status().as_u16();
    let text = resp.text()?;
    if !(200..300).contains(&status) {
        return Err(LlmError::Api { status, body: text });
    }
    Ok(serde_json::from_str(&text)?)
}

/// usage から概算コスト（USD）。Opus 5 の単価（$5 / $25 per MTok、cache read $0.5、cache write $6.25）
pub fn cost_usd(resp: &Value) -> f64 {
    let u = |k: &str| {
        resp.pointer(&format!("/usage/{k}"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            / 1e6
    };
    u("input_tokens") * 5.0
        + u("output_tokens") * 25.0
        + u("cache_read_input_tokens") * 0.5
        + u("cache_creation_input_tokens") * 6.25
}
