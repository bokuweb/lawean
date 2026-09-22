//! 起草した改め文の矛盾を Z3 で: 「存続期間を三十年以上二十年未満として」は適用される世界が無い（空振り）。
//! playground は同じ SMT-LIB をブラウザ内の z3 に渡す（lawean-wasm::smt_scripts）。z3 が無ければスキップ

use lawean_check::consolidated_document;
use lawean_extract::{extract, to_model};
use lawean_resolve::ResolvedModel;
use lawean_verify::{vacuous, z3_available, Verdict};

#[test]
fn a_contradictory_duration_condition_in_a_draft_is_vacuous() {
    if !z3_available() {
        eprintln!("z3 not on PATH; skipping");
        return;
    }
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/403AC0000000090.xml"
    ))
    .unwrap();
    // 第23条第1項「存続期間を三十年以上五十年未満として」を「三十年以上二十年未満」にしてしまった改め文
    let amendment = "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。\n　　第二十三条第一項中「三十年以上五十年未満」を「三十年以上二十年未満」に改める。\n";
    let doc = consolidated_document(&base, amendment).unwrap();
    let model = to_model(&doc, &extract(&doc));
    let rm = ResolvedModel::new(&model);
    let vs = vacuous(&rm).unwrap();
    let v = vs
        .iter()
        .find(|v| v.rule.0.contains("art:23/para:1/sent:1"))
        .expect("第23条第1項の Rule");
    assert_eq!(v.verdict, Verdict::Proved, "空振り: {:?}", v);
    // 元の本文（三十年以上五十年未満）なら世界が残る
    let doc0 = consolidated_document(
        &base,
        "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。\n",
    )
    .unwrap();
    let model0 = to_model(&doc0, &extract(&doc0));
    let rm0 = ResolvedModel::new(&model0);
    let v0 = vacuous(&rm0)
        .unwrap()
        .into_iter()
        .find(|v| v.rule.0.contains("art:23/para:1/sent:1"))
        .unwrap();
    assert!(matches!(v0.verdict, Verdict::Counterexample(_)), "{v0:?}");
}
