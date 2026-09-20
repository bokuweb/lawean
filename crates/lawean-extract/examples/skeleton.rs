//! 層 1 の抽出結果を条ごとに表示する。層 2（LLM）に渡す入力の確認用。
//! cargo run -p lawean-extract --example skeleton -- fixtures/403AC0000000090.xml 22

use lawean_extract::*;
use lawean_source::*;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("fixture");
    let want: Option<u32> = args.next().and_then(|s| s.parse().ok());
    let doc = parse_response(&std::fs::read_to_string(path).unwrap()).unwrap();
    for s in extract(&doc) {
        if let Some(n) = want {
            if !s.sentence.0.contains(&format!("/art:{n}/")) {
                continue;
            }
        }
        println!(
            "{} [{:?}] {:?} {}",
            s.sentence,
            s.function,
            s.effect,
            s.effect_tail.as_deref().unwrap_or("")
        );
        for c in &s.conditions {
            println!("    if   {} ({})", c.text, c.marker);
        }
        for o in &s.overrides {
            println!("    over {o:?}");
        }
        for c in &s.concessions {
            println!("    conc {}", c.text);
        }
        for (t, r) in &s.references {
            println!("    ref  {t} -> {r:?}");
        }
    }
}
