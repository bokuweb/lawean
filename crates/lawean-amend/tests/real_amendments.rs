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

    // 助言（1 項だけの条に項を加えたときの「第二十二条」の精密化）は数えない
    let cands: Vec<HaneCandidate> = hane_candidates(&before, &units[0])
        .into_iter()
        .filter(|c| !c.advisory)
        .collect();
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
    // 助言（1 項だけの条に項を加えたときの「第二十二条」の精密化）は数えない
    let cands: Vec<HaneCandidate> = hane_candidates(&before, &units[0])
        .into_iter()
        .filter(|c| !c.advisory)
        .collect();
    assert_eq!(cands.len(), 2);
    assert!(cands.iter().all(|c| !c.handled));
}

/// 令和4年法律第48号の段階施行: 第73条（2023-02-20）→ 第74条（2026-05-21）の順で現行に一致する
#[test]
fn reiwa4_act48_stages_in_enforcement_order_reach_current_law() {
    let units = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let base = revision("403AC0000000090_20220525_504AC0000000048");
    let stages = vec![
        Stage {
            label: "第73条".into(),
            unit: units[0].clone(),
            enforcement: Enforcement::Fixed("2023-02-20".into()),
        },
        Stage {
            label: "第74条".into(),
            unit: units[1].clone(),
            enforcement: Enforcement::Fixed("2026-05-21".into()),
        },
    ];
    let (doc, log) = run_sequence(&base, &stages);
    assert!(log.iter().all(|s| s.outcome.is_ok()), "{log:?}");
    assert_same_main(&doc.unwrap(), &current());
}

/// 施行日が未確定だとしたら: 2 段の順序 2 通りのうち成功するのは 1 通りだけ。
/// 「第74条が先」は第61条が無くて失敗する = 調整規定が必要になるケースを機械的に検出できる
#[test]
fn undetermined_dates_expose_the_order_dependency() {
    let units = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let base = revision("403AC0000000090_20220525_504AC0000000048");
    let stages = vec![
        Stage {
            label: "第73条".into(),
            unit: units[0].clone(),
            enforcement: Enforcement::Undetermined {
                note: "政令で定める日".into(),
            },
        },
        Stage {
            label: "第74条".into(),
            unit: units[1].clone(),
            enforcement: Enforcement::Undetermined {
                note: "政令で定める日".into(),
            },
        },
    ];
    let report = explore(&base, &stages);
    assert_eq!(report.outcomes.len(), 2);
    assert_eq!(report.successes, 1);
    let failed = report.outcomes.iter().find(|o| o.result.is_err()).unwrap();
    assert_eq!(failed.order, ["第74条", "第73条"]);
    assert_eq!(
        failed.result.as_ref().unwrap_err().1,
        ApplyError::ArticleNotFound("61".into())
    );
}

