//! 指定した条の参照表現を解決して表示する。
//! cargo run -p lawean-resolve --example refs -- fixtures/403AC0000000090.xml 7

use lawean_resolve::*;
use lawean_source::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("fixture");
    let want: u32 = args.next().and_then(|s| s.parse().ok()).expect("article");
    let doc = parse_response(&std::fs::read_to_string(path).unwrap()).unwrap();
    let index = Index::build(&doc);
    fn walk<'a>(ps: &'a [Provision], want: u32, out: &mut Vec<&'a Article>) {
        for p in ps {
            match p {
                Provision::Container(c) => walk(&c.children, want, out),
                Provision::Article(a)
                    if a.num
                        == (ArticleNum::Single {
                            base: want,
                            branch: vec![],
                        }) =>
                {
                    out.push(a)
                }
                _ => {}
            }
        }
    }
    let mut arts = Vec::new();
    walk(&doc.main_provision, want, &mut arts);
    for a in arts {
        for c in &a.children {
            if let ArticleChild::Paragraph(p) = c {
                let sents: Vec<(StableId, String)> = p
                    .sentences
                    .iter()
                    .map(|s| (s.stable_id.clone(), s.plain_text()))
                    .collect();
                println!("{}", p.stable_id);
                for r in resolve_paragraph(&index, &sents) {
                    println!("  {:<24} {:?}", r.span.text, r.resolution);
                }
            }
        }
    }
}
