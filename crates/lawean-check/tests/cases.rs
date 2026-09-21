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
        None,
        None,
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
    assert!(
        d.contains("art:38/para:3")
            && d.contains("「前項」→「第三項」")
            && d.contains("手当てが無い"),
        "{d}"
    );
    assert_eq!(
        r.suggested_fixes,
        ["第三十八条第三項中「前項」を「第三項」に改める。"]
    );
    let r = get("hane-wrong-number");
    let d = detail(&r, Kind::Hane);
    assert!(d.contains("改め文にあるのは「第四項」"), "{d}");
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

/// 起草中の改正法: 発射台を選び、改め文と附則を書き、公布予定日と施行日を与えて検査する（playground の draft の経路）。
/// 附則「公布の日から起算して一年を超えない範囲内において政令で定める日」に対し、施行日が上限の内か外か
#[test]
fn drafting_with_own_suppl_provision() {
    let base = fixture("403AC0000000090.xml");
    let amendment = "第一条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。\n　　第三条中「三十年」を「四十年」に改める。\n";
    let suppl = "第一条　この法律は、公布の日から起算して一年を超えない範囲内において政令で定める日から施行する。";
    let ok = run_texts(
        &base,
        amendment,
        None,
        None,
        &[],
        Some("2027-03-31"),
        Some(suppl),
        Some("2026-10-01"),
    );
    assert!(ok.ok, "{:#?}", ok.checks);
    let e = ok
        .checks
        .iter()
        .find(|c| c.kind == Kind::Enforcement)
        .unwrap();
    assert_eq!(e.status, Status::Pass, "{e:?}");
    assert!(e.details[0].contains("2026-10-01〜2027-09-30"), "{e:?}");
    // 上限を過ぎた施行日
    let bad = run_texts(
        &base,
        amendment,
        None,
        None,
        &[],
        Some("2027-10-01"),
        Some(suppl),
        Some("2026-10-01"),
    );
    assert!(!bad.ok);
    assert_eq!(bad.status(Kind::Enforcement), Some(Status::Fail));
    // ただし書きで第三条だけ公布の日: 施行日が公布日ならその単位は範囲内
    let suppl2 = "第一条　この法律は、公布の日から起算して一年を超えない範囲内において政令で定める日から施行する。ただし、第三条の改正規定は、公布の日から施行する。";
    let r = run_texts(
        &base,
        amendment,
        None,
        None,
        &[],
        Some("2026-10-01"),
        Some(suppl2),
        Some("2026-10-01"),
    );
    let e = r
        .checks
        .iter()
        .find(|c| c.kind == Kind::Enforcement)
        .unwrap();
    assert_eq!(e.status, Status::Pass, "{e:?}");
    assert!(e.details[0].contains("被改正法の条で"), "{e:?}");
    // 公布予定日が無ければ検査しない（Skip）
    let r = run_texts(
        &base,
        amendment,
        None,
        None,
        &[],
        Some("2027-03-31"),
        Some(suppl),
        None,
    );
    assert_eq!(r.status(Kind::Enforcement), Some(Status::Skip));
}
