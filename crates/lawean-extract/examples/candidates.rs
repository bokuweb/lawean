//! 候補を JSON Lines で: cargo run -p lawean-extract --example candidates -- <xml> [--text]
use lawean_extract::candidates::candidates;
use lawean_source::parse_response;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let doc = parse_response(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let with_text = args.iter().any(|a| a == "--text");
    let texts: std::collections::BTreeMap<String, String> = doc
        .sentence_groups()
        .iter()
        .flat_map(|g| {
            g.sentences
                .iter()
                .map(|s| (s.sentence.stable_id.0.clone(), s.sentence.plain_text()))
        })
        .collect();
    let mut last = String::new();
    for c in candidates(&doc) {
        if with_text && c.evidence.sentence.0 != last {
            last = c.evidence.sentence.0.clone();
            println!(
                "# {} {}",
                last.rsplit("/main/").next().unwrap_or(&last),
                texts[&last]
            );
        }
        println!("{}", serde_json::to_string(&c).unwrap());
    }
}
