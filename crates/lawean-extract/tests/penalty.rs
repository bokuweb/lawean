//! 罰則の空振りの検査を、公職選挙法の 3 つの実リビジョンで。
//! 平成30年法律第75号の改正漏れ（2018-10-25〜2021-06-02）は、改め文を見ずに改正後の本文だけで出る

use lawean_extract::penalty::*;
use lawean_source::parse_response;

fn rev(name: &str) -> lawean_source::LegalDocument {
    let p = format!(
        "{}/../../fixtures/revisions/{name}.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn koshoku_penalties_are_extracted() {
    let doc = rev("325AC1000000100_20210602_503AC0000000051");
    let ps = penalties(&doc);
    assert!(ps.len() >= 80, "{}", ps.len());
    let targets: usize = ps.iter().map(|p| p.targets.len()).sum();
    let with_refs = ps
        .iter()
        .flat_map(|p| &p.targets)
        .filter(|t| !t.refs.is_empty())
        .count();
    assert!(
        targets >= 70 && with_refs == targets,
        "targets {targets}, with refs {with_refs}"
    );
    assert!(ps
        .iter()
        .flat_map(|p| &p.targets)
        .all(|t| t.unresolved.is_empty()));
    // 第244条: 一年以下の禁錮又は三十万円以下の罰金
    let p244 = ps
        .iter()
        .find(|p| p.sentence.0.contains("/art:244/para:1/"))
        .unwrap();
    assert_eq!(p244.sanctions.len(), 2);
    assert_eq!(p244.sanctions[0].max, Some(12));
    assert_eq!(p244.sanctions[1].max, Some(300_000));
    let t = p244
        .targets
        .iter()
        .find(|t| t.sentence.0.contains("item:2_2"))
        .unwrap();
    assert_eq!(t.act.as_deref(), Some("同項に規定する事項を表示しなかつた"));
    assert!(
        t.refs.iter().any(|r| r.0.ends_with("/art:142_4/para:7")),
        "{:?}",
        t.refs
    );
}

#[test]
fn the_2018_error_is_the_only_mismatch_and_only_while_it_lasted() {
    assert!(mismatches(&rev("325AC1000000100_20180620_430AC0000000059")).is_empty());
    let ms = mismatches(&rev("325AC1000000100_20181025_430AC0100000075"));
    assert_eq!(ms.len(), 1, "{ms:#?}");
    assert!(ms[0].target.0.contains("/art:244/para:1/item:2_2/"));
    assert!(ms[0].referenced.0.ends_with("/art:142_4/para:6"));
    assert_eq!(
        ms[0].kind,
        MismatchKind::ActNotInTarget {
            word: "表示".into()
        }
    );
    assert!(mismatches(&rev("325AC1000000100_20210602_503AC0000000051")).is_empty());
}