/// 触る条が違う改正単位は順序に依らず合流する（Lean の applyOp_comm の実データ版）:
/// 令和3年法律第37号 第35条（第22・38・39条）と 令和4年法律第48号 第73条（目次・第42・61条）
#[test]
fn units_touching_different_articles_are_confluent() {
    let a35 = parse_units(&fixture("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let a73 = parse_units(&fixture("amendments/504AC0000000048_art73-74.txt"))
        .unwrap()
        .remove(0);
    let base = revision("403AC0000000090_20210519_503AC0000000037");
    let stages = vec![
        Stage {
            label: "令3-35".into(),
            unit: a35,
            enforcement: Enforcement::Undetermined { note: "".into() },
        },
        Stage {
            label: "令4-73".into(),
            unit: a73,
            enforcement: Enforcement::Undetermined { note: "".into() },
        },
    ];
    let report = explore(&base, &stages);
    assert_eq!(report.successes, 2, "{:?}", report.outcomes);
    assert!(report.confluent);
}

/// 手当ての生成: 令3-37 第35条の 2 箇所は、生成した手当てが実際の改め文と同じ字句になる
#[test]
fn generated_hane_fixes_match_the_real_amendment() {
    let units = parse_units(&fixture("amendments/503AC0000000037_art35.txt")).unwrap();
    let before = revision("403AC0000000090_20210519_503AC0000000037");
    // 助言（1 項だけの条に項を加えたときの「第二十二条」の精密化）は数えない
    let cands: Vec<HaneCandidate> = hane_candidates(&before, &units[0])
        .into_iter()
        .filter(|c| !c.advisory)
        .collect();
    let fixes: Vec<(String, String, Option<String>)> = cands
        .iter()
        .map(|c| {
            (
                c.sentence.0.rsplit("/main/").next().unwrap().to_string(),
                c.text.clone(),
                c.fix.clone(),
            )
        })
        .collect();
    assert_eq!(
        fixes,
        [
            (
                "chap:3/sec:3/art:38/para:2/sent:1".to_string(),
                "前項".to_string(),
                Some("第一項".to_string())
            ),
            (
                "chap:3/sec:3/art:38/para:3/sent:1".to_string(),
                "前項".to_string(),
                Some("第三項".to_string())
            ),
        ]
    );
    assert!(
        cands.iter().all(|c| c.handled && c.found_to == c.fix),
        "{cands:#?}"
    );
    // 生成した手当てを改め文の操作にすると、番号は改正前のもの
    let op = cands[1].fix_op.clone().unwrap();
    assert_eq!(
        op,
        Op::Replace {
            at: Loc {
                article: ArticleNum::Single {
                    base: 38,
                    branch: vec![]
                },
                paragraph: Some(ParaRef::Num(3)),
                item: None,
                part: None,
                suppl: false,
            },
            from: "前項".into(),
            to: "第三項".into(),
        }
    );
}

// ---------------------------------------------------------------- 公職選挙法（実際に起きた改正漏れ）

/// 平成30年法律第75号（公職選挙法の一部を改正する法律、参議院の特定枠）は第142条の4に第4項を挿入して第6項を第7項に繰り下げたが、
/// 罰則の第244条第1項第2号の2の「第百四十二条の四第六項」を改めなかった。表示義務違反の罰則が消えた状態が
/// 施行（2018-10-25）から令和3年法律第51号（2021-06-02）まで続いた（衆議院 第204回国会 質問第120号）。
/// 改め文は衆議院「制定法律」からの写し（34 文のうち、入れ子の読替え規定の書き換え 1 文と別表 1 文を除く 32 文）
#[test]
fn h30_act75_koshoku_senkyo_missed_the_penalty_reference() {
    let units = parse_units(&fixture("amendments/430AC0100000075.txt")).unwrap();
    let before = revision("325AC1000000100_20180620_430AC0000000059");
    // 助言（1 項だけの条に項を加えたときの「第二十二条」の精密化）は数えない
    let cands: Vec<HaneCandidate> = hane_candidates(&before, &units[0])
        .into_iter()
        .filter(|c| !c.advisory)
        .collect();
    let unhandled: Vec<&HaneCandidate> = cands.iter().filter(|c| !c.handled).collect();
    // 唯一の未手当てが、実際に見落とされた第244条の参照
    assert_eq!(unhandled.len(), 1, "{cands:#?}");
    let c = unhandled[0];
    assert!(c.sentence.0.ends_with("/art:244/para:1/item:2_2/sent:1"));
    assert_eq!(c.text, "第百四十二条の四第六項");
    assert_eq!(c.new_target_paragraph, Some(7));
    assert_eq!(c.fix.as_deref(), Some("第百四十二条の四第七項"));
    // 手当てされている参照（第243条の「第五項」→「第六項」、削られる字句の中の「前項」等）は候補に挙がるが handled
    assert!(cands.len() > 1);
    // 生成した手当てを改め文にすると、3 年後に成立した令和3年法律第51号の第一文と一字違わず同じ
    let fix = lawean_render::render_instruction(&Instruction {
        text: String::new(),
        ops: vec![c.fix_op.clone().unwrap()],
    });
    let fix_law = fixture("amendments/503AC0000000051.txt");
    assert!(
        fix_law.contains(fix.trim()),
        "generated: {fix}\nreal: {fix_law}"
    );
    // 溶け込みは、除いた 1 文（第86条の3第2項）以外は e-Gov の改正後リビジョンと一致する
    let got = apply_unit(&before, &units[0], "test").unwrap();
    let want = revision("325AC1000000100_20181025_430AC0100000075");
    let d = diff_snapshots(&snapshot_main(&got), &snapshot_main(&want));
    assert_eq!(d.len(), 1, "{}", d.join("\n"));
    assert!(d[0].starts_with("art 86_3"), "{}", d[0]);
}

/// 令和3年法律第51号（同法の誤りを正す改正）は 2020-12-12 版に当てると 2021-06-02 版に一致する（本則 1167 項）
#[test]
fn r3_act51_koshoku_senkyo_fix_reproduces_egov() {
    let units = parse_units(&fixture("amendments/503AC0000000051.txt")).unwrap();
    let before = revision("325AC1000000100_20201212_502AC1000000045");
    let after = revision("325AC1000000100_20210602_503AC0000000051");
    let got = apply_unit(&before, &units[0], "test").unwrap();
    assert_same_main(&got, &after);
}

/// 令和5年法律第53号 第125条: 条ずれ（第47条〜第61条 → 第49条〜第64条）、3 条の新設、見出しの改め、後段の読替え表
#[test]
fn reiwa5_act53_art125_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/505AC0000000053_art125.txt")).unwrap();
    assert_eq!(units.len(), 1);
    let unit = &units[0];
    let ops: Vec<&Op> = unit.instructions.iter().flat_map(|i| &i.ops).collect();
    assert!(ops
        .iter()
        .any(|o| matches!(o, Op::RenumberArticle { from, to }
        if from.to_num_string() == "61" && to.to_num_string() == "64")));
    assert!(ops.iter().any(|o| matches!(
        o,
        Op::ShiftArticles {
            from: 49,
            to: 53,
            by: 3
        }
    )));
    assert!(ops
        .iter()
        .any(|o| matches!(o, Op::ReplaceCaption { from, to, .. }
        if from == "適用除外" && to == "適用関係")));
    assert!(ops.iter().any(|o| matches!(o, Op::SetCaption { text, .. }
        if text == "（非電磁的事件記録の閲覧等）")));
    assert!(ops.iter().any(
        |o| matches!(o, Op::ReplaceSentencePart { part: SentencePart::Back, text, .. }
        if text.len() > 1 && text[0].starts_with("この場合において、次の表"))
    ));
    assert!(ops.iter().any(|o| matches!(o, Op::InsertArticleAfter { after, text }
        if after.to_num_string() == "46" && text.iter().filter(|l| l.starts_with("第四十")).count() == 2)));

    let got = apply_unit(
        &revision("403AC0000000090_20260521_504AC0000000048"),
        unit,
        "test",
    )
    .unwrap();
    assert_same_main(&got, &revision("403AC0000000090_20280613_505AC0000000053"));
    assert!(lawean_amend::numbering::check_document(&got).is_empty());

    // 条ずれのハネ: 本則で番号の変わる条を指す絶対参照 5 件が、改め文の字句改めで全部手当て済み
    let cands = hane_candidates(&current(), unit);
    let arts: Vec<&HaneCandidate> = cands
        .iter()
        .filter(|c| c.new_target_article.is_some())
        .collect();
    assert_eq!(arts.len(), 5, "{arts:#?}");
    assert!(arts.iter().all(|c| c.handled));
    assert!(arts
        .iter()
        .any(|c| c.text == "第五十五条第一項" && c.fix.as_deref() == Some("第五十八条第一項")));
}

/// 令和4年法律第68号（刑法等の一部改正に伴う整備法、拘禁刑）: 5 法令を改正する 1 つの改め文。
/// 位置の列挙（「、」「及び」「第N条から第M条までの規定」「第七号ロ」）と、
/// 「次に掲げる法律の規定中「懲役」を「拘禁刑」に改める」＋号の列挙形（医師法）
#[test]
fn reiwa4_act68_five_laws_reproduce_egov_revisions() {
    let units = parse_units(&fixture("amendments/504AC0000000068_5laws.txt")).unwrap();
    assert_eq!(units.len(), 5);
    let ishi = units.iter().find(|u| u.target_title == "医師法").unwrap();
    assert_eq!(ishi.article_of_amending_law, "第二百二十一条");
    assert!(ishi.instructions[0]
        .ops
        .iter()
        .any(|o| matches!(o, Op::Replace { at, .. }
        if matches!(&at.article, ArticleNum::Range { from, to }
            if from.to_num_string() == "31" && to.to_num_string() == "33"))));
    for (law, before, after) in [
        (
            "古物営業法",
            "324AC0000000108_20240401_505AC0000000063",
            "324AC0000000108_20250601_504AC0000000068",
        ),
        (
            "質屋営業法",
            "325AC0000000158_20240401_505AC0000000063",
            "325AC0000000158_20250601_504AC0000000068",
        ),
        (
            "旅館業法",
            "323AC0000000138_20231213_505AC0000000052",
            "323AC0000000138_20250601_504AC0000000068",
        ),
        (
            "宅地建物取引業法",
            "327AC1000000176_20250401_506AC0000000053",
            "327AC1000000176_20250601_504AC0000000068",
        ),
        (
            "医師法",
            "323AC0000000201_20250401_503AC0000000049",
            "323AC0000000201_20250601_504AC0000000068",
        ),
    ] {
        let unit = units.iter().find(|u| u.target_title == law).unwrap();
        let got = apply_unit(&revision(before), unit, "test").unwrap();
        assert_same_main(&got, &revision(after));
    }
}

/// 1 項だけの条に項を加えると、その条を丸ごと指す「前条」「第N条」は助言の候補になる。
/// 令3-37 第44条（高齢者居住法）は第52条に第2項を加え、第53〜57条の 6 箇所を「前条第一項」「第五十二条第一項」にしている。
/// 第17条にも第2項を加えるが、第75条の「第十五条から第十七条まで」（省令の根拠規定の列挙）はそのまま = 助言止まりでよい例
#[test]
fn appending_a_paragraph_to_a_single_paragraph_article_advises_refinement() {
    let units = parse_units(&fixture("amendments/503AC0000000037_art44.txt")).unwrap();
    let before = parse_response(&fixture(
        "laws/413AC0000000026_20210519_503AC0000000037.xml",
    ))
    .unwrap();
    let cands = hane_candidates(&before, &units[0]);
    let adv: Vec<&HaneCandidate> = cands.iter().filter(|c| c.advisory).collect();
    assert_eq!(adv.len(), 7, "{adv:#?}");
    assert_eq!(adv.iter().filter(|c| c.handled).count(), 6);
    let left = adv.iter().find(|c| !c.handled).unwrap();
    assert!(left.sentence.0.contains("/art:75/"));
    assert_eq!(left.text, "第十七条");
    assert_eq!(left.fix.as_deref(), Some("第十七条第一項"));
    assert!(adv
        .iter()
        .any(|c| c.text == "前条" && c.fix.as_deref() == Some("前条第一項") && c.handled));
}

/// 令和7年法律第47号 第2条: 被災区分所有建物の再建等に関する特別措置法（大部分を削り、区分所有法の新しい章に委ねる）。
/// テストを先に書く（test first）: 通るまで改め文の語彙を足す
#[test]
fn reiwa7_act47_art2_hisai_mansion_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/507AC0000000047_art2.txt")).unwrap();
    assert_eq!(units.len(), 1);
    let got = apply_unit(
        &revision("407AC0000000043_20250530_507AC0000000047"),
        &units[0],
        "test",
    )
    .unwrap();
    assert_same_main(&got, &revision("407AC0000000043_20260401_507AC0000000047"));
    assert!(lawean_amend::numbering::check_document(&got).is_empty());
}

