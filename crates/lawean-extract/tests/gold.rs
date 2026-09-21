//! fixtures/gold: dev（規則作成に使った文）は完全一致、eval（使っていない法令）は下限を割らない。
//! eval の数字を上げるために eval の文を見て規則を直したら、その文は dev に移す（fixtures/gold/README.md）

use lawean_extract::candidates::candidates;
use lawean_extract::eval::{evaluate, parse_gold, Counts};
use lawean_source::parse_response;

fn run(gold_name: &str) -> (usize, Counts) {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let law_id = gold_name
        .trim_start_matches("eval_")
        .trim_start_matches("dev_");
    let xml = std::fs::read_dir(format!("{root}/laws"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(law_id))
        .unwrap();
    let doc = parse_response(&std::fs::read_to_string(&xml).unwrap()).unwrap();
    let gold =
        parse_gold(&std::fs::read_to_string(format!("{root}/gold/{gold_name}.jsonl")).unwrap())
            .unwrap();
    let r = evaluate(&gold, &candidates(&doc));
    for (s, f, raw) in &r.false_positives {
        eprintln!("{gold_name} FP {s} {f:?} 「{raw}」");
    }
    for (s, f, raw) in &r.false_negatives {
        eprintln!("{gold_name} FN {s} {f:?} 「{raw}」");
    }
    (gold.len(), r.total)
}

#[test]
fn dev_gold_matches_exactly() {
    let (n, c) = run("dev_413AC0000000026");
    assert!(n >= 50);
    assert_eq!((c.fp, c.fn_), (0, 0), "{c:?}");
}

#[test]
fn eval_gold_stays_above_the_recorded_floor() {
    let mut total = Counts::default();
    for g in [
        "eval_425AC0000000061",
        "eval_504CO0000000187",
        "eval_504M60000010029",
    ] {
        let (_, c) = run(g);
        total.tp += c.tp;
        total.fp += c.fp;
        total.fn_ += c.fn_;
    }
    // 2026-09-22: tp 14 fp 2 fn 2（誤りは事象の句の境界 2 件）。下回ったら退行
    assert!(total.tp + total.fn_ >= 16);
    assert!(total.precision() >= 0.85, "{total:?}");
    assert!(total.recall() >= 0.85, "{total:?}");
}
