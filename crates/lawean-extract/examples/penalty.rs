//! 罰則の一覧と空振り: cargo run -p lawean-extract --example penalty -- <xml> [--all]
use lawean_extract::penalty::*;
use lawean_source::parse_response;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let doc = parse_response(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let all = args.iter().any(|a| a == "--all");
    let ps = penalties(&doc);
    let (mut targets, mut with_refs, mut unresolved) = (0, 0, 0);
    for p in &ps {
        if all {
            println!(
                "{} {:?} {}",
                p.sentence.0.rsplit("/main/").next().unwrap_or(""),
                p.sanctions
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<Vec<_>>(),
                p.text.chars().take(60).collect::<String>()
            );
        }
        for t in &p.targets {
            targets += 1;
            if !t.refs.is_empty() {
                with_refs += 1;
            }
            if !t.unresolved.is_empty() {
                unresolved += 1;
            }
            if all {
                println!(
                    "    -> refs {:?} act {:?} {}",
                    t.refs
                        .iter()
                        .map(|r| r.0.rsplit("/main/").next().unwrap_or("").to_string())
                        .collect::<Vec<_>>(),
                    t.act,
                    t.text.chars().take(50).collect::<String>()
                );
            }
        }
    }
    println!(
        "penalties {}, targets {targets}, with refs {with_refs}, with unresolved refs {unresolved}",
        ps.len()
    );
    for m in mismatches(&doc) {
        println!(
            "MISMATCH {:?}\n  罰則 {} 「{}」\n  対象 {} 「{}」",
            m.kind,
            m.target.0.rsplit("/main/").next().unwrap_or(""),
            m.target_text.chars().take(70).collect::<String>(),
            m.referenced.0.rsplit("/main/").next().unwrap_or(""),
            m.referenced_text
        );
    }
}
