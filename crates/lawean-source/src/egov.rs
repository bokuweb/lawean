//! e-Gov 法令 API v2 のレスポンス（`law_data_response`）の読み取り。
//! 法令本体は `law_full_text/Law`。生の `Law` 要素が渡された場合もそのまま受ける。

use crate::ir::LegalDocument;
use crate::parse::{parse_law, ParseError, ParseOptions};
use crate::xml::Element;

#[derive(Debug, thiserror::Error)]
pub enum EgovError {
    #[error("xml: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("law_full_text/Law not found in law_data_response")]
    NoLaw,
    #[error(transparent)]
    Parse(#[from] ParseError),
}

/// API レスポンスから `Law` 要素と law_id / law_revision_id を取り出す
pub fn law_element(root: &Element) -> Result<(&Element, ParseOptions), EgovError> {
    if root.name == "Law" {
        return Ok((
            root,
            ParseOptions {
                law_id: None,
                version_id: None,
            },
        ));
    }
    let law = root
        .first_child("law_full_text")
        .and_then(|f| f.first_child("Law"))
        .ok_or(EgovError::NoLaw)?;
    let law_id = root
        .first_child("law_info")
        .and_then(|i| i.first_child("law_id"))
        .map(|e| e.text());
    let version_id = root
        .first_child("revision_info")
        .and_then(|i| i.first_child("law_revision_id"))
        .map(|e| e.text());
    Ok((law, ParseOptions { law_id, version_id }))
}

pub fn parse_response(xml: &str) -> Result<LegalDocument, EgovError> {
    let root = Element::parse(xml)?;
    let (law, opts) = law_element(&root)?;
    Ok(parse_law(law, opts)?)
}

/// e-Gov の `law_data` の応答でも、裸の `<Law>` 要素（`emit_law` の出力）でも読む。
/// 裸の `Law` には法令 ID が無いので `law_id` で補う（無ければ None）
pub fn parse_law_xml(xml: &str) -> Result<LegalDocument, EgovError> {
    let root = Element::parse(xml)?;
    if root.name == "Law" {
        return Ok(parse_law(
            &root,
            ParseOptions {
                law_id: None,
                version_id: None,
            },
        )?);
    }
    let (law, opts) = law_element(&root)?;
    Ok(parse_law(law, opts)?)
}
