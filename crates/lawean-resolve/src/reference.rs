//! 条文中の参照表現の認識。「前項」「前二項」「次条第一項」「第三十八条第二項」「附則第二条」「同条」「民法第六百四条」など。
//! 解決はしない（`resolve.rs`）。

use crate::numeral::kanji_to_u32;
use lawean_source::ArticleNum;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// 前段
    Front,
    /// 後段
    Back,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    /// 「前条」「前二条」
    PrevArticle(u32),
    /// 「次条」
    NextArticle,
    /// 「前項」「前二項」
    PrevParagraph(u32),
    /// 「前各項」
    AllPrevParagraphs,
    /// 「次項」
    NextParagraph,
    /// 「同条」— 直前に言及された条
    SameArticle,
    /// 「同項」— 直前に言及された項
    SameParagraph,
    /// 「第N条」「附則第N条」— この法令内
    Article { suppl: bool, num: ArticleNum },
    /// 「民法第六百四条」「同法第百三十三条」— 他法令
    External { law: String, num: ArticleNum },
    /// 「第一項」— 同じ条の項（条を伴わない）
    Paragraph(u32),
    /// 「第三号」— 同じ項の号（条・項を伴わない）
    Item(u32),
    /// 「前号」「前二号」— 同じ項の前の号
    PrevItem(u32),
    /// 「前各号」
    AllPrevItems,
    /// 「次号」
    NextItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRef {
    pub kind: RefKind,
    pub paragraph: Option<u32>,
    pub item: Option<u32>,
    pub part: Option<Part>,
}

/// 文中で見つかった参照表現
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSpan {
    pub text: String,
    /// 文字列中のバイト範囲
    pub start: usize,
    pub end: usize,
    pub parsed: ParsedRef,
}

const N: &str = "[一二三四五六七八九十百千]+";

fn re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?x)
            (?P<suppl>附則)?第(?P<art>{N})条(?P<branch>(?:の{N})*)(?:第(?P<para>{N})項)?(?:第(?P<item>{N})号)?(?P<part1>前段|後段)?
            |
            (?P<rel>前各項|前各号|前(?P<reln>{N})?条|前(?P<relp>{N})?項|前(?P<reli>{N})?号|次条|次項|次号|同条|同項)(?:第(?P<relpara>{N})項)?(?:第(?P<relitem>{N})号)?(?P<part2>前段|後段)?
            |
            第(?P<bpara>{N})項(?:第(?P<bitem>{N})号)?(?P<part3>前段|後段)?
            |
            第(?P<bitem2>{N})号
            "
        ))
        .unwrap()
    })
}

/// 参照の直前にある法令名（「民法（明治二十九年法律第八十九号）」「同法」「非訟事件手続法」）を拾う。
/// 直前が「）」なら括弧内の法令番号ごと遡る。
fn preceding_law_name(text: &str, start: usize) -> Option<String> {
    let before = &text[..start];
    let mut chars: Vec<char> = before.chars().collect();
    // 「（明治二十九年法律第八十九号）」を飛ばす
    if chars.last() == Some(&'）') {
        let open = chars.iter().rposition(|c| *c == '（')?;
        chars.truncate(open);
    }
    let stop = |c: char| "、。（「」〔〕 \n\t".contains(c);
    let tail: String = chars
        .iter()
        .rev()
        .take_while(|c| !stop(**c))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    // 末尾が法令名らしいものだけ。「同法」「〜法」「〜令」「〜規則」
    let is_law = tail.ends_with('法')
        || tail.ends_with('令')
        || tail.ends_with("規則")
        || tail.ends_with("同法");
    if !is_law {
        return None;
    }
    // 「この法律の規定は」のような非名称は除く
    if tail == "法" {
        return None;
    }
    // 助詞・接続詞から後ろだけを名称とみなす（雑だが v0.1）
    let name = tail
        .rsplit(|c: char| "はがをにのとでも及び並びに又若しくは中".contains(c))
        .next()
        .unwrap_or(&tail)
        .to_string();
    if name.is_empty() || name == "法" {
        return Some(tail);
    }
    Some(name)
}

