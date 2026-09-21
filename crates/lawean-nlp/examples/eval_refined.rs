//! gold に対して、規則の候補を係り受けで補正する前後の適合率・再現率:
//! JEWEL_GINZA_BUNDLE=... cargo run --release -p lawean-nlp --example eval_refined
use lawean_extract::candidates::candidates;
use lawean_extract::eval::{evaluate, parse_gold, Counts};
use lawean_nlp::Parser;
use lawean_source::parse_response;
use std::collections::BTreeMap;

fn main() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let p = Parser::from_env().unwrap();
    let mut golds: Vec<_> = std::fs::read_dir(format!("{root}/gold"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    golds.sort();
    let (mut before, mut after) = (Counts::default(), Counts::default());
    for g in golds {
        let name = g.file_stem().unwrap().to_string_lossy().to_string();
        let law_id = name.trim_start_matches("eval_").trim_start_matches("dev_");
        let xml = std::fs::read_dir(format!("{root}/laws"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(law_id))
            .unwrap();
        let doc = parse_response(&std::fs::read_to_string(&xml).unwrap()).unwrap();
        let gold = parse_gold(&std::fs::read_to_string(&g).unwrap()).unwrap();
        let texts: BTreeMap<String, String> = doc
            .sentence_groups()
            .iter()
            .flat_map(|g| {
                g.sentences
                    .iter()
                    .map(|s| (s.sentence.stable_id.0.clone(), s.sentence.plain_text()))
            })
            .collect();
        let cs = candidates(&doc);
        let r0 = evaluate(&gold, &cs);
        // 文ごとに補正
        let mut by: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for c in cs {
            by.entry(c.evidence.sentence.0.clone()).or_default().push(c);
        }
        let mut refined = Vec::new();
        for (sid, v) in by {
            refined.extend(p.refine_events(&texts[&sid], v));
        }
        let r1 = evaluate(&gold, &refined);
        println!(
            "{name}: rule P {:.2} R {:.2} → +ginza P {:.2} R {:.2}",
            r0.total.precision(),
            r0.total.recall(),
            r1.total.precision(),
            r1.total.recall()
        );
        for (s, f, raw) in r1
            .false_positives
            .iter()
            .filter(|x| !r0.false_positives.contains(x))
        {
            println!(
                "  new FP {} {f:?} 「{raw}」",
                s.split_once("/main/").map(|x| x.1).unwrap_or(s)
            );
        }
        for x in r0
            .false_positives
            .iter()
            .filter(|x| !r1.false_positives.contains(x))
        {
            println!(
                "  fixed  {} {:?} 「{}」",
                x.0.split_once("/main/").map(|y| y.1).unwrap_or(&x.0),
                x.1,
                x.2
            );
        }
        for c in [&r0.total] {
            before.tp += c.tp;
            before.fp += c.fp;
            before.fn_ += c.fn_;
        }
        for c in [&r1.total] {
            after.tp += c.tp;
            after.fp += c.fp;
            after.fn_ += c.fn_;
        }
    }
    println!(
        "all: rule P {:.3} R {:.3} → +ginza P {:.3} R {:.3}",
        before.precision(),
        before.recall(),
        after.precision(),
        after.recall()
    );
}
