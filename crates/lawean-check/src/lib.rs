//! 改正案の検査 API（ADR-0015 のサービスが呼ぶもの）。
//!
//! 入力は発射台リビジョン（e-Gov XML）と改め文（+ 任意で改正後の e-Gov XML、新旧対照表、他法令）。
//! 出力は検査ごとの pass / fail と、人が読める説明。中身は既存の crate を順に呼ぶだけで、意味論はここに無い:
//!
//! | 検査 | 呼ぶもの | 落ちる例 |
//! |---|---|---|
//! | `Parse` | `lawean-amend::parse_units` | 語彙に無い改め文 |
//! | `Base`（発射台） | `ident::bind` | 「第七項」が無い、「前項」が無い（発射台の取り違え、引用ミス） |
//! | `Order`（施行順序） | `ident::bind` を他の単位の後で再試行、`depends_on` / `schedule_ok` | 第74条を第73条より先に当てる |
//! | `Conflict`（衝突） | `ident::apply_unit` の `conflicts` | 同じ項を 2 つの改正が別々に改める |
//! | `Hane`（ハネ） | `hane_candidates` | 項の繰り下げで「前項」がずれるのに手当てが無い |
//! | `Consolidate` | `ident::apply_unit`、`wf` | id の重複 |
//! | `Expected` | `IdentRevision::render` の比較 | e-Gov の改正後リビジョンと本文が違う |
//! | `Taisho`（新旧対照表） | 本文の突き合わせ | 新旧対照表の「新」欄が溶け込み後の本文と違う（2021 年のデジタル改革関連法案の誤りの型） |
//! | `CrossLaw`（他法令） | `lawean-space::impact` | 他法令からの参照切れ・ずれ |
//!
//! Lean との関係: `Consolidate` / `Conflict` / `Order` は Lean の `Ident.applyUnit` の Rust 写しで判定している。
//! 同じデータを `lawean-lean` が Lean に出し、`lean/Lawean/Cases.lean` が `native_decide` で同じ結論を確かめる（docs/12）。

use lawean_amend::ident::{self, IdentOp, IdentRevision};
use lawean_amend::{hane_candidates, parse_units, AmendUnit};
use lawean_source::{parse_response, ArticleNum, LegalDocument};
use lawean_space::{impact, locate, ImpactKind, LawSpace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Parse,
    Base,
    Order,
    Conflict,
    Hane,
    Consolidate,
    Expected,
    Taisho,
    CrossLaw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Warn,
    /// 入力が無くて検査していない
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub kind: Kind,
    pub status: Status,
    /// 1 行の要約
    pub message: String,
    /// 明細（該当箇所ごと）
    pub details: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitSummary {
    pub label: String,
    pub instructions: usize,
    pub ops: usize,
    /// 束縛した id ベースの操作（人が読む形）
    pub ident_ops: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub ok: bool,
    pub units: Vec<UnitSummary>,
    pub checks: Vec<Check>,
    /// 溶け込み後の本則（条, 本文）。失敗時は途中まで
    pub after: Vec<(String, String)>,
    /// 発射台との差分（変わった・増えた・消えた項）
    pub diff: Vec<String>,
    /// 生成したハネの手当て（改め文の形。番号は改正前で、繰り下げの文より前に置く）。手当てが足りないときだけ
    pub suggested_fixes: Vec<String>,
}

impl Report {
    pub fn failed(&self, k: Kind) -> bool {
        self.checks
            .iter()
            .any(|c| c.kind == k && c.status == Status::Fail)
    }
    pub fn status(&self, k: Kind) -> Option<Status> {
        self.checks.iter().find(|c| c.kind == k).map(|c| c.status)
    }
}

/// 検査の入力
pub struct Input<'a> {
    pub base: &'a LegalDocument,
    /// 改正単位（施行の順）。ラベルは表示用
    pub units: Vec<(String, AmendUnit)>,
    /// e-Gov の改正後リビジョン（あれば本文を突き合わせる）
    pub expected: Option<&'a LegalDocument>,
    /// 新旧対照表（`taisho` の形式）
    pub taisho: Option<&'a str>,
    /// 他法令（波及を見る）。`enforced` は改正の施行日
    pub space: Option<&'a LawSpace>,
    pub enforced: Option<&'a str>,
}

