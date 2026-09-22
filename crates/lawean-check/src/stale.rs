//! 起草時と施行時の発射台のずれ（先行改正との競合）。
//!
//! 改正法 X は起草時の法令の状態（`base_draft`）に対して書かれるが、当たるのは施行日の直前の状態（`base`）。
//! その間に別の改正 B が施行されていると、
//! - X の指す字句が消えている（**空振り**）
//! - X の「第N項」が B の挿入で別の項を指す（**誤爆**）
//! - X が加える本文の中の参照「第二十八条第五項」が、B の挿入で動いた項を旧番号で指す
//! - X と B が同じ項を改める（順序で結果が変わる）
//!
//! が起きる。実例: 令和2年法律第62号 第2条（マンション建替え円滑化法）に対し、後から公布された令和3年法律第37号が
//! 先に施行されて（2021-09-01）第28条に 2 項を挿入し第124条第3項を書き換えたので、令3-37 附則第63条が
//! 令2-62 の改め文そのものを改めた（docs/13）。
//!
//! 判定は id で行う: X を両方の発射台に束縛し（`ident::bind`）、B が変えた項（本文が同じ項が起草時の版に無いもの）と
//! X が触る項が交わらなければ**独立** = 順序を入れ替えても同じ結果（Lean `applyUnit_comm`）。交われば調整規定が要る。
//! 加える本文の中の参照は、起草時の版で指す項の本文を施行時の版で探し、番号が違えば手当てを生成する。

use lawean_amend::body::{body_of, changed_refs, Body};
use lawean_amend::ident::{apply_unit as apply_ident, bind, derive_unit, from_document, IdentOp};
use lawean_amend::{para_text, AmendUnit, Op};
use lawean_resolve::numeral::to_kanji;
use lawean_resolve::{find_references, RefKind};
use lawean_source::{ArticleChild, ArticleNum, LegalDocument, Provision};
use lawean_space::LawSpace;
use std::collections::BTreeMap;

/// 条（`ArticleNum::to_num_string`）→ その条の (項番号, 本文, id)
type Paras = BTreeMap<String, Vec<(u32, String, String)>>;

