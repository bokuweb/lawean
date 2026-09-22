//! 文の一覧（gold 作成用）: cargo run -p lawean-extract --example sentences -- <xml> [pattern]
use lawean_source::parse_response;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let doc = parse_response(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let pat = args.get(1).cloned();
    for g in doc.sentence_groups() {
        for s in &g.sentences {
            let t = s.sentence.plain_text();
            if pat.as_ref().is_none_or(|p| t.contains(p.as_str())) {
                println!("{}\t{}", s.sentence.stable_id.0, t);
            }
        }
    }
}
