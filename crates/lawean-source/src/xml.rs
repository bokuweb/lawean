//! 汎用の XML 要素ツリー。Source IR が型付けしない要素（TOC、別表、Ruby など）は
//! この形のまま `Raw` として保持することで lossless を担保する。
//! 名前空間・コメント・処理命令は e-Gov XML に出てこないので扱わない。

use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    /// 出現順を保つ
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Element {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn push_attr(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attrs.push((key.into(), value.into()));
    }

    pub fn push(&mut self, child: Element) {
        self.children.push(Node::Element(child));
    }

    pub fn push_text(&mut self, text: impl Into<String>) {
        self.children.push(Node::Text(text.into()));
    }

    pub fn child_elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    pub fn first_child(&self, name: &str) -> Option<&Element> {
        self.child_elements().find(|e| e.name == name)
    }

    /// 子孫のテキストを連結する（要素境界は無視）
    pub fn text(&self) -> String {
        let mut out = String::new();
        collect_text(self, &mut out);
        out
    }

    pub fn from_roxml(node: roxmltree::Node<'_, '_>) -> Element {
        let mut el = Element::new(node.tag_name().name());
        for a in node.attributes() {
            el.push_attr(a.name(), a.value());
        }
        for c in node.children() {
            if c.is_element() {
                el.push(Element::from_roxml(c));
            } else if c.is_text() {
                el.push_text(c.text().unwrap_or(""));
            }
        }
        el
    }

    pub fn parse(xml: &str) -> Result<Element, roxmltree::Error> {
        let doc = roxmltree::Document::parse(xml)?;
        Ok(Element::from_roxml(doc.root_element()))
    }

    /// 要素間の ASCII 空白だけのテキストノードを落とし、属性をソートする。往復比較用。
    /// U+3000（全角空白）は法令本文に意味を持つので落とさない。
    pub fn canonicalize(&self) -> Element {
        let mut el = Element::new(self.name.clone());
        el.attrs = self.attrs.clone();
        el.attrs.sort();
        let has_element_child = self.child_elements().next().is_some();
        for c in &self.children {
            match c {
                Node::Element(e) => el.push(e.canonicalize()),
                Node::Text(t) => {
                    if has_element_child
                        && t.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
                            .is_empty()
                    {
                        continue;
                    }
                    el.push_text(t.clone());
                }
            }
        }
        el
    }

    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        write_element(self, &mut out);
        out
    }
}

fn collect_text(el: &Element, out: &mut String) {
    for c in &el.children {
        match c {
            Node::Element(e) => collect_text(e, out),
            Node::Text(t) => out.push_str(t),
        }
    }
}

fn write_element(el: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&el.name);
    for (k, v) in &el.attrs {
        let _ = write!(out, " {}=\"{}\"", k, escape(v, true));
    }
    if el.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &el.children {
        match c {
            Node::Element(e) => write_element(e, out),
            Node::Text(t) => out.push_str(&escape(t, false)),
        }
    }
    let _ = write!(out, "</{}>", el.name);
}

fn escape(s: &str, in_attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if in_attr => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_simple() {
        let src = r#"<A x="1"><B>t &amp; u</B>  <C/></A>"#;
        let el = Element::parse(src).unwrap();
        assert_eq!(el.attr("x"), Some("1"));
        let back = Element::parse(&el.to_xml()).unwrap();
        assert_eq!(el.canonicalize(), back.canonicalize());
    }

    #[test]
    fn canonicalize_keeps_ideographic_space() {
        let el = Element::parse("<L>附　則</L>").unwrap().canonicalize();
        assert_eq!(el.text(), "附　則");
    }
}
