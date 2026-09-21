//! 法令の全文から時間表現を取り、被覆率を出す。
//! cargo run -p lawean-extract --example temporal -- fixtures/403AC0000000090.xml [--all]

use lawean_extract::temporal::*;
use lawean_source::parse_response;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let doc = parse_response(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let all = args.iter().any(|a| a == "--all");
    let (mut sentences, mut exprs, mut bare, mut bare_list) = (0, 0, 0, Vec::new());
    for g in doc.sentence_groups() {
        for s in &g.sentences {
            let text = s.sentence.plain_text();
            let es = time_exprs(&text);
            if es.is_empty() {
                continue;
            }
            sentences += 1;
            exprs += es.len();
            for e in &es {
                if matches!(e.kind, TimeKind::Duration(_)) {
                    bare += 1;
                    bare_list.push(format!(
                        "{}: 「{}」 in {}",
                        s.sentence.stable_id.0.rsplit("/main/").next().unwrap_or(""),
                        e.text,
                        text.chars().take(60).collect::<String>()
                    ));
                }
                if all {
                    println!("{} {:?}", e.text, e.kind);
                }
            }
        }
    }
    println!("sentences with time expressions: {sentences}");
    println!(
        "time expressions: {exprs}, structured: {} ({:.0}%), bare durations: {bare}",
        exprs - bare,
        100.0 * (exprs - bare) as f64 / exprs.max(1) as f64
    );
    for b in bare_list {
        println!("  bare: {b}");
    }
}