pub fn find_references(text: &str) -> Vec<RefSpan> {
    let mut out = Vec::new();
    for m in re().captures_iter(text) {
        let whole = m.get(0).unwrap();
        let num = |name: &str| m.name(name).and_then(|x| kanji_to_u32(x.as_str()));
        let part = m.name("part1").or_else(|| m.name("part2")).map(|p| {
            if p.as_str() == "前段" {
                Part::Front
            } else {
                Part::Back
            }
        });

        let parsed = if let Some(art) = m.name("art") {
            let base = match kanji_to_u32(art.as_str()) {
                Some(b) => b,
                None => continue,
            };
            let branch: Vec<u32> = m
                .name("branch")
                .map(|b| {
                    b.as_str()
                        .split('の')
                        .filter(|s| !s.is_empty())
                        .filter_map(kanji_to_u32)
                        .collect()
                })
                .unwrap_or_default();
            let num = ArticleNum::Single { base, branch };
            let suppl = m.name("suppl").is_some();
            let kind = match (suppl, preceding_law_name(text, whole.start())) {
                (false, Some(law)) => RefKind::External { law, num },
                _ => RefKind::Article { suppl, num },
            };
            ParsedRef {
                kind,
                paragraph: num_of(&m, "para"),
                item: num_of(&m, "item"),
                part,
            }
        } else if let Some(p) = num_of(&m, "bpara") {
            ParsedRef {
                kind: RefKind::Paragraph(p),
                paragraph: Some(p),
                item: num_of(&m, "bitem"),
                part,
            }
        } else if let Some(i) = num_of(&m, "bitem2") {
            // 「法律第八十九号」「政令第百号」は法令番号であって号ではない
            let before = &text[..whole.start()];
            if [
                "法律", "政令", "省令", "府令", "規則", "条例", "告示", "通達", "号外",
            ]
            .iter()
            .any(|k| before.ends_with(k))
            {
                continue;
            }
            // 「第一号法定受託事務」（地方自治法の用語）は号の参照ではない
            if text[whole.end()..].starts_with("法定受託事務") {
                continue;
            }
            ParsedRef {
                kind: RefKind::Item(i),
                paragraph: None,
                item: Some(i),
                part: None,
            }
        } else {
            let rel = m.name("rel").unwrap().as_str();
            let kind = match rel {
                "前各項" => RefKind::AllPrevParagraphs,
                "前各号" => RefKind::AllPrevItems,
                "次条" => RefKind::NextArticle,
                "次項" => RefKind::NextParagraph,
                "次号" => RefKind::NextItem,
                "同条" => RefKind::SameArticle,
                "同項" => RefKind::SameParagraph,
                r if r.ends_with('条') => RefKind::PrevArticle(num("reln").unwrap_or(1)),
                r if r.ends_with('号') => RefKind::PrevItem(num("reli").unwrap_or(1)),
                _ => RefKind::PrevParagraph(num("relp").unwrap_or(1)),
            };
            ParsedRef {
                kind,
                paragraph: num_of(&m, "relpara"),
                item: num_of(&m, "relitem"),
                part,
            }
        };
        out.push(RefSpan {
            text: whole.as_str().to_string(),
            start: whole.start(),
            end: whole.end(),
            parsed,
        });
    }
    out
}

fn num_of(m: &regex::Captures<'_>, name: &str) -> Option<u32> {
    m.name(name).and_then(|x| kanji_to_u32(x.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(s: &str) -> ParsedRef {
        let v = find_references(s);
        assert_eq!(v.len(), 1, "{s}: {v:?}");
        v.into_iter().next().unwrap().parsed
    }

    #[test]
    fn relative_forms() {
        assert_eq!(one("前項の規定").kind, RefKind::PrevParagraph(1));
        assert_eq!(one("前二項の規定").kind, RefKind::PrevParagraph(2));
        assert_eq!(one("前各項").kind, RefKind::AllPrevParagraphs);
        assert_eq!(one("前条の").kind, RefKind::PrevArticle(1));
        let r = one("次条第一項において");
        assert_eq!(r.kind, RefKind::NextArticle);
        assert_eq!(r.paragraph, Some(1));
        let r = one("前項前段の特約");
        assert_eq!(r.kind, RefKind::PrevParagraph(1));
        assert_eq!(r.part, Some(Part::Front));
    }

    #[test]
    fn absolute_forms() {
        let r = one("第三十八条第二項及び");
        assert_eq!(
            r.kind,
            RefKind::Article {
                suppl: false,
                num: ArticleNum::Single {
                    base: 38,
                    branch: vec![]
                }
            }
        );
        assert_eq!(r.paragraph, Some(2));
        let r = one("第二十二条の二の規定");
        assert_eq!(
            r.kind,
            RefKind::Article {
                suppl: false,
                num: ArticleNum::Single {
                    base: 22,
                    branch: vec![2]
                }
            }
        );
        let r = one("附則第二条の規定による");
        assert!(matches!(r.kind, RefKind::Article { suppl: true, .. }));
        let r = one("第九条第一項第三号");
        assert_eq!((r.paragraph, r.item), (Some(1), Some(3)));
    }

    #[test]
    fn external_forms() {
        let r = one("民法（明治二十九年法律第八十九号）第六百四条の規定");
        assert_eq!(
            r.kind,
            RefKind::External {
                law: "民法".into(),
                num: ArticleNum::Single {
                    base: 604,
                    branch: vec![]
                }
            }
        );
        let r = one("この場合において、同法第百三十三条の");
        assert!(matches!(r.kind, RefKind::External { ref law, .. } if law == "同法"));
        let r = one("については、非訟事件手続法第二十三条の");
        assert!(matches!(r.kind, RefKind::External { ref law, .. } if law == "非訟事件手続法"));
        // 「この法律の規定は」の後の第N条は内部参照
        let r = one("この法律の施行前の第九条");
        assert!(matches!(r.kind, RefKind::Article { .. }));
    }

    #[test]
    fn bare_paragraph_and_item() {
        let r = one("第一項の規定による");
        assert_eq!(r.kind, RefKind::Paragraph(1));
        let r = one("第三号に掲げる");
        assert_eq!(r.kind, RefKind::Item(3));
        assert!(find_references("平成三年法律第九十号").is_empty());
        let r = one("第二項第一号");
        assert_eq!((r.kind, r.item), (RefKind::Paragraph(2), Some(1)));
    }

    #[test]
    fn multiple_in_order() {
        let v = find_references("第九条及び第十六条の規定にかかわらず、前項の");
        assert_eq!(
            v.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
            ["第九条", "第十六条", "前項"]
        );
    }
}