fn check(kind: Kind, status: Status, message: impl Into<String>, details: Vec<String>) -> Check {
    Check {
        kind,
        status,
        message: message.into(),
        details,
    }
}

fn op_line(op: &IdentOp) -> String {
    fn short(s: &str) -> String {
        let t: String = s.chars().take(24).collect();
        if s.chars().count() > 24 {
            format!("{t}…")
        } else {
            t
        }
    }
    fn tail(id: &str) -> &str {
        id.rsplit("/main/").next().unwrap_or(id)
    }
    match op {
        IdentOp::Replace { id, expected, new } => {
            format!(
                "replace {} 「{}」→「{}」",
                tail(id),
                short(expected),
                short(new)
            )
        }
        IdentOp::InsertAfter {
            anchor,
            new_id,
            art,
            text,
        } => format!(
            "insertAfter {} → {} (第{art}条) 「{}」",
            tail(anchor),
            tail(new_id),
            short(text)
        ),
        IdentOp::Delete { id } => format!("delete {}", tail(id)),
        IdentOp::Resolve { id, text } => format!("resolve {} 「{}」", tail(id), short(text)),
    }
}

/// 検査本体
pub fn run(input: &Input<'_>) -> Report {
    let mut checks = Vec::new();
    let mut units_out = Vec::new();
    let base_rev = ident::from_document(input.base);
    let mut cur_doc = input.base.clone();
    // 再パース済みの改正後文書（新旧対照表・他法令の検査で番号から引くため）
    let mut clean_doc = input.base.clone();
    let mut cur_rev = base_rev.clone();
    let mut all_ops: Vec<Vec<IdentOp>> = Vec::new();
    let mut base_fail = Vec::new();
    let mut order_fail = Vec::new();
    let mut conflict_fail = Vec::new();
    let mut hane_fail = Vec::new();
    let mut hane_ok = 0usize;
    let mut suggested: Vec<lawean_amend::Op> = Vec::new();
    let mut stopped = false;

    for (i, (label, unit)) in input.units.iter().enumerate() {
        if stopped {
            units_out.push(UnitSummary {
                label: label.clone(),
                instructions: unit.instructions.len(),
                ops: unit.instructions.iter().map(|x| x.ops.len()).sum(),
                ident_ops: vec![],
            });
            continue;
        }
        // ハネ: この単位を当てる直前の状態で
        for c in hane_candidates(&cur_doc, unit) {
            if c.handled {
                hane_ok += 1;
            } else {
                let expected = match &c.fix {
                    Some(f) => format!("正しい手当ては「{}」→「{f}」", c.text),
                    None => {
                        "参照先が削られるので、参照を消すか別の規定に向ける（人が決める）".into()
                    }
                };
                let found = match &c.found_to {
                    Some(t) => format!("改め文にあるのは「{t}」（番号違い）"),
                    None => "改め文に手当てが無い".into(),
                };
                hane_fail.push(format!(
                    "{label}: {} の「{}」は {} を指すが、改正後は{}。{expected}。{found}",
                    c.sentence
                        .0
                        .rsplit("/main/")
                        .next()
                        .unwrap_or(&c.sentence.0),
                    c.text,
                    c.target.0.rsplit("/main/").next().unwrap_or(&c.target.0),
                    match c.new_target_paragraph {
                        Some(n) => format!("第{n}項になる"),
                        None => "削られる".into(),
                    }
                ));
                if let Some(op) = c.fix_op {
                    suggested.push(op);
                }
            }
        }
        let amend_id = format!("unit{}", i + 1);
        match ident::bind(&cur_doc, unit, &amend_id) {
            Ok(b) => {
                let next = ident::apply_unit(&cur_rev, &b.ops);
                match next {
                    Some(r) if r.has_conflict() => {
                        for (id, text, news) in r.conflicts() {
                            conflict_fail.push(format!(
                                "{label}: {} の本文が期待と違う（今「{}…」、当てようとした「{}…」）",
                                id.rsplit("/main/").next().unwrap_or(id),
                                text.chars().take(20).collect::<String>(),
                                news[0].chars().take(20).collect::<String>()
                            ));
                        }
                        stopped = true;
                    }
                    Some(r) => {
                        cur_rev = r;
                        cur_doc = b.doc;
                        if let Ok(d) = lawean_amend::apply_unit(&clean_doc, unit, "after") {
                            clean_doc = d;
                        }
                    }
                    None => {
                        order_fail.push(format!("{label}: 束縛した操作の対象が発射台に無い"));
                        stopped = true;
                    }
                }
                units_out.push(UnitSummary {
                    label: label.clone(),
                    instructions: unit.instructions.len(),
                    ops: unit.instructions.iter().map(|x| x.ops.len()).sum(),
                    ident_ops: b.ops.iter().map(op_line).collect(),
                });
                all_ops.push(b.ops);
            }
            Err(e) => {
                units_out.push(UnitSummary {
                    label: label.clone(),
                    instructions: unit.instructions.len(),
                    ops: unit.instructions.iter().map(|x| x.ops.len()).sum(),
                    ident_ops: vec![],
                });
                // 分類: 発射台そのものに当たるか、先行する単位の後でだけ当たらないか、後続の単位の後なら当たるか
                if i > 0 {
                    if let Ok(b0) = ident::bind(input.base, unit, &amend_id) {
                        match ident::apply_unit(&cur_rev, &b0.ops) {
                            Some(r) if r.has_conflict() => {
                                for (id, text, news) in r.conflicts() {
                                    conflict_fail.push(format!(
                                        "{label}: 先行する改正と同じ項 {} を別々に改めている（先行後「{}…」、この改正「{}…」）",
                                        id.rsplit("/main/").next().unwrap_or(id),
                                        text.chars().take(20).collect::<String>(),
                                        news[0].chars().take(20).collect::<String>()
                                    ));
                                }
                            }
                            _ => order_fail.push(format!(
                                "{label}: 発射台には当たるが、先行する改正の後では当たらない（{e}）。順序に依存する"
                            )),
                        }
                        stopped = true;
                        continue;
                    }
                }
                // 後続の単位の後なら当たるか（順序の入れ替わり）
                let mut after_others = None;
                for (j, (l2, u2)) in input.units.iter().enumerate() {
                    if j == i {
                        continue;
                    }
                    if let Ok(b2) = ident::bind(input.base, u2, "probe") {
                        if ident::bind(&b2.doc, unit, &amend_id).is_ok() {
                            after_others = Some(l2.clone());
                            break;
                        }
                    }
                }
                match after_others {
                    Some(l2) => order_fail.push(format!(
                        "{label}: この順では当たらない（{e}）が、{l2} の後なら当たる。{l2} に依存する"
                    )),
                    None => base_fail.push(format!("{label}: {e}")),
                }
                stopped = true;
            }
        }
    }

    checks.push(if base_fail.is_empty() {
        check(
            Kind::Base,
            Status::Pass,
            "改め文の指す条・項・字句が発射台にある",
            vec![],
        )
    } else {
        check(
            Kind::Base,
            Status::Fail,
            "発射台に無いものを指している",
            base_fail,
        )
    });
    // 順序と依存
    let mut order_info = Vec::new();
    for i in 0..all_ops.len() {
        for j in 0..i {
            if ident::depends_on(&all_ops[i], &all_ops[j]) {
                order_info.push(format!(
                    "{} は {} が作った項を触る（依存。この順でよい）",
                    units_out[i].label, units_out[j].label
                ));
            } else if ident::independent_units(&all_ops[i], &all_ops[j]) {
                order_info.push(format!(
                    "{} と {} は独立（触る項が交わらない。順序を入れ替えても同じ）",
                    units_out[i].label, units_out[j].label
                ));
            } else {
                order_info.push(format!(
                    "{} と {} は同じ項を触る（順序に意味がある）",
                    units_out[i].label, units_out[j].label
                ));
            }
        }
    }
    let refs: Vec<&[IdentOp]> = all_ops.iter().map(|v| v.as_slice()).collect();
    checks.push(if !order_fail.is_empty() {
        check(
            Kind::Order,
            Status::Fail,
            "施行順序が依存に反する",
            order_fail,
        )
    } else if !ident::schedule_ok(&refs) {
        check(
            Kind::Order,
            Status::Fail,
            "先に施行される単位が後の単位に依存している",
            order_info,
        )
    } else if input.units.len() > 1 {
        check(
            Kind::Order,
            Status::Pass,
            "施行順序は依存の順になっている",
            order_info,
        )
    } else {
        check(Kind::Order, Status::Skip, "改正単位が 1 つ", vec![])
    });
    checks.push(if conflict_fail.is_empty() {
        check(
            Kind::Conflict,
            Status::Pass,
            "同じ項を別々に改める衝突は無い",
            vec![],
        )
    } else {
        check(Kind::Conflict, Status::Fail, "衝突がある", conflict_fail)
    });
    checks.push(if hane_fail.is_empty() {
        check(
            Kind::Hane,
            Status::Pass,
            format!("ハネ改正の手当て漏れは無い（候補 {hane_ok} 件はすべて手当て済み）"),
            vec![],
        )
    } else {
        check(
            Kind::Hane,
            Status::Fail,
            "ハネ改正の手当てが無い、または番号が違う参照がある",
            hane_fail,
        )
    });
    checks.push(if stopped {
        check(
            Kind::Consolidate,
            Status::Skip,
            "溶け込みは途中で止まった",
            vec![],
        )
    } else if !cur_rev.wf() {
        check(Kind::Consolidate, Status::Fail, "id が重複している", vec![])
    } else {
        check(
            Kind::Consolidate,
            Status::Pass,
            format!("溶け込んだ（本則 {} 項）", cur_rev.nodes.len()),
            vec![],
        )
    });

    // 期待するリビジョンとの一致
    checks.push(match input.expected {
        None => check(
            Kind::Expected,
            Status::Skip,
            "改正後のリビジョンが無い",
            vec![],
        ),
        Some(_) if stopped => check(Kind::Expected, Status::Skip, "溶け込みが止まった", vec![]),
        Some(exp) => {
            let d = render_diff(&cur_rev, &ident::from_document(exp));
            if d.is_empty() {
                check(
                    Kind::Expected,
                    Status::Pass,
                    "e-Gov の改正後リビジョンと本則が一致する",
                    vec![],
                )
            } else {
                check(
                    Kind::Expected,
                    Status::Fail,
                    "e-Gov の改正後リビジョンと違う",
                    d,
                )
            }
        }
    });

    // 新旧対照表
    checks.push(match input.taisho {
        None => check(Kind::Taisho, Status::Skip, "新旧対照表が無い", vec![]),
        Some(_) if stopped => check(Kind::Taisho, Status::Skip, "溶け込みが止まった", vec![]),
        Some(t) => {
            let d = check_taisho(t, input.base, &clean_doc);
            if d.is_empty() {
                check(
                    Kind::Taisho,
                    Status::Pass,
                    "新旧対照表は改正前後の本文と一致する",
                    vec![],
                )
            } else {
                check(Kind::Taisho, Status::Fail, "新旧対照表が本文と食い違う", d)
            }
        }
    });

    // 他法令
    checks.push(match (input.space, input.enforced) {
        (Some(space), Some(day)) if !stopped => {
            let target = input.base.law_id.clone().unwrap_or_default();
            let mut fails = Vec::new();
            let mut warns = Vec::new();
            for (label, unit) in &input.units {
                match impact(space, &target, unit, day) {
                    Ok(imps) => {
                        for i in imps {
                            let r = &i.reference;
                            // 「同項」「同条」は先行詞に追随するので、先行詞の側の報告で足りる
                            if r.text.starts_with('同') {
                                continue;
                            }
                            let where_ = format!(
                                "{} の {}「{}」",
                                r.from_law,
                                r.sentence
                                    .0
                                    .rsplit("/main/")
                                    .next()
                                    .unwrap_or(&r.sentence.0),
                                r.text
                            );
                            match i.kind {
                                ImpactKind::Dangling => {
                                    fails.push(format!("{label}: {where_} が参照切れになる"))
                                }
                                ImpactKind::Shifted { moved_to, .. } => {
                                    let new_ref = render_ref_of(&moved_to.0);
                                    fails.push(format!(
                                        "{label}: {where_} の指す先が {} に動くのに参照は旧番号のまま。手当て: 「{}」→「{new_ref}」",
                                        moved_to.0.rsplit("/main/").next().unwrap_or(&moved_to.0),
                                        r.text
                                    ))
                                }
                                ImpactKind::SemanticChange { .. } => {
                                    warns.push(format!("{label}: {where_} の参照先の本文が変わる"))
                                }
                                ImpactKind::TimingGap { from, to, .. } => warns.push(format!(
                                    "{label}: {where_} は {from}〜{to} の間、改正後と違う意味になる"
                                )),
                            }
                        }
                    }
                    Err(e) => fails.push(format!("{label}: 波及の計算に失敗: {e}")),
                }
            }
            if !fails.is_empty() {
                check(Kind::CrossLaw, Status::Fail, "他法令への波及がある", fails)
            } else if !warns.is_empty() {
                check(
                    Kind::CrossLaw,
                    Status::Warn,
                    "他法令の参照の意味が変わる",
                    warns,
                )
            } else {
                check(
                    Kind::CrossLaw,
                    Status::Pass,
                    "他法令への参照切れ・ずれは無い",
                    vec![],
                )
            }
        }
        (Some(_), _) if stopped => {
            check(Kind::CrossLaw, Status::Skip, "溶け込みが止まった", vec![])
        }
        _ => check(Kind::CrossLaw, Status::Skip, "他法令が無い", vec![]),
    });

    let ok = checks.iter().all(|c| c.status != Status::Fail);
    let diff = id_diff(&base_rev, &cur_rev);
    let suggested_fixes = if suggested.is_empty() {
        vec![]
    } else {
        vec![
            lawean_render::render_instruction(&lawean_amend::Instruction {
                text: String::new(),
                ops: suggested,
            })
            .trim()
            .to_string(),
        ]
    };
    Report {
        ok,
        units: units_out,
        checks,
        after: cur_rev.render(),
        diff,
        suggested_fixes,
    }
}

