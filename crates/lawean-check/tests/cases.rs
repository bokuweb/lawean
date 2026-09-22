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
    #[serde(default)]
    base_draft: Option<String>,
    #[serde(default)]
    other_laws_draft: Vec<String>,
    expect: Expect,
}

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn run_case(c: &Case) -> Report {
    let others: Vec<String> = c.other_laws.iter().map(|p| fixture(p)).collect();
    let others_draft: Vec<String> = c.other_laws_draft.iter().map(|p| fixture(p)).collect();
    run_text_input(&TextInput {
        base_xml: &fixture(&c.base),
        amendment: &fixture(&c.amendment),
        expected_xml: c.expected.as_deref().map(fixture).as_deref(),
        taisho: c.taisho.as_deref().map(fixture).as_deref(),
        other_laws: &others,
        enforced: c.enforced.as_deref(),
        base_draft_xml: c.base_draft.as_deref().map(fixture).as_deref(),
        other_laws_draft: &others_draft,
        ..Default::default()
    })
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
    // 令3-37 附則第63条が令2-62 の改め文に施した 3 つの手当てを、起草時・施行時の発射台の突き合わせから生成する
    let r = get("r2-62-vs-r3-37-stale");
    let d = detail(&r, Kind::Stale);
    assert!(
        d.contains("空振り") && d.contains("「、区分所有法第六十三条第五項」が無い"),
        "{d}"
    );
    assert!(
        d.contains("「第二十八条第五項」→「第二十八条第七項」"),
        "{d}"
    );
    assert!(
        d.contains("「第四項」→「第六項」") && d.contains("「第六項」→「第八項」"),
        "{d}"
    );
    assert!(
        r.suggested_fixes
            .iter()
            .any(|f| f == "第百七十八条中「第二十八条第五項」を「第二十八条第七項」に改める。"),
        "{:?}",
        r.suggested_fixes
    );
    let r = get("r4-68-takken-commutes");
    let d = detail(&r, Kind::Stale);
    assert!(d.contains("独立") && d.contains("applyUnit_comm"), "{d}");
    let r = get("r5-53-art125-future-base");
    assert_eq!(
        r.checks
            .iter()
            .find(|c| c.kind == Kind::Stale)
            .unwrap()
            .status,
        Status::Warn
    );
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
    // ただし書きが改め文に無い条（第九十五条）を挙げている: 単位は本文に落ちて範囲外（Fail）、加えて条番号の齟齬を明細に
    let suppl3 = "第一条　この法律は、令和三年九月一日から施行する。ただし、第九十五条の規定は、公布の日から起算して一年を超えない範囲内において政令で定める日から施行する。";
    let real = fixture("revisions/403AC0000000090_20210519_503AC0000000037.xml");
    let amend35 = fixture("amendments/503AC0000000037_art35.txt");
    let r = run_texts(
        &real,
        &amend35,
        None,
        None,
        &[],
        Some("2022-05-18"),
        Some(suppl3),
        Some("2021-05-19"),
    );
    let e = r
        .checks
        .iter()
        .find(|c| c.kind == Kind::Enforcement)
        .unwrap();
    assert_eq!(e.status, Status::Fail, "{e:?}");
    assert!(
        e.details
            .iter()
            .any(|d| d.contains("第九十五条の規定") && d.contains("改め文のどの単位にも無い")),
        "{e:?}"
    );
    // 正しく第三十五条なら範囲内で、齟齬の明細も出ない
    let suppl4 = suppl3.replace("第九十五条", "第三十五条");
    let r = run_texts(
        &real,
        &amend35,
        None,
        None,
        &[],
        Some("2022-05-18"),
        Some(&suppl4),
        Some("2021-05-19"),
    );
    let e = r
        .checks
        .iter()
        .find(|c| c.kind == Kind::Enforcement)
        .unwrap();
    assert_eq!(e.status, Status::Pass, "{e:?}");
    assert_eq!(e.details.len(), 1);
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

/// 溶け込みから生成した新旧対照表は、そのまま Taisho の検査を通る（転記ミスの余地が無い）。
/// 令和3年法律第37号 第35条: 手で起こした fixtures/taisho と同じ項が挙がる
#[test]
fn generated_taisho_passes_the_taisho_check() {
    let base = fixture("revisions/403AC0000000090_20210519_503AC0000000037.xml");
    let amend = fixture("amendments/503AC0000000037_art35.txt");
    let r = run_texts(&base, &amend, None, None, &[], None, None, None);
    assert!(r.ok);
    let generated = r.taisho_generated.join("\n");
    assert!(
        generated.contains(
            "新 第三十八条第五項\u{3000}建物の賃貸人が第三項の規定による説明をしなかったときは"
        ),
        "{generated}"
    );
    assert!(
        generated.contains(
            "旧 第三十八条第三項\u{3000}建物の賃貸人が前項の規定による説明をしなかったときは"
        ),
        "{generated}"
    );
    assert!(
        generated.contains("新 第二十二条第二項\u{3000}前項前段の特約"),
        "{generated}"
    );
    // 生成した表を新旧対照表として与えると通る
    let r2 = run_texts(&base, &amend, None, Some(&generated), &[], None, None, None);
    assert_eq!(
        r2.status(Kind::Taisho),
        Some(Status::Pass),
        "{:?}",
        r2.checks.iter().find(|c| c.kind == Kind::Taisho)
    );
    // 枝番の条（公職選挙法 令和3年法律第51号: 第百四十二条の四を引く第二百四十四条など）でも生成した表は通る
    let base = fixture("revisions/325AC1000000100_20201212_502AC1000000045.xml");
    let amend = fixture("amendments/503AC0000000051.txt");
    let r3 = run_texts(&base, &amend, None, None, &[], None, None, None);
    let g3 = r3.taisho_generated.join("\n");
    assert!(g3.contains("新 第二百四十四条\u{3000}"), "{g3}");
    let r4 = run_texts(&base, &amend, None, Some(&g3), &[], None, None, None);
    assert_eq!(
        r4.status(Kind::Taisho),
        Some(Status::Pass),
        "{:?}",
        r4.checks.iter().find(|c| c.kind == Kind::Taisho)
    );
    // 手で起こした fixtures/taisho の行はすべて生成にも含まれる
    let hand = fixture("taisho/503AC0000000037_art35.txt");
    for line in hand
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
    {
        let norm: String = line.split_whitespace().collect();
        assert!(
            r.taisho_generated
                .iter()
                .any(|g| g.split_whitespace().collect::<String>() == norm),
            "hand line missing: {line}"
        );
    }
}
