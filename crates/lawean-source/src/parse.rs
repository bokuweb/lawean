//! e-Gov 法令標準 XML（`Law` 要素）→ Source IR。

use crate::ir::*;
use crate::xml::{Element, Node};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("root element is <{0}>, expected <Law>")]
    NotLaw(String),
    #[error("<LawBody> not found")]
    NoLawBody,
}

pub struct ParseOptions {
    pub law_id: Option<String>,
    pub version_id: Option<String>,
}

pub fn parse_law(law: &Element, opts: ParseOptions) -> Result<LegalDocument, ParseError> {
    if law.name != "Law" {
        return Err(ParseError::NotLaw(law.name.clone()));
    }
    let root = StableId(opts.law_id.clone().unwrap_or_else(|| "law".to_string()));

    let mut doc = LegalDocument {
        stable_id: root.clone(),
        law_id: opts.law_id,
        version_id: opts.version_id,
        law_attrs: law.attrs.clone(),
        law_num: None,
        title: None,
        enact_statements: Vec::new(),
        toc: None,
        preamble: None,
        main_provision: Vec::new(),
        suppl_provisions: Vec::new(),
        appendices: Vec::new(),
        body_order: Vec::new(),
        law_extra: Vec::new(),
        main_attrs: Vec::new(),
    };

    let mut body = None;
    for c in law.child_elements() {
        match c.name.as_str() {
            "LawNum" => doc.law_num = Some(c.text()),
            "LawBody" => body = Some(c),
            _ => doc.law_extra.push(c.clone()),
        }
    }
    let body = body.ok_or(ParseError::NoLawBody)?;

    for c in body.child_elements() {
        match c.name.as_str() {
            "LawTitle" => {
                doc.title = Some(LawTitle {
                    text: inlines(c),
                    attrs: c.attrs.clone(),
                });
                doc.body_order.push(BodySlot::Title);
            }
            "EnactStatement" => {
                doc.enact_statements.push(c.clone());
                doc.body_order
                    .push(BodySlot::EnactStatement(doc.enact_statements.len() - 1));
            }
            "TOC" => {
                doc.toc = Some(c.clone());
                doc.body_order.push(BodySlot::Toc);
            }
            "Preamble" => {
                doc.preamble = Some(c.clone());
                doc.body_order.push(BodySlot::Preamble);
            }
            "MainProvision" => {
                doc.main_attrs = c.attrs.clone();
                let id = root.child("main");
                doc.main_provision = c.child_elements().map(|e| provision(e, &id)).collect();
                doc.body_order.push(BodySlot::MainProvision);
            }
            "SupplProvision" => {
                let idx = doc.suppl_provisions.len();
                doc.suppl_provisions
                    .push(suppl_provision(c, &root.child(format!("suppl:{idx}"))));
                doc.body_order.push(BodySlot::SupplProvision(idx));
            }
            _ => {
                doc.appendices.push(c.clone());
                doc.body_order
                    .push(BodySlot::Appendix(doc.appendices.len() - 1));
            }
        }
    }
    Ok(doc)
}

fn inlines(el: &Element) -> Vec<Inline> {
    el.children
        .iter()
        .map(|n| match n {
            Node::Text(t) => Inline::Text(t.clone()),
            Node::Element(e) => Inline::Raw(e.clone()),
        })
        .collect()
}

fn attrs_except(el: &Element, keys: &[&str]) -> Vec<(String, String)> {
    el.attrs
        .iter()
        .filter(|(k, _)| !keys.contains(&k.as_str()))
        .cloned()
        .collect()
}

fn provision(el: &Element, parent: &StableId) -> Provision {
    if let Some(kind) = ContainerKind::from_element_name(&el.name) {
        return Provision::Container(container(el, kind, parent));
    }
    if el.name == "Article" {
        return Provision::Article(article(el, parent));
    }
    if el.name == "Paragraph" {
        return Provision::Paragraph(paragraph(el, parent));
    }
    Provision::Raw(el.clone())
}

fn container(el: &Element, kind: ContainerKind, parent: &StableId) -> Container {
    let num = el.attr("Num").map(str::to_string);
    let id = parent.child(format!(
        "{}:{}",
        kind.id_prefix(),
        num.as_deref().unwrap_or("_")
    ));
    let title_name = format!("{}Title", kind.element_name());
    let mut title = None;
    let mut children = Vec::new();
    for c in el.child_elements() {
        if c.name == title_name && title.is_none() {
            title = Some(inlines(c));
        } else {
            children.push(provision(c, &id));
        }
    }
    Container {
        stable_id: id,
        kind,
        num,
        title,
        attrs: attrs_except(el, &["Num"]),
        children,
    }
}

fn article(el: &Element, parent: &StableId) -> Article {
    let num_str = el.attr("Num").unwrap_or("_");
    let id = parent.child(format!("art:{num_str}"));
    let mut caption = None;
    let mut title = None;
    let mut children = Vec::new();
    for c in el.child_elements() {
        match c.name.as_str() {
            "ArticleCaption" if caption.is_none() => caption = Some(inlines(c)),
            "ArticleTitle" if title.is_none() => title = Some(inlines(c)),
            "Paragraph" => children.push(ArticleChild::Paragraph(paragraph(c, &id))),
            _ => children.push(ArticleChild::Raw(c.clone())),
        }
    }
    Article {
        stable_id: id,
        num: ArticleNum::parse(num_str),
        caption,
        title,
        attrs: attrs_except(el, &["Num"]),
        children,
    }
}

