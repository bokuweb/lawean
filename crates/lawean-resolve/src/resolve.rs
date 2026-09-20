//! 参照表現の解決。「前項」などを、参照元の位置（stable_id）と先行詞から具体的な stable_id 群に落とす。

use crate::index::{position_in_article, ArticleEntry, Index, ParagraphEntry, Region};
use crate::reference::{find_references, ParsedRef, Part, RefKind, RefSpan};
use lawean_source::{ArticleNum, StableId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// この法令内の構造ノード群
    Internal(Vec<StableId>),
    /// 他法令。v0.1 では法令名と条番号を残すだけ。
    /// `inferred` は「この法令に無い条番号で、文中に他法令への言及がある」ことからの推定
    External {
        law: String,
        num: ArticleNum,
        paragraph: Option<u32>,
        item: Option<u32>,
        inferred: bool,
    },
    Unresolved(Unresolved),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unresolved {
    /// 「同条」「同項」の先行詞が無い
    NoAntecedent,
    /// 参照元の位置が索引に無い
    UnknownOrigin(StableId),
    /// 参照先の条・項・号・文が存在しない
    NotFound(String),
}

/// 「同条」「同項」「同法」のための先行詞。項の中では文をまたいで引き継ぐ
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Antecedent {
    pub article: Option<StableId>,
    pub paragraph: Option<StableId>,
    /// 直前に言及された他法令の条（「同条」「同項」がこれを指すことがある）
    pub external: Option<(String, ArticleNum, Option<u32>)>,
    /// 直前に名前の出た他法令（「同法」の解決先）
    pub last_law: Option<String>,
    /// 直前の参照が他法令だったか（列挙の引き継ぎ判定に使う）
    pub prev_was_external: bool,
}

impl Antecedent {
    fn set_external(&mut self, law: &str, num: &ArticleNum, para: Option<u32>) {
        self.external = Some((law.to_string(), num.clone(), para));
        if law != "同法" {
            self.last_law = Some(law.to_string());
        }
        self.prev_was_external = true;
    }
    fn law_name(&self, law: &str) -> String {
        if law == "同法" {
            self.last_law.clone().unwrap_or_else(|| law.to_string())
        } else {
            law.to_string()
        }
    }
}

pub fn resolve(index: &Index, from: &StableId, r: &ParsedRef, ante: &mut Antecedent) -> Resolution {
    let res = resolve_inner(index, from, r, ante);
    ante.prev_was_external = matches!(res, Resolution::External { .. });
    res
}

fn external(
    ante: &mut Antecedent,
    law: &str,
    num: &ArticleNum,
    r: &ParsedRef,
    inferred: bool,
) -> Resolution {
    let law = ante.law_name(law);
    ante.set_external(&law, num, r.paragraph);
    Resolution::External {
        law,
        num: num.clone(),
        paragraph: r.paragraph,
        item: r.item,
        inferred,
    }
}

fn resolve_inner(
    index: &Index,
    from: &StableId,
    r: &ParsedRef,
    ante: &mut Antecedent,
) -> Resolution {
    let Some((ai, art)) = index.article_of(from) else {
        return Resolution::Unresolved(Unresolved::UnknownOrigin(from.clone()));
    };
    let (cur_para, _) = position_in_article(from);

    let (target_art, target_paras): (&ArticleEntry, Vec<&ParagraphEntry>) = match &r.kind {
        RefKind::External { law, num } => return external(ante, law, num, r, false),
        // 列挙の続き（「第百三十三条の二第五項及び第六項」）は `resolve_sentence_with` が
        // SameArticle / External に書き換えてから渡してくる。ここに来る裸の「第N項」は現在の条の項
        RefKind::Paragraph(n) => (art, art.paragraphs.iter().filter(|p| p.num == *n).collect()),
        RefKind::Item(n) => {
            let Some(cp) = cur_para else {
                return not_found(format!("第{n}号 from {from}: no paragraph"));
            };
            (art, art.paragraphs.iter().filter(|p| p.num == cp).collect())
        }
        RefKind::PrevArticle(n) => {
            let mut v = Vec::new();
            for k in (1..=*n as isize).rev() {
                match index.neighbor(ai, -k) {
                    Some((_, a)) => v.push(a),
                    None => return not_found(format!("前{n}条 from {from}")),
                }
            }
            if *n == 1 {
                let a = v[0];
                let paras = narrow_paras(a, r.paragraph);
                (a, paras)
            } else {
                // 複数条: 条単位で返す
                let ids = v.iter().map(|a| a.stable_id.clone()).collect();
                ante.article = v.last().map(|a| a.stable_id.clone());
                return Resolution::Internal(ids);
            }
        }
        RefKind::NextArticle => match index.neighbor(ai, 1) {
            Some((_, a)) => (a, narrow_paras(a, r.paragraph)),
            None => return not_found(format!("次条 from {from}")),
        },
        RefKind::PrevParagraph(n) => {
            let Some(cp) = cur_para else {
                return not_found(format!("前項 from {from}: no paragraph"));
            };
            if cp <= *n {
                return not_found(format!("前{n}項 from {from}"));
            }
            let paras: Vec<&ParagraphEntry> = art
                .paragraphs
                .iter()
                .filter(|p| p.num >= cp - n && p.num < cp)
                .collect();
            (art, paras)
        }
        RefKind::AllPrevParagraphs => {
            let Some(cp) = cur_para else {
                return not_found(format!("前各項 from {from}"));
            };
            (art, art.paragraphs.iter().filter(|p| p.num < cp).collect())
        }
        RefKind::NextParagraph => {
            let Some(cp) = cur_para else {
                return not_found(format!("次項 from {from}"));
            };
            (
                art,
                art.paragraphs.iter().filter(|p| p.num == cp + 1).collect(),
            )
        }
        RefKind::SameArticle => {
            if ante.prev_was_external {
                if let Some((law, num, _)) = ante.external.clone() {
                    return external(ante, &law, &num, r, false);
                }
            }
            let Some(a) = ante.article.as_ref().and_then(|id| index.by_id(id)) else {
                return Resolution::Unresolved(Unresolved::NoAntecedent);
            };
            (a, narrow_paras(a, r.paragraph))
        }
        RefKind::SameParagraph => {
            if ante.prev_was_external {
                if let Some((law, num, para)) = ante.external.clone() {
                    let r2 = ParsedRef {
                        paragraph: para,
                        ..r.clone()
                    };
                    return external(ante, &law, &num, &r2, false);
                }
            }
            let Some(pid) = ante.paragraph.as_ref() else {
                return Resolution::Unresolved(Unresolved::NoAntecedent);
            };
            let Some((_, a)) = index.article_of(pid) else {
                return Resolution::Unresolved(Unresolved::NoAntecedent);
            };
            (
                a,
                a.paragraphs
                    .iter()
                    .filter(|p| &p.stable_id == pid)
                    .collect(),
            )
        }
        RefKind::Article { suppl, num } => {
            // 「附則第N条」は原始附則、それ以外は参照元と同じ領域（附則から本則の「第N条」を指す場合は本則）
            let region = if *suppl {
                Region::Suppl(0)
            } else {
                Region::Main
            };
            let found = index.find(&region, num).or_else(|| {
                if *suppl {
                    None
                } else {
                    index.find(&art.region, num)
                }
            });
            match found {
                Some((_, a)) => (a, narrow_paras(a, r.paragraph)),
                None => {
                    // この法令に無い条番号。文中に他法令への言及があれば、その法令の条と推定する
                    if !*suppl {
                        if let Some(law) = ante.last_law.clone() {
                            return external(ante, &law, num, r, true);
                        }
                    }
                    return not_found(format!(
                        "第{}条 (suppl={suppl}) from {from}",
                        num.to_num_string()
                    ));
                }
            }
        }
    };

    if let (Some(p), true) = (r.paragraph, target_paras.is_empty()) {
        return not_found(format!("{:?} 第{p}項", r.kind));
    }

    // 先行詞の更新
    ante.article = Some(target_art.stable_id.clone());
    ante.paragraph = if target_paras.len() == 1 {
        Some(target_paras[0].stable_id.clone())
    } else {
        None
    };
    ante.external = None;

    // 号・前段/後段で絞る
    if let Some(item) = r.item {
        let Some(p) = single_para(&target_paras, target_art) else {
            return not_found(format!("第{item}号: paragraph ambiguous"));
        };
        return match p
            .items
            .iter()
            .find(|(n, _)| n.parse::<u32>().ok() == Some(item))
        {
            Some((_, id)) => Resolution::Internal(vec![id.clone()]),
            None => not_found(format!("{} 第{item}号", p.stable_id)),
        };
    }
    if let Some(part) = &r.part {
        let Some(p) = single_para(&target_paras, target_art) else {
            return not_found("前段/後段: paragraph ambiguous".into());
        };
        let i = match part {
            Part::Front => 0,
            Part::Back => 1,
        };
        return match p.sentences.get(i) {
            Some(id) => Resolution::Internal(vec![id.clone()]),
            None => not_found(format!("{} {part:?}", p.stable_id)),
        };
    }
    if r.paragraph.is_some()
        || matches!(
            r.kind,
            RefKind::PrevParagraph(_)
                | RefKind::AllPrevParagraphs
                | RefKind::NextParagraph
                | RefKind::SameParagraph
                | RefKind::Paragraph(_)
                | RefKind::Item(_)
        )
    {
        Resolution::Internal(target_paras.iter().map(|p| p.stable_id.clone()).collect())
    } else {
        Resolution::Internal(vec![target_art.stable_id.clone()])
    }
}

fn narrow_paras(a: &ArticleEntry, para: Option<u32>) -> Vec<&ParagraphEntry> {
    match para {
        Some(n) => a.paragraphs.iter().filter(|p| p.num == n).collect(),
        None => a.paragraphs.iter().collect(),
    }
}

/// 項が 1 つに定まるならそれ。条だけ指定で条が 1 項しか無ければその項
fn single_para<'a>(
    paras: &[&'a ParagraphEntry],
    art: &'a ArticleEntry,
) -> Option<&'a ParagraphEntry> {
    if paras.len() == 1 {
        return Some(paras[0]);
    }
    (art.paragraphs.len() == 1).then(|| &art.paragraphs[0])
}

