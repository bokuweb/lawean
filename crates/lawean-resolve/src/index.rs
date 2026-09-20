//! Source IR の位置索引。stable_id ↔ （領域, 条, 項, 文）の相互変換と、前後関係の探索。

use lawean_source::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Region {
    Main,
    Suppl(usize),
}

#[derive(Debug, Clone)]
pub struct ParagraphEntry {
    pub stable_id: StableId,
    pub num: u32,
    pub sentences: Vec<StableId>,
    pub items: Vec<(String, StableId)>,
}

#[derive(Debug, Clone)]
pub struct ArticleEntry {
    pub stable_id: StableId,
    pub region: Region,
    /// 附則直下の Paragraph 群のような、条を持たない擬似エントリでは None
    pub num: Option<ArticleNum>,
    pub paragraphs: Vec<ParagraphEntry>,
}

/// 領域ごとに文書順で条を並べた索引
#[derive(Debug, Clone)]
pub struct Index {
    pub articles: Vec<ArticleEntry>,
    by_id: HashMap<StableId, usize>,
    /// 文・項・号 の stable_id → 所属する ArticleEntry の index
    owner: HashMap<StableId, usize>,
}

impl Index {
    pub fn build(doc: &LegalDocument) -> Index {
        let mut articles = Vec::new();
        fn walk(
            ps: &[Provision],
            region: &Region,
            out: &mut Vec<ArticleEntry>,
            loose: &mut Vec<ParagraphEntry>,
        ) {
            for p in ps {
                match p {
                    Provision::Container(c) => walk(&c.children, region, out, loose),
                    Provision::Paragraph(p) => loose.push(para_entry(p)),
                    Provision::Article(a) => out.push(ArticleEntry {
                        stable_id: a.stable_id.clone(),
                        region: region.clone(),
                        num: Some(a.num.clone()),
                        paragraphs: a
                            .children
                            .iter()
                            .filter_map(|c| match c {
                                ArticleChild::Paragraph(p) => Some(para_entry(p)),
                                _ => None,
                            })
                            .collect(),
                    }),
                    Provision::Raw(_) => {}
                }
            }
        }
        let mut main_loose = Vec::new();
        walk(
            &doc.main_provision,
            &Region::Main,
            &mut articles,
            &mut main_loose,
        );
        if !main_loose.is_empty() {
            // 条を持たない政令・省令: 本則全体を 1 つの擬似条として扱う
            articles.push(ArticleEntry {
                stable_id: doc.stable_id.child("main"),
                region: Region::Main,
                num: None,
                paragraphs: main_loose,
            });
        }
        for (i, s) in doc.suppl_provisions.iter().enumerate() {
            let region = Region::Suppl(i);
            let mut loose = Vec::new();
            for c in &s.children {
                match c {
                    SupplChild::Provision(p) => {
                        walk(std::slice::from_ref(p), &region, &mut articles, &mut loose)
                    }
                    SupplChild::Paragraph(p) => loose.push(para_entry(p)),
                    SupplChild::Raw(_) => {}
                }
            }
            if !loose.is_empty() {
                articles.push(ArticleEntry {
                    stable_id: s.stable_id.clone(),
                    region,
                    num: None,
                    paragraphs: loose,
                });
            }
        }
        let mut by_id = HashMap::new();
        let mut owner = HashMap::new();
        for (i, a) in articles.iter().enumerate() {
            by_id.insert(a.stable_id.clone(), i);
            owner.insert(a.stable_id.clone(), i);
            for p in &a.paragraphs {
                owner.insert(p.stable_id.clone(), i);
                for s in &p.sentences {
                    owner.insert(s.clone(), i);
                }
                for (_, it) in &p.items {
                    owner.insert(it.clone(), i);
                }
            }
        }
        Index {
            articles,
            by_id,
            owner,
        }
    }

    /// stable_id（条・項・文・号のどれでも）から所属する条エントリを引く
    pub fn article_of(&self, id: &StableId) -> Option<(usize, &ArticleEntry)> {
        // 文より深い ID（欄など）は親へ遡る
        let mut path = id.0.as_str();
        loop {
            if let Some(i) = self.owner.get(&StableId(path.to_string())) {
                return Some((*i, &self.articles[*i]));
            }
            path = &path[..path.rfind('/')?];
        }
    }

    pub fn by_id(&self, id: &StableId) -> Option<&ArticleEntry> {
        self.by_id.get(id).map(|i| &self.articles[*i])
    }

    /// 同じ領域内で条番号から引く
    pub fn find(&self, region: &Region, num: &ArticleNum) -> Option<(usize, &ArticleEntry)> {
        self.articles
            .iter()
            .enumerate()
            .find(|(_, a)| &a.region == region && a.num.as_ref() == Some(num))
    }

    /// 文書順で n 個前 / 後の条（同じ領域内）
    pub fn neighbor(&self, i: usize, offset: isize) -> Option<(usize, &ArticleEntry)> {
        let j = i.checked_add_signed(offset)?;
        let a = self.articles.get(j)?;
        (a.region == self.articles[i].region && a.num.is_some()).then_some((j, a))
    }
}

fn para_entry(p: &Paragraph) -> ParagraphEntry {
    ParagraphEntry {
        stable_id: p.stable_id.clone(),
        num: p.num.parse().unwrap_or(0),
        sentences: p.sentences.iter().map(|s| s.stable_id.clone()).collect(),
        items: p
            .children
            .iter()
            .filter_map(|c| match c {
                ParagraphChild::Item(i) => {
                    Some((i.num.clone().unwrap_or_default(), i.stable_id.clone()))
                }
                _ => None,
            })
            .collect(),
    }
}

/// stable_id から (項番号, 文番号) を取り出す
pub fn position_in_article(id: &StableId) -> (Option<u32>, Option<u32>) {
    let mut para = None;
    let mut sent = None;
    for seg in id.0.split('/') {
        if let Some(n) = seg.strip_prefix("para:") {
            para = n.parse().ok();
        } else if let Some(n) = seg.strip_prefix("sent:") {
            sent = n.parse().ok();
        }
    }
    (para, sent)
}
