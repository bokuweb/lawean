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
    Regex::new(&s.replace("{N}", N)).unwrap()
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
    for raw in text.lines() {
        let indent = raw
            .chars()
            .take_while(|c| *c == '\u{3000}' || *c == ' ')
            .count();
        let line = raw.trim_start_matches(['\u{3000}', ' ']).trim_end();
        if line.is_empty() || line.starts_with('（') && indent == 0 {
            continue;
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
    for c in s.trim_end_matches('。').chars() {
        match c {
            '「' => q += 1,
            // 余る閉じ括弧は字句の一部（take_quoted と同じ扱い）
            '」' => q = (q - 1).max(0),
            '（' => p += 1,
            '）' => p -= 1,
            '、' if q == 0 && p == 0 => {
                out.push(std::mem::take(&mut cur));
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
        if loc_only {
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
            match (la.paragraph.clone(), lb.paragraph.clone()) {
                (Some(ParaRef::Num(p)), Some(ParaRef::Num(q))) => {
                    for n in p..=q {
                        out.push(Loc {
                            article: la.article.clone(),
                            paragraph: Some(ParaRef::Num(n)),
                            item: None,
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
                }),
                _ => return Err(ParseError::Unrecognized(tok.to_string())),
            }
            continue;
        }
        out.push(loc(tok, ante)?);
    }
    Ok(out)
}

struct Ante {
    article: Option<ArticleNum>,
    paragraph: Option<u32>,
    item: Option<String>,
}

pub(crate) fn art_num(s: &str) -> ArticleNum {
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
        re(r"^(?:(第{N}条(?:の{N})*)|同条)?(?:第({N})項|同項)?(?:第({N}号(?:の{N})*)|同号)?[イロハニホヘトチリヌルヲワカヨタレソツネナラム]?(?:ただし書|各号列記以外の部分|本文)?$")
    });
    let Some(c) = r.captures(s) else {
        return Err(ParseError::Unrecognized(s.to_string()));
    };
    let article = match c.get(1) {
        Some(a) => {
            let n = art_num(a.as_str());
            ante.article = Some(n.clone());
            n
        }
        None => ante
            .article
            .clone()
            .ok_or_else(|| ParseError::NoAntecedent(s.to_string()))?,
    };
    let paragraph = if let Some(p) = c.get(2) {
        let n = kanji_to_u32(p.as_str()).unwrap();
        ante.paragraph = Some(n);
        Some(ParaRef::Num(n))
    } else if s.ends_with("同項") || s == "同項" {
        Some(ParaRef::Num(
            ante.paragraph
                .ok_or_else(|| ParseError::NoAntecedent(s.to_string()))?,
        ))
    } else {
        if c.get(1).is_some() {
            ante.paragraph = None;
        }
        None
    };
    let item = match c.get(3) {
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
        None if s.ends_with("同号") => ante.item.clone(),
        None => {
            if c.get(1).is_some() || c.get(2).is_some() {
                ante.item = None;
            }
            None
        }
    };
    Ok(Loc {
        article,
        paragraph,
        item,
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
fn parse_phrase_op(seg: &str, ante: &mut Ante) -> Result<Option<Op>, ParseError> {
    let (loc_part, rest) = match seg.find("中「") {
        Some(i) if !seg.starts_with('「') => (Some(&seg[..i]), &seg[i + '中'.len_utf8()..]),
        _ if seg.starts_with('「') => (None, seg),
        _ => return Ok(None),
    };
    let Some((a, rest)) = take_quoted(rest) else {
        return Ok(None);
    };
    if loc_part == Some("目次") {
        // 目次は正規表現の規則に任せる
        return Ok(None);
    }
    let at = match loc_part {
        Some(l) => loc(l, ante)?,
        None => ante_loc(ante, seg)?,
    };
    if let Some(rest) = rest.strip_prefix("を") {
        if rest == "削り" || rest == "削る" {
            return Ok(Some(Op::Replace {
                at,
                from: a,
                to: String::new(),
            }));
        }
        if let Some((b, tail)) = take_quoted(rest) {
            if matches!(tail, "に" | "に改め" | "に改める") {
                return Ok(Some(Op::Replace { at, from: a, to: b }));
            }
        }
        return Ok(None);
    }
    if let Some(rest) = rest.strip_prefix("の下に") {
        if let Some((b, tail)) = take_quoted(rest) {
            if matches!(tail, "を" | "を加え" | "を加える") {
                return Ok(Some(Op::InsertAfterPhrase {
                    at,
                    anchor: a,
                    text: b,
                }));
            }
        }
    }
    Ok(None)
}

/// 直前の位置（条・項）を引き継ぐ
fn ante_loc(ante: &Ante, seg: &str) -> Result<Loc, ParseError> {
    Ok(Loc {
        article: ante
            .article
            .clone()
            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?,
        paragraph: ante.paragraph.map(ParaRef::Num),
        item: ante.item.clone(),
    })
}

pub fn parse_instruction(line: &str) -> Result<Vec<Op>, ParseError> {
    static RULES: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            ("toc", r"^目次中「(?P<a>.+)」を「(?P<b>.+)」に改める$"),
            // 見出しは「中」の規則より先に（「第四十二条の見出し中「A」を「B」に改め」）
            ("caption_replace", r"^(?P<loc>第{N}条(?:の{N})*|同条)の見出し中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("caption_set", r"^(?P<loc>第{N}条(?:の{N})*|同条)の見出しを「(?P<a>.+?)」に(?:改め(?:る)?)?$"),
            // 「改める」「加える」が付かない形は、同じ文の中で「、」で連なる列挙の途中（「A」を「B」に、「C」を「D」に改める）
            ("replace", r"^(?P<loc>.+?)中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("insert_phrase", r"^(?P<loc>.+?)中「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            // 位置を省いた続き: 「第X中「A」の下に「B」を加え、「C」を「D」に改める」の後半。直前の位置を引き継ぐ
            ("replace_cont", r"^「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("insert_phrase_cont", r"^「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            ("renumber_para", r"^(?P<loc>.+?)中第(?P<p>{N})項を第(?P<q>{N})項と(?:し|する)$"),
            ("shift_paras", r"^(?:(?P<loc>.+?)中|同条)?第(?P<p>{N})項から第(?P<q>{N})項までを(?P<k>{N})項ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("renumber_para_same", r"^(?P<loc>同項|.+?第{N}項)を同条第(?P<q>{N})項と(?:し|する)$"),
            // 条ずれ: 「第六十一条を第六十四条とする」「同条を第六十三条とし」
            ("renumber_art", r"^(?P<loc>第{N}条(?:の{N})*|同条)を第(?P<q>{N})条(?P<qb>(?:の{N})*)と(?:し|する)$"),
            ("shift_arts", r"^第(?P<p>{N})条から第(?P<q>{N})条までを(?P<k>{N})条ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("insert_para_after", r"^(?P<loc>.+?)の次に次の{N}項を加え(?:る)?$"),
            ("append_para", r"^(?P<loc>.+?)に次の{N}項を加え(?:る)?$"),
            ("append_art", r"^第(?P<ch>{N})章に次の一条を加え(?:る)?$"),
            ("replace_whole", r"^(?P<loc>.+?)を次のように改め(?:る)?$"),
            ("delete", r"^(?P<loc>.+?)を削(?:り|る)$"),
            ("insert_arts_after", r"^(?P<loc>第{N}条(?:の{N})*|同条)の次に次の(?P<k>{N})条を加え(?:る)?$"),
            ("append_sentence", r"^(?P<loc>.+?)に後段として次のように加え(?:る)?$"),
        ]
        .iter()
        .map(|(n, s)| (*n, re(s)))
        .collect()
    });
    let mut ante = Ante {
        article: None,
        paragraph: None,
        item: None,
    };
    let mut ops = Vec::new();
    for seg in split_segments(line) {
        let seg = seg.trim();
        // 字句の置換・追加・削除は正規表現でなく手で読む（置換先に「」が入れ子になることがある）。
        // ただし「第百条第三項及び第百三条第四項中」の複数位置は下の規則で展開する。見出しの中は別
        let loc_part = seg.split("中「").next().unwrap_or("");
        let listed =
            loc_part.contains("及び") || loc_part.contains('、') || loc_part.contains("まで");
        if (!listed || seg.starts_with('「')) && !loc_part.contains("見出し") {
            if let Some(op) = parse_phrase_op(seg, &mut ante)? {
                ops.push(op);
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
                for at in expand_locs(&g("loc"), &mut ante)? {
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
                "toc" => Op::ReplaceToc {
                    from: g("a"),
                    to: g("b"),
                },
                "caption_replace" => Op::ReplaceCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    from: g("a"),
                    to: g("b"),
                },
                "caption_set" => Op::SetCaption {
                    article: loc(&g("loc"), &mut ante)?.article,
                    text: g("a"),
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
                "renumber_para" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    ante.paragraph = Some(num("p"));
                    Op::RenumberParagraph {
                        article: l.article,
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
                    let from = loc(&g("loc"), &mut ante)?.article;
                    let to = art_num(&format!("第{}条{}", g("q"), g("qb")));
                    // 以後の「同条」は新しい番号
                    ante.article = Some(to.clone());
                    Op::RenumberArticle { from, to }
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
                    chapter: num("ch"),
                    text: Vec::new(),
                },
                "replace_whole" => {
                    let l = g("loc");
                    // 「同条後段を次のように改める」: 項の後段（前段）の差し替え
                    if let Some(part) =
                        [("後段", SentencePart::Back), ("前段", SentencePart::Front)]
                            .iter()
                            .find_map(|(sfx, p)| l.strip_suffix(sfx).map(|_| *p))
                    {
                        let base = l.trim_end_matches("後段").trim_end_matches("前段");
                        Op::ReplaceSentencePart {
                            at: loc(base, &mut ante)?,
                            part,
                            text: Vec::new(),
                        }
                    } else {
                        Op::ReplaceArticle {
                            article: loc(&l, &mut ante)?.article,
                            text: Vec::new(),
                        }
                    }
                }
                "delete" => Op::Delete {
                    at: loc(&g("loc"), &mut ante)?,
                },
                "insert_arts_after" => Op::InsertArticleAfter {
                    after: loc(&g("loc"), &mut ante)?.article,
                    text: Vec::new(),
                },
                _ => Op::AppendSentence {
                    at: loc(&g("loc"), &mut ante)?,
                    text: Vec::new(),
                },
            };
            ops.push(op);
            break;
        }
        if !matched {
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
                chapter: 4,
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
}