/// 同 第4条（第2号施行日 2025-11-28）→ 第5条（2026-04-01）: マンションの管理の適正化の推進に関する法律。
/// 発射台は令4-68 施行後の 2025-06-01 版。2 段で当てて、各段が e-Gov の版と一致する
#[test]
fn reiwa7_act47_art4_then_art5_kanri_tekiseika_reproduce_egov_revisions() {
    let u4 = parse_units(&fixture("amendments/507AC0000000047_art4.txt")).unwrap();
    let u5 = parse_units(&fixture("amendments/507AC0000000047_art5.txt")).unwrap();
    let mid = apply_unit(
        &revision("412AC1000000149_20250601_504AC0000000068"),
        &u4[0],
        "stage2",
    )
    .unwrap();
    assert_same_main(&mid, &revision("412AC1000000149_20251128_507AC0000000047"));
    let got = apply_unit(&mid, &u5[0], "main").unwrap();
    assert_same_main(&got, &revision("412AC1000000149_20260401_507AC0000000047"));
    assert!(lawean_amend::numbering::check_document(&got).is_empty());
}

/// 同 第6条（第3号施行日、2027-04-01 未施行）: 2026-04-01 版に当てると e-Gov の 2027-04-01 版（未施行リビジョン）になる
#[test]
fn reiwa7_act47_art6_kanri_tekiseika_reproduces_the_unenforced_revision() {
    let u6 = parse_units(&fixture("amendments/507AC0000000047_art6.txt")).unwrap();
    let got = apply_unit(
        &revision("412AC1000000149_20260401_507AC0000000047"),
        &u6[0],
        "stage3",
    )
    .unwrap();
    assert_same_main(&got, &revision("412AC1000000149_20270401_507AC0000000047"));
    assert!(lawean_amend::numbering::check_document(&got).is_empty());
}

