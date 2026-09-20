//! fixture を読んで、条文を stable_id 付きで出す。docs/03-examples を書くときの下調べ用。
//!
//! ```sh
//! cargo run -p lawean-source --example dump -- fixtures/403AC0000000090.xml 22
//! cargo run -p lawean-source --example dump -- fixtures/403AC0000000090.xml suppl:0 4
//! ```

use lawean_source::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: dump <fixture.xml> [suppl:N] [article]");
    let mut rest: Vec<String> = args.collect();
    let suppl = rest.first().filter(|s| s.starts_with("suppl:")).cloned();
    if suppl.is_some() {
        rest.remove(0);
    }
    let article: Option<u32> = rest.first().and_then(|s| s.parse().ok());

    let doc = parse_response(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    println!(
        "# {} {} ({})",
        doc.law_num.as_deref().unwrap_or(""),
        doc.title
            .as_ref()
            .map(|t| inline_text(&t.text))
            .unwrap_or_default(),
        doc.version_id.as_deref().unwrap_or("-")
    );

    match suppl {
        Some(s) => {
            let idx: usize = s["suppl:".len()..].parse().expect("suppl index");
            let sp = &doc.suppl_provisions[idx];
            println!(
                "## {} {}",
                sp.stable_id,
                sp.amend_law_num.as_deref().unwrap_or("(原始附則)")
            );
            for c in &sp.children {
                match c {
                    SupplChild::Provision(p) => provision(p, article),
                    SupplChild::Paragraph(p) => paragraph(p),
                    SupplChild::Raw(e) => println!("  [raw <{}>]", e.name),
                }
            }
        }
        None => doc
            .main_provision
            .iter()
            .for_each(|p| provision(p, article)),
    }
}

fn provision(p: &Provision, only: Option<u32>) {
    match p {
        Provision::Container(c) => {
            if only.is_none() {
                println!(
                    "## {} {}",
                    c.stable_id,
                    c.title.as_ref().map(|t| inline_text(t)).unwrap_or_default()
                );
            }
            c.children.iter().for_each(|c| provision(c, only));
        }
        Provision::Article(a) => {
            if let Some(n) = only {
                if a.num
                    != (ArticleNum::Single {
                        base: n,
                        branch: vec![],
                    })
                {
                    return;
                }
            }
            println!(
                "### {} {} {}",
                a.stable_id,
                a.title.as_ref().map(|t| inline_text(t)).unwrap_or_default(),
                a.caption
                    .as_ref()
                    .map(|t| inline_text(t))
                    .unwrap_or_default()
            );
            for c in &a.children {
                match c {
                    ArticleChild::Paragraph(p) => paragraph(p),
                    ArticleChild::Raw(e) => println!("  [raw <{}>]", e.name),
                }
            }
        }
        Provision::Raw(e) => println!("[raw <{}>]", e.name),
    }
}

fn paragraph(p: &Paragraph) {
    for s in &p.sentences {
        println!(
            "  {:<60} [{:?}] {}",
            s.stable_id,
            s.function,
            s.plain_text()
        );
    }
    for c in &p.children {
        match c {
            ParagraphChild::Item(i) => item(i),
            ParagraphChild::Raw(e) => println!("  [raw <{}>]", e.name),
        }
    }
}

fn item(i: &Item) {
    let indent = "  ".repeat(2 + i.depth as usize);
    let title = i.title.as_ref().map(|t| inline_text(t)).unwrap_or_default();
    match &i.body {
        ItemBody::Sentences(ss) => {
            for s in ss {
                println!(
                    "{indent}{:<56} {title} [{:?}] {}",
                    s.stable_id,
                    s.function,
                    s.plain_text()
                );
            }
        }
        ItemBody::Columns(cols) => {
            let cells: Vec<String> = cols
                .iter()
                .map(|c| {
                    c.sentences
                        .iter()
                        .map(|s| s.plain_text())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .collect();
            println!(
                "{indent}{:<56} {title} | {}",
                i.stable_id,
                cells.join(" | ")
            );
        }
        ItemBody::Mixed(e) => println!("{indent}{} [mixed <{}>]", i.stable_id, e.name),
        ItemBody::None => println!("{indent}{} {title}", i.stable_id),
    }
    for c in &i.children {
        match c {
            ItemChild::Subitem(s) => item(s),
            ItemChild::Raw(e) => println!("{indent}[raw <{}>]", e.name),
        }
    }
}