/// stable_id（`…/art:38/para:6`）から参照の字句「第三十八条第六項」を作る（他法令側の手当て）
fn render_ref_of(id: &str) -> String {
    use lawean_resolve::numeral::to_kanji;
    let art = id
        .split("/art:")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .map(|a| {
            let mut parts = a.split('_').filter_map(|x| x.parse::<u32>().ok());
            let mut s = format!("第{}条", to_kanji(parts.next().unwrap_or(0)));
            for b in parts {
                s.push_str(&format!("の{}", to_kanji(b)));
            }
            s
        })
        .unwrap_or_default();
    let para = id
        .split("/para:")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .and_then(|p| p.parse::<u32>().ok())
        .map(|p| format!("第{}項", to_kanji(p)))
        .unwrap_or_default();
    format!("{art}{para}")
}

/// 発射台と溶け込み後の差分。id で突き合わせる（発射台の id は残り、新しい項は改正法の id）
fn id_diff(base: &IdentRevision, after: &IdentRevision) -> Vec<String> {
    let mut out = Vec::new();
    let pos = |r: &IdentRevision, id: &str| -> String {
        match r.para_num(id) {
            Some(n) => {
                let art = &r.nodes.iter().find(|x| x.id == id).unwrap().art;
                if art == ident::TOC_ID {
                    "目次".into()
                } else {
                    format!("第{art}条第{n}項")
                }
            }
            None => id.into(),
        }
    };
    for n in &after.nodes {
        match base.nodes.iter().find(|b| b.id == n.id) {
            Some(b) if b.text == n.text => {}
            Some(b) => out.push(format!(
                "変更 {}（旧{}）: 「{}」→「{}」",
                pos(after, &n.id),
                pos(base, &n.id),
                short(&b.text),
                short(&n.text)
            )),
            None => out.push(format!(
                "追加 {}: 「{}」",
                pos(after, &n.id),
                short(&n.text)
            )),
        }
    }
    for b in &base.nodes {
        if !after.nodes.iter().any(|n| n.id == b.id) {
            out.push(format!("削除 {}: 「{}」", pos(base, &b.id), short(&b.text)));
        }
    }
    out
}

