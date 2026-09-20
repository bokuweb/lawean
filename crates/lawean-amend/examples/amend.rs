//! 改め文を改正前リビジョンに適用し、e-Gov のリビジョンと突き合わせ、ハネ候補を出す。
//! cargo run -p lawean-amend --example amend -- fixtures/amendments/503AC0000000037_art35.txt fixtures/revisions/403AC0000000090_20210519_503AC0000000037.xml [expected.xml]

use lawean_amend::*;
use lawean_source::parse_response;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let text = std::fs::read_to_string(&args[0]).expect("amendment text");
    let before =
        parse_response(&std::fs::read_to_string(&args[1]).expect("base xml")).expect("parse");
    let units = parse_units(&text).expect("parse 改め文");
    for u in &units {
        println!(
            "== {} {} ({} instructions)",
            u.article_of_amending_law,
            u.target_title,
            u.instructions.len()
        );
        for ins in &u.instructions {
            println!("  {}", ins.text);
            for op in &ins.ops {
                println!("    {op:?}");
            }
        }
        for c in hane_candidates(&before, u) {
            println!(
                "  hane: {} 「{}」 → {} (new {:?}) {}",
                c.sentence,
                c.text,
                c.target,
                c.new_target_paragraph,
                if c.handled { "handled" } else { "UNHANDLED" }
            );
        }
        match apply_unit(&before, u, "applied") {
            Ok(after) => {
                println!("  applied OK");
                if let Some(exp) = args.get(2) {
                    let expected = parse_response(&std::fs::read_to_string(exp).unwrap()).unwrap();
                    let d = diff_snapshots(&snapshot_main(&after), &snapshot_main(&expected));
                    println!("  diff vs expected: {} lines", d.len());
                    d.iter().for_each(|l| println!("    {l}"));
                }
            }
            Err(e) => println!("  FAILED: {e}"),
        }
    }
}
