//! 改め文のパース。語彙は閉じている（docs/08-amendment.md §3）。

use crate::op::*;
use lawean_resolve::numeral::kanji_to_u32;
use lawean_source::ArticleNum;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("unrecognized instruction segment: {0}")]
    Unrecognized(String),
    #[error("「同条」「同項」の先行詞が無い: {0}")]
    NoAntecedent(String),
    #[error("no header line (第N条　X法…の一部を次のように改正する。)")]
    NoHeader,
}

const N: &str = "[一二三四五六七八九十百千]+";

fn re(s: &'static str) -> Regex {
    Regex::new(&s.replace("{N}", N).replace("{K}", KANA)).unwrap()
}

/// 号の下の細目の記号（イロハ順）
const KANA: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラム";

/// 「ロ」→ 2
pub fn kana_index(k: &str) -> u32 {
    KANA.chars()
        .position(|c| k.starts_with(c))
        .map(|i| i as u32 + 1)
        .unwrap_or(0)
}

/// 2 →「ロ」
pub fn kana_of(n: u32) -> String {
    KANA.chars()
        .nth((n as usize).saturating_sub(1))
        .map(|c| c.to_string())
        .unwrap_or_default()
}

/// 衆議院の制定法律の本文の正規化: ルビ「瑕(か)疵(し)」の半角括弧のふりがなを落とし、
/// Shift_JIS に無い字の代替（剥→剝）を e-Gov の字に戻す
pub fn normalize_source_text(s: &str) -> String {
    static RUBY: OnceLock<Regex> = OnceLock::new();
    let ruby = RUBY.get_or_init(|| Regex::new(r"\([ぁ-ゖ]+\)").unwrap());
    ruby.replace_all(s, "").replace('剥', "剝")
}

/// 改正法の本文（借地借家法の部分）を改正単位に分ける。
/// 「第N条　X法（…）の一部を次のように改正する。」で始まり、インデント 2 の行が指示、インデント 1 の行が追加条文
pub fn parse_units(text: &str) -> Result<Vec<AmendUnit>, ParseError> {
    static HEADER: OnceLock<Regex> = OnceLock::new();
    let header =
        HEADER.get_or_init(|| re(r"^(第{N}条)　(.+?)(（[^）]*）)?の一部を次のように改正する。$"));
    // 「第N条　次に掲げる法律の規定中「A」を「B」に改める。」+「一　X法（…）第M条…」の列挙形（令4-68 第221条など）。
    // 号ごとに、その法律を改正する単位を作る
    static LIST_HEADER: OnceLock<Regex> = OnceLock::new();
    let list_header = LIST_HEADER
        .get_or_init(|| re(r"^(第{N}条)　次に掲げる法律の規定中「(.+?)」を「(.+?)」に改める。$"));
    static LIST_ITEM: OnceLock<Regex> = OnceLock::new();
    let list_item = LIST_ITEM.get_or_init(|| re(r"^{N}　(.+?)（[^）]*）(第.+)$"));
    let mut list: Option<(String, String, String)> = None;
    let mut units: Vec<AmendUnit> = Vec::new();
    let text = normalize_source_text(text);
    // 整備法の体裁: 条の見出し「（X法の一部改正）」と章の見出し「第二章　文部科学省関係」は改め文ではない。
    // 章の見出しは「次の一章を加える」の内容（章名の行）と字面が同じなので、次の行が見出し・条の頭なら読み飛ばす
    static CAPTION: OnceLock<Regex> = OnceLock::new();
    let caption = CAPTION.get_or_init(|| re(r"^（.+の一部改正）$"));
    let lines: Vec<&str> = text.lines().collect();
    // 整備法の条の見出し一般（「（X法の一部改正に伴う経過措置）」）: 括弧だけの行で、次の行が改正法の条（字下げ無しの「第N条　」）
    let any_caption = |i: usize| -> bool {
        let l = lines[i].trim_start_matches(['\u{3000}', ' ']).trim_end();
        if !(l.starts_with('（')
            && l.ends_with('）')
            && !l[..l.len() - '）'.len_utf8()].contains('）'))
        {
            return false;
        }
        lines[i + 1..]
            .iter()
            .find(|x| !x.trim().is_empty())
            .is_some_and(|n| {
                !n.starts_with('\u{3000}') && n.starts_with('第') && n.contains("条\u{3000}")
            })
    };
    static AMENDING_CHAPTER: OnceLock<Regex> = OnceLock::new();
    let amending_chapter = AMENDING_CHAPTER.get_or_init(|| re(r"^第{N}(?:編|章|節)　[^（）]+$"));
    for (li, raw) in lines.iter().enumerate() {
        let indent = raw
            .chars()
            .take_while(|c| *c == '\u{3000}' || *c == ' ')
            .count();
        let line = raw.trim_start_matches(['\u{3000}', ' ']).trim_end();
        if line.is_empty()
            || line.starts_with('（') && indent == 0
            || caption.is_match(line)
            || any_caption(li)
        {
            continue;
        }
        if amending_chapter.is_match(line) {
            let next = lines[li + 1..]
                .iter()
                .enumerate()
                .find(|(_, l)| !l.trim().is_empty());
            if next.is_none_or(|(k, n)| {
                let n = n.trim_start_matches(['\u{3000}', ' ']).trim_end();
                caption.is_match(n)
                    || header.is_match(n)
                    || list_header.is_match(n)
                    || any_caption(li + 1 + k)
            }) {
                continue;
            }
        }
        if let Some(c) = list_header.captures(line) {
            list = Some((c[1].to_string(), c[2].to_string(), c[3].to_string()));
            continue;
        }
        if let (Some((art, a, b)), Some(c)) = (&list, list_item.captures(line)) {
            if indent == 1 {
                let text = format!("{}中「{a}」を「{b}」に改める。", &c[2]);
                let ops = parse_instruction(&text)?;
                units.push(AmendUnit {
                    article_of_amending_law: art.clone(),
                    target_title: c[1].to_string(),
                    instructions: vec![Instruction { text, ops }],
                });
                continue;
            }
        }
        if let Some(c) = header.captures(line) {
            list = None;
            units.push(AmendUnit {
                article_of_amending_law: c[1].to_string(),
                target_title: c[2].to_string(),
                instructions: Vec::new(),
            });
            continue;
        }
        let Some(unit) = units.last_mut() else {
            return Err(ParseError::NoHeader);
        };
        // インデント 2 で「。」で終わる行が指示。ただし「一　…。」の号の行（加える項の中の号）は内容
        let is_item_line = line.split_once('\u{3000}').is_some_and(|(t, _)| {
            !t.is_empty() && t.chars().all(|c| "一二三四五六七八九十".contains(c))
        });
        let is_instruction =
            indent == 2 && line.ends_with('。') && !line.starts_with('（') && !is_item_line;
        if is_instruction {
            let ops = parse_instruction(line)?;
            unit.instructions.push(Instruction {
                text: line.to_string(),
                ops,
            });
        } else {
            // 追加する条文。直前の指示の、内容を取る最後の操作に付ける
            let Some(ins) = unit.instructions.last_mut() else {
                return Err(ParseError::Unrecognized(line.to_string()));
            };
            match ins.ops.iter_mut().rev().find(|o| o.takes_content()) {
                Some(op) => op.push_content(line.to_string()),
                None => return Err(ParseError::Unrecognized(line.to_string())),
            }
        }
    }
    Ok(units)
}