/// 2 つのリビジョンの本文の差分（条ごとに、位置合わせは単純。id が違う e-Gov の版との比較用）
fn render_diff(a: &IdentRevision, b: &IdentRevision) -> Vec<String> {
    let mut out = Vec::new();
    let (ra, rb) = (a.render(), b.render());
    // 条ごとにまとめて比較
    let group = |r: &[(String, String)]| -> Vec<(String, Vec<String>)> {
        let mut g: Vec<(String, Vec<String>)> = Vec::new();
        for (art, text) in r {
            match g.last_mut() {
                Some((a, v)) if a == art => v.push(text.clone()),
                _ => g.push((art.clone(), vec![text.clone()])),
            }
        }
        g
    };
    let (ga, gb) = (group(&ra), group(&rb));
    let arts: Vec<String> = {
        let mut v: Vec<String> = ga.iter().map(|x| x.0.clone()).collect();
        for (art, _) in &gb {
            if !v.contains(art) {
                v.push(art.clone());
            }
        }
        v
    };
    for art in arts {
        let pa = ga.iter().find(|x| x.0 == art).map(|x| &x.1);
        let pb = gb.iter().find(|x| x.0 == art).map(|x| &x.1);
        match (pa, pb) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                for i in 0..x.len().max(y.len()) {
                    let (l, r) = (x.get(i), y.get(i));
                    if l != r {
                        out.push(format!(
                            "第{art}条 #{}: {} → {}",
                            i + 1,
                            l.map(|t| short(t)).unwrap_or_else(|| "（無し）".into()),
                            r.map(|t| short(t)).unwrap_or_else(|| "（無し）".into())
                        ));
                    }
                }
            }
            (Some(_), None) => out.push(format!("第{art}条: 消えた")),
            (None, Some(y)) => out.push(format!("第{art}条: 新設（{} 項）", y.len())),
            (None, None) => {}
        }
    }
    out
}

