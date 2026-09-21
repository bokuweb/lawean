//! gold に対する適合率・再現率: cargo run -p lawean-extract --example eval
//! fixtures/gold/eval_<law_id>.jsonl ごとに fixtures/laws/<law_id>*.xml を読む
use lawean_extract::candidates::candidates;
use lawean_extract::eval::{evaluate, parse_gold, Counts};
use lawean_source::parse_response;
use std::collections::BTreeMap;

fn main() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut totals: BTreeMap<&str, Counts> = BTreeMap::new();
    let mut per_field: BTreeMap<String, Counts> = BTreeMap::new();
    let mut golds: Vec<_> = std::fs::read_dir(format!("{root}/gold"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    golds.sort();
    for g in golds {
        let name = g.file_stem().unwrap().to_string_lossy().to_string();
        let law_id = name.trim_start_matches("eval_").trim_start_matches("dev_");
        let xml = std::fs::read_dir(format!("{root}/laws"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(law_id))
            .unwrap_or_else(|| panic!("no fixture for {law_id}"));
        let doc = parse_response(&std::fs::read_to_string(&xml).unwrap()).unwrap();
        let gold = parse_gold(&std::fs::read_to_string(&g).unwrap()).unwrap();
        let r = evaluate(&gold, &candidates(&doc));
        println!(
            "{name}: sentences {}, tp {} fp {} fn {} (P {:.2} R {:.2})",
            gold.len(),
            r.total.tp,
            r.total.fp,
            r.total.fn_,
            r.total.precision(),
            r.total.recall()
        );
        for (s, f, raw) in &r.false_positives {
            println!(
                "  FP {} {f:?} 「{raw}」",
                s.rsplit('/')
                    .nth(3)
                    .map(|_| s.split_once("00/").map(|x| x.1).unwrap_or(s))
                    .unwrap_or(s)
            );
        }
        for (s, f, raw) in &r.false_negatives {
            println!(
                "  FN {} {f:?} 「{raw}」",
                s.split_once("00/").map(|x| x.1).unwrap_or(s)
            );
        }
        let split = if name.starts_with("eval_") {
            "eval（規則作成に使っていない）"
        } else {
            "dev（規則作成に使った）"
        };
        let t = totals.entry(split).or_default();
        t.tp += r.total.tp;
        t.fp += r.total.fp;
        t.fn_ += r.total.fn_;
        if !name.starts_with("eval_") {
            continue;
        }
        for (f, c) in r.per_field {
            let e = per_field.entry(format!("{f:?}")).or_default();
            e.tp += c.tp;
            e.fp += c.fp;
            e.fn_ += c.fn_;
        }
    }
    for (split, total) in &totals {
        println!(
            "\n{split}: tp {} fp {} fn {} → precision {:.2} recall {:.2}",
            total.tp,
            total.fp,
            total.fn_,
            total.precision(),
            total.recall()
        );
    }
    println!("eval のフィールド別:");
    for (f, c) in per_field {
        println!(
            "  {f:<14} tp {:2} fp {:2} fn {:2}  P {:.2} R {:.2}",
            c.tp,
            c.fp,
            c.fn_,
            c.precision(),
            c.recall()
        );
    }
}
