//! 条ずれの相対参照（「前条」）のハネ。Lean `ArtRefsExamples.lean` と同じ規則: 参照元と参照先が同じだけ動けばそのまま、
//! 間に条が入れば絶対形（「第五十五条」）にする。

use lawean_amend::*;
use lawean_source::*;

fn current() -> LegalDocument {
    let path = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn unit(text: &str) -> AmendUnit {
    parse_units(&format!(
        "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。\n{text}"
    ))
    .unwrap()
    .remove(0)
}

/// 第五十五条の次に一条を加え、第五十六条以下を繰り下げる: 旧第五十六条（新第五十七条）の「前条第一項」は
/// 旧第五十五条を指したまま間に新第五十六条が入るので「第五十五条第一項」にする
#[test]
fn prev_article_becomes_absolute_when_an_article_is_inserted_between() {
    let u = unit(
        "　　第六十一条を第六十二条とし、第五十六条から第六十条までを一条ずつ繰り下げ、第五十五条の次に次の一条を加える。
　　（新設）
　第五十六条　甲。",
    );
    let cands = hane_candidates(&current(), &u);
    let c = cands
        .iter()
        .find(|c| c.text.starts_with("前条") && c.sentence.0.contains("/art:56/"))
        .unwrap_or_else(|| panic!("{cands:#?}"));
    assert!(!c.handled);
    assert_eq!(c.fix.as_deref(), Some("第五十五条第一項"), "{c:#?}");
    assert!(c.fix_op.is_some());
}

/// 参照元と参照先が同じだけ動く（第五十五条の前に一条を加える）なら「前条」はそのまま: 候補にしない
#[test]
fn prev_article_stays_when_both_move_together() {
    let u = unit(
        "　　第六十一条を第六十二条とし、第五十五条から第六十条までを一条ずつ繰り下げ、第五十四条の次に次の一条を加える。
　　（新設）
　第五十五条　甲。",
    );
    let cands = hane_candidates(&current(), &u);
    assert!(
        !cands
            .iter()
            .any(|c| c.text.starts_with("前条") && c.sentence.0.contains("/art:56/")),
        "{cands:#?}"
    );
}