fn not_found(msg: String) -> Resolution {
    Resolution::Unresolved(Unresolved::NotFound(msg))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpan {
    pub span: RefSpan,
    pub resolution: Resolution,
}

/// 1 文の中の参照表現をすべて認識し、左から順に先行詞を追跡しながら解決する
pub fn resolve_sentence_with(
    index: &Index,
    from: &StableId,
    text: &str,
    ante: &mut Antecedent,
) -> Vec<ResolvedSpan> {
    let spans = find_references(text);
    let mut out = Vec::with_capacity(spans.len());
    // 括弧の深さごとに「直前の参照の終わり」を持つ。括弧書きの中の参照が外側の列挙判定を壊さないように
    let mut prev_end: Vec<usize> = vec![0];
    for span in spans {
        // 列挙（「、」「及び」「並びに」「又は」「若しくは」「から〜まで」だけで繋がる）は直前の参照の続き。
        // 「第十九条第一項（同条第七項において準用する場合を含む。）若しくは第三項」のような括弧書きは飛ばす
        let depth = paren_depth(&text[..span.start]);
        if prev_end.len() <= depth {
            prev_end.resize(depth + 1, span.start);
        }
        let between = strip_parentheticals(&text[prev_end[depth]..span.start]);
        let list_continues = !between.is_empty()
            && between
                .chars()
                .all(|c| "、及び並に又は若しくからまで".contains(c));
        // 「同法」の解決先が未知なら、この文でそれまでに名前の出た法令を拾う
        if ante.last_law.is_none() {
            ante.last_law = last_law_mention(&text[..span.start]);
        }
        let parsed = match (&span.parsed.kind, list_continues) {
            // 他法令の条の列挙: 「非訟事件手続法第二十七条、第四十条及び第六十三条」
            (RefKind::Article { suppl: false, num }, true) if ante.prev_was_external => {
                match ante.external.clone() {
                    Some((law, _, _)) => ParsedRef {
                        kind: RefKind::External {
                            law,
                            num: num.clone(),
                        },
                        ..span.parsed.clone()
                    },
                    None => span.parsed.clone(),
                }
            }
            // 他法令の条の項の列挙: 「同法第百三十三条の二第五項及び第六項」
            (RefKind::Paragraph(n), true) if ante.prev_was_external => {
                match ante.external.clone() {
                    Some((law, num, _)) => ParsedRef {
                        kind: RefKind::External { law, num },
                        paragraph: Some(*n),
                        ..span.parsed.clone()
                    },
                    None => span.parsed.clone(),
                }
            }
            // 条の項の列挙: 「前条第五項及び第六項」「第十七条第一項から第三項まで」
            (RefKind::Paragraph(n), true) if ante.article.is_some() => ParsedRef {
                kind: RefKind::SameArticle,
                paragraph: Some(*n),
                ..span.parsed.clone()
            },
            _ => span.parsed.clone(),
        };
        let resolution = resolve(index, from, &parsed, ante);
        prev_end[depth] = span.end;
        prev_end.truncate(depth + 1);
        out.push(ResolvedSpan { span, resolution });
    }
    out
}

/// `s` の末尾時点での全角括弧の深さ
fn paren_depth(s: &str) -> usize {
    s.chars().fold(0usize, |d, c| match c {
        '（' => d + 1,
        '）' => d.saturating_sub(1),
        _ => d,
    })
}

/// 全角括弧の中身を（入れ子込みで）取り除く
fn strip_parentheticals(s: &str) -> String {
    let mut depth = 0u32;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '（' => depth += 1,
            '）' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// `text` の中で最後に名前の出た法令（「民事訴訟法」「非訟事件手続法（平成二十三年法律第五十一号）」）。
/// 「この法律」「同法」「借地借家法」は除く（自法令や代名詞）
fn last_law_mention(text: &str) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"([^、。（）「」\s]{1,30}?(?:法|令|規則))(?:（[^）]*号）)?第").unwrap()
    });
    re.captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .filter(|name| !name.ends_with("この法律") && *name != "同法" && *name != "法")
        .map(|name| {
            // 助詞から後ろだけを名称とみなす
            name.rsplit(|c: char| "はがをにのとでも中".contains(c))
                .next()
                .unwrap_or(name)
                .to_string()
        })
        .filter(|n| !n.is_empty())
        .last()
}

pub fn resolve_sentence(index: &Index, from: &StableId, text: &str) -> Vec<ResolvedSpan> {
    resolve_sentence_with(index, from, text, &mut Antecedent::default())
}

/// 項の中の複数文を、先行詞を引き継ぎながら解決する（本文の「第一項」をただし書きの「同項」が受ける）
pub fn resolve_paragraph(index: &Index, sentences: &[(StableId, String)]) -> Vec<ResolvedSpan> {
    let mut ante = Antecedent::default();
    sentences
        .iter()
        .flat_map(|(id, text)| resolve_sentence_with(index, id, text, &mut ante))
        .collect()
}
