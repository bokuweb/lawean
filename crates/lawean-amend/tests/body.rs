//! 参照を id で持つ本文（`body.rs` = Refs.lean の写し）。
//! 1. 往復: 本文を id の参照にして同じリビジョンで描けば元の本文（ケース集の改め文が加える全部の項、160 項・200 参照以上）
//! 2. 実例: 令2-62 第2条が加える第178条・第180条第4項を、先行改正（令3-37）を当てた後で描くと、
//!    e-Gov の 2022-04-01 版（令3-37 附則第63条で改めた後）の本文と一致する

use lawean_amend::body::{body_of, changed_refs, render_body, Body, Piece};
use lawean_amend::ident::{apply_unit, bind, derive_unit, from_document};
use lawean_amend::parse_units;
use lawean_source::parse_response;

fn fixture(rel: &str) -> String {
    let path = format!("{}/../../fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn bodies_render_back_to_the_same_text_on_the_same_revision() {
    let cases: serde_json::Value = serde_json::from_str(&fixture("cases/cases.json")).unwrap();
    let mut refs = 0;
    let mut paras = 0;
    for c in cases.as_array().unwrap() {
        // 起草時の発射台があるケース（令2-62 の抜粋など）はそちらに束縛する
        let base_path = c["base_draft"].as_str().or(c["base"].as_str()).unwrap();
        let base = parse_response(&fixture(base_path)).unwrap();
        let title = base
            .title
            .as_ref()
            .map(|t| lawean_source::inline_text(&t.text))
            .unwrap_or_default();
        let units = parse_units(&fixture(c["amendment"].as_str().unwrap())).unwrap();
        let rd = from_document(&base);
        for u in units.iter().filter(|u| u.target_title == title) {
            let Ok(b) = bind(&base, u, "x") else { continue };
            let rx = apply_unit(&rd, &b.ops).unwrap();
            for id in b.ops.iter().flat_map(|op| op.creates()) {
                let n = rx.nodes.iter().find(|n| n.id == id).unwrap();
                let body: Body = body_of(&rx, id, &n.text);
                refs += body
                    .iter()
                    .filter(|p| matches!(p, Piece::Ref { .. }))
                    .count();
                paras += 1;
                assert_eq!(render_body(&rx, id, &body), n.text, "{}: {id}", c["id"]);
            }
        }
    }
    assert!(paras > 160 && refs > 200, "paras={paras} refs={refs}");
}

#[test]
fn r2_62_added_articles_render_like_egov_after_the_intervening_amendment() {
    let draft = parse_response(&fixture(
        "revisions/414AC0000000078_20200624_502AC0000000062.xml",
    ))
    .unwrap();
    let enf = parse_response(&fixture(
        "revisions/414AC0000000078_20220401_502AC0000000008.xml",
    ))
    .unwrap();
    let after = parse_response(&fixture(
        "revisions/414AC0000000078_20220401_502AC0000000062.xml",
    ))
    .unwrap();
    let units = parse_units(&fixture("amendments/502AC0000000062_art2_excerpt.txt")).unwrap();
    let rd = from_document(&draft);
    let x = bind(&draft, &units[0], "r2-62").unwrap().ops;
    let rx = apply_unit(&rd, &x).unwrap();
    let b = derive_unit(&rd, &from_document(&enf), "r3-37");
    let rxb = apply_unit(&rx, &b).unwrap();
    let bodies: Vec<(String, Body)> = x
        .iter()
        .flat_map(|op| op.creates())
        .map(|id| {
            let n = rx.nodes.iter().find(|n| n.id == id).unwrap();
            (id.to_string(), body_of(&rx, id, &n.text))
        })
        .collect();
    // 描画が変わる参照は令3-37 附則第63条が改めた 3 箇所そのもの
    let changed: Vec<(String, String)> = changed_refs(&rx, &rxb, &bodies)
        .into_iter()
        .map(|(_, a, b)| (a, b))
        .collect();
    assert_eq!(
        changed,
        [
            (
                "第二十八条第五項".to_string(),
                "第二十八条第七項".to_string()
            ),
            ("第四項".to_string(), "第六項".to_string()),
            ("第六項".to_string(), "第八項".to_string()),
        ]
    );
    // 描き直した本文は e-Gov の 2022-04-01 版（令2-62 第2条の施行後）の第178条・第180条第4項と一致する
    let ra = from_document(&after);
    let egov = |art: &str, p: usize| {
        ra.nodes
            .iter()
            .filter(|n| n.art == art)
            .nth(p - 1)
            .map(|n| n.text.clone())
            .unwrap()
    };
    let rendered = |art: &str, p: usize| {
        let (id, body) = bodies
            .iter()
            .filter(|(id, _)| rxb.nodes.iter().any(|n| &n.id == id && n.art == art))
            .nth(p - 1)
            .unwrap();
        render_body(&rxb, id, body)
    };
    assert_eq!(rendered("178", 1), egov("178", 1));
    assert_eq!(rendered("180", 4), egov("180", 4));
}

/// 監査: fixture の全法令の全項（1 万項超・参照 1 万 5 千超）で、参照を id にして同じリビジョンで描き直すと元の本文に戻る
#[test]
fn every_paragraph_of_every_fixture_law_round_trips() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<std::path::PathBuf> = ["revisions", "laws"]
        .iter()
        .flat_map(|d| std::fs::read_dir(format!("{root}/{d}")).unwrap())
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "xml"))
        .collect();
    files.sort();
    let (mut paras, mut refs) = (0, 0);
    for f in files {
        let Ok(doc) = parse_response(&std::fs::read_to_string(&f).unwrap()) else {
            continue;
        };
        let r = from_document(&doc);
        for n in &r.nodes {
            let body = body_of(&r, &n.id, &n.text);
            refs += body
                .iter()
                .filter(|p| matches!(p, Piece::Ref { .. }))
                .count();
            paras += 1;
            assert_eq!(
                render_body(&r, &n.id, &body),
                n.text,
                "{}: {}",
                f.display(),
                n.id
            );
        }
    }
    assert!(paras > 10000 && refs > 15000, "paras={paras} refs={refs}");
}