/// 条ごとの (項番号, 本文, id)
fn paragraphs_of(doc: &LegalDocument) -> Paras {
    fn walk(ps: &[Provision], out: &mut Paras) {
        for p in ps {
            match p {
                Provision::Container(c) => walk(&c.children, out),
                Provision::Article(a) => {
                    let mut v = Vec::new();
                    for c in &a.children {
                        if let ArticleChild::Paragraph(p) = c {
                            v.push((
                                p.num.parse().unwrap_or(0),
                                para_text(p),
                                p.stable_id.0.clone(),
                            ));
                        }
                    }
                    out.insert(a.num.to_num_string(), v);
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&doc.main_provision, &mut out);
    out
}

/// 起草時の版と施行時の版の項の対応（同じ条の中で本文が同じ項）と、施行時の版で変わった・新しい項の id
pub struct ParaMap {
    /// (条, 起草時の項番号) → 施行時の項番号
    pub to_enf: BTreeMap<(String, u32), u32>,
    /// 施行時の版にあって、起草時の版に同じ本文の項が無いもの（B が改めた・加えた項）の id
    pub changed: Vec<String>,
    /// 起草時の版にあって、施行時の版に同じ本文の項が無いもの (条, 項番号)
    pub gone: Vec<(String, u32)>,
}

/// 文字 2-gram の Dice 係数（0〜1）。B が字句を改めた項を、本文の近さで対応づけるのに使う
fn similarity(a: &str, b: &str) -> f64 {
    fn grams(s: &str) -> Vec<(char, char)> {
        let cs: Vec<char> = s.chars().collect();
        cs.windows(2).map(|w| (w[0], w[1])).collect()
    }
    let (ga, gb) = (grams(a), grams(b));
    if ga.is_empty() || gb.is_empty() {
        return if a == b { 1.0 } else { 0.0 };
    }
    let mut counts: BTreeMap<(char, char), i64> = BTreeMap::new();
    for g in &ga {
        *counts.entry(*g).or_default() += 1;
    }
    let mut common = 0i64;
    for g in &gb {
        if let Some(c) = counts.get_mut(g) {
            if *c > 0 {
                *c -= 1;
                common += 1;
            }
        }
    }
    2.0 * common as f64 / (ga.len() + gb.len()) as f64
}

pub fn para_map(draft: &LegalDocument, enf: &LegalDocument) -> ParaMap {
    let d = paragraphs_of(draft);
    let e = paragraphs_of(enf);
    let mut to_enf = BTreeMap::new();
    let mut changed = Vec::new();
    let mut gone = Vec::new();
    for (art, eps) in &e {
        let Some(dps) = d.get(art) else {
            changed.extend(eps.iter().map(|(_, _, id)| id.clone()));
            continue;
        };
        // まず本文が同じ項どうし
        let mut used_d: Vec<bool> = vec![false; dps.len()];
        let mut pairs: Vec<(usize, usize)> = Vec::new(); // (draft idx, enf idx)
        for (ei, (_, et, _)) in eps.iter().enumerate() {
            if let Some(di) = dps.iter().position(|(_, t, _)| t == et) {
                if !used_d[di] {
                    used_d[di] = true;
                    pairs.push((di, ei));
                }
            }
        }
        // 残り（B が字句を改めた項）は、同じ条の残りの中で本文が最も近いものに（順序を保って）
        let mut used_e: Vec<bool> = vec![false; eps.len()];
        for (_, ei) in &pairs {
            used_e[*ei] = true;
        }
        for (di, (_, dt, _)) in dps.iter().enumerate() {
            if used_d[di] {
                continue;
            }
            let lo = pairs
                .iter()
                .filter(|(a, _)| *a < di)
                .map(|(_, b)| *b + 1)
                .max()
                .unwrap_or(0);
            let hi = pairs
                .iter()
                .filter(|(a, _)| *a > di)
                .map(|(_, b)| *b)
                .min()
                .unwrap_or(eps.len());
            let best = (lo..hi)
                .filter(|ei| !used_e[*ei])
                .map(|ei| (ei, similarity(dt, &eps[ei].1)))
                .filter(|(_, s)| *s >= 0.6)
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            if let Some((ei, _)) = best {
                used_d[di] = true;
                used_e[ei] = true;
                pairs.push((di, ei));
                // 対応は付くが本文は変わっている
                changed.push(eps[ei].2.clone());
            }
        }
        for (di, ei) in &pairs {
            to_enf.insert((art.clone(), dps[*di].0), eps[*ei].0);
        }
        for (ei, (_, _, eid)) in eps.iter().enumerate() {
            if !used_e[ei] {
                changed.push(eid.clone());
            }
        }
        for (di, (dn, _, _)) in dps.iter().enumerate() {
            if !used_d[di] {
                gone.push((art.clone(), *dn));
            }
        }
    }
    for (art, dps) in &d {
        if !e.contains_key(art) {
            gone.extend(dps.iter().map(|(dn, _, _)| (art.clone(), *dn)));
        }
    }
    ParaMap {
        to_enf,
        changed,
        gone,
    }
}

pub struct StaleOutcome {
    pub fails: Vec<String>,
    pub warns: Vec<String>,
    pub infos: Vec<String>,
    /// 改め文（の本文）に施すべき手当て
    pub fixes: Vec<String>,
}

fn art_label(n: &str) -> String {
    lawean_amend::article_label(&ArticleNum::parse(n))
}

/// 改正単位 1 つを起草時・施行時の発射台で突き合わせる
pub fn check_unit(
    label: &str,
    unit: &AmendUnit,
    base_draft: &LegalDocument,
    base: &LegalDocument,
    space_draft: Option<&LawSpace>,
    space: Option<&LawSpace>,
) -> StaleOutcome {
    let mut o = StaleOutcome {
        fails: vec![],
        warns: vec![],
        infos: vec![],
        fixes: vec![],
    };
    let draft_ver = base_draft.version_id.clone().unwrap_or_default();
    let enf_ver = base.version_id.clone().unwrap_or_default();
    let map = para_map(base_draft, base);

    // 1. 束縛の比較
    let bd = bind(base_draft, unit, "draft");
    let be = bind(base, unit, "enf");
    match (&bd, &be) {
        (Ok(_), Err(e)) => o.fails.push(format!(
            "{label}: 起草時の発射台（{draft_ver}）には当たるが、施行時の発射台（{enf_ver}）には当たらない（空振り）: {e}。施行前の改正で字句が消えた・番号が動いた。改め文を改める（先行改正の側で「改正法の改正」）か、調整規定が要る"
        )),
        (Err(e), Ok(_)) => o.warns.push(format!(
            "{label}: 起草時の発射台（{draft_ver}）には当たらず（{e}）、施行時の発射台（{enf_ver}）に当たる。先行改正の施行後を前提に書かれている。施行順が逆になれば空振りするので、順序が確定していなければ調整規定が要る"
        )),
        (Err(_), Err(_)) => {}
        (Ok(d), Ok(e)) => {
            // 同じ順番の Replace が指す項の本文が違えば、X の「第N項」が別の項を指している（誤爆）
            let dr: Vec<&IdentOp> = d
                .ops
                .iter()
                .filter(|op| matches!(op, IdentOp::Replace { .. }))
                .collect();
            let er: Vec<&IdentOp> = e
                .ops
                .iter()
                .filter(|op| matches!(op, IdentOp::Replace { .. }))
                .collect();
            for (x, y) in dr.iter().zip(er.iter()) {
                if let (
                    IdentOp::Replace {
                        expected: a, id: ia, ..
                    },
                    IdentOp::Replace {
                        expected: b, id: ib, ..
                    },
                ) = (x, y)
                {
                    if a != b {
                        let same_art = ia.split("/para:").next() == ib.split("/para:").next();
                        let short = |s: &str| s.chars().take(30).collect::<String>();
                        if map.changed.contains(ib) && same_art {
                            o.fails.push(format!(
                                "{label}: {} は施行前の改正が改めた項で、X も同じ項を改める（起草時「{}…」→ 施行時「{}…」）。順序で結果が変わるので、施行時の本文に対して書き直すか調整規定が要る",
                                ib.rsplit("/main/").next().unwrap_or(ib), short(a), short(b)
                            ));
                        } else {
                            o.fails.push(format!(
                                "{label}: 起草時は {} 「{}…」を改めるのに、施行時は {} 「{}…」を改める（別の項を指す）。施行前の改正で項が動いた",
                                ia.rsplit("/main/").next().unwrap_or(ia), short(a),
                                ib.rsplit("/main/").next().unwrap_or(ib), short(b)
                            ));
                        }
                    }
                }
            }
            // 独立性: X が触る id と B が変えた id
            let touched: Vec<&str> = e.ops.iter().flat_map(|op| op.modifies()).collect();
            let anchors: Vec<&str> = e
                .ops
                .iter()
                .filter_map(|op| match op {
                    IdentOp::InsertAfter { anchor, .. } => Some(anchor.as_str()),
                    _ => None,
                })
                .collect();
            let overlap: Vec<&str> = touched
                .iter()
                .copied()
                .filter(|id| map.changed.iter().any(|c| c == id))
                .collect();
            let anchor_overlap: Vec<&str> = anchors
                .iter()
                .copied()
                .filter(|id| map.changed.iter().any(|c| c == id))
                .collect();
            if overlap.is_empty() {
                o.infos.push(format!(
                    "{label}: 施行前の改正（{draft_ver} → {enf_ver}、変わった項 {} 個）と触る項が交わらない = 独立。順序を入れ替えても同じ結果（Lean applyUnit_comm）。調整規定は要らない",
                    map.changed.len()
                ));
            } else if !o.fails.iter().any(|f| f.contains("同じ項を改める")) {
                o.fails.push(format!(
                    "{label}: 施行前の改正が改めた項を X も改める: {}。順序で結果が変わるので調整規定が要る",
                    overlap
                        .iter()
                        .map(|id| id.rsplit("/main/").next().unwrap_or(id).to_string())
                        .collect::<Vec<_>>()
                        .join("、")
                ));
            }
            if !anchor_overlap.is_empty() {
                o.warns.push(format!(
                    "{label}: 挿入位置の項を施行前の改正が改めている: {}（位置は変わらないが本文を確かめる）",
                    anchor_overlap
                        .iter()
                        .map(|id| id.rsplit("/main/").next().unwrap_or(id).to_string())
                        .collect::<Vec<_>>()
                        .join("、")
                ));
            }
        }
    }

    // 2. X が加える本文の中の自法令への参照を id で持ち、先行改正を当てた後で描き直す（Refs.lean の写し）。
    //    描画が変わった参照が手当て。B は改め文が無くても 2 つの版の差から id の操作列にできる（derive_unit）
    let mut id_fixed: Vec<String> = Vec::new(); // id で手当てした参照（下の字面の突き合わせで重複して出さない）
    if let Ok(bx) = &bd {
        let rd = from_document(base_draft);
        if let Some(rx) = apply_ident(&rd, &bx.ops) {
            let b = derive_unit(&rd, &from_document(base), "intervening");
            if let Some(rxb) = apply_ident(&rx, &b) {
                let bodies: Vec<(String, Body)> = bx
                    .ops
                    .iter()
                    .flat_map(|op| op.creates())
                    .filter_map(|id| {
                        let n = rx.nodes.iter().find(|n| n.id == id)?;
                        Some((id.to_string(), body_of(&rx, id, &n.text)))
                    })
                    .collect();
                for (id, before, after) in changed_refs(&rx, &rxb, &bodies) {
                    let art = rx
                        .nodes
                        .iter()
                        .find(|n| n.id == id)
                        .map(|n| lawean_amend::article_label(&ArticleNum::parse(&n.art)))
                        .unwrap_or_default();
                    o.fails.push(format!(
                        "{label}: X が加える{art}の本文の「{before}」は、先行改正を当てた後は「{after}」を指すべき項（参照を id で持って描き直した。Refs.lean renderBody）。手当て: 「{before}」→「{after}」"
                    ));
                    o.fixes
                        .push(format!("{art}中「{before}」を「{after}」に改める。"));
                    id_fixed.push(format!("{art}|{before}"));
                }
            }
        }
    }

    // 3. 他法令への参照と、置換先の中の参照は字面で: 起草時の項の本文を施行時の版で探す。
    //    起草時の版に当たらず施行時の版に当たる（先行改正の施行後を前提に書かれている）なら、加える本文の参照も
    //    施行時の版に対して書かれているので、起草時の版からの対応で直す提案はしない（令6-53 第8条の「第七十八条の三第一項」）
    if bd.is_err() && be.is_ok() {
        o.fixes.dedup();
        return o;
    }
    let external = |law: &str| -> Option<(LegalDocument, LegalDocument)> {
        let id = space?.resolve_name(law)?;
        let e = space?.get(id)?.clone();
        let d = space_draft?.get(id)?.clone();
        Some((d, e))
    };
    let mut ext_maps: BTreeMap<String, (ParaMap, Paras)> = BTreeMap::new();
    let draft_paras = paragraphs_of(base_draft);
    for ins in &unit.instructions {
        for op in &ins.ops {
            let (where_, lines): (String, Vec<String>) = match op {
                Op::Replace { at, to, .. } => (loc_name(at), vec![to.clone()]),
                Op::InsertAfterPhrase { at, text, .. } => (loc_name(at), vec![text.clone()]),
                _ if op.takes_content() => (String::new(), content_of(op)),
                _ => continue,
            };
            let mut cur_art: Option<String> = None;
            for line in &lines {
                // 内容の行なら、条の見出し行から今の条を取る（「第百七十八条　…」）
                if let Some((t, _)) = line.split_once('\u{3000}') {
                    if t.starts_with('第') && t.ends_with('条') {
                        cur_art = Some(t.to_string());
                    }
                }
                let here = if where_.is_empty() {
                    cur_art.clone().unwrap_or_default()
                } else {
                    where_.clone()
                };
                let mut ante: Option<(Option<String>, ArticleNum)> = None; // (法令名, 条)
                let mut prev_end = 0usize;
                for r in find_references(line) {
                    let gap = &line[prev_end.min(r.start)..r.start];
                    prev_end = r.end;
                    let connective = !gap.is_empty()
                        && gap
                            .replace("から", "")
                            .replace("まで", "")
                            .replace("及び", "")
                            .replace("並びに", "")
                            .replace("若しくは", "")
                            .replace("又は", "")
                            .replace(['、', '（', '）'], "")
                            .is_empty();
                    let (law, art, para) = match (&r.parsed.kind, r.parsed.paragraph) {
                        (RefKind::Article { suppl: false, num }, Some(p)) => {
                            ante = Some((None, num.clone()));
                            (None, num.clone(), p)
                        }
                        (RefKind::External { law, num }, Some(p)) => {
                            ante = Some((Some(law.clone()), num.clone()));
                            (Some(law.clone()), num.clone(), p)
                        }
                        (RefKind::Article { suppl: false, num }, None) => {
                            ante = Some((None, num.clone()));
                            continue;
                        }
                        (RefKind::External { law, num }, None) => {
                            ante = Some((Some(law.clone()), num.clone()));
                            continue;
                        }
                        (RefKind::Paragraph(p), _) if connective => match &ante {
                            Some((law, num)) => (law.clone(), num.clone(), *p),
                            None => continue,
                        },
                        _ => {
                            ante = None;
                            continue;
                        }
                    };
                    let art_key = art.to_num_string();
                    // 対応表（自法令か他法令か）
                    let (m, dparas): (&ParaMap, &Paras) = match &law {
                        None => (&map, &draft_paras),
                        Some(l) => {
                            if !ext_maps.contains_key(l) {
                                let Some((d, e)) = external(l) else { continue };
                                ext_maps.insert(l.clone(), (para_map(&d, &e), paragraphs_of(&d)));
                            }
                            let (m, d) = ext_maps.get(l).unwrap();
                            (m, d)
                        }
                    };
                    let Some(dp) = dparas.get(&art_key) else {
                        continue;
                    }; // X 自身が作る条など
                    let Some((_, dtext, _)) = dp.iter().find(|(n, _, _)| *n == para) else {
                        continue;
                    };
                    let law_label = law.clone().unwrap_or_default();
                    if law.is_none()
                        && where_.is_empty()
                        && id_fixed.iter().any(|k| k == &format!("{here}|{}", r.text))
                    {
                        continue;
                    }
                    let text_changed = |np: u32| {
                        let enf_id = m
                            .changed
                            .iter()
                            .any(|id| id.contains(&format!("/art:{art_key}/para:{np}")));
                        if enf_id {
                            "（本文も変わっている）"
                        } else {
                            ""
                        }
                    };
                    match m.to_enf.get(&(art_key.clone(), para)) {
                        Some(&np) if np != para => {
                            let new_text = r.text.replacen(
                                &format!("第{}項", to_kanji(para)),
                                &format!("第{}項", to_kanji(np)),
                                1,
                            );
                            o.fails.push(format!(
                                "{label}: {here}の本文の「{}{}」は起草時の{}{}第{}項「{}…」を指すが、施行時の版ではその項は第{}項{}。手当て: 「{}」→「{new_text}」",
                                law_label, r.text, law_label, art_label(&art_key), para,
                                dtext.chars().take(20).collect::<String>(), np, text_changed(np), r.text
                            ));
                            o.fixes.push(format!(
                                "{here}中「{}」を「{new_text}」に改める。",
                                r.text
                            ));
                        }
                        Some(_) => {}
                        None => o.warns.push(format!(
                            "{label}: {here}の本文の「{}{}」が指す{}{}第{}項は、施行時の版では本文が変わっている（同じ本文の項が無い）。指す先を確かめる",
                            law_label, r.text, law_label, art_label(&art_key), para
                        )),
                    }
                }
            }
        }
    }
    o.fixes.dedup();
    o
}

fn loc_name(at: &lawean_amend::Loc) -> String {
    let mut s = lawean_amend::article_label(&at.article);
    if let Some(lawean_amend::ParaRef::Num(n)) = &at.paragraph {
        s.push_str(&format!("第{}項", to_kanji(*n)));
    }
    s
}

fn content_of(op: &Op) -> Vec<String> {
    match op {
        Op::AppendParagraph { text, .. }
        | Op::InsertParagraphAfter { text, .. }
        | Op::AppendArticle { text, .. }
        | Op::InsertArticleAfter { text, .. }
        | Op::InsertContainersAfter { text, .. }
        | Op::InsertContainersBefore { text, .. }
        | Op::ReplaceArticle { text, .. }
        | Op::ReplaceParagraph { text, .. }
        | Op::ReplaceItem { text, .. }
        | Op::InsertItemAfter { text, .. }
        | Op::InsertItemBefore { text, .. }
        | Op::AppendItem { text, .. }
        | Op::ReplaceItems { text, .. }
        | Op::ReplaceArticles { text, .. }
        | Op::ReplaceContainers { text, .. }
        | Op::AppendSentence { text, .. }
        | Op::ReplaceSentencePart { text, .. } => text.clone(),
        _ => vec![],
    }
}