/// 同 第7・12・13・14・15・17条: 令7-47 が改める残りの法律（住宅金融支援機構法・耐震改修促進法・密集市街地整備法・
/// 都市再生機構法・長期優良住宅法・所有者不明土地法）。各法令の直前の版に当てて e-Gov の改正後と一致する
#[test]
fn reiwa7_act47_other_six_laws_reproduce_egov_revisions() {
    for (art, before, after) in [
        (
            "7",
            "417AC0000000082_20251001_506AC0000000043",
            "417AC0000000082_20260401_507AC0000000047",
        ),
        (
            "12",
            "407AC0000000123_20250530_507AC0000000047",
            "407AC0000000123_20260401_507AC0000000047",
        ),
        (
            "13",
            "409AC0000000049_20250601_504AC0000000068",
            "409AC0000000049_20260401_507AC0000000047",
        ),
        (
            "14",
            "415AC0000000100_20241108_506AC0000000040",
            "415AC0000000100_20260401_507AC0000000047",
        ),
        (
            "15",
            "420AC0000000087_20250530_507AC0000000047",
            "420AC0000000087_20251128_507AC0000000047",
        ),
        (
            "17",
            "430AC0000000049_20250601_504AC0000000068",
            "430AC0000000049_20260401_507AC0000000047",
        ),
    ] {
        let units = parse_units(&fixture(&format!(
            "amendments/507AC0000000047_art{art}.txt"
        )))
        .unwrap_or_else(|e| panic!("art{art}: {e}"));
        let got = apply_unit(&revision(before), &units[0], "test")
            .unwrap_or_else(|e| panic!("art{art}: {e}"));
        let d = diff_snapshots(&snapshot_main(&got), &snapshot_main(&revision(after)));
        assert!(
            d.is_empty(),
            "art{art}: {} differences:\n{}",
            d.len(),
            d.join("\n")
        );
    }
}

