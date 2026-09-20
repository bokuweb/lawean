//! ハネ改正の候補: 項の挿入・繰り下げ・削除で、指す先がずれる参照を改正前のリビジョンから列挙する。
//!
//! 判定は改正前後の項番号の対応（`paragraph_mapping`）で行う:
//! - 絶対参照（第N条第M項、第M項）: 指す先の番号が変わるなら候補
//! - 相対参照（前項、前二項、次項）: 参照元と参照先の距離が変わるなら候補
//!   （第38条の「前二項」のように、元も先も同じだけ繰り下がるものは候補にしない）

use crate::apply::paragraph_mapping;
use crate::op::*;
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
    /// この改正単位の Replace で手当てされているか
    pub handled: bool,
}

fn para_of(id: &str) -> Option<u32> {
    id.split("/para:").nth(1)?.split('/').next()?.parse().ok()
}

/// 改正単位が項の番号を動かす条について、影響を受ける参照を列挙する
pub fn hane_candidates(doc: &LegalDocument, unit: &AmendUnit) -> Vec<HaneCandidate> {
    let mut articles: BTreeSet<ArticleNum> = BTreeSet::new();
    let mut replaced: Vec<(Loc, String)> = Vec::new();
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
                        },
                } => {
                    articles.insert(article.clone());
                }
                Op::Replace { at, from, .. } => replaced.push((at.clone(), from.clone())),
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
    if mappings.is_empty() {
        return Vec::new();
    }

    let index = Index::build(doc);
    let mut out = Vec::new();
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
                if matches!(
                    r.span.parsed.kind,
                    RefKind::SameParagraph | RefKind::SameArticle
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
                        let handled = replaced.iter().any(|(loc, from_text)| {
                            loc.article.to_num_string() == *art
                                && from_text == &r.span.text
                                && match &loc.paragraph {
                                    Some(ParaRef::Num(n)) => sp == Some(*n),
                                    None => true,
                                }
                        });
                        out.push(HaneCandidate {
                            sentence: id.clone(),
                            text: r.span.text.clone(),
                            target: t.clone(),
                            new_target_paragraph: new_tp,
                            handled,
                        });
                    }
                }
            }
        }
    }
    out.dedup();
    out
}