fn short(t: &str) -> String {
    let s: String = t.chars().take(40).collect();
    if t.chars().count() > 40 {
        format!("{s}…")
    } else {
        s
    }
}

/// 新旧対照表の形式: 1 行 1 項。`新 第三十八条第五項　本文` / `旧 第三十八条第三項　本文`。
/// 「新」は改正後の番号で改正後の本文と、「旧」は改正前の番号で改正前の本文と突き合わせる（空白は無視）
pub fn check_taisho(
    text: &str,
    base_doc: &LegalDocument,
    after_doc: &LegalDocument,
) -> Vec<String> {
    let mut out = Vec::new();
    let base_rev = ident::from_document(base_doc);
    let after_rev = ident::from_document(after_doc);
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (side, rest) = match line.chars().next() {
            Some('新') => ("新", &line['新'.len_utf8()..]),
            Some('旧') => ("旧", &line['旧'.len_utf8()..]),
            _ => {
                out.push(format!("{}行目: 「新」か「旧」で始まらない", n + 1));
                continue;
            }
        };
        let rest = rest.trim_start_matches([' ', '\u{3000}']);
        let Some((pos, body)) = rest.split_once('\u{3000}').or_else(|| rest.split_once(' ')) else {
            out.push(format!("{}行目: 位置と本文の間に空白が無い", n + 1));
            continue;
        };
        let Some((art, para)) = parse_pos(pos) else {
            out.push(format!("{}行目: 位置「{pos}」が読めない", n + 1));
            continue;
        };
        let (doc, rev, name) = if side == "新" {
            (after_doc, &after_rev, "溶け込み後")
        } else {
            (base_doc, &base_rev, "改正前")
        };
        let want: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        let got = locate(doc, &art, Some(para), None).and_then(|id| {
            rev.nodes
                .iter()
                .find(|nd| nd.id == id.0)
                .map(|nd| nd.text.clone())
        });
        match got {
            None => out.push(format!("{}行目: {side}の {pos} が{name}に無い", n + 1)),
            Some(g) if g == want => {}
            Some(g) => out.push(format!(
                "{}行目: {side}の {pos} の本文が違う。表「{}」、{name}「{}」",
                n + 1,
                short(&want),
                short(&g)
            )),
        }
    }
    out
}