/// 同 第10条: 地方税法の原始附則の条（「附則第十五条の九の三第一項中」、枝番つき）の字句改め。
/// 地方税法の XML は 12MB あるので fixture に入れない（`.gitignore`）。手元に無ければ飛ばす
#[test]
fn reiwa7_act47_art10_chihozei_suppl_article_is_amended() {
    let root = format!("{}/../../fixtures/revisions", env!("CARGO_MANIFEST_DIR"));
    let before = format!("{root}/325AC0000000226_20251121_507AC0000000007.xml");
    let after = format!("{root}/325AC0000000226_20251128_507AC0000000047.xml");
    if !std::path::Path::new(&before).exists() || !std::path::Path::new(&after).exists() {
        eprintln!("skip: 地方税法の XML が無い（e-Gov から取ってくる）");
        return;
    }
    let read = |p: &str| parse_response(&std::fs::read_to_string(p).unwrap()).unwrap();
    let units = parse_units(&fixture("amendments/507AC0000000047_art10.txt")).unwrap();
    let got = apply_unit(&read(&before), &units[0], "test").unwrap();
    let want = read(&after);
    // 本則は変わらない
    assert_same_main(&got, &want);
    // 原始附則第15条の9の3第1項が e-Gov と一致する
    let suppl_text = |d: &LegalDocument| -> String {
        let sp = d
            .suppl_provisions
            .iter()
            .find(|s| s.amend_law_num.is_none())
            .unwrap();
        for c in &sp.children {
            if let SupplChild::Provision(Provision::Article(a)) = c {
                if a.num.to_num_string() == "15_9_3" {
                    return lawean_amend::para_text(match &a.children[0] {
                        ArticleChild::Paragraph(p) => p,
                        _ => panic!(),
                    });
                }
            }
        }
        panic!("附則第十五条の九の三が無い")
    };
    let (g, w) = (suppl_text(&got), suppl_text(&want));
    assert!(g.contains("第五条の二十第一項"), "{g}");
    assert_eq!(g, w);
}

