//! 法令をまたぐ参照: 法令 B の文中の「借地借家法第三十八条第四項」を、法令 A の stable_id に解決する。

use crate::space::LawSpace;
use lawean_resolve::index::Region;
use lawean_resolve::{resolve_sentence_with, Antecedent, Index, Resolution};
use lawean_source::{ArticleNum, LegalDocument, StableId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRef {
    pub from_law: String,
    /// 参照を含む文
    pub sentence: StableId,
    /// 「借地借家法第三十八条第四項」の参照部分（「第三十八条第四項」）
    pub text: String,
    pub to_law: String,
    pub article: ArticleNum,
    pub paragraph: Option<u32>,
    pub item: Option<u32>,
    /// 参照先の stable_id（解決できなければ None = 参照切れ）
    pub target: Option<StableId>,
}

/// A の中で (条, 項, 号) を stable_id に解決する
pub fn locate(
    a: &LegalDocument,
    article: &ArticleNum,
    paragraph: Option<u32>,
    item: Option<u32>,
) -> Option<StableId> {
    let index = Index::build(a);
    let (_, entry) = index.find(&Region::Main, article)?;
    let Some(p) = paragraph else {
        return Some(entry.stable_id.clone());
    };
    let para = entry.paragraphs.iter().find(|x| x.num == p)?;
    let Some(i) = item else {
        return Some(para.stable_id.clone());
    };
    para.items
        .iter()
        .find(|(n, _)| n.parse::<u32>().ok() == Some(i))
        .map(|(_, id)| id.clone())
}

/// 法令 `from` の中の、法令 `to` への参照をすべて集める（本則・附則）
pub fn cross_refs(space: &LawSpace, from: &str, to: &str) -> Vec<CrossRef> {
    let Some(doc) = space.get(from) else {
        return Vec::new();
    };
    let Some(target_doc) = space.get(to) else {
        return Vec::new();
    };
    let index = Index::build(doc);
    let mut out = Vec::new();
    for g in doc.sentence_groups() {
        let mut ante = Antecedent::default();
        for s in &g.sentences {
            let text = s.sentence.plain_text();
            for r in resolve_sentence_with(&index, &s.sentence.stable_id, &text, &mut ante) {
                let Resolution::External {
                    law,
                    num,
                    paragraph,
                    item,
                    ..
                } = &r.resolution
                else {
                    continue;
                };
                if space.resolve_name(law) != Some(to) {
                    continue;
                }
                out.push(CrossRef {
                    from_law: from.into(),
                    sentence: s.sentence.stable_id.clone(),
                    text: r.span.text.clone(),
                    to_law: to.into(),
                    article: num.clone(),
                    paragraph: *paragraph,
                    item: *item,
                    target: locate(target_doc, num, *paragraph, *item),
                });
            }
        }
    }
    out
}
