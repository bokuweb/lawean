//! ハネ改正: 項の挿入・繰り下げ・削除で、指す先がずれる参照を改正前のリビジョンから列挙し、
//! **正しい手当て（置換先の字句）を生成して**、改め文にあるものと突き合わせる（ADR-0012「手当ては探すものではなく生成するもの」）。
//!
//! 判定は改正前後の項番号の対応（`paragraph_mapping`）で行う:
//! - 絶対参照（第N条第M項、第M項）: 指す先の番号が変わるなら候補。手当ては新しい番号の「第N項」
//! - 相対参照（前項、前二項、次項）: 参照元と参照先の距離が変わるなら候補
//!   （第38条の「前二項」のように、元も先も同じだけ繰り下がるものは候補にしない）。
//!   手当ては、新しい距離が 1 なら「前項」「次項」、それ以外は絶対形「第N項」（法制執務の慣行。令3-37 第35条もそう書いている）
//!
//! 同じ規則を Lean の `Lawean.Ident.Refs`（`renderRef`）に写し、「描画が変わるのは参照先の番号か距離が動いたときだけ」（ハネの完全性）を証明している。

use crate::apply::paragraph_mapping;
use crate::op::*;
use lawean_resolve::numeral::to_kanji;
use lawean_resolve::{resolve_sentence_with, Antecedent, Index, RefKind, Resolution};
use lawean_source::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HaneCandidate {
    /// 参照を含む文
    pub sentence: StableId,
    /// 「前項」「第三十八条第二項」
    pub text: String,
    /// 改正前の解決先
    pub target: StableId,
    /// 改正後に指すべき項（削られた項なら None）
    pub new_target_paragraph: Option<u32>,
    /// 条ずれの候補なら、改正後に指すべき条（項の候補では None）
    pub new_target_article: Option<ArticleNum>,
    /// 生成した手当て: 参照の字句をこれに置き換えればよい（削られた項への参照は None = 人が決める）
    pub fix: Option<String>,
    /// 生成した手当てを改め文の操作にしたもの（番号は改正前。繰り下げの文より前に置く）
    pub fix_op: Option<Op>,
    /// 改め文の中に、この参照を改正後の正しい項に向ける Replace があるか
    /// （置換先が生成した手当てと同じか、同値な形: 「第N項」の N か、距離の合う「前N項」「次項」）
    pub handled: bool,
    /// 改め文の中にあった、この参照への置換先（あれば）。handled でなければ「番号違い」
    pub found_to: Option<String>,
}

/// 参照の字句 `text`（「前項」「前二項」「次項」「第三項」「第三十八条第二項」）を、改正後の番号で描き直す。
/// 相対形は新しい距離が元の距離と同じならそのまま、距離 1 なら「前項」「次項」、それ以外は絶対形
pub fn render_fix(
    text: &str,
    kind: &RefKind,
    old_tp: u32,
    new_tp: u32,
    new_sp: Option<u32>,
) -> String {
    let abs = |t: &str, old: u32| {
        t.replacen(
            &format!("第{}項", to_kanji(old)),
            &format!("第{}項", to_kanji(new_tp)),
            1,
        )
    };
    match (kind, new_sp) {
        (RefKind::PrevParagraph(k), Some(ns)) if ns > new_tp => {
            let d = ns - new_tp;
            if d == *k {
                text.to_string()
            } else if d == 1 {
                text.replacen(&format!("前{}項", to_kanji(*k)), "前項", 1)
            } else {
                let old_rel = if *k == 1 {
                    "前項".to_string()
                } else {
                    format!("前{}項", to_kanji(*k))
                };
                text.replacen(&old_rel, &format!("第{}項", to_kanji(new_tp)), 1)
            }
        }
        (RefKind::PrevParagraph(k), _) => {
            let old_rel = if *k == 1 {
                "前項".to_string()
            } else {
                format!("前{}項", to_kanji(*k))
            };
            text.replacen(&old_rel, &format!("第{}項", to_kanji(new_tp)), 1)
        }
        (RefKind::NextParagraph, Some(ns)) if new_tp == ns + 1 => text.to_string(),
        (RefKind::NextParagraph, _) => {
            text.replacen("次項", &format!("第{}項", to_kanji(new_tp)), 1)
        }
        _ => abs(text, old_tp),
    }
}