/// 同 第9条: 地方自治法 別表第二の「マンションの建替え等の円滑化に関する法律の項」の字句改め（別表の行）。
/// 地方自治法の XML は 2.4MB あるので fixture に入れない。手元に無ければ飛ばす。
/// 突き合わせは別表の行の本文（本則は e-Gov の 2 つの版の間で他の法律も変えているので見ない）
#[test]
fn reiwa7_act47_art9_chihojichi_appendix_row_is_amended() {
    let root = format!("{}/../../fixtures/revisions", env!("CARGO_MANIFEST_DIR"));
    let before = format!("{root}/322AC0000000067_20251001_507AC0000000022.xml");
    let after = format!("{root}/322AC0000000067_20260401_507AC0000000047.xml");
    if !std::path::Path::new(&before).exists() || !std::path::Path::new(&after).exists() {
        eprintln!("skip: 地方自治法の XML が無い");
        return;
    }
    let read = |p: &str| parse_response(&std::fs::read_to_string(p).unwrap()).unwrap();
    let units = parse_units(&fixture("amendments/507AC0000000047_art9.txt")).unwrap();
    let got = apply_unit(&read(&before), &units[0], "test").unwrap();
    let want = read(&after);
    let row = |d: &LegalDocument, key: &str| -> String {
        for ap in &d.appendices {
            let x = ap.to_xml();
            if let Some(i) = x.find(key) {
                let j = x[..i].rfind("<TableRow").unwrap();
                let k = x[i..].find("</TableRow>").unwrap() + i;
                let t = regex_strip(&x[j..k]);
                return t;
            }
        }
        panic!("{key} の項が無い")
    };
    fn regex_strip(x: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in x.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                c if !in_tag && !c.is_whitespace() => out.push(c),
                _ => {}
            }
        }
        out
    }
    let g = row(
        &got,
        "マンションの再生等の円滑化に関する法律（平成十四年法律第七十八号）",
    );
    let w = row(
        &want,
        "マンションの再生等の円滑化に関する法律（平成十四年法律第七十八号）",
    );
    assert_eq!(g, w);
    assert!(
        g.contains("第九条第六項") && g.contains("第九十七条第一項及び第三項"),
        "{g}"
    );
}

