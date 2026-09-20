//! 実際の改正法（改め文）を改正前のリビジョンに適用し、e-Gov の改正後リビジョンと本則が一致することを確かめる。

use lawean_amend::*;
use lawean_source::*;

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn revision(id: &str) -> LegalDocument {
    parse_response(&fixture(&format!("revisions/{id}.xml"))).unwrap()
}

fn current() -> LegalDocument {
    parse_response(&fixture("403AC0000000090.xml")).unwrap()
}

fn assert_same_main(got: &LegalDocument, want: &LegalDocument) {
    let d = diff_snapshots(&snapshot_main(got), &snapshot_main(want));
    assert!(d.is_empty(), "{} differences:\n{}", d.len(), d.join("\n"));
}

/// 令和3年法律第37号 第35条: 第22条2項・第38条2項/4項・第39条3項の追加、第38条の項の繰り下げと「前項」の手当て
#[test]
fn reiwa3_act37_art35_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/503AC0000000037_art35.txt")).unwrap();
    assert_eq!(units.len(), 1);
    let unit = &units[0];
    assert_eq!(unit.target_title, "借地借家法");
    assert_eq!(unit.instructions.len(), 4);

    let before = revision("403AC0000000090_20210519_503AC0000000037");
    let after = revision("403AC0000000090_20220518_503AC0000000037");
    let got = apply_unit(&before, unit, "403AC0000000090_20220518_503AC0000000037").unwrap();
    assert_same_main(&got, &after);
    assert_eq!(
        got.version_id.as_deref(),
        Some("403AC0000000090_20220518_503AC0000000037")
    );
    // 再パースで stable_id が振り直されている
    assert!(got
        .stable_ids()
        .iter()
        .any(|s| s.0.ends_with("/art:38/para:9/sent:1")));
}

/// 令和4年法律第48号 第73条（2 段目、2023-02-20 施行）: 目次、第42条、第61条の追加
#[test]
fn reiwa4_act48_art73_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    assert_eq!(units.len(), 2);
    let before = revision("403AC0000000090_20220525_504AC0000000048");
    let after = revision("403AC0000000090_20230220_504AC0000000048");
    let got = apply_unit(&before, &units[0], "test").unwrap();
    assert_same_main(&got, &after);
}

/// 同 第74条（3 段目、2026-05-21 施行）: 第61条の全部改正 → 現行
#[test]
fn reiwa4_act48_art74_reproduces_current_law() {
    let units = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let before = revision("403AC0000000090_20230614_505AC0000000053");
    let got = apply_unit(&before, &units[1], "test").unwrap();
    assert_same_main(&got, &current());
}

/// 発射台の不一致: 改正済みの版に同じ改め文を当てると「第七項」が無い、で失敗する
#[test]
fn applying_to_wrong_base_fails_with_missing_target() {
    let units = parse_units(&fixture("amendments/503AC0000000037_art35.txt")).unwrap();
    let already = revision("403AC0000000090_20220518_503AC0000000037");
    let err = apply_unit(&already, &units[0], "x").unwrap_err();
    // 第38条は改正後 9 項あるので第七項は存在する。失敗するのは「前項」の置換（第3項の「前項」は既に「第三項」）
    assert!(matches!(err, ApplyError::PhraseNotFound { .. }), "{err:?}");
}

/// 順序依存: 第74条（第61条の全部改正）を第73条（第61条の追加）より先に当てると対象が無い
#[test]
fn stage_order_matters() {
    let units = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let before = revision("403AC0000000090_20220525_504AC0000000048");
    let err = apply_unit(&before, &units[1], "x").unwrap_err();
    assert_eq!(err, ApplyError::ArticleNotFound("61".into()));
}

/// ハネ: 第38条への項の挿入で、条内の「前項」がずれる。改め文はそれを「第三項」「第一項」に手当てしている。
/// 「前二項」（元も先も 2 項ずつ繰り下がる）や「第一項」（動かない項）は候補にならない
#[test]
fn hane_candidates_for_art38_are_exactly_the_two_handled_ones() {
    let units = parse_units(&fixture("amendments/503AC0000000037_art35.txt")).unwrap();
    let before = revision("403AC0000000090_20210519_503AC0000000037");
    let map = paragraph_mapping(
        &before,
        &units[0],
        &ArticleNum::Single {
            base: 38,
            branch: vec![],
        },
    )
    .unwrap();
    assert_eq!(
        map,
        [(1, 1), (2, 3), (3, 5), (4, 6), (5, 7), (6, 8), (7, 9)]
            .into_iter()
            .collect()
    );

    let cands = hane_candidates(&before, &units[0]);
    for c in &cands {
        eprintln!(
            "{} 「{}」 → {} (new para {:?}) handled={}",
            c.sentence.0.rsplit("art:").next().unwrap(),
            c.text,
            c.target.0.rsplit("art:").next().unwrap(),
            c.new_target_paragraph,
            c.handled
        );
    }
    let summary: Vec<(String, bool)> = cands
        .iter()
        .map(|c| {
            (
                format!("{} {}", c.sentence.0.rsplit("art:").next().unwrap(), c.text),
                c.handled,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("38/para:2/sent:1 前項".to_string(), true),
            ("38/para:3/sent:1 前項".to_string(), true),
        ]
    );
}

/// ハネ漏れの検出: 手当ての Replace を改め文から取り除くと、候補が unhandled になる
#[test]
fn removing_the_fixups_makes_hane_unhandled() {
    let mut units = parse_units(&fixture("amendments/503AC0000000037_art35.txt")).unwrap();
    for ins in &mut units[0].instructions {
        ins.ops.retain(|o| !matches!(o, Op::Replace { .. }));
    }
    let before = revision("403AC0000000090_20210519_503AC0000000037");
    let cands = hane_candidates(&before, &units[0]);
    assert_eq!(cands.len(), 2);
    assert!(cands.iter().all(|c| !c.handled));
}