/// 既存の本文の参照を id で持てば、ハネの手当ても描画の差として出る（`hane.rs` と同じ結果。Lean `haneFixes`）:
/// 令3-37 第35条から手当てを落としたものを当てると、第38条第2項・第3項の「前項」が「第一項」「第三項」に変わる = 実際の改め文の 2 箇所
#[test]
fn body_rendering_regenerates_the_real_hane_fixes_of_r3_37_art35() {
    let base = parse_response(&fixture(
        "revisions/403AC0000000090_20210519_503AC0000000037.xml",
    ))
    .unwrap();
    let units = parse_units(&fixture(
        "amendments/failures/503AC0000000037_art35_hane_missing.txt",
    ))
    .unwrap();
    let x = bind(&base, &units[0], "x").unwrap().ops;
    let rd = from_document(&base);
    let rx = apply_unit(&rd, &x).unwrap();
    let bodies: Vec<(String, Body)> = rd
        .nodes
        .iter()
        .map(|n| (n.id.clone(), body_of(&rd, &n.id, &n.text)))
        .collect();
    let changed: Vec<(String, String, String)> = changed_refs(&rd, &rx, &bodies)
        .into_iter()
        .map(|(id, a, b)| (id.rsplit("/main/").next().unwrap().to_string(), a, b))
        .collect();
    assert_eq!(
        changed,
        [
            (
                "chap:3/sec:3/art:38/para:2".to_string(),
                "前項".to_string(),
                "第一項".to_string()
            ),
            (
                "chap:3/sec:3/art:38/para:3".to_string(),
                "前項".to_string(),
                "第三項".to_string()
            ),
        ]
    );
}

/// 令3-37 第24条（区分所有法）から字句の置換を全部落として当てると、描画の差が実際の手当て 6 箇所と一致する
/// （「前項の」→「同項の」は慣行の層 `render_pieces_styled`。「第九項本文」の「本文」は平文として残る）
#[test]
fn body_rendering_regenerates_the_six_hane_fixes_of_r3_37_art24() {
    use lawean_amend::Op;
    let base = parse_response(&fixture(
        "revisions/337AC0000000069_20210519_503AC0000000037.xml",
    ))
    .unwrap();
    let mut units = parse_units(&fixture("amendments/503AC0000000037_art24.txt")).unwrap();
    for ins in &mut units[0].instructions {
        ins.ops.retain(|o| !matches!(o, Op::Replace { .. }));
    }
    let x = bind(&base, &units[0], "x").unwrap().ops;
    let rd = from_document(&base);
    let rx = apply_unit(&rd, &x).unwrap();
    let bodies: Vec<(String, Body)> = rd
        .nodes
        .iter()
        .map(|n| (n.id.clone(), body_of(&rd, &n.id, &n.text)))
        .collect();
    let changed: Vec<(String, String, String)> = changed_refs(&rd, &rx, &bodies)
        .into_iter()
        .map(|(id, a, b)| (id.rsplit("/art:").next().unwrap().to_string(), a, b))
        .collect();
    let s = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    assert_eq!(
        changed,
        [
            s("61/para:9", "第十三項", "第十五項"),
            s("61/para:11", "前項", "第十一項"),
            s("61/para:11", "前項", "同項"),
            s("61/para:13", "第九項", "第十項"),
            s("63/para:2", "前項", "第一項"),
            s("63/para:4", "第二項", "第三項"),
            s("63/para:6", "第四項", "第五項"),
        ]
    );
}
