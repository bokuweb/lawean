//! 公開 API のテスト（test first）: 発射台の XML と改め文から、溶け込み後の e-Gov 形式 XML と検査の報告を得る

use lawean_consolidate::*;

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// 溶け込み: 令3-37 第35条を 2021-05-19 版に当てた XML は、読み直すと e-Gov の 2022-05-18 版と本則が一致する
#[test]
fn consolidate_returns_egov_xml_that_matches_the_next_revision() {
    let base = fixture("revisions/403AC0000000090_20210519_503AC0000000037.xml");
    let amend = fixture("amendments/503AC0000000037_art35.txt");
    let out = consolidate(&base, &amend).unwrap();
    assert!(out.xml.starts_with("<Law "), "{}", &out.xml[..60]);
    assert_eq!(out.units.len(), 1);
    assert!(!out.diff.is_empty());
    // 出した XML をもう一度読んで、e-Gov の改正後リビジョンと突き合わせる
    let got = lawean_source::parse_law_xml(&out.xml).unwrap();
    let want = lawean_source::parse_response(&fixture(
        "revisions/403AC0000000090_20220518_503AC0000000037.xml",
    ))
    .unwrap();
    let d = lawean_amend::diff_snapshots(
        &lawean_amend::snapshot_main(&got),
        &lawean_amend::snapshot_main(&want),
    );
    assert!(d.is_empty(), "{}", d.join("\n"));
}

/// 改め文が発射台に当たらなければ Err（どの単位のどこが、を含む）
#[test]
fn consolidate_fails_with_the_cause_when_the_base_is_wrong() {
    let base = fixture("revisions/403AC0000000090_20220518_503AC0000000037.xml");
    let amend = fixture("amendments/503AC0000000037_art35.txt");
    let err = consolidate(&base, &amend).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("第三十五条") && msg.contains("前項"), "{msg}");
}

/// 検査: 改正後リビジョンと施行日を渡せば、`lawean-check` の報告がそのまま返る
#[test]
fn verify_reports_the_checks() {
    let r = verify(&Input {
        base_xml: &fixture("revisions/403AC0000000090_20210519_503AC0000000037.xml"),
        amendment: &fixture("amendments/failures/503AC0000000037_art35_hane_missing.txt"),
        expected_xml: Some(&fixture(
            "revisions/403AC0000000090_20220518_503AC0000000037.xml",
        )),
        enforced: Some("2022-05-18"),
        ..Default::default()
    });
    assert!(!r.ok);
    assert!(r.failed(Kind::Hane) && r.failed(Kind::Expected));
    assert_eq!(
        r.suggested_fixes,
        ["第三十八条第三項中「前項」を「第三項」に改める。"]
    );
}

/// 溶け込みと検査を一度に。溶け込みが止まれば XML は無い
#[test]
fn consolidate_and_verify_go_together() {
    let (out, r) = consolidate_and_verify(&Input {
        base_xml: &fixture("revisions/403AC0000000090_20210519_503AC0000000037.xml"),
        amendment: &fixture("amendments/failures/503AC0000000037_art35_wrong_ref.txt"),
        ..Default::default()
    });
    assert!(out.is_none());
    assert!(r.failed(Kind::Base));
}
