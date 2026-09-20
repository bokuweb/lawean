//! fixtures/cases/cases.json: 実際の改正（成功）と、実際の改め文から作った失敗例。
//! 成功例はすべての検査を通り、失敗例は指定した検査だけが落ちる。

use lawean_check::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Expect {
    ok: bool,
    fail: Vec<Kind>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    base: String,
    amendment: String,
    expected: Option<String>,
    taisho: Option<String>,
    other_laws: Vec<String>,
    enforced: Option<String>,
    expect: Expect,
}

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn run_case(c: &Case) -> Report {
    let others: Vec<String> = c.other_laws.iter().map(|p| fixture(p)).collect();
    run_texts(
        &fixture(&c.base),
        &fixture(&c.amendment),
        c.expected.as_deref().map(fixture).as_deref(),
        c.taisho.as_deref().map(fixture).as_deref(),
        &others,
        c.enforced.as_deref(),
    )
}

#[test]
fn every_case_has_the_expected_verdict() {
    let cases: Vec<Case> = serde_json::from_str(&fixture("cases/cases.json")).unwrap();
    assert!(cases.len() >= 10);
    for c in &cases {
        let r = run_case(c);
        let failed: Vec<Kind> = r
            .checks
            .iter()
            .filter(|x| x.status == Status::Fail)
            .map(|x| x.kind)
            .collect();
        assert_eq!(
            r.ok,
            c.expect.ok,
            "{}: ok={} failed={failed:?}\n{}",
            c.id,
            r.ok,
            serde_json::to_string_pretty(&r.checks).unwrap()
        );
        assert_eq!(
            failed,
            c.expect.fail,
            "{}: failing checks differ\n{}",
            c.id,
            serde_json::to_string_pretty(&r.checks).unwrap()
        );
    }
}

/// 失敗例の明細が、原因の場所を言えている
#[test]
fn failure_details_name_the_cause() {
    let cases: Vec<Case> = serde_json::from_str(&fixture("cases/cases.json")).unwrap();
    let get = |id: &str| run_case(cases.iter().find(|c| c.id == id).unwrap());
    let detail = |r: &Report, k: Kind| {
        r.checks
            .iter()
            .find(|c| c.kind == k)
            .unwrap()
            .details
            .join("\n")
    };
    let r = get("wrong-base");
    assert!(
        detail(&r, Kind::Base).contains("「前項」"),
        "{}",
        detail(&r, Kind::Base)
    );
    let r = get("r4-48-reversed");
    let d = detail(&r, Kind::Order);
    assert!(d.contains("第七十三条") && d.contains("依存"), "{d}");
    let r = get("hane-missing");
    let d = detail(&r, Kind::Hane);
    assert!(d.contains("art:38/para:3") && d.contains("前項"), "{d}");
    let r = get("wrong-ref");
    assert!(
        detail(&r, Kind::Base).contains("第38条第11項"),
        "{}",
        detail(&r, Kind::Base)
    );
    let r = get("taisho-wrong");
    let d = detail(&r, Kind::Taisho);
    assert!(
        d.contains("第三十八条第五項") && d.contains("第三十九条第三項"),
        "{d}"
    );
    let r = get("conflict-22");
    let d = detail(&r, Kind::Conflict);
    assert!(d.contains("art:22/para:1"), "{d}");
    let r = get("delete-28-dangling");
    let d = detail(&r, Kind::CrossLaw);
    assert!(
        d.contains("413AC0000000026") && d.contains("参照切れ"),
        "{d}"
    );
}
