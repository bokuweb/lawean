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

/// dev は完全一致。例外は既知の 1 件: 「暴力団員又は同号に規定する暴力団員でなくなった日」の「又は」が
/// 号の選択肢か事象の並列かは規則では決まらない（係り受けが要る。docs/11 L1.5）
#[test]
fn dev_gold_matches_exactly() {
    let mut total = Counts::default();
    for g in [
        "dev_413AC0000000026",
        "dev_337AC0000000069",
        "dev_425AC0000000061",
        "dev_327AC1000000176",
        "dev_323AC0000000138",
        "dev_325AC0000000158",
        "dev_323AC0000000201",
    ] {
        let (n, c) = run(g);
        assert!(n >= 15, "{g}: {n}");
        total.tp += c.tp;
        total.fp += c.fp;
        total.fn_ += c.fn_;
    }
    assert!(total.tp >= 190, "{total:?}");
    assert!(total.fp <= 1 && total.fn_ <= 1, "{total:?}");
}

#[test]
fn eval_gold_stays_above_the_recorded_floor() {
    let mut total = Counts::default();
    for g in [
        "eval_324AC0000000108",
        "eval_504CO0000000187",
        "eval_504M60000010029",
    ] {
        let (_, c) = run(g);
        total.tp += c.tp;
        total.fp += c.fp;
        total.fn_ += c.fn_;
    }
    // 2026-09-22: 古物営業法（規則を直さずに測った）tp 45 fp 2 fn 2 → P 0.96 / R 0.96。
    // 誤り: 「前二条の帳簿等を最終の記載をした日から三年間」（目的語「〜を」が事象に入る。「を」の節境界は refine_events が縮める）
    assert!(total.tp + total.fn_ >= 45);
    assert!(total.precision() >= 0.85, "{total:?}");
    assert!(total.recall() >= 0.85, "{total:?}");
}
