//! 手書きの Semantic IR（借地借家法 8 条分）に対する overrides 逆引き・scope 展開・定義語 scope。

use lawean_resolve::ResolvedModel;
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_semantic::*;
use lawean_source::StableId;

fn sid(p: &str) -> StableId {
    StableId(format!("403AC0000000090/{p}"))
}

#[test]
fn exceptions_are_reverse_of_overrides() {
    let model = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&model);
    let ex = |id: &str| {
        rm.exceptions_of(&RuleId(id.into()))
            .iter()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(ex("R3-1"), ["R3-2"]);
    assert_eq!(ex("R4-1"), ["R4-1'", "R4-2"]);
    assert_eq!(ex("R5-1"), ["R5-1-proviso"]);
    // 強行規定 R9 は定期借地権 R22-1a に上書きされる
    assert_eq!(ex("R9"), ["R22-1a"]);
    assert!(ex("R22-1a").is_empty());
}

#[test]
fn override_chain_orders_most_specific_first() {
    let model = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&model);
    let chain = rm.override_chain(&RuleId("R4-1".into()));
    let chain: Vec<&str> = chain.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(chain, ["R4-2", "R4-1'", "R4-1"]);
}

#[test]
fn rules_under_section_expands_kouko_kitei_scope() {
    let model = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&model);
    // 第9条「この節の規定」= 第2章第1節。手書き済みの第3・4・5・6・9条の Rule が入る
    let ids: Vec<&str> = rm
        .rules_under(&sid("main/chap:2/sec:1"))
        .iter()
        .map(|r| r.id.0.as_str())
        .collect();
    assert!(
        ids.contains(&"R3-1")
            && ids.contains(&"R5-1")
            && ids.contains(&"R6")
            && ids.contains(&"R9")
    );
    assert!(!ids.contains(&"R22-1a"), "第4節は含まない");
    assert!(!ids.contains(&"R26-1"), "第3章は含まない");
}

#[test]
fn article_local_definition_wins_inside_its_article() {
    let model = shakuchi_shakuya::model();
    let rm = ResolvedModel::new(&model);
    let in_art6 = sid("main/chap:2/sec:1/art:6/para:1/sent:1");
    let in_art5 = sid("main/chap:2/sec:1/art:5/para:1/sent:1");
    assert_eq!(
        rm.definition_at("借地権者", &in_art6).unwrap().id.0,
        "D:借地権者@art6"
    );
    assert_eq!(
        rm.definition_at("借地権者", &in_art5).unwrap().id.0,
        "D:借地権者"
    );
    // 飛び飛びの scope: 第38条第2項では「電磁的記録」が見えるが第38条第1項では見えない
    assert!(rm
        .definition_at("電磁的記録", &sid("main/chap:3/sec:3/art:38/para:2/sent:1"))
        .is_some());
    assert!(rm
        .definition_at("電磁的記録", &sid("main/chap:3/sec:3/art:38/para:1/sent:1"))
        .is_none());
}