/// 「第三十八条第五項」→ (38, 5)。項が無ければ第1項
fn parse_pos(pos: &str) -> Option<(ArticleNum, u32)> {
    let rest = pos.strip_prefix('第')?;
    let (art_k, rest) = rest.split_once('条')?;
    let base = lawean_resolve::numeral::kanji_to_u32(art_k)?;
    let para = if rest.is_empty() {
        1
    } else {
        let p = rest.strip_prefix('第')?.strip_suffix('項')?;
        lawean_resolve::numeral::kanji_to_u32(p)?
    };
    Some((
        ArticleNum::Single {
            base,
            branch: vec![],
        },
        para,
    ))
}

/// 文字列だけで動く版（WASM・playground 用）。`other_laws` は他法令の XML
pub fn run_texts(
    base_xml: &str,
    amendment: &str,
    expected_xml: Option<&str>,
    taisho: Option<&str>,
    other_laws: &[String],
    enforced: Option<&str>,
) -> Report {
    let base = match parse_response(base_xml) {
        Ok(d) => d,
        Err(e) => {
            return Report {
                ok: false,
                units: vec![],
                checks: vec![check(
                    Kind::Parse,
                    Status::Fail,
                    format!("発射台の XML が読めない: {e}"),
                    vec![],
                )],
                after: vec![],
                diff: vec![],
                suggested_fixes: vec![],
            }
        }
    };
    let units = match parse_units(amendment) {
        Ok(u) => u,
        Err(e) => {
            return Report {
                ok: false,
                units: vec![],
                checks: vec![check(
                    Kind::Parse,
                    Status::Fail,
                    format!("改め文が読めない: {e}"),
                    vec![],
                )],
                after: vec![],
                diff: vec![],
                suggested_fixes: vec![],
            }
        }
    };
    let expected = expected_xml.and_then(|x| parse_response(x).ok());
    let mut space = None;
    if !other_laws.is_empty() {
        let mut s = LawSpace::new();
        s.add(base.clone());
        for x in other_laws {
            if let Ok(d) = parse_response(x) {
                s.add(d);
            }
        }
        space = Some(s);
    }
    let labels: Vec<(String, AmendUnit)> = units
        .into_iter()
        .map(|u| (u.article_of_amending_law.clone(), u))
        .collect();
    let mut report = run(&Input {
        base: &base,
        units: labels,
        expected: expected.as_ref(),
        taisho,
        space: space.as_ref(),
        enforced,
    });
    report.checks.insert(
        0,
        check(
            Kind::Parse,
            Status::Pass,
            format!("改め文を {} 単位に読んだ", report.units.len()),
            vec![],
        ),
    );
    report
}