/// 置換先 `to` が改正後の項 `new_tp` を正しく指すか。「第N項」か、参照元（改正後 `new_sp`）からの距離が合う相対形
fn fixes_to(to: &str, new_tp: Option<u32>, new_sp: Option<u32>) -> bool {
    let Some(nt) = new_tp else {
        // 削られた項への参照は、参照そのものを消すか別の規定に向ける。ここでは判定しない
        return true;
    };
    if to.contains(&format!("第{}項", to_kanji(nt))) {
        return true;
    }
    match new_sp {
        Some(ns) if ns > nt => {
            let d = ns - nt;
            let rel = if d == 1 {
                "前項".to_string()
            } else {
                format!("前{}項", to_kanji(d))
            };
            to == rel || to.ends_with(&rel)
        }
        Some(ns) if nt == ns + 1 => to == "次項" || to.ends_with("次項"),
        _ => false,
    }
}

fn para_of(id: &str) -> Option<u32> {
    id.split("/para:").nth(1)?.split('/').next()?.parse().ok()
}

/// 改正単位が項の番号を動かす条について、影響を受ける参照を列挙する
pub fn hane_candidates(doc: &LegalDocument, unit: &AmendUnit) -> Vec<HaneCandidate> {
    let mut articles: BTreeSet<ArticleNum> = BTreeSet::new();
    let mut replaced: Vec<(Loc, String, String)> = Vec::new();
    for ins in &unit.instructions {
        for op in &ins.ops {
            match op {
                Op::ShiftParagraphs { article, .. }
                | Op::RenumberParagraph { article, .. }
                | Op::InsertParagraphAfter { article, .. } => {
                    articles.insert(article.clone());
                }
                Op::Delete {
                    at:
                        Loc {
                            article,
                            paragraph: Some(_),
                            ..
                        },
                } => {
                    articles.insert(article.clone());
                }
                Op::Replace { at, from, to } => {
                    replaced.push((at.clone(), from.clone(), to.clone()))
                }
                _ => {}
            }
        }
    }
    // 条 → (旧項番号 → 新項番号)。番号が変わらない条は対象外
    let mappings: BTreeMap<String, BTreeMap<u32, u32>> = articles
        .iter()
        .filter_map(|a| {
            paragraph_mapping(doc, unit, a)
                .ok()
                .map(|m| (a.to_num_string(), m))
        })
        .filter(|(_, m)| m.iter().any(|(k, v)| k != v))
        .collect();
    let mut out = article_hane_candidates(doc, unit, &replaced);
    if mappings.is_empty() {
        return out;
    }

    let index = Index::build(doc);
    for g in doc.sentence_groups() {
        let mut ante = Antecedent::default();
        for s in &g.sentences {
            let id = &s.sentence.stable_id;
            let text = s.sentence.plain_text();
            for r in resolve_sentence_with(&index, id, &text, &mut ante) {
                let Resolution::Internal(ids) = &r.resolution else {
                    continue;
                };
                // 「同項」「同条」は直前の参照（先行詞）に追随する。先行詞の側で判定するので、ここでは扱わない
                // 「前各項」は挿入があっても「前の全部」のままなので手当て不要
                if matches!(
                    r.span.parsed.kind,
                    RefKind::SameParagraph | RefKind::SameArticle | RefKind::AllPrevParagraphs
                ) {
                    continue;
                }
                let relative = matches!(
                    r.span.parsed.kind,
                    RefKind::PrevParagraph(_) | RefKind::NextParagraph | RefKind::AllPrevParagraphs
                );
                for t in ids {
                    for (art, map) in &mappings {
                        let seg = format!("/art:{art}/");
                        if !t.0.contains(&seg) {
                            continue;
                        }
                        let Some(tp) = para_of(&t.0) else { continue };
                        let new_tp = map.get(&tp).copied();
                        let affected = if relative {
                            // 参照元も同じ条の項。距離が保たれるなら影響なし
                            let Some(sp) = para_of(&id.0).filter(|_| id.0.contains(&seg)) else {
                                continue;
                            };
                            match (map.get(&sp), new_tp) {
                                (Some(ns), Some(nt)) => {
                                    (*ns as i64 - nt as i64) != (sp as i64 - tp as i64)
                                }
                                _ => true,
                            }
                        } else {
                            new_tp != Some(tp)
                        };
                        if !affected {
                            continue;
                        }
                        let sp = para_of(&id.0);
                        // 参照元の新しい項番号（同じ条なら）。相対形の手当てを認めるのに使う
                        let new_sp = sp
                            .filter(|_| id.0.contains(&seg))
                            .and_then(|s| map.get(&s).copied());
                        // 手当ては参照元（この文）の条・項にある。参照先の条ではない
                        let src_art =
                            id.0.split("/art:")
                                .nth(1)
                                .and_then(|x| x.split('/').next())
                                .unwrap_or("")
                                .to_string();
                        // この文の条・項に対する置換のうち、参照の字句そのもの（番号まで見る）か、
                        // 参照を含む字句ごと書き換える・削るもの（「前項の規定により定められた」を削り = 参照ごと消える）
                        let at_here = |loc: &Loc| {
                            loc.article.to_num_string() == src_art
                                && match &loc.paragraph {
                                    Some(ParaRef::Num(n)) => sp == Some(*n),
                                    None => true,
                                }
                        };
                        let found: Vec<&String> = replaced
                            .iter()
                            .filter(|(loc, from_text, _)| at_here(loc) && from_text == &r.span.text)
                            .map(|(_, _, to)| to)
                            .collect();
                        let rewritten = replaced.iter().any(|(loc, from_text, _)| {
                            at_here(loc)
                                && from_text != &r.span.text
                                && from_text.contains(&r.span.text)
                        });
                        let handled =
                            rewritten || found.iter().any(|to| fixes_to(to, new_tp, new_sp));
                        let fix = new_tp.map(|nt| {
                            render_fix(&r.span.text, &r.span.parsed.kind, tp, nt, new_sp)
                        });
                        let fix_op = match (&fix, sp) {
                            (Some(f), Some(p)) => Some(Op::Replace {
                                at: Loc {
                                    article: ArticleNum::parse(&src_art),
                                    paragraph: Some(ParaRef::Num(p)),
                                    item: id
                                        .0
                                        .split("/item:")
                                        .nth(1)
                                        .and_then(|x| x.split('/').next())
                                        .map(String::from),
                                },
                                from: r.span.text.clone(),
                                to: f.clone(),
                            }),
                            _ => None,
                        };
                        out.push(HaneCandidate {
                            sentence: id.clone(),
                            text: r.span.text.clone(),
                            target: t.clone(),
                            new_target_paragraph: new_tp,
                            new_target_article: None,
                            fix,
                            fix_op,
                            handled,
                            found_to: found.first().map(|s| s.to_string()),
                        });
                    }
                }
            }
        }
    }
    out.dedup();
    out
}

