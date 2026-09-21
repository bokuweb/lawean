//! 係り受けと主語・目的語を表示: JEWEL_GINZA_BUNDLE=... cargo run -p lawean-nlp --example parse -- "文"
use lawean_nlp::*;
fn main() {
    let text = std::env::args().nth(1).unwrap();
    let p = Parser::from_env().unwrap();
    let st = Stripped::new(&text);
    let words = p.words(&st.text).unwrap();
    let text = st.text.clone();
    for w in &words {
        println!(
            "{:3} {:<12} {:<28} {:<10} -> {}",
            w.i,
            w.text,
            w.tag.unwrap_or("?"),
            w.dep.unwrap_or("?"),
            w.head
        );
    }
    for f in frames(&words, &text) {
        println!(
            "FRAME{} pred={} subj={:?} case={:?} obj={:?} obl={:?}",
            if f.is_root { "*" } else { "" },
            f.predicate_text,
            f.subject_text,
            f.subject_case,
            f.object_text,
            f.obliques
                .iter()
                .map(|(_, t, c)| format!("{t}{c}"))
                .collect::<Vec<_>>()
        );
    }
}
