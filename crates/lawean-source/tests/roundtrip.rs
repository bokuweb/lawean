//! XML → Source IR → XML の往復テスト。要素間の空白と属性順だけを正規化して比較する。

use lawean_source::egov::law_element;
use lawean_source::xml::Element;
use lawean_source::{emit_law, parse_law};

const FIXTURES: &[&str] = &["403AC0000000090", "129AC0000000089"];

fn fixture(id: &str) -> String {
    let path = format!("{}/../../fixtures/{id}.xml", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn roundtrip_fixtures() {
    for id in FIXTURES {
        let root = Element::parse(&fixture(id)).unwrap();
        let (law, opts) = law_element(&root).unwrap();
        let doc = parse_law(law, opts).unwrap();
        let back = emit_law(&doc);

        let expected = law.canonicalize();
        let actual = back.canonicalize();
        if expected != actual {
            let (path, a, b) = first_diff(&expected, &actual, String::from("Law"));
            panic!("{id}: roundtrip mismatch at {path}\n  expected: {a}\n  actual:   {b}");
        }
        // 文字列化した上でもう一度パースして同じになること
        let reparsed = Element::parse(&back.to_xml()).unwrap().canonicalize();
        assert_eq!(reparsed, actual, "{id}: serialize/parse not stable");
    }
}

fn first_diff(a: &Element, b: &Element, path: String) -> (String, String, String) {
    if a.name != b.name || a.attrs != b.attrs {
        return (
            path,
            format!("<{} {:?}>", a.name, a.attrs),
            format!("<{} {:?}>", b.name, b.attrs),
        );
    }
    for (i, (x, y)) in a.children.iter().zip(b.children.iter()).enumerate() {
        use lawean_source::xml::Node;
        match (x, y) {
            (Node::Element(x), Node::Element(y)) => {
                if x != y {
                    return first_diff(x, y, format!("{path}/{}[{i}]", x.name));
                }
            }
            (Node::Text(x), Node::Text(y)) => {
                if x != y {
                    return (format!("{path}/#text[{i}]"), x.clone(), y.clone());
                }
            }
            _ => return (format!("{path}/[{i}]"), format!("{x:?}"), format!("{y:?}")),
        }
    }
    (
        path,
        format!("{} children", a.children.len()),
        format!("{} children", b.children.len()),
    )
}