/// 改正単位が動かす条の対応（旧 → 新）。「第N条を第M条とする」「第N条から第M条までをK条ずつ繰り下げ」から作る
pub fn article_mapping(doc: &LegalDocument, unit: &AmendUnit) -> BTreeMap<ArticleNum, ArticleNum> {
    let mut map = BTreeMap::new();
    for ins in &unit.instructions {
        for op in &ins.ops {
            match op {
                Op::RenumberArticle { from, to } => {
                    map.insert(from.clone(), to.clone());
                }
                Op::ShiftArticles { from, to, by } => {
                    for n in crate::numbering::article_nums(doc) {
                        if let ArticleNum::Single { base, branch } = &n {
                            if *base >= *from && *base <= *to {
                                let target = ArticleNum::Single {
                                    base: (*base as i32 + by) as u32,
                                    branch: branch.clone(),
                                };
                                map.insert(n.clone(), target);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    map.retain(|k, v| k != v);
    map
}

fn art_of(id: &str) -> Option<ArticleNum> {
    id.split("/art:")
        .nth(1)?
        .split('/')
        .next()
        .map(ArticleNum::parse)
}

/// 条ずれ（第N条を第M条とする）で、指す先の条番号が変わる絶対参照「第N条…」を列挙する。
/// 手当ては「第N条」を「第M条」に置き換えた字句。相対形（前条・次条）は参照元も同じだけ動くのが普通なので、
/// 参照元と参照先の距離が変わるときだけ候補にする
fn article_hane_candidates(
    doc: &LegalDocument,
    unit: &AmendUnit,
    replaced: &[(Loc, String, String)],
) -> Vec<HaneCandidate> {
    let map = article_mapping(doc, unit);
    if map.is_empty() {
        return Vec::new();
    }
    let new_of = |a: &ArticleNum| map.get(a).cloned().unwrap_or_else(|| a.clone());
    let base_of = |a: &ArticleNum| match a {
        ArticleNum::Single { base, .. } => Some(*base as i64),
        _ => None,
    };
    let index = Index::build(doc);
    let mut out = Vec::new();
    for g in doc.sentence_groups() {
        let mut ante = Antecedent::default();
        for s in &g.sentences {
            let id = &s.sentence.stable_id;
            let text = s.sentence.plain_text();
            // 附則（過去の改正法の附則）は改正当時の番号のままにするのが慣行なので、本則だけ見る
            if id.0.contains("/suppl:") {
                continue;
            }
            let Some(src_art) = art_of(&id.0) else {
                continue;
            };
            for r in resolve_sentence_with(&index, id, &text, &mut ante) {
                let Resolution::Internal(ids) = &r.resolution else {
                    continue;
                };
                let relative = match r.span.parsed.kind {
                    RefKind::PrevArticle(_) | RefKind::NextArticle => true,
                    RefKind::Article { suppl: false, .. } => false,
                    _ => continue,
                };
                let Some(t) = ids.first() else { continue };
                let Some(tgt) = art_of(&t.0) else { continue };
                let new_tgt = new_of(&tgt);
                let affected = if relative {
                    let (Some(os), Some(ot), Some(ns), Some(nt)) = (
                        base_of(&src_art),
                        base_of(&tgt),
                        base_of(&new_of(&src_art)),
                        base_of(&new_tgt),
                    ) else {
                        continue;
                    };
                    os - ot != ns - nt
                } else {
                    new_tgt != tgt
                };
                if !affected {
                    continue;
                }
                let sp = para_of(&id.0);
                let old_label = crate::apply::article_label(&tgt);
                let new_label = crate::apply::article_label(&new_tgt);
                let fix = if relative {
                    None
                } else {
                    Some(r.span.text.replacen(&old_label, &new_label, 1))
                };
                let at_here = |loc: &Loc| {
                    loc.article == src_art
                        && match &loc.paragraph {
                            Some(ParaRef::Num(n)) => sp == Some(*n),
                            None => true,
                        }
                };
                let found: Vec<&String> = replaced
                    .iter()
                    .filter(|(loc, from_text, _)| at_here(loc) && from_text == &r.span.text)
                    .map(|(_, _, to)| to)
                    .collect();
                let rewritten = replaced.iter().any(|(loc, from_text, _)| {
                    at_here(loc) && from_text != &r.span.text && from_text.contains(&r.span.text)
                });
                let handled = rewritten
                    || found.iter().any(|to| to.contains(&new_label))
                    // 参照元の条の本文が丸ごと差し替えられる（次のように改める）なら手当て済み
                    || unit.instructions.iter().flat_map(|i| &i.ops).any(|op| {
                        matches!(op, Op::ReplaceArticle { article, .. } if *article == src_art)
                    });
                let fix_op = match (&fix, sp) {
                    (Some(f), Some(p)) => Some(Op::Replace {
                        at: Loc {
                            article: src_art.clone(),
                            paragraph: Some(ParaRef::Num(p)),
                            item: id
                                .0
                                .split("/item:")
                                .nth(1)
                                .and_then(|x| x.split('/').next())
                                .map(String::from),
                        },
                        from: r.span.text.clone(),
                        to: f.clone(),
                    }),
                    _ => None,
                };
                out.push(HaneCandidate {
                    sentence: id.clone(),
                    text: r.span.text.clone(),
                    target: t.clone(),
                    new_target_paragraph: None,
                    new_target_article: Some(new_tgt),
                    fix,
                    fix_op,
                    handled,
                    found_to: found.first().map(|s| s.to_string()),
                });
            }
        }
    }
    out.dedup();
    out
}