/// 「、」で区切る。ただし「」（）の中は区切らない
fn split_segments(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut q, mut p) = (0i32, 0i32);
    let body = s.trim_end_matches('。');
    // 字句そのものが「」を含んで括弧が釣り合わないとき（読替え規定の書き換え）は深さでは切れない。
    // 前が指示の終わり（「」に」「」を」「改め」「加え」「削り」「とし」）で、後が指示の始まり（「「」「同条」「第N条」…）なら切る
    let boundary = |before: &str, after: &str| {
        let end_ok = [
            "」に",
            "」を",
            "改め",
            "加え",
            "削り",
            "とし",
            "とする",
            "改める",
            "加える",
            "削る",
        ]
        .iter()
        .any(|m| before.ends_with(m));
        let start_ok = after.starts_with('「')
            || after.starts_with("同条")
            || after.starts_with("同項")
            || after.starts_with("同号")
            || after.starts_with("第");
        end_ok && start_ok
    };
    for (i, c) in body.char_indices() {
        match c {
            '「' => q += 1,
            // 余る閉じ括弧は字句の一部（take_quoted と同じ扱い）
            '」' => q = (q - 1).max(0),
            // 「」の中の（）は数えない（字句「）は」のように片方だけのことがある）
            '（' if q == 0 => p += 1,
            '）' if q == 0 => p = (p - 1).max(0),
            '、' if (q == 0 && p == 0) || boundary(&cur, &body[i + '、'.len_utf8()..]) => {
                out.push(std::mem::take(&mut cur));
                if q > 0 {
                    q = 0;
                }
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    // 「第五条第一項第五号、第十八条第一項第六号及び第五十二条第七号ロ中「A」を…」の「、」は位置の列挙。
    // 「」も動詞も含まない位置だけの断片は、次の断片につなぐ
    let mut merged: Vec<String> = Vec::new();
    let mut pending = String::new();
    for seg in out {
        let loc_only = (seg.starts_with('第') || seg.starts_with('同'))
            && !seg.contains('「')
            && !seg.ends_with(['し', 'る', 'め', 'え', 'り', 'げ']);
        // 「X中「A」、「B」及び「C」を削り」の「A」までの断片（動詞が無く「」で終わる）も次につなぐ
        let phrase_only = seg.ends_with('」');
        if loc_only || phrase_only {
            pending.push_str(&seg);
            pending.push('、');
            continue;
        }
        merged.push(format!("{pending}{seg}"));
        pending.clear();
    }
    if !pending.is_empty() {
        merged.push(pending.trim_end_matches('、').to_string());
    }
    merged
}

/// 位置の列挙「第七条第一項及び第二項並びに第八条」「第三十一条から第三十三条までの規定及び第三十六条」
/// 「第五条第一項第五号、第十八条第一項第六号及び第五十二条第七号ロ」を位置の列にする。
/// 「第N条から第M条まで」は条の範囲（`ArticleNum::Range`。当てるときに発射台の条に展開）、
/// 「第N項から第M項まで」は項ごとに展開する
fn expand_locs(s: &str, ante: &mut Ante) -> Result<Vec<Loc>, ParseError> {
    static RANGE: OnceLock<Regex> = OnceLock::new();
    let range = RANGE.get_or_init(|| re(r"^(?P<a>.+?)から(?P<b>.+?)まで$"));
    let mut out = Vec::new();
    let s = s.trim_end_matches("の規定");
    for tok in s
        .split("並びに")
        .flat_map(|x| x.split("及び"))
        .flat_map(|x| x.split('、'))
    {
        let tok = tok.trim_end_matches("の規定");
        if tok.is_empty() {
            continue;
        }
        if let Some(c) = range.captures(tok) {
            let (a, b) = (c["a"].to_string(), c["b"].to_string());
            let la = loc(&a, ante)?;
            let lb = loc(&b, ante)?;
            // 「第一号から第四号まで」: 号の範囲（同じ項の中）
            if let (Some(ia), Some(ib)) = (&la.item, &lb.item) {
                let (p, q) = (
                    ia.parse::<u32>().unwrap_or(0),
                    ib.parse::<u32>().unwrap_or(0),
                );
                for n in p..=q {
                    out.push(Loc {
                        item: Some(n.to_string()),
                        ..la.clone()
                    });
                }
                continue;
            }
            match (la.paragraph.clone(), lb.paragraph.clone()) {
                (Some(ParaRef::Num(p)), Some(ParaRef::Num(q))) => {
                    for n in p..=q {
                        out.push(Loc {
                            article: la.article.clone(),
                            paragraph: Some(ParaRef::Num(n)),
                            item: None,
                            part: None,
                            suppl: false,
                            sub: None,
                        });
                    }
                }
                (None, None) => out.push(Loc {
                    article: ArticleNum::Range {
                        from: Box::new(la.article),
                        to: Box::new(lb.article),
                    },
                    paragraph: None,
                    item: None,
                    part: None,
                    suppl: false,
                    sub: None,
                }),
                _ => return Err(ParseError::Unrecognized(tok.to_string())),
            }
            continue;
        }
        out.push(loc(tok, ante)?);
    }
    Ok(out)
}

impl Ante {
    fn empty() -> Ante {
        Ante {
            article: None,
            paragraph: None,
            item: None,
            toc: false,
            container: Vec::new(),
            part: None,
            locs: Vec::new(),
            suppl: false,
            sub: None,
            appdx: None,
        }
    }
}

/// 附則の範囲欄の、被改正法の側で書かれた改正規定の列挙「医師法第十六条の十一第一項の改正規定及び同法第十七条の改正規定」を
/// 位置の列に読む（法令名は落とす。「同法」「同条」は直前の位置）。単位を施行日ごとに分ける（`AmendUnit::split_by_locs`）ときに使う
pub fn parse_scope_locs(scope: &str) -> Result<Vec<Loc>, ParseError> {
    let mut ante = Ante::empty();
    let mut out = Vec::new();
    for tok in scope
        .split("並びに")
        .flat_map(|x| x.split("及び"))
        .flat_map(|x| x.split('、'))
    {
        let tok = tok.trim();
        let Some(tok) = tok.strip_suffix("の改正規定") else {
            continue;
        };
        // 法令名（「医師法」「同法」）を落とす: 最初の「第」「同条」から
        let start = tok
            .find("附則第")
            .into_iter()
            .chain(tok.find("第"))
            .chain(tok.find("同条"))
            .min()
            .unwrap_or(tok.len());
        let tok = &tok[start..];
        if tok.is_empty() {
            continue;
        }
        out.extend(expand_locs(tok, &mut ante)?);
    }
    Ok(out)
}

#[derive(Clone)]
struct Ante {
    article: Option<ArticleNum>,
    paragraph: Option<u32>,
    item: Option<String>,
    /// 直前の位置が目次（「目次中「A」を「B」に、「C」を「D」に改める」の続き）
    toc: bool,
    /// 直前の章（「第一章中第六節を第八節とし、第五節の次に次の二節を加える」の「第五節」の外側）
    container: Vec<(lawean_source::ContainerKind, String)>,
    /// 直前の位置の文（「同項ただし書中「A」を「B」に、「C」を「D」に改め」の続きはただし書の中）
    part: Option<SentencePart>,
    /// 直前の位置の列挙（「第九十四条第一項及び第三項中」）。位置を省いた続きはこの全部に当てる
    locs: Vec<Loc>,
    /// 直前の位置が附則の条
    suppl: bool,
    /// 直前の位置の号の下のイロハ（「同号ロ中」）
    sub: Option<String>,
    /// 直前の位置が別表の行（表, 行の上欄）
    appdx: Option<(String, String)>,
}

pub(crate) fn art_num(s: &str) -> ArticleNum {
    // 「第百二条及び第百三条」「第百四条から第百五条の二まで」→ 範囲
    if let Some((a, b)) = s
        .split_once("及び")
        .or_else(|| s.strip_suffix("まで").and_then(|x| x.split_once("から")))
    {
        if a.starts_with('第') && b.starts_with('第') {
            return ArticleNum::Range {
                from: Box::new(art_num(a)),
                to: Box::new(art_num(b)),
            };
        }
    }
    // 「第二十二条の二」→ 22_2
    let parts: Vec<u32> = s
        .trim_start_matches('第')
        .trim_end_matches('条')
        .split("条の")
        .flat_map(|p| p.split('の'))
        .filter_map(kanji_to_u32)
        .collect();
    ArticleNum::Single {
        base: parts[0],
        branch: parts[1..].to_vec(),
    }
}

fn loc(s: &str, ante: &mut Ante) -> Result<Loc, ParseError> {
    static LOC: OnceLock<Regex> = OnceLock::new();
    // 号（「第三号」「第二号の二」「同号」）とただし書・各号列記以外の部分は位置として読むが、操作は項全体に当てる
    // （字句の置換は項の中の全出現に及ぶ。号を限定した置換は未対応で、号の外にも同じ字句があれば置き換わる）
    let r = LOC.get_or_init(|| {
        re(r"^(附則)?(?:(第{N}条(?:の{N})*)|同条)?(?:第({N})項|(同項))?(?:第({N}号(?:の{N})*)|(同号))?([イロハニホヘトチリヌルヲワカヨタレソツネナラム])?(?:各号)?(ただし書|各号列記以外の部分|本文|前段|後段)?$")
    });
    let Some(c) = r.captures(s) else {
        return Err(ParseError::Unrecognized(s.to_string()));
    };
    let suppl = c.get(1).is_some() || (c.get(2).is_none() && ante.suppl);
    let article = match c.get(2) {
        Some(a) => {
            let n = art_num(a.as_str());
            ante.article = Some(n.clone());
            ante.toc = false;
            ante.suppl = suppl;
            n
        }
        None => ante
            .article
            .clone()
            .ok_or_else(|| ParseError::NoAntecedent(s.to_string()))?,
    };
    let paragraph = if let Some(p) = c.get(3) {
        let n = kanji_to_u32(p.as_str()).unwrap();
        ante.paragraph = Some(n);
        Some(ParaRef::Num(n))
    } else if c.get(4).is_some() {
        Some(ParaRef::Num(
            ante.paragraph
                .ok_or_else(|| ParseError::NoAntecedent(s.to_string()))?,
        ))
    } else {
        if c.get(2).is_some() {
            ante.paragraph = None;
        }
        None
    };
    let item = match c.get(5) {
        Some(i) => {
            let parts: Vec<String> = i
                .as_str()
                .trim_end_matches('号')
                .split("号の")
                .flat_map(|p| p.split('の'))
                .filter_map(kanji_to_u32)
                .map(|n| n.to_string())
                .collect();
            let it = parts.join("_");
            ante.item = Some(it.clone());
            Some(it)
        }
        None if c.get(6).is_some() => ante.item.clone(),
        None => {
            if c.get(2).is_some() || c.get(3).is_some() {
                ante.item = None;
            }
            None
        }
    };
    // 「同号」「第三号」で項を言わなければ直前の項の中
    let paragraph = match (paragraph, &item) {
        (None, Some(_)) if c.get(2).is_none() => ante.paragraph.map(ParaRef::Num),
        (p, _) => p,
    };
    // 号の下のイロハ: 号を言い直せば解ける
    let sub = match c.get(7) {
        Some(k) => Some(k.as_str().to_string()),
        None if c.get(5).is_some() => None,
        None => ante.sub.clone(),
    };
    ante.sub = sub.clone();
    let part = c.get(8).map(|m| match m.as_str() {
        "ただし書" => SentencePart::Proviso,
        "本文" => SentencePart::Main,
        "前段" => SentencePart::Front,
        "後段" => SentencePart::Back,
        _ => SentencePart::Chapeau,
    });
    // 位置を新しく言えば文の限定と列挙は解ける
    ante.part = part;
    ante.locs.clear();
    ante.appdx = None;
    Ok(Loc {
        article,
        paragraph,
        sub,
        item,
        part,
        suppl,
    })
}

/// 先頭の「…」を、入れ子の「」を数えて取り出す。返すのは (中身, 残り)
fn take_quoted(s: &str) -> Option<(String, &str)> {
    let rest = s.strip_prefix('「')?;
    let mut depth = 1;
    for (i, c) in rest.char_indices() {
        match c {
            '「' => depth += 1,
            '」' => {
                depth -= 1;
                if depth == 0 {
                    // 「の売買の相手方」」のように閉じ括弧が余る = 字句そのものが「」を含む（読替え規定の書き換え）。
                    // 余る「」」は字句に入れる
                    let mut end = i;
                    let mut after = &rest[i + '」'.len_utf8()..];
                    while let Some(more) = after.strip_prefix('」') {
                        end += '」'.len_utf8();
                        after = more;
                    }
                    return Some((rest[..end].to_string(), after));
                }
            }
            _ => {}
        }
    }
    None
}

/// 字句の置換・追加・削除を、入れ子の「」に耐える形で読む:
/// `[位置中]「A」を「B」に[改め(る)]` / `[位置中]「A」の下に「B」を[加え(る)]` / `[位置中]「A」を削(り|る)`
fn parse_phrase_op(seg: &str, ante: &mut Ante) -> Result<Option<PhraseOps>, ParseError> {
    let (loc_part, rest) = match seg.find("中「") {
        Some(i) if !seg.starts_with('「') => (Some(&seg[..i]), &seg[i + '中'.len_utf8()..]),
        _ if seg.starts_with('「') => (None, seg),
        _ => return Ok(None),
    };
    let Some((a, rest)) = take_quoted(rest) else {
        return Ok(None);
    };
    // 「A」、「B」及び「C」を削り / 「A」及び「B」を「C」に改め: 字句の列挙
    let mut phrases = vec![a];
    let mut rest = rest;
    while let Some(r) = rest
        .strip_prefix('、')
        .or_else(|| rest.strip_prefix("及び"))
    {
        let Some((b, r2)) = take_quoted(r) else { break };
        phrases.push(b);
        rest = r2;
    }
    let a = phrases[0].clone();
    // 別表の行: 「別表第二X法（…）の項中「A」を「B」に、「C」を「D」に改め、「E」の下に「F」を加える」
    static APPDX: OnceLock<Regex> = OnceLock::new();
    let appdx_re = APPDX.get_or_init(|| re(r"^(?P<table>別表(?:第{N})?)(?P<row>.+?)の項$"));
    let appdx: Option<(String, String)> = match loc_part {
        Some(l) => appdx_re
            .captures(l)
            .map(|c| (c["table"].to_string(), c["row"].to_string())),
        None if ante.article.is_none() && !ante.toc => ante.appdx.clone(),
        None => None,
    };
    if let Some((table, row)) = appdx {
        ante.appdx = Some((table.clone(), row.clone()));
        ante.article = None;
        let mk = |from: String, to: String| Op::ReplaceAppdxRow {
            table: table.clone(),
            row: row.clone(),
            from,
            to,
        };
        if let Some(r) = rest.strip_prefix("を") {
            if r == "削り" || r == "削る" {
                return Ok(Some(PhraseOps(
                    phrases.into_iter().map(|f| mk(f, String::new())).collect(),
                )));
            }
            if let Some((b, tail)) = take_quoted(r) {
                if matches!(tail, "に" | "に改め" | "に改める") {
                    return Ok(Some(PhraseOps(
                        phrases.into_iter().map(|f| mk(f, b.clone())).collect(),
                    )));
                }
            }
        }
        if let Some(r) = rest.strip_prefix("の下に") {
            if let Some((b, tail)) = take_quoted(r) {
                if matches!(tail, "を" | "を加え" | "を加える") {
                    return Ok(Some(PhraseOps(
                        phrases
                            .into_iter()
                            .map(|f| mk(f.clone(), format!("{f}{b}")))
                            .collect(),
                    )));
                }
            }
        }
        return Ok(None);
    }
    // 目次の字句: 「目次中「A」を削り」「目次中「A」を「B」に、「C」を「D」に改め」
    if loc_part == Some("目次") || (loc_part.is_none() && ante.toc) {
        ante.toc = true;
        if let Some(r) = rest.strip_prefix("を") {
            if r == "削り" || r == "削る" {
                return Ok(Some(PhraseOps(
                    phrases
                        .into_iter()
                        .map(|from| Op::ReplaceToc {
                            from,
                            to: String::new(),
                        })
                        .collect(),
                )));
            }
            if let Some((b, tail)) = take_quoted(r) {
                if matches!(tail, "に" | "に改め" | "に改める") {
                    return Ok(Some(PhraseOps(
                        phrases
                            .into_iter()
                            .map(|from| Op::ReplaceToc {
                                from,
                                to: b.clone(),
                            })
                            .collect(),
                    )));
                }
            }
        }
        return Ok(None);
    }
    // 位置を省いた続きは、直前の位置の列挙（「第九十四条第一項及び第三項中「A」を「B」に、「C」を「D」に改める」）全部に当てる
    // 条・項の中の表の行: 「第三十八条の表第七十条第二項の項中「A」を「B」に改め」（別表は上で）
    static TROW: OnceLock<Regex> = OnceLock::new();
    let trow = TROW.get_or_init(|| re(r"^(?P<loc>.+?)の表(?P<row>.+?)の項$"));
    if let Some(c) = loc_part.and_then(|l| trow.captures(l)) {
        let at = loc(&c["loc"], ante)?;
        let row = c["row"].to_string();
        let mk = |from: String, to: String| Op::ReplaceTableRow {
            at: at.clone(),
            row: row.clone(),
            from,
            to,
        };
        if let Some(r) = rest.strip_prefix("を") {
            if r == "削り" || r == "削る" {
                return Ok(Some(PhraseOps(
                    phrases.into_iter().map(|f| mk(f, String::new())).collect(),
                )));
            }
            if let Some((b, tail)) = take_quoted(r) {
                if matches!(tail, "に" | "に改め" | "に改める") {
                    return Ok(Some(PhraseOps(
                        phrases.into_iter().map(|f| mk(f, b.clone())).collect(),
                    )));
                }
            }
        }
        return Ok(None);
    }
    // 「第百八十五条（見出しを含む。）中「A」を「B」に改める」: 見出しも
    let (loc_part, with_caption) = match loc_part {
        Some(l) => match l.strip_suffix("（見出しを含む。）") {
            Some(base) => (Some(base), true),
            None => (Some(l), false),
        },
        None => (None, false),
    };
    let ats: Vec<Loc> = match loc_part {
        // 「同条第四項及び第六項中「A」を削る」: 位置の列挙（置換・追加の列挙は規則の側で展開する）
        Some(l) if l.contains("及び") || l.contains('、') || l.contains("まで") => {
            let v = expand_locs(l, ante)?;
            ante.locs = v.clone();
            v
        }
        Some(l) => vec![loc(l, ante)?],
        None if ante.locs.len() > 1 => ante.locs.clone(),
        None => vec![ante_loc(ante, seg)?],
    };
    if let Some(rest) = rest.strip_prefix("を") {
        if rest == "削り" || rest == "削る" {
            return Ok(Some(PhraseOps(
                ats.iter()
                    .flat_map(|at| {
                        phrases.iter().map(move |from| Op::Replace {
                            at: at.clone(),
                            from: from.clone(),
                            to: String::new(),
                        })
                    })
                    .collect(),
            )));
        }
        if let Some((b, tail)) = take_quoted(rest) {
            if matches!(tail, "に" | "に改め" | "に改める") {
                let mut v: Vec<Op> = Vec::new();
                if with_caption {
                    for at in &ats {
                        for from in &phrases {
                            v.push(Op::ReplaceCaption {
                                article: at.article.clone(),
                                from: from.clone(),
                                to: b.clone(),
                            });
                        }
                    }
                }
                v.extend(ats.iter().flat_map(|at| {
                    phrases.iter().map(|from| Op::Replace {
                        at: at.clone(),
                        from: from.clone(),
                        to: b.clone(),
                    })
                }));
                return Ok(Some(PhraseOps(v)));
            }
        }
        return Ok(None);
    }
    if let Some(rest) = rest.strip_prefix("の下に") {
        if let Some((b, tail)) = take_quoted(rest) {
            if matches!(tail, "を" | "を加え" | "を加える") {
                let _ = &a;
                return Ok(Some(PhraseOps(
                    ats.iter()
                        .flat_map(|at| {
                            phrases.iter().map(|anchor| Op::InsertAfterPhrase {
                                at: at.clone(),
                                anchor: anchor.clone(),
                                text: b.clone(),
                            })
                        })
                        .collect(),
                )));
            }
        }
    }
    Ok(None)
}

/// 1 つの断片から出る字句の操作（列挙なら複数）
struct PhraseOps(Vec<Op>);

/// 「X中「A」を「B」に、「C」を「D」に改め」を「」に、「」で片に分け、各片を緩く読む
fn parse_phrase_list_loose(merged: &str, ante: &mut Ante) -> Option<Vec<Op>> {
    let mut pieces: Vec<String> = Vec::new();
    let mut rest = merged;
    loop {
        let a = rest.find("」に、「");
        let b = rest.find("」に改め、「");
        match (a, b) {
            (None, None) => break,
            (Some(i), None) => {
                pieces.push(format!("{}」に", &rest[..i]));
                rest = &rest[i + "」に、".len()..];
            }
            (None, Some(j)) => {
                pieces.push(format!("{}」に", &rest[..j]));
                rest = &rest[j + "」に改め、".len()..];
            }
            (Some(i), Some(j)) if i < j => {
                pieces.push(format!("{}」に", &rest[..i]));
                rest = &rest[i + "」に、".len()..];
            }
            (_, Some(j)) => {
                pieces.push(format!("{}」に", &rest[..j]));
                rest = &rest[j + "」に改め、".len()..];
            }
        }
    }
    pieces.push(rest.to_string());
    let mut out = Vec::new();
    for p in &pieces {
        let PhraseOps(v) = parse_phrase_op_loose(p, ante).ok()??;
        out.extend(v);
    }
    Some(out)
}

/// 括弧が釣り合わない字句（「「規約」を「同条第六項ただし書中「規約」に」）: 最初の「」を「」で A と B を分け、
/// B は最後の「」に」（「」の下に「」なら「」を」）まで
fn parse_phrase_op_loose(seg: &str, ante: &mut Ante) -> Result<Option<PhraseOps>, ParseError> {
    let (loc_part, rest) = match seg.find("中「") {
        Some(i) if !seg.starts_with('「') => (Some(&seg[..i]), &seg[i + '中'.len_utf8()..]),
        _ if seg.starts_with('「') => (None, seg),
        _ => return Ok(None),
    };
    if loc_part == Some("目次") || (loc_part.is_none() && ante.toc) {
        return Ok(None);
    }
    let inner = &rest['「'.len_utf8()..];
    let mut at = || match loc_part {
        Some(l) => loc(l, ante),
        None => ante_loc(ante, seg),
    };
    // 「A」を削り
    for sfx in ["」を削り", "」を削る"] {
        if let Some(a) = inner.strip_suffix(sfx) {
            return Ok(Some(PhraseOps(vec![Op::Replace {
                at: at()?,
                from: a.to_string(),
                to: String::new(),
            }])));
        }
    }
    // 「A」を「B」に[改め(る)]
    if let Some(i) = inner.find("」を「") {
        let a = &inner[..i];
        let b_part = &inner[i + "」を「".len()..];
        for sfx in ["」に改める", "」に改め", "」に"] {
            if let Some(b) = b_part.strip_suffix(sfx) {
                return Ok(Some(PhraseOps(vec![Op::Replace {
                    at: at()?,
                    from: a.to_string(),
                    to: b.to_string(),
                }])));
            }
        }
    }
    // 「A」の下に「B」を[加え(る)]
    if let Some(i) = inner.find("」の下に「") {
        let a = &inner[..i];
        let b_part = &inner[i + "」の下に「".len()..];
        for sfx in ["」を加える", "」を加え", "」を"] {
            if let Some(b) = b_part.strip_suffix(sfx) {
                return Ok(Some(PhraseOps(vec![Op::InsertAfterPhrase {
                    at: at()?,
                    anchor: a.to_string(),
                    text: b.to_string(),
                }])));
            }
        }
    }
    Ok(None)
}

/// 「第一章第八節」→ [(章, 1), (節, 8)]
fn container_path(s: &str) -> Vec<(lawean_source::ContainerKind, String)> {
    static P: OnceLock<Regex> = OnceLock::new();
    let pr = P.get_or_init(|| re(r"第({N})(編|章|節|款|目)((?:の{N})*)"));
    pr.captures_iter(s)
        .map(|c| {
            let kind = match &c[2] {
                "編" => lawean_source::ContainerKind::Part,
                "章" => lawean_source::ContainerKind::Chapter,
                "節" => lawean_source::ContainerKind::Section,
                "款" => lawean_source::ContainerKind::Subsection,
                _ => lawean_source::ContainerKind::Division,
            };
            (kind, container_num(&c[1], &c[3]))
        })
        .collect()
}

/// 「二」「の二」→「2_2」（第二章の二）
fn container_num(n: &str, branch: &str) -> String {
    let mut s = kanji_to_u32(n).unwrap_or(0).to_string();
    for b in branch.split('の').filter(|x| !x.is_empty()) {
        s.push('_');
        s.push_str(&kanji_to_u32(b).unwrap_or(0).to_string());
    }
    s
}

/// 「第一章中第八節」の「第一章」を先行詞に取り、「第六節」だけの位置には直前の章を補う
fn container_path_with_ante(
    pre: &str,
    path: &str,
    ante: &mut Ante,
) -> Vec<(lawean_source::ContainerKind, String)> {
    fn depth(k: lawean_source::ContainerKind) -> u8 {
        use lawean_source::ContainerKind::*;
        match k {
            Part => 0,
            Chapter => 1,
            Section => 2,
            Subsection => 3,
            Division => 4,
        }
    }
    let mut full = container_path(pre);
    if !full.is_empty() {
        ante.container = full.clone();
    }
    let own = container_path(path);
    if full.is_empty() {
        // 直前の位置のうち、この位置より外側の部分を補う（「第四章第二節中第二款を第三款とし、第一款の次に」の「第一款」は第四章第二節の中）
        if let Some(first) = own.first() {
            full = ante
                .container
                .iter()
                .take_while(|(k, _)| depth(*k) < depth(first.0))
                .cloned()
                .collect();
        }
    }
    full.extend(own);
    full
}

/// 直前の位置（条・項）を引き継ぐ
fn ante_loc(ante: &Ante, seg: &str) -> Result<Loc, ParseError> {
    Ok(Loc {
        article: ante
            .article
            .clone()
            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?,
        sub: ante.sub.clone(),
        paragraph: ante.paragraph.map(ParaRef::Num),
        item: ante.item.clone(),
        part: ante.part,
        suppl: ante.suppl,
    })
}

pub fn parse_instruction(line: &str) -> Result<Vec<Op>, ParseError> {
    static RULES: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            ("toc", r"^目次中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            // 見出しは「中」の規則より先に（「第四十二条の見出し中「A」を「B」に改め」）
            ("caption_replace", r"^(?P<loc>第{N}条(?:の{N})*|同条)の見出し中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            // 見出しの中の字句の追加・削除: 「第九条の見出し中「A」の下に「B」を加え」「同条の見出し中「A」を削り」
            ("caption_insert", r"^(?P<loc>第{N}条(?:の{N})*|同条)の見出し中「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            ("caption_delete_phrase", r"^(?P<loc>第{N}条(?:の{N})*|同条)の見出し中「(?P<a>.+?)」を削(?:り|る)$"),
            ("container_title", r"^(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名中「(?P<a>.+?)」を(?:「(?P<b>.+?)」に(?:改め(?:る)?)?|削(?:り|る))$"),
            ("caption_set", r"^(?P<loc>第{N}条(?:の{N})*|同条)の見出しを「(?P<a>.+?)」に(?:改め(?:る)?)?$"),
            ("caption_attach", r"^(?P<loc>第{N}条(?:の{N})*|同条)の前に見出しとして「(?P<a>.+?)」を付(?:し|する)$"),
            ("caption_delete", r"^(?P<loc>第{N}条(?:の{N})*|同条)の(?:前の)?見出しを削(?:り|る)$"),
            // 「改める」「加える」が付かない形は、同じ文の中で「、」で連なる列挙の途中（「A」を「B」に、「C」を「D」に改める）
            ("replace", r"^(?P<loc>.+?)中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("insert_phrase", r"^(?P<loc>.+?)中「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            // 位置を省いた続き: 「第X中「A」の下に「B」を加え、「C」を「D」に改める」の後半。直前の位置を引き継ぐ
            ("replace_cont", r"^「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("insert_phrase_cont", r"^「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            // 号ずれ（項の中）。「第N項中第A号を第B号とし」「第A号を同条第B号とし」「同号の次に次のK号を加える」
            ("renumber_item", r"^(?P<loc>.+?中|同条|同項|第{N}条(?:の{N})*(?:第{N}項)?)?(?:第(?P<p>{N})号(?P<pb>(?:の{N})*)|(?P<same>同号))を(?:同条|同項)?第(?P<q>{N})号(?P<qb>(?:の{N})*)と(?:し|する)$"),
            ("shift_items", r"^(?P<loc>.+?中|同条|同項|第{N}条(?:の{N})*(?:第{N}項)?)?第(?P<p>{N})号から第(?P<q>{N})号までを(?P<k>{N})号ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            // 号の下のイロハ: 「同号ロを同号ハとし」「同号中ヘをトとし」「ハからホまでをニからヘまでとし」「同号イの次に次のように加える」
            ("renumber_sub", r"^(?P<loc>.+?中|同号|第{N}条(?:の{N})*(?:第{N}項)?第{N}号(?:の{N})*|同項第{N}号(?:の{N})*|同条第{N}項第{N}号(?:の{N})*)?(?P<a>[{K}])を(?:同号)?(?P<b>[{K}])と(?:し|する)$"),
            ("shift_sub", r"^(?P<loc>.+?中|同号|第{N}条(?:の{N})*(?:第{N}項)?第{N}号(?:の{N})*|同項第{N}号(?:の{N})*)?(?P<a>[{K}])から(?P<b>[{K}])までを(?P<c>[{K}])から(?P<d>[{K}])までと(?:し|する)$"),
            ("insert_sub_after", r"^(?P<loc>同号|第{N}条(?:の{N})*(?:第{N}項)?第{N}号(?:の{N})*|同項第{N}号(?:の{N})*)?(?P<a>[{K}])の次に次のように加え(?:る)?$"),
            ("insert_item_after", r"^(?P<loc>.+?)の(?P<side>次|前)に次の{N}号を加え(?:る)?$"),
            ("append_item", r"^(?P<loc>.+?)に次の(?:{N}号|各号)を加え(?:る)?$"),
            // 「同号に次のように加える」+ イロハ: 号の下の列記を足す
            ("append_subitems", r"^(?P<loc>.+?号)に次のように加え(?:る)?$"),
            ("renumber_para", r"^(?P<loc>.+?)中第(?P<p>{N})項を第(?P<q>{N})項と(?:し|する)$"),
            // 「同条中第三項を第五項とし、第二項を第四項とし」の続き（条は直前のもの）
            ("renumber_para_cont", r"^第(?P<p>{N})項を第(?P<q>{N})項と(?:し|する)$"),
            ("shift_paras", r"^(?:(?P<loc>.+?)中|同条)?第(?P<p>{N})項から第(?P<q>{N})項までを(?P<k>{N})項ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("renumber_para_same", r"^(?P<loc>同項|.+?第{N}項)を同条第(?P<q>{N})項と(?:し|する)$"),
            // 条ずれ: 「第六十一条を第六十四条とする」「同条を第六十三条とし」
            ("renumber_art", r"^(?:本則中|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+中)?(?P<loc>附則第{N}条(?:の{N})*|第{N}条(?:の{N})*|同条)を(?P<qs>附則)?第(?P<q>{N})条(?P<qb>(?:の{N})*)と(?:し|する)$"),
            ("shift_arts", r"^第(?P<p>{N})条から第(?P<q>{N})条までを(?P<k>{N})条ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("insert_para_after", r"^(?P<loc>.+?)の次に次の{N}項を加え(?:る)?$"),
            ("append_para", r"^(?P<loc>.+?)に次の{N}項を加え(?:る)?$"),
            ("append_art", r"^(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)に次の{N}条を加え(?:る)?$"),
            ("append_suppl_arts", r"^附則に次の(?:見出し及び)?{N}条を加え(?:る)?$"),
            ("append_table", r"^(?P<loc>.+?)に次の表を加え(?:る)?$"),
            ("delete_appdx", r"^(?P<list>別表(?:第{N})?(?:(?:及び|、)別表(?:第{N})?)*)を削(?:り|る)$"),
            ("append_containers", r"^(?P<path>本則|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)に次の{N}(?:編|章|節|款|目)を加え(?:る)?$"),
            ("insert_arts_before", r"^(?:(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+中)?(?P<loc>附則第{N}条(?:の{N})*|第{N}条(?:の{N})*|同条)の前に次の{N}条を加え(?:る)?$"),
            // 「第二章の次に次の二章を加える」「第一章中第五節の次に次の二節を加える」「第五節の次に…」（章は直前のもの）
            ("insert_containers_after", r"^(?:(?P<pre>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+|同章|同節|同款)の(?P<side>次|前)に次の{N}(?:編|章|節|款|目)を加え(?:る)?$"),
            // 「第三章を第五章とする」「第一章中第八節を第十節とし」「第六節を第八節とし」
            ("renumber_container", r"^(?:(?P<pre>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)を(?:同章|同節|同編|同款)?第(?P<q>{N})(?:編|章|節|款|目)(?P<qb>(?:の{N})*)と(?:し|する)$"),
            ("set_title", r"^題名を次のように改め(?:る)?$"),
            ("set_toc", r"^題名の次に次の目次を付する$"),
            ("set_container_title", r"^(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名を次のように改め(?:る)?$"),
            ("replace_whole", r"^(?P<loc>.+?)を次のように改め(?:る)?$"),
            ("delete", r"^(?P<loc>.+?)を削(?:り|る)$"),
            ("insert_arts_after", r"^(?:本則中|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+中)?(?P<loc>附則第{N}条(?:の{N})*|第{N}条(?:の{N})*|同条)の次に次の(?P<k>{N})条を加え(?:る)?$"),
            ("append_sentence", r"^(?P<loc>.+?)に(?:後段として次のように|次のただし書を)加え(?:る)?$"),
        ]
        .iter()
        .map(|(n, s)| (*n, re(s)))
        .collect()
    });
    let mut ante = Ante {
        article: None,
        paragraph: None,
        item: None,
        toc: false,
        container: Vec::new(),
        part: None,
        locs: Vec::new(),
        suppl: false,
        sub: None,
        appdx: None,
    };
    let mut ops = Vec::new();
    let segs: Vec<String> = split_segments(line);
    // 断片ごとの操作の始まり（読めない断片が出たとき、前の断片とつないで読み直すのに使う）
    let mut op_start: Vec<usize> = Vec::new();
    let mut skip_until = 0usize;
    for (si, seg) in segs.iter().enumerate() {
        if si < skip_until {
            continue;
        }
        op_start.push(ops.len());
        let seg = seg.trim();
        // 字句そのものに「」に、「」が入る読替え規定の書き換えは「、」で切れてしまう。
        // 読めない断片は、前の断片（字句の操作）とつないで、改め・加え・削りで終わるところまで緩く読み直す
        let try_merge = |ops: &mut Vec<Op>, ante: &mut Ante| -> Option<usize> {
            for start in (0..si).rev() {
                if !segs[start].contains('「') {
                    break;
                }
                for end in si..segs.len() {
                    let merged = segs[start..=end].join("、");
                    let ends = ["改め", "改める", "加え", "加える", "削り", "削る"]
                        .iter()
                        .any(|e| merged.ends_with(e));
                    if !ends {
                        continue;
                    }
                    let mut a2 = Ante {
                        article: ante.article.clone(),
                        paragraph: ante.paragraph,
                        item: ante.item.clone(),
                        toc: ante.toc,
                        container: ante.container.clone(),
                        part: ante.part,
                        locs: ante.locs.clone(),
                        suppl: ante.suppl,
                        sub: ante.sub.clone(),
                        appdx: ante.appdx.clone(),
                    };
                    // 「「A」を「B」に、「C」を「D」に改め」の列挙: 「」に、「」で区切ってから、各片を緩く読む
                    // （B や D の中の「」が釣り合わなくても、最初の「」を「」で A と B が分かれる）
                    if let Some(v) = parse_phrase_list_loose(&merged, &mut a2) {
                        ops.truncate(op_start[start]);
                        ops.extend(v);
                        return Some(end + 1);
                    }
                    break;
                }
            }
            None
        };
        // 字句の置換・追加・削除は正規表現でなく手で読む（置換先に「」が入れ子になることがある）。
        // ただし「第百条第三項及び第百三条第四項中」の複数位置は下の規則で展開する。見出しの中は別
        let loc_part = seg.split("中「").next().unwrap_or("");
        let listed =
            loc_part.contains("及び") || loc_part.contains('、') || loc_part.contains("まで");
        if (!listed || seg.starts_with('「') || seg.ends_with("削り") || seg.ends_with("削る"))
            && (!loc_part.contains("見出し") || loc_part.ends_with("（見出しを含む。）"))
            && !loc_part.ends_with("名")
        {
            if let Some(PhraseOps(v)) = parse_phrase_op(seg, &mut ante)? {
                ops.extend(v);
                continue;
            }
            if let Some(PhraseOps(v)) = parse_phrase_op_loose(seg, &mut ante)? {
                ops.extend(v);
                continue;
            }
        }
        let mut matched = false;
        for (name, r) in rules.iter() {
            let Some(c) = r.captures(seg) else { continue };
            matched = true;
            let g = |n: &str| {
                c.name(n)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default()
            };
            let num = |n: &str| kanji_to_u32(&g(n)).unwrap_or(0);
            // 「第百条第三項及び第百三条第四項中「A」を「B」に改める」: 位置ごとに同じ操作
            if (*name == "replace" || *name == "insert_phrase")
                && (g("loc").contains("及び")
                    || g("loc").contains('、')
                    || g("loc").contains("まで"))
            {
                // 「第五十四条の見出し並びに同条第一項及び第二項中」: 見出しは見出しの置換に
                let mut rest_tokens: Vec<String> = Vec::new();
                static TROW_LIST: OnceLock<Regex> = OnceLock::new();
                let trow_list = TROW_LIST.get_or_init(|| re(r"^(?P<loc>.+?)の表(?P<row>.+?)の項$"));
                // 「A」の下に「B」を加える は、字句の側では「A」を「AB」に改めるのと同じ
                let (from, to) = if *name == "replace" {
                    (g("a"), g("b"))
                } else {
                    (g("a"), format!("{}{}", g("a"), g("b")))
                };
                for tok in g("loc")
                    .split("並びに")
                    .flat_map(|x| x.split("及び"))
                    .flat_map(|x| x.split('、'))
                {
                    if tok == "目次" {
                        ops.push(Op::ReplaceToc {
                            from: from.clone(),
                            to: to.clone(),
                        });
                    } else if let Some(c) = trow_list.captures(tok) {
                        // 「第三十八条の表第七十条第二項の項及び第五十一条第六項の表第七十条第二項の項中」
                        let l = loc(&c["loc"], &mut ante)?;
                        ops.push(Op::ReplaceTableRow {
                            at: l,
                            row: c["row"].to_string(),
                            from: from.clone(),
                            to: to.clone(),
                        });
                    } else if let Some(base) = tok.strip_suffix("（見出しを含む。）") {
                        // 「第七十六条の二（見出しを含む。）及び第七十七条中」: 見出しと本文の両方
                        let l = loc(base, &mut ante)?;
                        ops.push(Op::ReplaceCaption {
                            article: l.article,
                            from: from.clone(),
                            to: to.clone(),
                        });
                        rest_tokens.push(base.to_string());
                    } else if let Some(base) = tok.strip_suffix("の見出し") {
                        let l = loc(base, &mut ante)?;
                        ops.push(Op::ReplaceCaption {
                            article: l.article,
                            from: from.clone(),
                            to: to.clone(),
                        });
                    } else if let Some(base) = ["の章名", "の節名", "の款名", "の編名", "の目名"]
                        .iter()
                        .find_map(|s| tok.strip_suffix(s))
                    {
                        ops.push(Op::ReplaceContainerTitle {
                            path: container_path(base),
                            from: from.clone(),
                            to: to.clone(),
                        });
                    } else {
                        rest_tokens.push(tok.to_string());
                    }
                }
                let ats = expand_locs(&rest_tokens.join("及び"), &mut ante)?;
                ante.locs = ats.clone();
                for at in ats {
                    ops.push(if *name == "replace" {
                        Op::Replace {
                            at,
                            from: g("a"),
                            to: g("b"),
                        }
                    } else {
                        Op::InsertAfterPhrase {
                            at,
                            anchor: g("a"),
                            text: g("b"),
                        }
                    });
                }
                break;
            }
            let op = match *name {
                "toc" => {
                    ante.toc = true;
                    Op::ReplaceToc {
                        from: g("a"),
                        to: g("b"),
                    }
                }
                "caption_replace" => Op::ReplaceCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    from: g("a"),
                    to: g("b"),
                },
                "caption_insert" => Op::ReplaceCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    from: g("a"),
                    to: format!("{}{}", g("a"), g("b")),
                },
                "caption_delete_phrase" => Op::ReplaceCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    from: g("a"),
                    to: String::new(),
                },
                "caption_set" => Op::SetCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    text: g("a"),
                },
                "caption_attach" => Op::AttachCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    text: g("a"),
                },
                "caption_delete" => Op::DeleteCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                },
                "replace" => Op::Replace {
                    at: loc(&g("loc"), &mut ante)?,
                    from: g("a"),
                    to: g("b"),
                },
                "insert_phrase" => Op::InsertAfterPhrase {
                    at: loc(&g("loc"), &mut ante)?,
                    anchor: g("a"),
                    text: g("b"),
                },
                "replace_cont" if ante.toc => Op::ReplaceToc {
                    from: g("a"),
                    to: g("b"),
                },
                "replace_cont" => Op::Replace {
                    at: ante_loc(&ante, seg)?,
                    from: g("a"),
                    to: g("b"),
                },
                "insert_phrase_cont" => Op::InsertAfterPhrase {
                    at: ante_loc(&ante, seg)?,
                    anchor: g("a"),
                    text: g("b"),
                },
                "renumber_sub" | "shift_sub" | "insert_sub_after" => {
                    let l = g("loc");
                    let l = l.trim_end_matches('中');
                    let mut at = if l.is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(l, &mut ante)?
                    };
                    at.sub = None;
                    at.part = None;
                    if at.item.is_none() {
                        return Err(ParseError::NoAntecedent(seg.to_string()));
                    }
                    match *name {
                        "renumber_sub" => Op::RenumberSubitem {
                            at,
                            from: g("a"),
                            to: g("b"),
                        },
                        "shift_sub" => {
                            let by = kana_index(&g("c")) as i32 - kana_index(&g("a")) as i32;
                            Op::ShiftSubitems {
                                at,
                                from: g("a"),
                                to: g("b"),
                                by,
                            }
                        }
                        _ => Op::InsertSubitemAfter {
                            at,
                            after: g("a"),
                            text: Vec::new(),
                        },
                    }
                }
                "renumber_item" => {
                    let at = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(g("loc").trim_end_matches('中'), &mut ante)?
                    };
                    let item_num = |n: &str, branch: &str| {
                        let mut s = kanji_to_u32(n).unwrap_or(0).to_string();
                        for b in branch.split('の').filter(|x| !x.is_empty()) {
                            s.push('_');
                            s.push_str(&kanji_to_u32(b).unwrap_or(0).to_string());
                        }
                        s
                    };
                    let from = if g("same").is_empty() {
                        item_num(&g("p"), &g("pb"))
                    } else {
                        ante.item
                            .clone()
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?
                    };
                    // 「同号を同項第二十一号とし、同号の次に」の「同号」は番号を変えた後の号
                    ante.item = Some(item_num(&g("q"), &g("qb")));
                    Op::RenumberItem {
                        at: Loc { item: None, ..at },
                        from,
                        to: item_num(&g("q"), &g("qb")),
                    }
                }
                "shift_items" => {
                    let at = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(g("loc").trim_end_matches('中'), &mut ante)?
                    };
                    let k = num("k") as i32;
                    Op::ShiftItems {
                        at: Loc { item: None, ..at },
                        from: num("p"),
                        to: num("q"),
                        by: if g("dir") == "下げ" { k } else { -k },
                    }
                }
                "insert_item_after" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let after = l
                        .item
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    if g("side") == "前" {
                        Op::InsertItemBefore {
                            at: Loc { item: None, ..l },
                            before: after,
                            text: Vec::new(),
                        }
                    } else {
                        Op::InsertItemAfter {
                            at: Loc { item: None, ..l },
                            after,
                            text: Vec::new(),
                        }
                    }
                }
                "append_item" | "append_subitems" => Op::AppendItem {
                    at: loc(&g("loc"), &mut ante)?,
                    text: Vec::new(),
                },
                "renumber_para" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    ante.paragraph = Some(num("p"));
                    Op::RenumberParagraph {
                        article: l.article,
                        from: ParaRef::Num(num("p")),
                        to: num("q"),
                    }
                }
                "renumber_para_cont" => {
                    let article = ante
                        .article
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    ante.paragraph = Some(num("p"));
                    Op::RenumberParagraph {
                        article,
                        from: ParaRef::Num(num("p")),
                        to: num("q"),
                    }
                }
                "shift_paras" => {
                    let article = ante
                        .article
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    let by = num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 };
                    Op::ShiftParagraphs {
                        article,
                        from: num("p"),
                        to: num("q"),
                        by,
                    }
                }
                "renumber_para_same" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let from = l
                        .paragraph
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    Op::RenumberParagraph {
                        article: l.article,
                        from,
                        to: num("q"),
                    }
                }
                "renumber_art" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let to = art_num(&format!("第{}条{}", g("q"), g("qb")));
                    // 以後の「同条」は新しい番号
                    ante.article = Some(to.clone());
                    Op::RenumberArticle {
                        from: l.article,
                        to,
                        suppl: l.suppl || g("qs") == "附則",
                    }
                }
                "shift_arts" => {
                    let by = num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 };
                    Op::ShiftArticles {
                        from: num("p"),
                        to: num("q"),
                        by,
                    }
                }
                "insert_para_after" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let after = l
                        .paragraph
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    Op::InsertParagraphAfter {
                        article: l.article,
                        after,
                        text: Vec::new(),
                    }
                }
                "append_para" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    Op::AppendParagraph {
                        article: l.article,
                        text: Vec::new(),
                    }
                }
                "append_art" => Op::AppendArticle {
                    path: container_path(&g("path")),
                    text: Vec::new(),
                },
                "append_suppl_arts" => Op::AppendSupplArticles { text: Vec::new() },
                "append_table" => Op::AppendTable {
                    at: loc(&g("loc"), &mut ante)?,
                    text: Vec::new(),
                },
                "delete_appdx" => Op::DeleteAppdx {
                    tables: g("list")
                        .split("及び")
                        .flat_map(|x| x.split('、'))
                        .map(str::to_string)
                        .collect(),
                },
                "append_containers" => Op::AppendContainers {
                    path: if g("path") == "本則" {
                        Vec::new()
                    } else {
                        container_path(&g("path"))
                    },
                    text: Vec::new(),
                },
                "insert_arts_before" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    Op::InsertArticleBefore {
                        before: l.article,
                        text: Vec::new(),
                        suppl: l.suppl,
                    }
                }
                "container_title" => Op::ReplaceContainerTitle {
                    path: container_path(&g("path")),
                    from: g("a"),
                    to: g("b"),
                },
                "set_title" => Op::SetTitle { text: Vec::new() },
                "set_toc" => Op::SetToc { text: Vec::new() },
                "set_container_title" => Op::SetContainerTitle {
                    path: container_path(&g("path")),
                    text: Vec::new(),
                },
                "insert_containers_after" => {
                    let path = if g("path").starts_with('同') {
                        ante.container.clone()
                    } else {
                        container_path_with_ante(&g("pre"), &g("path"), &mut ante)
                    };
                    if g("side") == "前" {
                        Op::InsertContainersBefore {
                            path,
                            text: Vec::new(),
                        }
                    } else {
                        Op::InsertContainersAfter {
                            path,
                            text: Vec::new(),
                        }
                    }
                }
                "renumber_container" => {
                    let path = container_path_with_ante(&g("pre"), &g("path"), &mut ante);
                    // 「同節」は番号を変えた後の節
                    let mut after = path.clone();
                    let to = container_num(&g("q"), &g("qb"));
                    if let Some(last) = after.last_mut() {
                        last.1 = to.clone();
                    }
                    ante.container = after;
                    Op::RenumberContainer { path, to }
                }
                "replace_whole" => {
                    // 「第九条第二項及び第三項を次のように改める」: 位置の列挙は最初の項に付け、内容の番号で各項に当てる
                    let l = g("loc");
                    let l = match l.split_once("及び") {
                        Some((first, _)) if l.contains('項') && !l.ends_with('号') => {
                            first.to_string()
                        }
                        _ => l,
                    };
                    // 「同条後段を次のように改める」: 項の後段（前段）の差し替え
                    if let Some(part) = [
                        ("後段", SentencePart::Back),
                        ("前段", SentencePart::Front),
                        ("ただし書", SentencePart::Proviso),
                    ]
                    .iter()
                    .find_map(|(sfx, p)| l.strip_suffix(sfx).map(|_| *p))
                    {
                        let base = l
                            .trim_end_matches("後段")
                            .trim_end_matches("前段")
                            .trim_end_matches("ただし書");
                        Op::ReplaceSentencePart {
                            at: loc(base, &mut ante)?,
                            part,
                            text: Vec::new(),
                        }
                    } else if let Some(base) = l.strip_suffix("各号") {
                        Op::ReplaceItems {
                            at: loc(base, &mut ante)?,
                            text: Vec::new(),
                        }
                    } else if l.ends_with(['章', '節', '款', '編', '目']) && l.starts_with('第')
                    {
                        // 「第四章及び第五章を次のように改める」
                        let paths = l
                            .split("及び")
                            .flat_map(|x| x.split('、'))
                            .map(container_path)
                            .collect();
                        Op::ReplaceContainers {
                            paths,
                            text: Vec::new(),
                        }
                    } else if l.trim_end_matches("まで").ends_with('号')
                        && (l.contains("及び") || l.contains('、') || l.contains("まで"))
                    {
                        // 「第百条の二第三号及び第四号を次のように改める」「第九十六条第六号から第八号までを次のように改める」
                        let range = l.contains("まで");
                        let locs = expand_locs(&l, &mut ante)?;
                        let items: Vec<String> =
                            locs.iter().filter_map(|x| x.item.clone()).collect();
                        let mut at = locs
                            .into_iter()
                            .next()
                            .ok_or_else(|| ParseError::Unrecognized(l.clone()))?;
                        at.item = None;
                        Op::ReplaceItemSet {
                            at,
                            items,
                            range,
                            text: Vec::new(),
                        }
                    } else if l.contains("から") && l.ends_with("条まで") {
                        // 「第七条から第九条までを次のように改める」: 条の範囲をまとめて
                        let articles = expand_locs(&l, &mut ante)?
                            .into_iter()
                            .map(|x| x.article)
                            .collect();
                        Op::ReplaceArticles {
                            articles,
                            text: Vec::new(),
                        }
                    } else if l.contains("及び") && !l.contains('項') && l.ends_with('条') {
                        // 「第百二条及び第百三条を次のように改める」: 複数の条をまとめて（「削除」の条に）
                        let articles = expand_locs(&l, &mut ante)?
                            .into_iter()
                            .map(|x| x.article)
                            .collect();
                        Op::ReplaceArticles {
                            articles,
                            text: Vec::new(),
                        }
                    } else {
                        let at = loc(&l, &mut ante)?;
                        if at.item.is_some() {
                            Op::ReplaceItem {
                                at,
                                text: Vec::new(),
                            }
                        } else if at.paragraph.is_some() {
                            Op::ReplaceParagraph {
                                at,
                                text: Vec::new(),
                            }
                        } else {
                            Op::ReplaceArticle {
                                article: at.article,
                                text: Vec::new(),
                            }
                        }
                    }
                }
                "delete" => {
                    let mut l = g("loc");
                    // 「第八条第二項中第三号から第六号までを削り」: 「X中」は号の列挙の入れ物。先に先行詞にして、残りを号として読む
                    if let Some((ctx, rest)) = l.split_once('中') {
                        if rest.starts_with('第') || rest.starts_with("同号") {
                            let mut probe = ante.clone();
                            if loc(ctx, &mut probe).is_ok() {
                                loc(ctx, &mut ante)?;
                                l = rest.to_string();
                            }
                        }
                    }
                    // 章名・節名、章・節の範囲、条の範囲の列挙: 「第三章の章名及び同章第一節の節名を削る」
                    //「第百四条から第百五条の二まで及び第三章第二節から第五節までを削る」
                    static CRANGE: OnceLock<Regex> = OnceLock::new();
                    let crange = CRANGE.get_or_init(|| {
                        re(r"^(?P<pre>(?:第{N}(?:編|章|節|款|目))*)第(?P<p>{N})(?P<k>編|章|節|款|目)から第(?P<q>{N})(?:編|章|節|款|目)までの?$")
                    });
                    let tokens: Vec<&str> = l
                        .split("並びに")
                        .flat_map(|x| x.split("及び"))
                        .flat_map(|x| x.split('、'))
                        .collect();
                    static CSINGLE: OnceLock<Regex> = OnceLock::new();
                    let csingle = CSINGLE.get_or_init(|| {
                        re(r"^(?P<pre>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)*)第(?P<p>{N})(?P<k>編|章|節|款|目)$")
                    });
                    let structural = tokens.iter().any(|t| {
                        t.ends_with('名')
                            || crange.is_match(t)
                            || csingle.is_match(t)
                            || t.contains("まで")
                    }) || tokens.len() > 1;
                    if structural {
                        let mut last_container: Vec<(lawean_source::ContainerKind, String)> =
                            Vec::new();
                        for t in tokens {
                            if let Some(base) = t
                                .strip_suffix("の章名")
                                .or_else(|| t.strip_suffix("の節名"))
                                .or_else(|| t.strip_suffix("の款名"))
                                .or_else(|| t.strip_suffix("の編名"))
                                .or_else(|| t.strip_suffix("の目名"))
                            {
                                // 「同章第一節」は直前の章の中
                                let path = if let Some(rest) = base.strip_prefix("同章") {
                                    let mut p = last_container.clone();
                                    p.truncate(1);
                                    p.extend(container_path(rest));
                                    p
                                } else {
                                    container_path(base)
                                };
                                last_container = path.clone();
                                ops.push(Op::DeleteContainerTitle { path });
                            } else if let Some(c) =
                                crange.captures(t).or_else(|| csingle.captures(t))
                            {
                                let kind = match &c["k"] {
                                    "編" => lawean_source::ContainerKind::Part,
                                    "章" => lawean_source::ContainerKind::Chapter,
                                    "節" => lawean_source::ContainerKind::Section,
                                    "款" => lawean_source::ContainerKind::Subsection,
                                    _ => lawean_source::ContainerKind::Division,
                                };
                                let from = kanji_to_u32(&c["p"]).unwrap_or(0);
                                ops.push(Op::DeleteContainers {
                                    path: container_path(&c["pre"]),
                                    kind,
                                    from,
                                    to: c
                                        .name("q")
                                        .and_then(|q| kanji_to_u32(q.as_str()))
                                        .unwrap_or(from),
                                });
                            } else {
                                for at in expand_locs(t, &mut ante)? {
                                    ops.push(Op::Delete { at });
                                }
                            }
                        }
                        break;
                    }
                    match [
                        ("後段", SentencePart::Back),
                        ("前段", SentencePart::Front),
                        ("ただし書", SentencePart::Proviso),
                    ]
                    .iter()
                    .find_map(|(sfx, p)| l.strip_suffix(sfx).map(|base| (base.to_string(), *p)))
                    {
                        Some((base, part)) => Op::DeleteSentencePart {
                            at: loc(&base, &mut ante)?,
                            part,
                        },
                        None => Op::Delete {
                            at: loc(&l, &mut ante)?,
                        },
                    }
                }
                "insert_arts_after" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    Op::InsertArticleAfter {
                        after: l.article,
                        text: Vec::new(),
                        suppl: l.suppl,
                    }
                }
                _ => Op::AppendSentence {
                    at: loc(&g("loc"), &mut ante)?,
                    text: Vec::new(),
                },
            };
            ops.push(op);
            break;
        }
        if !matched {
            if let Some(next) = try_merge(&mut ops, &mut ante) {
                skip_until = next;
                continue;
            }
            return Err(ParseError::Unrecognized(seg.to_string()));
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(n: u32) -> ArticleNum {
        ArticleNum::Single {
            base: n,
            branch: vec![],
        }
    }

    #[test]
    fn simple_forms() {
        assert_eq!(
            parse_instruction("目次中「第六十条」を「第六十一条」に改める。").unwrap(),
            [Op::ReplaceToc {
                from: "第六十条".into(),
                to: "第六十一条".into()
            }]
        );
        assert_eq!(
            parse_instruction("第四十二条第一項中「第四十条」の下に「、第四十二条の二」を加える。")
                .unwrap(),
            [Op::InsertAfterPhrase {
                at: Loc {
                    article: a(42),
                    paragraph: Some(ParaRef::Num(1)),
                    item: None,
                    part: None,
                    suppl: false,
                    sub: None,
                },
                anchor: "第四十条".into(),
                text: "、第四十二条の二".into()
            }]
        );
        assert_eq!(
            parse_instruction("第二十二条に次の一項を加える。").unwrap(),
            [Op::AppendParagraph {
                article: a(22),
                text: vec![]
            }]
        );
        assert_eq!(
            parse_instruction("第四章に次の一条を加える。").unwrap(),
            [Op::AppendArticle {
                path: vec![(lawean_source::ContainerKind::Chapter, "4".to_string())],
                text: vec![]
            }]
        );
        assert_eq!(
            parse_instruction("第三十八条第一項の次に次の一項を加える。").unwrap(),
            [Op::InsertParagraphAfter {
                article: a(38),
                after: ParaRef::Num(1),
                text: vec![]
            }]
        );
        assert_eq!(
            parse_instruction("第六十一条を次のように改める。").unwrap(),
            [Op::ReplaceArticle {
                article: a(61),
                text: vec![]
            }]
        );
    }

    #[test]
    fn compound_renumbering_sentence() {
        let ops = parse_instruction(
            "第三十八条中第七項を第九項とし、第四項から第六項までを二項ずつ繰り下げ、同条第三項中「前項」を「第三項」に改め、同項を同条第五項とし、同条第二項中「前項」を「第一項」に改め、同項を同条第三項とし、同項の次に次の一項を加える。",
        )
        .unwrap();
        assert_eq!(
            ops,
            [
                Op::RenumberParagraph {
                    article: a(38),
                    from: ParaRef::Num(7),
                    to: 9
                },
                Op::ShiftParagraphs {
                    article: a(38),
                    from: 4,
                    to: 6,
                    by: 2
                },
                Op::Replace {
                    at: Loc {
                        article: a(38),
                        paragraph: Some(ParaRef::Num(3)),
                        item: None,
                        part: None,
                        suppl: false,
                        sub: None,
                    },
                    from: "前項".into(),
                    to: "第三項".into()
                },
                Op::RenumberParagraph {
                    article: a(38),
                    from: ParaRef::Num(3),
                    to: 5
                },
                Op::Replace {
                    at: Loc {
                        article: a(38),
                        paragraph: Some(ParaRef::Num(2)),
                        item: None,
                        part: None,
                        suppl: false,
                        sub: None,
                    },
                    from: "前項".into(),
                    to: "第一項".into()
                },
                Op::RenumberParagraph {
                    article: a(38),
                    from: ParaRef::Num(2),
                    to: 3
                },
                Op::InsertParagraphAfter {
                    article: a(38),
                    after: ParaRef::Num(2),
                    text: vec![]
                },
            ]
        );
    }

    #[test]
    fn quoted_commas_do_not_split() {
        let ops = parse_instruction("第一条中「甲、乙」を「丙」に改める。").unwrap();
        assert!(matches!(&ops[0], Op::Replace { from, .. } if from == "甲、乙"));
    }

    /// 「同条第四項及び第六項中「A」を削る」: 位置の列挙に字句の削除（令5-79）
    #[test]
    fn phrase_delete_over_listed_locations() {
        let ops = parse_instruction(
            "第四十三条第二項中「甲」を「乙」に改め、同条第四項及び第六項中「、丙」を削る。",
        )
        .unwrap();
        assert_eq!(ops.len(), 3);
        assert!(
            matches!(&ops[1], Op::Replace { at, from, to } if at.paragraph == Some(ParaRef::Num(4)) && from == "、丙" && to.is_empty())
        );
        assert!(matches!(&ops[2], Op::Replace { at, .. } if at.paragraph == Some(ParaRef::Num(6))));
    }

    /// 整備法の体裁: 条の見出し「（X法の一部改正）」と章の見出し「第二章　文部科学省関係」は読み飛ばす。
    /// 「次の一章を加える」の内容の章名（同じ字面）は内容として残す
    #[test]
    fn captions_and_chapter_headings_of_the_amending_law_are_skipped() {
        let t = "　　　第一章　総務省関係
　（甲法の一部改正）
第一条　甲法（昭和二十二年法律第一号）の一部を次のように改正する。
　　第一条中「甲」を「乙」に改める。
　　第三章の次に次の一章を加える。
　　　　第四章　雑則
　第九条　削除
　　　第二章　文部科学省関係
　（丙法の一部改正）
第二条　丙法（昭和二十二年法律第二号）の一部を次のように改正する。
　　第二条中「丙」を「丁」に改める。";
        let units = parse_units(t).unwrap();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0].instructions.len(), 2);
        // 加える章の章名は内容
        assert!(
            matches!(&units[0].instructions[1].ops[0], Op::InsertContainersAfter { text, .. } if text.len() == 2 && text[0] == "第四章　雑則")
        );
        assert_eq!(units[1].instructions.len(), 1);
    }
}