/// 令和5年法律第63号（デジタル社会形成基本法等の一部改正、令5-6-16 公布）第7条（古物営業法）・第10条（質屋営業法）。
/// 施行 2024-04-01。e-Gov は公布日に改正法の附則だけを付けた版（`_20230616_`）を挟むので、その版を発射台にする。
/// 古物営業法: 「の下に…を加える」の中で定義語を足し、同条に一項を加える。質屋営業法: 見出しつきの条の全部改正
#[test]
fn reiwa5_act63_kobutsu_and_shichiya_reproduce_egov_revisions() {
    for (art, before, after) in [
        (
            "7",
            "324AC0000000108_20230616_505AC0000000063",
            "324AC0000000108_20240401_505AC0000000063",
        ),
        (
            "10",
            "325AC0000000158_20230616_505AC0000000063",
            "325AC0000000158_20240401_505AC0000000063",
        ),
    ] {
        let units = parse_units(&fixture(&format!(
            "amendments/505AC0000000063_art{art}.txt"
        )))
        .unwrap_or_else(|e| panic!("art{art}: {e}"));
        assert_eq!(units.len(), 1, "art{art}");
        let got = apply_unit(&revision(before), &units[0], "test")
            .unwrap_or_else(|e| panic!("art{art}: {e}"));
        let d = diff_snapshots(&snapshot_main(&got), &snapshot_main(&revision(after)));
        assert!(
            d.is_empty(),
            "art{art}: {} differences:\n{}",
            d.len(),
            d.join("\n")
        );
    }
}

/// 令和3年法律第44号（第11次地方分権一括法、令3-5-26 公布）第7条（宅地建物取引業法）。施行は公布から 3 年以内の政令日 = 2024-05-25。
/// 起草から施行までの 3 年の間に宅建業法は令3-37・令2-8・令4-61・令4-68・令5-79 で改正されている（先行改正との競合の実例）。
/// 第78条の3の全部改正（見出し・号・第2項つき）と第78条の4の字句改め・削除
#[test]
fn reiwa3_act44_art7_takken_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/503AC0000000044_art7.txt")).unwrap();
    assert_eq!(units.len(), 1);
    let got = apply_unit(
        &revision("327AC1000000176_20240401_505AC0000000079"),
        &units[0],
        "test",
    )
    .unwrap();
    assert_same_main(&got, &revision("327AC1000000176_20240525_503AC0000000044"));
}

/// 令和6年法律第53号（第14次地方分権一括法、令6-6-19 公布）第8条（宅地建物取引業法）。施行 2025-04-01。
/// 号の繰り下げの連鎖（「同項中第四号を第八号とし、第三号を第五号とし、同号の次に次の二号を加える」）、号の範囲の削除、
/// 見出しの改め、第78条の3（令3-44 が 2024-05-25 に全部改正したばかりの条）の字句改め
#[test]
fn reiwa6_act53_art8_takken_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/506AC0000000053_art8.txt")).unwrap();
    assert_eq!(units.len(), 1);
    let got = apply_unit(
        &revision("327AC1000000176_20240619_506AC0000000053"),
        &units[0],
        "test",
    )
    .unwrap();
    assert_same_main(&got, &revision("327AC1000000176_20250401_506AC0000000053"));
}

