//! 溶け込み後のリビジョンに意味層を当てる（docs/08 §6 の 6）。
//! Semantic IR の Provenance はリビジョンの stable_id に束縛されるので、改正で条文が増減すると整合性が変わる。

use lawean_amend::*;
use lawean_extract::{extract, to_model};
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_semantic::validate;
use lawean_source::*;

fn fixture(rel: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../fixtures/{rel}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// 手書きの Semantic IR（現行法に対するもの）は、令和3年改正前のリビジョンには当てはまらない
/// — 第22条第2項（電磁的記録の定義）が存在しないため。改め文を適用すると当てはまる
#[test]
fn hand_written_model_binds_to_the_amended_revision() {
    let before = parse_response(&fixture(
        "revisions/403AC0000000090_20210519_503AC0000000037.xml",
    ))
    .unwrap();
    let unit = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let after = apply_unit(&before, &unit, "403AC0000000090_20220518_503AC0000000037").unwrap();

    let mut model = shakuchi_shakuya::model();
    // version_id は照合対象に合わせる（Provenance の stable_id の検査だけを見たい）
    model.document = before.version_id.clone().unwrap();
    let issues_before = validate(&model, &before);
    assert!(
        issues_before
            .iter()
            .any(|i| format!("{i:?}").contains("art:22/para:2")),
        "改正前には第22条第2項が無いはず: {issues_before:?}"
    );

    model.document = after.version_id.clone().unwrap();
    let issues_after = validate(&model, &after);
    assert!(issues_after.is_empty(), "{issues_after:#?}");
}

/// 層 1 の抽出は溶け込み後のリビジョンにそのまま走り、追加された項の Rule が骨組みとして現れる
#[test]
fn layer1_extraction_runs_on_the_applied_revision() {
    let before = parse_response(&fixture(
        "revisions/403AC0000000090_20210519_503AC0000000037.xml",
    ))
    .unwrap();
    let unit = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let after = apply_unit(&before, &unit, "applied").unwrap();

    let sk_before = extract(&before);
    let sk_after = extract(&after);
    assert_eq!(
        sk_after.len(),
        sk_before.len() + 5,
        "追加された 4 項分の文（第38条第4項は 2 文）"
    );
    let new22 = sk_after
        .iter()
        .find(|s| s.sentence.0.ends_with("/art:22/para:2/sent:1"))
        .unwrap();
    assert_eq!(new22.effect, lawean_extract::EffectKind::DeemAndApply);

    let model = to_model(&after, &sk_after);
    assert!(validate(&model, &after).is_empty());
    // 追加された「電磁的記録」の定義（第22条第2項の括弧書き）は Column ではないので層 1 の定義抽出には乗らない — 層 2 の仕事
    assert!(model.definitions.iter().all(|d| d.term != "電磁的記録"));
}
