//! 手書き例が Source IR と整合していること（stable_id・RuleId・DefinitionId が全部解決できる）。

use lawean_semantic::examples::shakuchi_shakuya;
use lawean_semantic::*;

fn source() -> lawean_source::LegalDocument {
    let path = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    lawean_source::parse_response(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn shakuchi_examples_resolve_against_source_ir() {
    let model = shakuchi_shakuya::model();
    let issues = validate(&model, &source());
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn every_rule_has_provenance_and_proviso_rules_override_something() {
    let model = shakuchi_shakuya::model();
    for r in &model.rules {
        assert!(
            !r.provenance.source.0.is_empty(),
            "{}: no provenance",
            r.id.0
        );
        // ただし書きは本文の例外（overrides）か、本文が適用された上での追加規定（条件が本文を Ref）のどちらか。
        // 第26条第1項のただし書きは後者（docs/03-examples/art-26.md）。両方付けると 07 の意味論で矛盾する
        if r.provenance.source.0.ends_with("/sent:2") && r.id.0.contains("proviso") {
            let refers_main = matches!(&r.condition, Expr::Ref(RefTarget::Rule(_)));
            assert!(
                !r.overrides.is_empty() || refers_main,
                "{}: proviso must override or refer to its main sentence",
                r.id.0
            );
            assert!(
                !(!r.overrides.is_empty() && refers_main),
                "{}: proviso must not both override and refer to its main sentence",
                r.id.0
            );
        }
    }
}

#[test]
fn definitions_scope_is_narrower_for_article_local_terms() {
    let model = shakuchi_shakuya::model();
    let global = model
        .definitions
        .iter()
        .find(|d| d.id.0 == "D:借地権者")
        .unwrap();
    let local = model
        .definitions
        .iter()
        .find(|d| d.id.0 == "D:借地権者@art6")
        .unwrap();
    assert_eq!(global.term, local.term);
    assert!(
        local.scope[0].0.starts_with(&global.scope[0].0),
        "local scope is inside the global one"
    );
    assert!(local.scope[0].0.len() > global.scope[0].0.len());
}

#[test]
fn validator_reports_dangling_references() {
    let mut model = shakuchi_shakuya::model();
    model.rules.push(
        build::rule("R-bad")
            .effect(build::exception("R-nonexistent"))
            .provenance(
                "403AC0000000090/main/chap:9/art:999",
                Confidence::Low,
                "test",
            ),
    );
    let issues = validate(&model, &source());
    assert!(issues
        .iter()
        .any(|i| matches!(i, Issue::UnknownRule { .. })));
    assert!(issues
        .iter()
        .any(|i| matches!(i, Issue::UnknownStableId { .. })));
}