/// 令和5年法律第52号（旅館業法等の一部改正、令5-6-14 公布、施行 2023-12-13）第1条（旅館業法）。
/// 条の繰り下げ「第三条の四を第三条の五とし、第三条の三を第三条の四とする」の後に第3条の2の字句改めと項の追加、
/// 号の中のイロ、「同条第一号中…に改め、同条に次の一号を加える」
#[test]
fn reiwa5_act52_art1_ryokan_reproduces_egov_revision() {
    let units = parse_units(&fixture("amendments/505AC0000000052_art1.txt")).unwrap();
    assert_eq!(units.len(), 1);
    let got = apply_unit(
        &revision("323AC0000000138_20230713_505AC0000000067"),
        &units[0],
        "test",
    )
    .unwrap();
    assert_same_main(&got, &revision("323AC0000000138_20231213_505AC0000000052"));
}

/// 令和3年法律第49号（医療法等の一部改正、令3-5-28 公布）第5条・第6条（医師法）。医師法の 3 段の施行:
/// 第5条 → 2023-04-01（附則第一条第六号）、第6条のうち「医師法第十六条の十一第一項の改正規定」→ 本文 2024-04-01、
/// 第6条の残り → 2025-04-01（第七号「第六条の規定（医師法第十六条の十一第一項の改正規定を除く。）」）。
/// 1 つの条を号の範囲（被改正法の条で書く「〜の改正規定」）で 2 つの単位に分けて順に当てる
#[test]
fn reiwa3_act49_ishi_stages_split_by_the_enforcement_scope() {
    let u5 = parse_units(&fixture("amendments/503AC0000000049_art5.txt"))
        .unwrap()
        .remove(0);
    let s1 = apply_unit(
        &revision("323AC0000000201_20230101_504AC0000000047"),
        &u5,
        "test",
    )
    .unwrap();
    assert_same_main(&s1, &revision("323AC0000000201_20230401_503AC0000000049"));

    let u6 = parse_units(&fixture("amendments/503AC0000000049_art6.txt"))
        .unwrap()
        .remove(0);
    let locs = parse_scope_locs("医師法第十六条の十一第一項の改正規定").unwrap();
    let (part, rest) = u6.split_by_locs(&locs);
    assert_eq!(part.instructions.len(), 1);
    assert!(part.instructions[0]
        .text
        .starts_with("第十六条の十一第一項中"));
    assert_eq!(rest.instructions.len(), u6.instructions.len() - 1);
    let s2 = apply_unit(
        &revision("323AC0000000201_20230401_503AC0000000049"),
        &part,
        "test",
    )
    .unwrap();
    assert_same_main(&s2, &revision("323AC0000000201_20240401_503AC0000000049"));
    let s3 = apply_unit(&s2, &rest, "test").unwrap();
    assert_same_main(&s3, &revision("323AC0000000201_20250401_503AC0000000049"));
}

/// 同 第7条・第8条（歯科医師法）。第7条（本文 2024-04-01）は「題名の次に次の目次を付する」（目次の無い法律に目次を足す）と
/// 「本則中第三十一条の三を第三十一条の四とし」。第8条（第八号 2026-04-01）は令4-68（2025-06-01）の後の版に当てる
#[test]
fn reiwa3_act49_art7_and_art8_shika_ishi_reproduce_egov_revisions() {
    for (art, before, after) in [
        (
            "7",
            "323AC0000000202_20230101_504AC0000000047",
            "323AC0000000202_20240401_503AC0000000049",
        ),
        (
            "8",
            "323AC0000000202_20250601_504AC0000000068",
            "323AC0000000202_20260401_503AC0000000049",
        ),
    ] {
        let units = parse_units(&fixture(&format!(
            "amendments/503AC0000000049_art{art}.txt"
        )))
        .unwrap_or_else(|e| panic!("art{art}: {e}"));
        let got = apply_unit(&revision(before), &units[0], "test")
            .unwrap_or_else(|e| panic!("art{art}: {e}"));
        let d = diff_snapshots(&snapshot_main(&got), &snapshot_main(&revision(after)));
        assert!(
            d.is_empty(),
            "art{art}: {} differences:\n{}",
            d.len(),
            d.join("\n")
        );
        if art == "7" {
            // 目次が付いた
            assert!(got.toc.is_some(), "art7: 目次が無い");
        }
    }
}
