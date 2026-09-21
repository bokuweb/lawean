//! 参照の認識率・解決率: cargo run -p lawean-resolve --example refcov -- <xml> [--list]
//!
//! 「第N条」「前項」「同条」の形をした字句（広い正規表現）のうち、参照として認識されたもの、
//! さらに stable_id に解決できたものの割合。認識されなかった形と、解決できなかった理由を出す。

use lawean_resolve::*;
use lawean_source::*;
use regex::Regex;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let doc = parse_response(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let list = args.iter().any(|a| a == "--list");
    let index = Index::build(&doc);
    const N: &str = "[一二三四五六七八九十百千]+";
    let broad = Regex::new(&format!(
        r"(?:附則)?第{N}条(?:の{N})*(?:第{N}項)?(?:第{N}号)?|前各項|前{N}?条|前{N}?項|次条|次項|同条|同項|第{N}項(?:第{N}号)?|第{N}号|前各号|前号|次号|各号"
    ))
    .unwrap();
    let law_num = Regex::new(&format!(
        r"(?:明治|大正|昭和|平成|令和){N}年(?:法律|政令|勅令|省令|規則)第{N}号"
    ))
    .unwrap();
    let (mut total, mut recognized, mut resolved) = (0, 0, 0);
    let mut unrec: BTreeMap<String, usize> = BTreeMap::new();
    let mut unres: BTreeMap<String, usize> = BTreeMap::new();
    let mut external = 0;
    for g in doc.sentence_groups() {
        let mut ante = Antecedent::default();
        for s in &g.sentences {
            let text = s.sentence.plain_text();
            let spans = resolve_sentence_with(&index, &s.sentence.stable_id, &text, &mut ante);
            let taken: Vec<(usize, usize)> = law_num
                .find_iter(&text)
                .map(|m| (m.start(), m.end()))
                .collect();
            for m in broad.find_iter(&text) {
                if taken.iter().any(|(a, b)| m.start() < *b && *a < m.end()) {
                    continue;
                }
                // 「各号」「次の各号」は自分の項の号の列挙で、参照ではない
                if matches!(m.as_str(), "各号") {
                    continue;
                }
                total += 1;
                let Some(sp) = spans
                    .iter()
                    .find(|r| r.span.start <= m.start() && m.end() <= r.span.end)
                else {
                    *unrec.entry(shape(m.as_str())).or_insert(0) += 1;
                    if list {
                        println!(
                            "UNREC {} 「{}」 {}",
                            s.sentence.stable_id.0.rsplit("/main/").next().unwrap_or(""),
                            m.as_str(),
                            text.chars().take(50).collect::<String>()
                        );
                    }
                    continue;
                };
                recognized += 1;
                match &sp.resolution {
                    Resolution::Internal(_) => resolved += 1,
                    Resolution::External { .. } => {
                        resolved += 1;
                        external += 1;
                    }
                    Resolution::Unresolved(u) => {
                        let k = match u {
                            Unresolved::NoAntecedent => "先行詞なし".to_string(),
                            Unresolved::UnknownOrigin(_) => "参照元が索引にない".to_string(),
                            Unresolved::NotFound(_) => format!("参照先なし: {}", shape(m.as_str())),
                            Unresolved::InReadReplace => "読替えの「」の中".to_string(),
                        };
                        *unres.entry(k).or_insert(0) += 1;
                        if list {
                            println!(
                                "UNRES {} 「{}」 {:?} {}",
                                s.sentence.stable_id.0.rsplit("/main/").next().unwrap_or(""),
                                m.as_str(),
                                u,
                                text.chars().take(50).collect::<String>()
                            );
                        }
                    }
                }
            }
        }
    }
    println!(
        "reference-like tokens {total}: recognized {recognized} ({:.1}%), resolved {resolved} ({:.1}%, external {external})",
        100.0 * recognized as f64 / total.max(1) as f64,
        100.0 * resolved as f64 / total.max(1) as f64
    );
    for (k, v) in &unrec {
        println!("  unrecognized {v:4} {k}");
    }
    for (k, v) in &unres {
        println!("  unresolved   {v:4} {k}");
    }
}

/// 数を N に潰した形
fn shape(s: &str) -> String {
    Regex::new("[一二三四五六七八九十百千]+")
        .unwrap()
        .replace_all(s, "N")
        .into_owned()
}
