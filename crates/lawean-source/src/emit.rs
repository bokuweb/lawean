//! Source IR → e-Gov 法令標準 XML（`Law` 要素）。`parse` の逆。

use crate::ir::*;
use crate::xml::{Element, Node};

pub fn emit_law(doc: &LegalDocument) -> Element {
    let mut law = Element::new("Law");
    law.attrs = doc.law_attrs.clone();
    if let Some(n) = &doc.law_num {
        let mut e = Element::new("LawNum");
        e.push_text(n.clone());
        law.push(e);
    }
    let mut body = Element::new("LawBody");
    for slot in &doc.body_order {
        match slot {
            BodySlot::Title => {
                if let Some(t) = &doc.title {
                    let mut e = Element::new("LawTitle");
                    e.attrs = t.attrs.clone();
                    push_inlines(&mut e, &t.text);
                    body.push(e);
                }
            }
            BodySlot::EnactStatement(i) => body.push(doc.enact_statements[*i].clone()),
            BodySlot::Toc => {
                if let Some(t) = &doc.toc {
                    body.push(t.clone());
                }
            }
            BodySlot::Preamble => {
                if let Some(p) = &doc.preamble {
                    body.push(p.clone());
                }
            }
            BodySlot::MainProvision => {
                let mut e = Element::new("MainProvision");
                e.attrs = doc.main_attrs.clone();
                for p in &doc.main_provision {
                    e.push(provision(p));
                }
                body.push(e);
            }
            BodySlot::SupplProvision(i) => body.push(suppl_provision(&doc.suppl_provisions[*i])),
            BodySlot::Appendix(i) => body.push(doc.appendices[*i].clone()),
        }
    }
    law.push(body);
    for e in &doc.law_extra {
        law.push(e.clone());
    }
    law
}

fn push_inlines(el: &mut Element, inl: &[Inline]) {
    for i in inl {
        match i {
            Inline::Text(t) => el.children.push(Node::Text(t.clone())),
            Inline::Raw(e) => el.push(e.clone()),
        }
    }
}

fn inline_element(name: &str, inl: &[Inline]) -> Element {
    let mut e = Element::new(name);
    push_inlines(&mut e, inl);
    e
}

fn with_num(mut el: Element, num: Option<&str>, rest: &[(String, String)]) -> Element {
    if let Some(n) = num {
        el.push_attr("Num", n);
    }
    el.attrs.extend(rest.iter().cloned());
    el
}

fn provision(p: &Provision) -> Element {
    match p {
        Provision::Container(c) => container(c),
        Provision::Article(a) => article(a),
        Provision::Paragraph(p) => paragraph(p),
        Provision::Raw(e) => e.clone(),
    }
}

fn container(c: &Container) -> Element {
    let mut el = with_num(
        Element::new(c.kind.element_name()),
        c.num.as_deref(),
        &c.attrs,
    );
    if let Some(t) = &c.title {
        el.push(inline_element(
            &format!("{}Title", c.kind.element_name()),
            t,
        ));
    }
    for ch in &c.children {
        el.push(provision(ch));
    }
    el
}

fn article(a: &Article) -> Element {
    let num = a.num.to_num_string();
    let mut el = with_num(Element::new("Article"), Some(&num), &a.attrs);
    if let Some(c) = &a.caption {
        el.push(inline_element("ArticleCaption", c));
    }
    if let Some(t) = &a.title {
        el.push(inline_element("ArticleTitle", t));
    }
    for ch in &a.children {
        match ch {
            ArticleChild::Paragraph(p) => el.push(paragraph(p)),
            ArticleChild::Raw(e) => el.push(e.clone()),
        }
    }
    el
}

fn paragraph(p: &Paragraph) -> Element {
    let mut el = with_num(Element::new("Paragraph"), Some(&p.num), &p.attrs);
    if let Some(c) = &p.caption {
        el.push(inline_element("ParagraphCaption", c));
    }
    if let Some(n) = &p.num_text {
        el.push(inline_element("ParagraphNum", n));
    }
    if !p.sentences.is_empty() {
        let mut ps = Element::new("ParagraphSentence");
        for s in &p.sentences {
            ps.push(sentence(s));
        }
        el.push(ps);
    }
    for ch in &p.children {
        match ch {
            ParagraphChild::Item(i) => el.push(item(i)),
            ParagraphChild::Raw(e) => el.push(e.clone()),
        }
    }
    el
}

fn item(i: &Item) -> Element {
    let (name, title_name, sentence_name) = if i.depth == 0 {
        (
            "Item".to_string(),
            "ItemTitle".to_string(),
            "ItemSentence".to_string(),
        )
    } else {
        (
            format!("Subitem{}", i.depth),
            format!("Subitem{}Title", i.depth),
            format!("Subitem{}Sentence", i.depth),
        )
    };
    let mut el = with_num(Element::new(name), i.num.as_deref(), &i.attrs);
    if let Some(t) = &i.title {
        el.push(inline_element(&title_name, t));
    }
    match &i.body {
        ItemBody::Sentences(ss) => {
            let mut e = Element::new(sentence_name);
            for s in ss {
                e.push(sentence(s));
            }
            el.push(e);
        }
        ItemBody::Columns(cols) => {
            let mut e = Element::new(sentence_name);
            for c in cols {
                let mut ce = with_num(Element::new("Column"), c.num.as_deref(), &c.attrs);
                for s in &c.sentences {
                    ce.push(sentence(s));
                }
                e.push(ce);
            }
            el.push(e);
        }
        ItemBody::Mixed(e) => el.push(e.clone()),
        ItemBody::None => {}
    }
    for ch in &i.children {
        match ch {
            ItemChild::Subitem(s) => el.push(item(s)),
            ItemChild::Raw(e) => el.push(e.clone()),
        }
    }
    el
}

fn sentence(s: &Sentence) -> Element {
    let mut el = Element::new("Sentence");
    match s.function {
        SentenceFunction::Main => el.push_attr("Function", "main"),
        SentenceFunction::Proviso => el.push_attr("Function", "proviso"),
        SentenceFunction::Unspecified => {}
    }
    if let Some(n) = &s.num {
        el.push_attr("Num", n.clone());
    }
    el.attrs.extend(s.attrs.iter().cloned());
    push_inlines(&mut el, &s.text);
    el
}

fn suppl_provision(s: &SupplProvision) -> Element {
    let mut el = Element::new("SupplProvision");
    if let Some(n) = &s.amend_law_num {
        el.push_attr("AmendLawNum", n.clone());
    }
    if s.extract {
        el.push_attr("Extract", "true");
    }
    el.attrs.extend(s.attrs.iter().cloned());
    if let Some(l) = &s.label {
        el.push(inline_element("SupplProvisionLabel", l));
    }
    for ch in &s.children {
        match ch {
            SupplChild::Provision(p) => el.push(provision(p)),
            SupplChild::Paragraph(p) => el.push(paragraph(p)),
            SupplChild::Raw(e) => el.push(e.clone()),
        }
    }
    el
}
