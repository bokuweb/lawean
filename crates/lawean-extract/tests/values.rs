//! 層 1 が時間表現から埋めた Set の値・条件の比較を、手書き IR（借地借家法 8 条）と突き合わせる。
//! 手書きの Rule の provenance（文）と同じ文の自動 Rule が、同じ値を持つか

use lawean_extract::{extract, to_model};
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_semantic::*;
use lawean_source::parse_response;

fn cmp_durations(e: &Expr, out: &mut Vec<(CmpOp, Duration)>) {
    match e {
        Expr::Cmp(_, op, Value::Duration(d)) => out.push((*op, *d)),
        Expr::And(xs) | Expr::Or(xs) => xs.iter().for_each(|x| cmp_durations(x, out)),
        Expr::Not(x) => cmp_durations(x, out),
        _ => {}
    }
}

#[test]
fn extracted_values_match_the_hand_written_ir() {
    let p = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    let doc = parse_response(&std::fs::read_to_string(p).unwrap()).unwrap();
    let auto = to_model(&doc, &extract(&doc));
    let hand = shakuchi_shakuya::model();
    let (mut set_total, mut set_hit, mut cmp_total, mut cmp_hit) = (0, 0, 0, 0);
    let mut misses = Vec::new();
    for h in &hand.rules {
        let autos: Vec<&Rule> = auto
            .rules
            .iter()
            .filter(|a| a.provenance.source == h.provenance.source)
            .collect();
        // Set の値（期間・時点）
        if let Effect::Set { value, .. } = &h.effect {
            let typed = matches!(value, Value::Duration(_) | Value::Period(_));
            if typed {
                set_total += 1;
                let hit = autos
                    .iter()
                    .any(|a| matches!(&a.effect, Effect::Set { value: v, .. } if v == value));
                if hit {
                    set_hit += 1;
                } else {
                    misses.push(format!(
                        "{} Set {value:?} ← {:?}",
                        h.id.0,
                        autos.iter().map(|a| &a.effect).collect::<Vec<_>>()
                    ));
                }
            }
        }
        // 条件の中の期間の比較
        let mut hc = Vec::new();
        cmp_durations(&h.condition, &mut hc);
        for (op, d) in hc {
            cmp_total += 1;
            let mut ac = Vec::new();
            autos
                .iter()
                .for_each(|a| cmp_durations(&a.condition, &mut ac));
            if ac.iter().any(|(o, x)| *o == op && *x == d) {
                cmp_hit += 1;
            } else {
                misses.push(format!("{} Cmp {op:?} {d:?} ← {ac:?}", h.id.0));
            }
        }
    }
    for m in &misses {
        eprintln!("miss: {m}");
    }
    eprintln!("Set values: {set_hit}/{set_total}, Cmp durations: {cmp_hit}/{cmp_total}");
    // 2026-09-22: Set 2/3（R4-1' の「（…二十年）」は括弧書きの場合分けで、1 文から 2 Rule に分ける層 2 の仕事）、
    // Cmp 1/2（R3-2 の「これより長い期間」の「これ」は照応で、層 2）
    assert!(set_hit >= 2 && set_total >= 3, "{set_hit}/{set_total}");
    assert!(cmp_hit >= 1 && cmp_total >= 2, "{cmp_hit}/{cmp_total}");
}
