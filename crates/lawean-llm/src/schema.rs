//! LLM 出力の JSON Schema（`output_config.format`）。再帰不可・`additionalProperties: false` 必須・全 property を required に。
//! 設計は docs/06-llm-extraction.md。

use serde_json::{json, Value};

fn obj(props: Value) -> Value {
    let required: Vec<&str> = props
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

pub const ARG_KINDS: &[&str] = &[
    "definition",
    "entity",
    "text",
    "duration",
    "money",
    "var",
    "ref",
    "unknown",
];
pub const EFFECT_KINDS: &[&str] = &[
    "obligation",
    "prohibition",
    "permission",
    "power",
    "set",
    "deem",
    "presume",
    "void",
    "exception",
    "not_apply",
    "apply",
    "apply_mutatis",
    "deem_and_apply",
    "former_example",
    "preserve",
    "lapse",
    "suffice",
    "follow",
    "limit",
    "unknown",
];
pub const UNKNOWN_KINDS: &[&str] = &["intentional", "unparsed", "ambiguous", "external"];

pub fn output_schema() -> Value {
    let arg = obj(json!({
        "key": { "type": "string", "description": "by / to / object / content / timing / form / conditions など" },
        "kind": { "type": "string", "enum": ARG_KINDS },
        "value": { "type": "string", "description": "definition なら D:ID、ref なら入力に示した stable_id、duration なら '30 year' / '6 month'、money なら '100000 JPY' / 'unknown'" }
    }));
    let pred = obj(json!({
        "name": { "type": "string", "description": "述語。原文の動詞句を短く正規化したもの（「更新を請求した」「建物がある」）" },
        "args": { "type": "array", "items": arg },
        "negated": { "type": "boolean", "description": "原文に「〜でない」「〜しなかった」がある場合だけ true" }
    }));
    let any_of = obj(json!({ "any_of": { "type": "array", "items": pred } }));
    let condition = obj(
        json!({ "all_of": { "type": "array", "items": any_of, "description": "選言の連言。空なら条件なし" } }),
    );
    let effect = obj(json!({
        "kind": { "type": "string", "enum": EFFECT_KINDS, "description": "入力に示した層 1 の種別と一致させる。can は permission か power に振り分ける" },
        "head": { "type": "string", "description": "行為の動詞句 / みなす事実 / 定める属性名 / 無効になる対象。種別に応じて" },
        "args": { "type": "array", "items": arg }
    }));
    let temporal = obj(json!({
        "kind": { "type": "string", "enum": ["duration", "window", "before", "after", "elapsed", "indefinite"] },
        "from": { "type": "string", "description": "起算点となる事象（「契約の日」「期間の満了」「通知の日」）。無ければ空" },
        "length": { "type": "string", "description": "'30 year' / '6 month'。window なら 'from..to' の 2 つを '1 year..6 month'" },
        "direction": { "type": "string", "enum": ["forward", "backward", ""] }
    }));
    let unknown = obj(json!({
        "kind": { "type": "string", "enum": UNKNOWN_KINDS },
        "text": { "type": "string" }
    }));
    let rule = obj(json!({
        "suffix": { "type": "string", "description": "1 文から複数 Rule を出すとき a, b, …。1 つなら空" },
        "subject": { "type": "string", "description": "主体。定義語なら D:ID、それ以外は名詞（「当事者」「裁判所」）。無ければ空" },
        "condition": condition,
        "effect": effect,
        "temporal": { "type": "array", "items": temporal },
        "unknowns": { "type": "array", "items": unknown },
        "confidence": { "type": "string", "enum": ["low", "medium"] },
        "note": { "type": "string", "description": "IR に写さなかった譲歩・読み替え・他条との関係。無ければ空" }
    }));
    let sentence = obj(json!({
        "sentence": { "type": "string", "description": "入力の [sent:N] ラベル" },
        "rules": { "type": "array", "items": rule }
    }));
    obj(json!({ "sentences": { "type": "array", "items": sentence } }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn schema_has_no_open_objects() {
        fn walk(v: &serde_json::Value) {
            if let Some(o) = v.as_object() {
                if o.get("type") == Some(&serde_json::json!("object")) {
                    assert_eq!(
                        o.get("additionalProperties"),
                        Some(&serde_json::json!(false))
                    );
                    assert!(o.contains_key("required"));
                }
                o.values().for_each(walk);
            } else if let Some(a) = v.as_array() {
                a.iter().for_each(walk);
            }
        }
        walk(&super::output_schema());
    }
}