fn paragraph(el: &Element, parent: &StableId) -> Paragraph {
    let num = el.attr("Num").unwrap_or("_").to_string();
    let id = parent.child(format!("para:{num}"));
    let mut caption = None;
    let mut num_text = None;
    let mut sentences = Vec::new();
    let mut children = Vec::new();
    for c in el.child_elements() {
        match c.name.as_str() {
            "ParagraphCaption" if caption.is_none() => caption = Some(inlines(c)),
            "ParagraphNum" if num_text.is_none() => num_text = Some(inlines(c)),
            "ParagraphSentence" if sentences.is_empty() => sentences = sentences_of(c, &id),
            "Item" => children.push(ParagraphChild::Item(item(c, &id, 0))),
            _ => children.push(ParagraphChild::Raw(c.clone())),
        }
    }
    Paragraph {
        stable_id: id,
        num,
        caption,
        num_text,
        sentences,
        attrs: attrs_except(el, &["Num"]),
        children,
    }
}

fn item_names(depth: u8) -> (String, String, String) {
    if depth == 0 {
        ("Item".into(), "ItemTitle".into(), "ItemSentence".into())
    } else {
        (
            format!("Subitem{depth}"),
            format!("Subitem{depth}Title"),
            format!("Subitem{depth}Sentence"),
        )
    }
}

fn item(el: &Element, parent: &StableId, depth: u8) -> Item {
    let (_, title_name, sentence_name) = item_names(depth);
    let (child_name, _, _) = item_names(depth + 1);
    let num = el.attr("Num").map(str::to_string);
    let prefix = if depth == 0 {
        "item".to_string()
    } else {
        format!("sub{depth}")
    };
    let id = parent.child(format!("{}:{}", prefix, num.as_deref().unwrap_or("_")));
    let mut title = None;
    let mut body = ItemBody::None;
    let mut children = Vec::new();
    for c in el.child_elements() {
        if c.name == title_name && title.is_none() {
            title = Some(inlines(c));
        } else if c.name == sentence_name && matches!(body, ItemBody::None) {
            body = item_body(c, &id);
        } else if c.name == child_name {
            children.push(ItemChild::Subitem(item(c, &id, depth + 1)));
        } else {
            children.push(ItemChild::Raw(c.clone()));
        }
    }
    Item {
        stable_id: id,
        depth,
        num,
        title,
        body,
        attrs: attrs_except(el, &["Num"]),
        children,
    }
}

fn item_body(el: &Element, parent: &StableId) -> ItemBody {
    let names: Vec<&str> = el.child_elements().map(|e| e.name.as_str()).collect();
    if !names.is_empty() && names.iter().all(|n| *n == "Column") {
        ItemBody::Columns(el.child_elements().map(|c| column(c, parent)).collect())
    } else if names.iter().all(|n| *n == "Sentence") {
        ItemBody::Sentences(sentences_of(el, parent))
    } else {
        ItemBody::Mixed(el.clone())
    }
}

fn column(el: &Element, parent: &StableId) -> Column {
    let num = el.attr("Num").map(str::to_string);
    let id = parent.child(format!("col:{}", num.as_deref().unwrap_or("_")));
    Column {
        stable_id: id.clone(),
        num,
        sentences: sentences_of(el, &id),
        attrs: attrs_except(el, &["Num"]),
    }
}

/// `el` 直下の `Sentence*` を読む
fn sentences_of(el: &Element, parent: &StableId) -> Vec<Sentence> {
    el.child_elements()
        .enumerate()
        .map(|(i, c)| {
            let num = c.attr("Num").map(str::to_string);
            let seg = num.clone().unwrap_or_else(|| (i + 1).to_string());
            Sentence {
                stable_id: parent.child(format!("sent:{seg}")),
                num,
                function: match c.attr("Function") {
                    Some("main") => SentenceFunction::Main,
                    Some("proviso") => SentenceFunction::Proviso,
                    _ => SentenceFunction::Unspecified,
                },
                text: inlines(c),
                attrs: attrs_except(c, &["Num", "Function"]),
            }
        })
        .collect()
}

fn suppl_provision(el: &Element, id: &StableId) -> SupplProvision {
    let mut label = None;
    let mut children = Vec::new();
    for c in el.child_elements() {
        match c.name.as_str() {
            "SupplProvisionLabel" if label.is_none() => label = Some(inlines(c)),
            "Paragraph" => children.push(SupplChild::Paragraph(paragraph(c, id))),
            "Article" | "Part" | "Chapter" | "Section" | "Subsection" | "Division" => {
                children.push(SupplChild::Provision(provision(c, id)))
            }
            _ => children.push(SupplChild::Raw(c.clone())),
        }
    }
    // `Extract` は "true" のときだけ bool に写す。他の値は lossless のため attrs に残す
    let extract = el.attr("Extract") == Some("true");
    let strip: &[&str] = if extract {
        &["AmendLawNum", "Extract"]
    } else {
        &["AmendLawNum"]
    };
    SupplProvision {
        stable_id: id.clone(),
        amend_law_num: el.attr("AmendLawNum").map(str::to_string),
        extract,
        label,
        attrs: attrs_except(el, strip),
        children,
    }
}
