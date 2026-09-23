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
    /// 読めなかった文（改め文の 1 文）と理由
    #[error("{err}（{line}）")]
    InSentence { line: String, err: Box<ParseError> },
}

impl ParseError {
    /// 文の包みを外した理由
    pub fn cause(&self) -> &ParseError {
        match self {
            ParseError::InSentence { err, .. } => err.cause(),
            e => e,
        }
    }

    fn in_sentence(self, line: &str) -> ParseError {
        match self {
            e @ ParseError::InSentence { .. } => e,
            e => ParseError::InSentence {
                line: line.to_string(),
                err: Box::new(e),
            },
        }
    }
}

const N: &str = "[一二三四五六七八九十百千]+";

fn re(s: &'static str) -> Regex {
    Regex::new(&s.replace("{S}", SUB).replace("{N}", N).replace("{K}", KANA)).unwrap()
}

/// 号の下の細目（「ロ」「ロ(1)」「ロ(1)(i)」、上の細目を省いた「(3)」）
const SUB: &str = "(?:[{K}](?:[（(][^）)「」、]{1,6}[）)])*|(?:[（(][^）)「」、]{1,6}[）)])+)";

/// 「(3)」を直前の細目（「ロ(2)」）に続けて「ロ(3)」にする。括弧の書き方（数字・ローマ数字・漢数字）が
/// 直前の最後の括弧と同じなら同じ段（置き換え）、違えば下の段（付け足し）
fn full_sub(s: &str, ctx: Option<&str>) -> String {
    if !s.starts_with(['(', '（']) {
        return s.to_string();
    }
    let Some(ctx) = ctx else {
        return s.to_string();
    };
    let style = |g: &str| -> u8 {
        let inner = g
            .trim_start_matches(['(', '（'])
            .chars()
            .next()
            .unwrap_or(' ');
        if inner.is_ascii_digit() || ('０'..='９').contains(&inner) {
            0
        } else if "ivxｉｖｘ".contains(inner) {
            1
        } else {
            2
        }
    };
    // 直前の細目の括弧の段
    let groups: Vec<&str> = ctx.split_inclusive([')', '）']).collect();
    let first = s.split_inclusive([')', '）']).next().unwrap_or(s);
    let mut keep = String::new();
    for g in &groups {
        let paren = g.find(['(', '（']).map(|i| &g[i..]);
        match paren {
            Some(p) if style(p) == style(first) => {
                keep.push_str(&g[..g.len() - p.len()]);
                break;
            }
            _ => keep.push_str(g),
        }
    }
    format!("{keep}{s}")
}

/// 号の下の細目の記号（イロハ順）
const KANA: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス";

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
    ruby.replace_all(s, "")
        .replace('剥', "剝")
        .replace('｡', "。")
        .replace('､', "、")
        .replace('｢', "「")
        .replace('｣', "」")
}

/// 改正法の本文（借地借家法の部分）を改正単位に分ける。
/// 「第N条　X法（…）の一部を次のように改正する。」で始まり、インデント 2 の行が指示、インデント 1 の行が追加条文
pub fn parse_units(text: &str) -> Result<Vec<AmendUnit>, ParseError> {
    static HEADER: OnceLock<Regex> = OnceLock::new();
    let header = HEADER.get_or_init(|| {
        re(r"^(第{N}条(?:の{N})*)　(.+?)(（[^）]*）)?の一部を次のように改正する。$")
    });
    // 「第N条　次に掲げる法律の規定中「A」を「B」に改める。」+「一　X法（…）第M条…」の列挙形（令4-68 第221条など）。
    // 号ごとに、その法律を改正する単位を作る
    static LIST_HEADER: OnceLock<Regex> = OnceLock::new();
    let list_header = LIST_HEADER.get_or_init(|| {
        re(r"^(第{N}条(?:の{N})*)　次に掲げる法律の規定中「(.+?)」(?:を「(.+?)」に改める|の下に「(.+?)」を加える|を削る)。$")
    });
    static LIST_ITEM: OnceLock<Regex> = OnceLock::new();
    // 位置の無い号（「一　法人ノ役員処罰ニ関スル法律（大正四年法律第十八号）」）は法律の全部（本則）
    let list_item =
        LIST_ITEM.get_or_init(|| re(r"^{N}　(.+?)（[^）]*）((?:附則)?(?:第|別表).+)?$"));
    let mut list: Option<(String, String, String)> = None;
    let mut units: Vec<AmendUnit> = Vec::new();
    let text = normalize_source_text(text);
    // 整備法の体裁: 条の見出し「（X法の一部改正）」と章の見出し「第二章　文部科学省関係」は改め文ではない。
    // 章の見出しは「次の一章を加える」の内容（章名の行）と字面が同じなので、次の行が見出し・条の頭なら読み飛ばす
    static CAPTION: OnceLock<Regex> = OnceLock::new();
    let caption = CAPTION.get_or_init(|| re(r"^（.+の一部改正）$"));
    // 改め文が段落の途中で切れている（「同表中」+ 表 +「を」…）ページ: 「、」で終わる改め文の行は次の行とつなぐ
    let mut joined: Vec<String> = Vec::new();
    for l in text.lines() {
        match joined.last_mut() {
            // 「目次中「A」」の行の次が「を「B」に改める。」: 字句の後で切れた改め文
            Some(prev)
                if (prev.ends_with('、')
                    || prev.ends_with('」')
                        && l.trim_start_matches(['\u{3000}', ' '])
                            .starts_with(['を', 'に', '及', '並']))
                    && prev
                        .trim_start_matches(['\u{3000}', ' '])
                        .starts_with(['第', '同', '附', '別', '「', '目', '題', '本']) =>
            {
                prev.push_str(l.trim_start_matches(['\u{3000}', ' ']));
            }
            _ => joined.push(l.to_string()),
        }
    }
    // 1 行に 2 文（「…改める。　第二十八条第一項の表…」）: 「。」の後の空白で切る
    static TWO: OnceLock<Regex> = OnceLock::new();
    let two = TWO.get_or_init(|| re(r"。[\u{3000} ]*(第{N}|同|附則|別表|目次|題名|本則)"));
    let joined: Vec<String> = joined
        .into_iter()
        .flat_map(|l| {
            let t = l.trim_start_matches(['\u{3000}', ' ']);
            let indent = &l[..l.len() - t.len()];
            if !t.starts_with(['第', '同', '附', '別', '目', '題', '本', '「'])
                || starts_like_content(t)
                || !two.is_match(t)
            {
                return vec![l];
            }
            let mut out = Vec::new();
            let mut rest = t;
            while let Some(m) = two.find(rest) {
                // 「」の中は切らない
                let head = &rest[..m.start() + '。'.len_utf8()];
                // 改め文の終わり（「改める。」「加える。」…）で、「」が釣り合っているところだけ
                if head.matches('「').count() != head.matches('」').count()
                    || !looks_like_instruction(head.trim_start_matches(['\u{3000}', ' ']))
                {
                    break;
                }
                out.push(format!("{indent}{head}"));
                rest = rest[m.start() + '。'.len_utf8()..].trim_start_matches(['\u{3000}', ' ']);
            }
            out.push(format!("{indent}{rest}"));
            out
        })
        .collect();
    // 「同条第一項ただし書を次のように改める。ただし、…」: 内容が同じ行に続くページ。改め文と内容の行に分ける
    let joined: Vec<String> = joined
        .into_iter()
        .flat_map(|l| {
            let t = l.trim_start_matches(['\u{3000}', ' ']);
            let indent = &l[..l.len() - t.len()];
            for verb in ["次のように改める。", "次のように加える。"] {
                if let Some(i) = t.find(verb) {
                    let head = &t[..i + verb.len()];
                    let tail = &t[i + verb.len()..];
                    if !tail.trim().is_empty()
                        && head.matches('「').count() == head.matches('」').count()
                        && looks_like_instruction(head)
                    {
                        return vec![format!("{indent}{head}"), format!("\u{3000}{tail}")];
                    }
                }
            }
            vec![l]
        })
        .collect();
    let lines: Vec<&str> = joined.iter().map(|s| s.as_str()).collect();
    // 整備法の条の見出し一般（「（X法の一部改正に伴う経過措置）」）: 括弧だけの行で、次の行が改正法の条（字下げ無しの「第N条　」）
    let any_caption = |i: usize| -> bool {
        let l = lines[i].trim_start_matches(['\u{3000}', ' ']).trim_end();
        if !(l.starts_with('（')
            && l.ends_with('）')
            && !l[..l.len() - '）'.len_utf8()].contains('）'))
        {
            return false;
        }
        // 次の行が無い（整備法の条の途中で切った単位の末尾）も見出し
        lines[i + 1..]
            .iter()
            .find(|x| !x.trim().is_empty())
            .is_none_or(|n| {
                !n.starts_with('\u{3000}') && n.starts_with('第') && n.contains("条\u{3000}")
            })
    };
    static AMENDING_CHAPTER: OnceLock<Regex> = OnceLock::new();
    let amending_chapter = AMENDING_CHAPTER.get_or_init(|| re(r"^第{N}(?:編|章|節)　[^（）]+$"));
    // 単独の一部改正法（「X法（…）の一部を次のように改正する。」だけで条の見出しが無い）は、改め文が 1 字下げ・
    // 加える条文が字下げ無し（整備法より 1 段浅い）。その形の見出しの後は字下げを 1 段深く読む
    static SINGLE: OnceLock<Regex> = OnceLock::new();
    let single = SINGLE.get_or_init(|| re(r"^(.+?)(（[^）]*）)?の一部を次のように改正する。$"));
    let mut shift = 0usize;
    // 改正単位ごとの先行詞（文をまたいで「同条第二項中…」と書く古い改め文のため）
    let mut unit_ante = Ante::empty();
    // 字下げの無いページ（古い制定法律）: 改め文か追加する条文かを行の書き出しで見分ける
    let mut flat = false;
    let next_indent_of = |li: usize| -> Option<usize> {
        lines[li + 1..]
            .iter()
            .find(|l| !l.trim().is_empty())
            .map(|l| {
                l.chars()
                    .take_while(|c| *c == '\u{3000}' || *c == ' ')
                    .count()
            })
    };
    for (li, raw) in lines.iter().enumerate() {
        let indent = raw
            .chars()
            .take_while(|c| *c == '\u{3000}' || *c == ' ')
            .count()
            + shift;
        let line = raw.trim_start_matches(['\u{3000}', ' ']).trim_end();
        // 附則から先は改め文ではない（見出しは「附　則」と字を空ける。目次の行の「附則」とは別）
        if line.starts_with('附')
            && line.ends_with('則')
            && line.chars().count() >= 3
            && line
                .chars()
                .skip(1)
                .take(line.chars().count() - 2)
                .all(|c| c == '\u{3000}' || c == ' ')
        {
            break;
        }
        if line.is_empty()
            || line.starts_with('（') && indent == 0
            || caption.is_match(line)
            || any_caption(li)
        {
            continue;
        }
        if amending_chapter.is_match(line) {
            // 「第三章　内閣府関係」「第一節　本府関係」と続く見出しは読み飛ばして、その次の行で決める
            let next = lines[li + 1..].iter().enumerate().find(|(_, l)| {
                let t = l.trim_start_matches(['\u{3000}', ' ']).trim_end();
                !t.is_empty() && !amending_chapter.is_match(t)
            });
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
            // 「の下に「B」を加える」は「A」を「AB」に
            let to = match (c.get(3), c.get(4)) {
                (_, Some(b)) => format!("{}{}", &c[2], b.as_str()),
                (Some(b), None) => b.as_str().to_string(),
                (None, None) => String::new(),
            };
            list = Some((c[1].to_string(), c[2].to_string(), to));
            continue;
        }
        if let (Some((art, a, b)), Some(c)) = (&list, list_item.captures(line)) {
            if indent == 1 {
                let at = c.get(2).map(|m| m.as_str()).unwrap_or("本則");
                let text = if b.is_empty() {
                    format!("{at}中「{a}」を削る。")
                } else {
                    format!("{at}中「{a}」を「{b}」に改める。")
                };
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
            unit_ante = Ante::empty();
            // 改め文は 2 字下げ。ページによって 1 字（字下げを 1 段深く読む）、字下げ無し（書き出しで見分ける）
            let next_indent = next_indent_of(li).unwrap_or(2);
            shift = if indent == 0 {
                2usize.saturating_sub(next_indent.max(1))
            } else {
                0
            };
            flat = indent == 0 && next_indent == 0;
            units.push(AmendUnit {
                article_of_amending_law: c[1].to_string(),
                target_title: c[2].to_string(),
                instructions: Vec::new(),
            });
            continue;
        }
        if indent - shift <= 1 && !line.starts_with('第') {
            if let Some(c) = single.captures(line) {
                list = None;
                unit_ante = Ante::empty();
                // 改め文の字下げはページによって 1 字か 2 字。見出しの次の行（最初の改め文）の字下げに合わせる
                let next_indent = lines[li + 1..]
                    .iter()
                    .find(|l| !l.trim().is_empty())
                    .map(|l| {
                        l.chars()
                            .take_while(|c| *c == '\u{3000}' || *c == ' ')
                            .count()
                    })
                    .unwrap_or(1);
                shift = 2usize.saturating_sub(next_indent);
                flat = next_indent == 0;
                units.push(AmendUnit {
                    article_of_amending_law: "本則".to_string(),
                    target_title: c[1].to_string(),
                    instructions: Vec::new(),
                });
                continue;
            }
        }
        let Some(unit) = units.last_mut() else {
            return Err(ParseError::NoHeader);
        };
        // 「(1)　第八十七条を次のように改める。」: 番号を振った改め文
        static NUMBERED: OnceLock<Regex> = OnceLock::new();
        let numbered =
            NUMBERED.get_or_init(|| re(r"^[(（][０-９0-9一二三四五六七八九十]+[)）][　 ]+"));
        let unnumbered;
        let line = match numbered.find(line) {
            Some(m) if looks_like_instruction(&line[m.end()..]) => {
                unnumbered = line[m.end()..].to_string();
                unnumbered.as_str()
            }
            _ => line,
        };
        // 句点の落ちた改め文（「第三百十条第一項中「本款」を「本節」に改める」）
        let with_period;
        let line = if !line.ends_with('。') && looks_like_instruction(&format!("{line}。")) {
            with_period = format!("{line}。");
            with_period.as_str()
        } else {
            line
        };
        // 古い改め文は位置の前に法律名を冠する（「会計法目次中「A」を「B」に改める」「地方自治法第二条中…」）
        let line = match line.strip_prefix(unit.target_title.as_str()) {
            Some(r)
                if !unit.target_title.is_empty()
                    && ["目次", "目録", "第", "別表", "附則", "題名", "本則"]
                        .iter()
                        .any(|h| r.starts_with(h)) =>
            {
                r
            }
            _ => line,
        };
        // インデント 2 で「。」で終わる行が指示。ただし「一　…。」の号の行（加える項の中の号）は内容
        let is_item_line = line.split_once('\u{3000}').is_some_and(|(t, _)| {
            !t.is_empty() && t.chars().all(|c| "一二三四五六七八九十".contains(c))
        });
        // 「〔次のよう略〕」: 衆議院の本文が内容を略した印。内容として受け、無ければ読み飛ばす
        if line.starts_with('〔') && line.ends_with('〕') && line.contains('略') {
            if let Some(op) = unit
                .instructions
                .last_mut()
                .and_then(|ins| ins.ops.iter_mut().rev().find(|o| o.takes_content()))
            {
                op.push_content(line.to_string());
            }
            continue;
        }
        // 内容を待つ操作の後は、字下げが改め文と同じでも、改め文の形でない行（「第四条　…」「２　…」）は内容
        let pending = unit
            .instructions
            .last()
            .is_some_and(|ins| ins.ops.iter().any(|o| o.takes_content()));
        // 「第五十六条　…」（条の書き出し）や「２　…」は改め文でない
        let is_instruction = if flat {
            looks_like_instruction(line)
        } else {
            indent == 2
                && line.ends_with('。')
                && !line.starts_with('（')
                && !is_item_line
                && (looks_like_instruction(line) || !pending && !starts_like_content(line))
        };
        if is_instruction {
            let ops =
                parse_instruction_with(line, &mut unit_ante).map_err(|e| e.in_sentence(line))?;
            unit.instructions.push(Instruction {
                text: line.to_string(),
                ops,
            });
        } else {
            // 追加する条文。直前の指示の、内容を取る最後の操作に付ける
            let target = unit
                .instructions
                .last()
                .and_then(|ins| ins.ops.iter().rposition(|o| o.takes_content()));
            match target {
                Some(idx) => {
                    let ops = &mut unit.instructions.last_mut().unwrap().ops;
                    // 位置の列挙から出た同じ種類の操作（「第十項及び第十一項に後段として次のように加える」）は同じ内容を受ける
                    let before = content_len(&ops[idx]);
                    let kind = std::mem::discriminant(&ops[idx]);
                    ops[idx].push_content(line.to_string());
                    for k in (0..idx).rev() {
                        if before.is_none()
                            || std::mem::discriminant(&ops[k]) != kind
                            || content_len(&ops[k]) != before
                        {
                            break;
                        }
                        ops[k].push_content(line.to_string());
                    }
                }
                // 内容を待つ操作が無いのに字下げが内容の形: 字下げの崩れたページ。改め文の形なら改め文
                None if looks_like_instruction(line) => {
                    let ops = parse_instruction_with(line, &mut unit_ante)
                        .map_err(|e| e.in_sentence(line))?;
                    unit.instructions.push(Instruction {
                        text: line.to_string(),
                        ops,
                    });
                }
                None => {
                    // 古い改め文は「第六条　削除」「第九条　内閣ハ…」と、改める条をそのまま書く（「第N条を次のように改める」の省略）
                    let Some(text) = bare_article_instruction(line) else {
                        return Err(ParseError::Unrecognized(line.to_string()));
                    };
                    let mut ops = parse_instruction_with(&text, &mut unit_ante)?;
                    for op in ops.iter_mut() {
                        op.push_content(line.to_string());
                    }
                    unit.instructions.push(Instruction { text, ops });
                }
            }
        }
    }
    Ok(units)
}

/// 「第六条　削除」「第七十三条乃至第七十六条　削除」「第十条ノ三及第十条ノ四　削除」: 条の書き出しの行を、省かれた
/// 「第N条を次のように改める。」の文にする（条でなければ None）
fn bare_article_instruction(line: &str) -> Option<String> {
    static BARE: OnceLock<Regex> = OnceLock::new();
    let bare = BARE.get_or_init(|| {
        re(r"^(?P<nums>第{N}条(?:[のノ]{N})*(?:(?:及び?|、|乃至|から)第{N}条(?:[のノ]{N})*(?:まで)?)*)(?:（[^）]*）)?[　 ]")
    });
    let c = bare.captures(line)?;
    let nums = c["nums"].replace('ノ', "の");
    let nums = match nums.split_once("乃至") {
        Some((a, b)) => format!("{a}から{b}まで"),
        None => nums,
    };
    let nums = regex::Regex::new("及(?:び)?")
        .unwrap()
        .replace_all(&nums, "及び")
        .to_string();
    Some(format!("{nums}を次のように改める。"))
}

/// 条・項・号の書き出し（「第五十六条　」「２　」「一　」）: 追加する条文
fn starts_like_content(line: &str) -> bool {
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| {
        re(r"^(?:第{N}(?:条|編|章|節|款|目)(?:[のノ]{N})*(?:（[^）]*）)?[　 ]|[０-９0-9]+[　 ]|{N}[　 ]|備考[　 ]|[（(][０-９0-9一二三四五六七八九十]+[）)])")
    })
    .is_match(line)
}

/// 字下げの無いページで、行が改め文か（追加する条文でないか）。書き出しが位置（第N条中・同条・附則・目次・別表・題名・本則・「）で、
/// 指示の動詞で終わる。「第十条の二　…」「第十条の二（見出し）　…」「２　…」「一　…」「但し、…」は追加する条文
pub(crate) fn looks_like_instruction(line: &str) -> bool {
    static HEAD: OnceLock<Regex> = OnceLock::new();
    static ARTICLE: OnceLock<Regex> = OnceLock::new();
    static END: OnceLock<Regex> = OnceLock::new();
    let head = HEAD.get_or_init(|| {
        re(r"^(?:第{N}|同(?:条|項|号|章|節|款|編|表)|附則|目次|別表|題名|本則|「|前条|次条|章名|節名|この法律に|備考を|備考中|目録|[^「」、。第同こ　 ]{1,40}?表(?:中|を))")
    });
    let article =
        ARTICLE.get_or_init(|| re(r"^第{N}(?:条|編|章|節|款|目)(?:の{N})*(?:（[^）]*）)?[　 ]"));
    let end = END.get_or_init(|| {
        re(r"(?:改める|加える|削る|とする|[付附]する|繰り下げる|繰り上げる|繰下げる|繰上げる|掲げる|移す|移る)。$")
    });
    // 「…に規定する主務大臣は、…とする。」: 文の主語（「は、」）があれば条文
    let subject = {
        let mut depth = 0i32;
        let mut plain = String::new();
        for c in line.chars() {
            match c {
                '「' => depth += 1,
                '」' => depth = (depth - 1).max(0),
                _ if depth == 0 => plain.push(c),
                _ => {}
            }
        }
        plain.contains("は、")
    };
    line.ends_with('。')
        && head.is_match(line)
        && !article.is_match(line)
        && end.is_match(line)
        && !subject
}

/// 「、」で区切る。ただし「」（）の中は区切らない
fn split_segments(s: &str) -> Vec<String> {
    split_segments_with(s, false)
}

/// `strict`: 「」の深さによらず、動詞で終わり位置で始まる「、」（「…に改め、同条の次に…」）で切る。
/// 字句そのものに「」が入っていて深さがずれる文を読み直すときに使う
fn split_segments_with(s: &str, strict: bool) -> Vec<String> {
    // 「第百四十九条第四項にただし書を加え、同項を同条第六項とする改正規定中「A」を…」: 改正規定を言う位置の中の「、」は切らない
    const KEEP: char = '\u{E010}';
    if s.contains("改正規定") {
        // 各「改正規定」の前の、字句を含まない説明（「…を加え、…とする」）の中の「、」を守る
        let mut protected = String::new();
        let mut last = 0usize;
        let mut changed = false;
        for (i, _) in s.match_indices("改正規定") {
            let head = &s[last..i];
            // 説明の始まり: 最後の「」」の後の最初の「、」の後（無ければ区切りの始まり）
            let start = match head.rfind('」') {
                Some(j) => match head[j..].find('、') {
                    Some(k) => j + k + '、'.len_utf8(),
                    None => head.len(),
                },
                None => 0,
            };
            let desc = &head[start..];
            if !desc.contains('「') && desc.contains('、') && desc.ends_with(['る', 'す']) {
                protected.push_str(&head[..start]);
                protected.push_str(&desc.replace('、', &KEEP.to_string()));
                changed = true;
            } else {
                protected.push_str(head);
            }
            last = i;
        }
        protected.push_str(&s[last..]);
        if changed {
            return split_segments_with(&protected, strict)
                .into_iter()
                .map(|x| x.replace(KEEP, "、"))
                .collect();
        }
    }
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
    // 「」が釣り合っていれば深さだけで切る（改正規定の中の「、」を切らない）
    let unbalanced = body.matches('「').count() != body.matches('」').count();
    let verb_boundary = |before: &str, after: &str| {
        ["改め", "加え", "削り", "とし"]
            .iter()
            .any(|m| before.ends_with(m))
            && ["同条", "同項", "同号", "同表", "第", "附則", "別表"]
                .iter()
                .any(|h| after.starts_with(h))
    };
    for (i, c) in body.char_indices() {
        match c {
            '「' => q += 1,
            // 余る閉じ括弧は字句の一部（take_quoted と同じ扱い）
            '」' => q = (q - 1).max(0),
            // 「」の中の（）は数えない（字句「）は」のように片方だけのことがある）
            '（' if q == 0 => p += 1,
            '）' if q == 0 => p = (p - 1).max(0),
            '、' if (q == 0 && p == 0)
                || unbalanced && boundary(&cur, &body[i + '、'.len_utf8()..])
                || strict && verb_boundary(&cur, &body[i + '、'.len_utf8()..]) =>
            {
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
        // 「同条の次に次の二条、一章、章名及び一条を加える」の「、」は加えるものの列挙
        static STRUCT_PIECE: OnceLock<Regex> = OnceLock::new();
        let struct_piece = STRUCT_PIECE.get_or_init(|| {
            re(r"^(?:{N}(?:条|編|章|節|款|目|項|号)|(?:編|章|節|款|目)名|見出し)$")
        });
        let listing = !seg.contains('「')
            && (seg.contains("次の")
                && struct_piece.is_match(seg.rsplit("次の").next().unwrap_or(""))
                || struct_piece.is_match(&seg));
        if listing && !seg.ends_with(['え', 'る']) {
            pending.push_str(&seg);
            pending.push('、');
            continue;
        }
        let loc_only = (seg.starts_with('第')
            || seg.starts_with('同')
            || seg.starts_with("附則")
            || seg.starts_with("別表")
            || seg.ends_with("の項")
            // 「同改正規定のうち、…に係る部分」「…改め、同法第四十二条を第四十八条とし、…とする改正規定中」
            || seg.ends_with("のうち")
            || seg.ends_with("改正規定")
            || seg.starts_with("同法"))
            && !seg.contains('「')
            && (!seg.ends_with(['し', 'る', 'め', 'え', 'り', 'げ']) || seg.ends_with("見出し"));
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
            // 「同号イからハまで」: イロハの範囲
            let kana_end = |x: &str| x.chars().last().is_some_and(|c| KANA.contains(c));
            if let (Some(sa), Some(sb), true) = (&la.sub, &lb.sub, kana_end(&a) && kana_end(&b)) {
                for k in kana_index(sa)..=kana_index(sb) {
                    out.push(Loc {
                        sub: Some(kana_of(k)),
                        ..la.clone()
                    });
                }
                continue;
            }
            // 「第一号から第四号まで」: 号の範囲（同じ項の中）
            if let (Some(ia), Some(ib)) = (&la.item, &lb.item) {
                // 「第十六号から第十九号の二まで」: 枝番の端は基数の範囲の後に足す（中の枝番は当てるときに）
                let base = |x: &str| {
                    x.split('_')
                        .next()
                        .and_then(|b| b.parse::<u32>().ok())
                        .unwrap_or(0)
                };
                let (p, q) = (base(ia), base(ib));
                if ia.contains('_') {
                    out.push(Loc {
                        item: Some(ia.clone()),
                        ..la.clone()
                    });
                }
                for n in (p + u32::from(ia.contains('_')))..=q {
                    out.push(Loc {
                        item: Some(n.to_string()),
                        ..la.clone()
                    });
                }
                if ib.contains('_') {
                    out.push(Loc {
                        item: Some(ib.clone()),
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
            fresh: false,
            whole: false,
            title: false,
            tedit: None,
            scope: None,
            note: None,
            amend: None,
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
        if !tok.ends_with("改正規定") {
            continue;
        }
        // 法令名（「医師法」「同法」）を落とす: 最初の「第」「同条」から
        let start = tok
            .find("附則第")
            .into_iter()
            .chain(tok.find("第"))
            .chain(tok.find("同条"))
            .min()
            .unwrap_or(tok.len());
        let tok = &tok[start..];
        // 「第九条の改正規定」のほか「第九条に一項を加える改正規定」「第百二十一条の次に一条を加える改正規定」
        // 「第二十四条の四の七及び第二十四条の四の八を削る改正規定」: 頭の位置だけを取る
        static HEAD: OnceLock<Regex> = OnceLock::new();
        let head = HEAD.get_or_init(|| {
            re(r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*(?:第{N}項)?(?:第{N}号(?:の{N})*)?|同条(?:第{N}項)?)")
        });
        let tok = match tok.strip_suffix("の改正規定") {
            Some(t) => t.to_string(),
            None => match head.captures(tok) {
                Some(c) => c["loc"].to_string(),
                None => continue,
            },
        };
        if tok.is_empty() {
            continue;
        }
        out.extend(expand_locs(&tok, &mut ante)?);
    }
    Ok(out)
}

/// 外側から辿った容器（章・節…と番号）
type ContainerPath = Vec<(lawean_source::ContainerKind, String)>;

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
    /// 文の最初の断片（位置を書かない字句の操作は本則の全部）
    fresh: bool,
    /// 直前の字句の操作が本則の全部（「本則中「A」を「B」に、「C」を「D」に改める」の続き）
    whole: bool,
    /// 直前の字句の操作が題名（「題名中「A」を「B」に、「C」を「D」に改める」の続き）
    title: bool,
    /// 直前の位置が表の中（「別表第四表名称の欄中「A」を「B」に、「C」を「D」に改め」の続き、「同表」）
    tedit: Option<(TableRef, String)>,
    /// 直前の字句の操作の範囲（「第三章（第四十九条を除く。）中「A」を「B」に、「C」を「D」に改める」の続き）
    scope: Option<(ContainerPath, Vec<Loc>)>,
    /// 直前の字句の操作が条の付記（「第七十一条の付記中「A」を「B」に、「C」を「D」に改める」の続き）
    note: Option<ArticleNum>,
    /// 直前の改正規定（「同改正規定中」）
    amend: Option<String>,
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
        re(r"^(附則)?(?:(第{N}条(?:の{N})*)|同条)?(?:第({N})項|(同項))?(?:第({N}号(?:の{N})*)|(同号))?({S})?(?:各号)?(ただし書|各号列記以外の部分|本文|前段|後段)?$")
    });
    let s = s.strip_suffix('中').unwrap_or(s);
    let s = s.strip_suffix("の規定").unwrap_or(s);
    // 「第五章中第三十五条」: 容器の中の条（条の番号は法律で一意なので容器は先行詞にだけ）
    static CPRE: OnceLock<Regex> = OnceLock::new();
    let cpre =
        CPRE.get_or_init(|| re(r"^(?P<c>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中(?P<rest>第.+)$"));
    let owned;
    let s = match cpre.captures(s) {
        Some(c) => {
            ante.container = container_path(&c["c"]);
            owned = c["rest"].to_string();
            owned.as_str()
        }
        None => s,
    };
    // 「第四十八条の表以外の部分」: 条の本文（表は Raw なので字句の置換は本文だけに当たる）
    let s = s.strip_suffix("の表以外の部分").unwrap_or(s);
    // 「本則に次の一項を加える」「本則を次のように改める」: 条の無い本則（仮の第0条）
    if s == "本則" {
        let n = ArticleNum::Single {
            base: 0,
            branch: vec![],
        };
        ante.article = Some(n.clone());
        ante.paragraph = None;
        ante.toc = false;
        ante.suppl = false;
        return Ok(Loc::new(n, None));
    }
    // 「本則第二項」「本則ただし書」: 条の無い本則の項・文
    let s = match s.strip_prefix("本則") {
        Some(r) if r.starts_with('第') && !r.contains('条') => r,
        Some(r) if ["ただし書", "本文", "前段", "後段"].contains(&r) => {
            ante.article = Some(ArticleNum::Single {
                base: 0,
                branch: vec![],
            });
            ante.paragraph = None;
            ante.suppl = false;
            r
        }
        _ => s,
    };
    // 「第十四条第三項中第五号」「同条中第二項」: 位置の中の位置
    let joined;
    let s = if !r.is_match(s) && s.contains('中') {
        let cs: Vec<char> = s.chars().collect();
        joined = cs
            .iter()
            .enumerate()
            .filter(|(i, c)| {
                !(**c == '中'
                    && cs.get(i + 1).is_some_and(|n| {
                        "第同ただ本前後各（(イロハニホヘトチリヌルヲワカヨタレソツネナラム"
                            .contains(*n)
                    }))
            })
            .map(|(_, c)| *c)
            .collect::<String>();
        joined.as_str()
    } else {
        s
    };
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
        // 「附則第三項」「附則」: 条の無い附則（項だけ）。仮の条（第0条）
        None if c.get(1).is_some() => {
            let n = ArticleNum::Single {
                base: 0,
                branch: vec![],
            };
            ante.article = Some(n.clone());
            ante.toc = false;
            ante.suppl = true;
            n
        }
        // 「第一項中」「本則第二項中」で先行する条が無い: 条の無い本則（項だけ）。仮の条（第0条）
        None if (c.get(3).is_some() || c.get(5).is_some())
            && ante.article.is_none()
            && !ante.suppl =>
        {
            let n = ArticleNum::Single {
                base: 0,
                branch: vec![],
            };
            ante.article = Some(n.clone());
            ante.toc = false;
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
        // 「第百三十五条中「A」を「B」に改め、同項の表を削り」: 項を言わずに条を言った後の「同項」は 1 項だけの条の項
        match (ante.paragraph, &ante.article) {
            (Some(p), _) => Some(ParaRef::Num(p)),
            (None, Some(_)) => Some(ParaRef::Num(1)),
            (None, None) => return Err(ParseError::NoAntecedent(s.to_string())),
        }
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
        // 「(3)」: 同じ号なら直前の細目の中
        Some(k) if c.get(5).is_none() => Some(full_sub(k.as_str(), ante.sub.as_deref())),
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
    ante.whole = false;
    ante.tedit = None;
    ante.scope = None;
    ante.note = None;
    ante.amend = None;
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
    // 位置の後ろ全部（「題名及び第一条中…」「本則及び別表第一中…」で残りの位置に付け直す）
    let full_rest = rest;
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
    let appdx_re = APPDX.get_or_init(|| {
        re(r"^(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<row>.+?)の項(?P<sub>[イロハニホヘトチリヌルヲワカヨタレソツネナラム](?:[(（][^)）]{1,4}[)）])?|[(（][^)）]{1,4}[)）])?$")
    });
    let mut appdx_sub: Option<String> = None;
    // 「同項ニ中」: 別表の行の先行詞 + 細目（本則の項ではない）
    static APPDX_SAME: OnceLock<Regex> = OnceLock::new();
    let appdx_same = APPDX_SAME.get_or_init(|| {
        re(r"^同項(?P<sub>[イロハニホヘトチリヌルヲワカヨタレソツネナラム](?:[(（][^)）]{1,4}[)）])?|[(（][^)）]{1,4}[)）])?$")
    });
    if ante.article.is_none() && ante.appdx.is_some() {
        if let Some(c) = loc_part.and_then(|l| appdx_same.captures(l)) {
            appdx_sub = c.name("sub").map(|m| m.as_str().to_string());
            let (table, row) = ante.appdx.clone().unwrap();
            let mk = |from: String, to: String| Op::ReplaceAppdxRow {
                table: table.clone(),
                row: row.clone(),
                sub: appdx_sub.clone(),
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
        }
    }
    let appdx: Option<(String, String)> = match loc_part {
        Some(l) => appdx_re.captures(l).and_then(|c| {
            // 「同表の…の項」は直前の別表（直前が条の中の表なら、下の表の中の位置で読む）
            let table = match &c["table"] {
                "同表" => match (&ante.appdx, &ante.tedit) {
                    (Some((t, _)), _) => t.clone(),
                    (None, Some((TableRef::Appdx(t), _))) => t.clone(),
                    (None, Some(_)) => return None,
                    (None, None) => String::new(),
                },
                t => t.to_string(),
            };
            appdx_sub = c.name("sub").map(|m| m.as_str().to_string());
            Some((table, c["row"].to_string()))
        }),
        None if ante.article.is_none() && !ante.toc => ante.appdx.clone(),
        None => None,
    };
    if let Some((table, row)) = appdx {
        // 位置を省いた続き（「別表第一一七の項ホ中「A」を削り、「B」を削り」）は直前の細目のまま。
        // 別表の文脈では `ante.sub`（本則の号の下のイロハ）は使わないので、行の細目の先行詞として使う
        if loc_part.is_none() {
            appdx_sub = ante.sub.clone();
        } else {
            ante.sub = appdx_sub.clone();
        }
        ante.appdx = Some((table.clone(), row.clone()));
        ante.article = None;
        let mk = |from: String, to: String| Op::ReplaceAppdxRow {
            table: table.clone(),
            row: row.clone(),
            sub: appdx_sub.clone(),
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
    // 「第七十一条の付記中「A」を「B」に改める」: 条の付記
    let note = match loc_part.and_then(|l| {
        l.strip_suffix("の付記")
            .or_else(|| l.strip_suffix("の各付記"))
    }) {
        // 「第二十一条及び第二十五条から第二十六条までの各付記中」: 位置ごとに
        Some(a) if a.contains("及び") || a.contains('、') || a.contains("まで") => {
            let a = a.replace("の付記", "");
            let arts: Vec<ArticleNum> = expand_locs(&a, ante)?
                .into_iter()
                .map(|l| l.article)
                .collect();
            let mut v = Vec::new();
            for art in &arts {
                let Some(w) = phrase_tail(rest, &phrases, |from, to| Op::ReplaceSupplNote {
                    article: art.clone(),
                    from: from.clone(),
                    to,
                }) else {
                    return Ok(None);
                };
                v.extend(w);
            }
            return Ok(Some(PhraseOps(v)));
        }
        Some(a) => Some(loc(a, ante)?.article),
        None if loc_part.is_none() => ante.note.clone(),
        None => None,
    };
    if let Some(article) = note {
        ante.note = Some(article.clone());
        return Ok(
            phrase_tail(rest, &phrases, |from, to| Op::ReplaceSupplNote {
                article: article.clone(),
                from: from.clone(),
                to,
            })
            .map(PhraseOps),
        );
    }
    // 「前文中「A」を「B」に改める」「前文のうち第五項中」: 前文
    if let Some(r) = loc_part.and_then(|l| l.strip_prefix("前文")) {
        let path = r.trim_start_matches("のうち").to_string();
        ante.tedit = Some((TableRef::Preamble, path.clone()));
        return Ok(phrase_tail(rest, &phrases, |from, to| Op::TableEdit {
            table: TableRef::Preamble,
            path: path.clone(),
            action: TableAction::Phrase {
                from: from.clone(),
                to,
            },
        })
        .map(PhraseOps));
    }
    // 「附則第二項の改正規定中「A」を「B」に改める」「第二条のうち、X法…の改正規定のうち同項第二号中」
    // 「同改正規定中」（改正法を改める法律）: 改正規定の中。位置は字面のまま
    // 「第四条第一項の改正規定中「A」を「B」に、「C」を「D」に改める」の続き
    if loc_part.is_none() {
        if let Some(target) = ante.amend.clone() {
            return Ok(
                phrase_tail(rest, &phrases, |from, to| Op::ReplaceInAmendment {
                    article: ArticleNum::Single {
                        base: 0,
                        branch: vec![],
                    },
                    target: target.clone(),
                    from: from.clone(),
                    to,
                })
                .map(PhraseOps),
            );
        }
    }
    if let Some(l) = loc_part.filter(|l| l.contains("改正規定")) {
        let target = if l.starts_with("同改正規定") {
            match &ante.amend {
                Some(t) => format!(
                    "{}{}",
                    t.split("のうち").next().unwrap_or(t),
                    &l["同改正規定".len()..]
                ),
                None => l.to_string(),
            }
        } else {
            l.to_string()
        };
        ante.amend = Some(target.clone());
        return Ok(
            phrase_tail(rest, &phrases, |from, to| Op::ReplaceInAmendment {
                article: ArticleNum::Single {
                    base: 0,
                    branch: vec![],
                },
                target: target.clone(),
                from: from.clone(),
                to,
            })
            .map(PhraseOps),
        );
    }
    // 「第二条のうち、X法目次の改正規定中「A」を「B」に改める」: 改正法の改正規定の中
    if let Some((a, target)) = loc_part.and_then(|l| l.split_once("のうち")) {
        let article = loc(a, ante)?.article;
        let target = target.trim_start_matches('、').to_string();
        return Ok(
            phrase_tail(rest, &phrases, |from, to| Op::ReplaceInAmendment {
                article: article.clone(),
                target: target.clone(),
                from: from.clone(),
                to,
            })
            .map(PhraseOps),
        );
    }
    // 「第三十四条第一項及び同条第二項の表中」: 列挙した位置のそれぞれの表
    if let Some((locs, rest_path)) = loc_part.and_then(|l| {
        let i = l.find("の表")?;
        let (a, b) = (&l[..i], &l[i + "の表".len()..]);
        (a.contains("及び") || a.contains('、')).then_some((a, b))
    }) {
        let ats = expand_locs(locs, ante)?;
        let path = rest_path
            .trim_start_matches('中')
            .trim_start_matches('の')
            .to_string();
        let mut v = Vec::new();
        for at in &ats {
            let Some(w) = phrase_tail(rest, &phrases, |from, to| Op::TableEdit {
                table: TableRef::InArticle(at.clone()),
                path: path.clone(),
                action: TableAction::Phrase {
                    from: from.clone(),
                    to,
                },
            }) else {
                return Ok(None);
            };
            v.extend(w);
        }
        if let Some(last) = ats.last() {
            ante.tedit = Some((TableRef::InArticle(last.clone()), path));
        }
        return Ok(Some(PhraseOps(v)));
    }
    // 表の中の位置（欄・類・号・備考・行の列挙など）: 位置は字面のまま
    // 条・項の中の表の全部・1 つの行（「第三十八条の表第七十条第二項の項」）は下の ReplaceTableRow で
    static TROW_ONE: OnceLock<Regex> = OnceLock::new();
    let trow_one = TROW_ONE.get_or_init(|| re(r"^[^表]+?の表(?:[^、]+?の項)?$"));
    let generic = match loc_part {
        // 表の形の条の行（「第二条海の日の項中」）、「同欄３中」
        Some(l) if has_article_table(l) && !l.contains("の表") => split_table_target(l, ante)?,
        Some(l) if (l.starts_with("同欄") || l.starts_with("同注")) && ante.tedit.is_some() => {
            split_table_target(l, ante)?
        }
        // 表の行の中（「同項第六号中」）、表の号の中（「同号下欄中」）
        Some(l)
            if l.starts_with("同項")
                && ante.tedit.as_ref().is_some_and(|(_, p)| p.contains("の項"))
                || l.starts_with("同号") && ante.tedit.is_some() =>
        {
            split_table_target(l, ante)?
        }
        Some(l)
            if l.starts_with("別表")
                || l.starts_with("付表")
                || l.starts_with("附録")
                || l.starts_with("付録")
                || l.starts_with("様式")
                || l.starts_with("附則様式")
                || l.starts_with("附則別表")
                || l.starts_with("備考")
                || l.starts_with("同表")
                || l == "表"
                || (l.ends_with('表') && !l.ends_with("の表") && !l.contains("の表"))
                || (l.contains("の表") && !(trow_one.is_match(l) && !l.contains("及び"))) =>
        {
            split_table_target(l, ante)?
        }
        None if ante.tedit.is_some() => ante.tedit.clone(),
        _ => None,
    };
    if let Some((table, path)) = generic {
        let v = phrase_tail(rest, &phrases, |from, to| Op::TableEdit {
            table: table.clone(),
            path: path.clone(),
            action: TableAction::Phrase {
                from: from.clone(),
                to,
            },
        });
        ante.appdx = None;
        ante.tedit = Some((table, path));
        return Ok(v.map(PhraseOps));
    }
    // 「本則及び別表第一中「A」を「B」に改める」: 本則の全部と残りの位置
    if let Some(l) = loc_part.and_then(|l| l.strip_prefix("本則及び")) {
        let Some(mut v) = phrase_tail(rest, &phrases, |from, to| Op::ReplaceAll {
            from: from.clone(),
            to,
        }) else {
            return Ok(None);
        };
        match parse_phrase_op(&format!("{l}中{full_rest}"), ante)? {
            Some(PhraseOps(w)) => v.extend(w),
            None => return Ok(None),
        }
        return Ok(Some(PhraseOps(v)));
    }
    // 題名の字句: 「題名中「A」を「B」に改める」。「題名及び第一条中「A」を「B」に改める」は題名と残りの位置の両方
    let title_and = loc_part.and_then(|l| l.strip_prefix("題名及び"));
    if loc_part == Some("題名") || title_and.is_some() || (loc_part.is_none() && ante.title) {
        let Some(mut v) = phrase_tail(rest, &phrases, |from, to| Op::ReplaceTitle {
            from: from.clone(),
            to,
        }) else {
            return Ok(None);
        };
        ante.title = true;
        if let Some(l) = title_and {
            ante.title = false;
            match parse_phrase_op(&format!("{l}中{full_rest}"), ante)? {
                Some(PhraseOps(w)) => v.extend(w),
                None => return Ok(None),
            }
        }
        return Ok(Some(PhraseOps(v)));
    }
    ante.title = false;
    // 目次の字句: 「目次中「A」を削り」「目次中「A」を「B」に、「C」を「D」に改め」
    // 「目次第二章第一節第三款中…に改め、同節第七款中「A」を「B」に改める」: 目次の中の位置の続き
    let toc_cont = ante.toc
        && loc_part.is_some_and(|l| {
            static TC: OnceLock<Regex> = OnceLock::new();
            TC.get_or_init(|| re(r"^(?:同(?:編|章|節|款))?(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+$"))
                .is_match(l)
        });
    if loc_part == Some("目次") || (loc_part.is_none() && ante.toc) || toc_cont {
        ante.toc = true;
        if rest.starts_with("の下に") {
            return Ok(phrase_tail(rest, &phrases, |from, to| Op::ReplaceToc {
                from: from.clone(),
                to,
            })
            .map(PhraseOps));
        }
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
    // 「第三章中「第五節　収容」を…」「第二章第四節中「…」を削り」: 容器の中の字句
    // 「本則（第九条、第十条…を除く。）中」「第三章（第四十九条を除く。）中」: 除く条の外
    static CONT: OnceLock<Regex> = OnceLock::new();
    let cont = CONT.get_or_init(|| {
        re(r"^(?P<path>本則|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)(?:（(?P<ex>.+)を除く。）)?$")
    });
    let scope = match loc_part.and_then(|l| cont.captures(l)) {
        Some(c) if &c["path"] != "本則" || c.name("ex").is_some() => {
            let path = container_path(&c["path"]);
            let except = match c.name("ex") {
                Some(ex) => expand_locs(ex.as_str(), ante)?,
                None => Vec::new(),
            };
            Some((path, except))
        }
        Some(_) => None,
        None if loc_part.is_none() => ante.scope.clone(),
        None => None,
    };
    if let Some((path, except)) = scope {
        if !path.is_empty() {
            ante.container = path.clone();
        }
        ante.article = None;
        ante.scope = Some((path.clone(), except.clone()));
        return Ok(
            phrase_tail(rest, &phrases, |from, to| Op::ReplaceInContainer {
                path: path.clone(),
                except: except.clone(),
                from: from.clone(),
                to,
            })
            .map(PhraseOps),
        );
    }
    // 「本則中「A」を「B」に改める」、位置を書かない最初の字句（古い法律の「「勅令」を「政令」に改める」）: 本則の全部
    let whole = loc_part == Some("本則") || (loc_part.is_none() && (ante.fresh || ante.whole));
    if whole {
        ante.article = None;
        ante.whole = true;
        let mk = |from: &String, to: String| Op::ReplaceAll {
            from: from.clone(),
            to,
        };
        if let Some(r) = rest.strip_prefix("を") {
            if r == "削り" || r == "削る" {
                return Ok(Some(PhraseOps(
                    phrases.iter().map(|f| mk(f, String::new())).collect(),
                )));
            }
            if let Some((b, tail)) = take_quoted(r) {
                if matches!(tail, "に" | "に改め" | "に改める") {
                    return Ok(Some(PhraseOps(
                        phrases.iter().map(|f| mk(f, b.clone())).collect(),
                    )));
                }
            }
        }
        if let Some(r) = rest.strip_prefix("の下に") {
            if let Some((b, tail)) = take_quoted(r) {
                if matches!(tail, "を" | "を加え" | "を加える") {
                    return Ok(Some(PhraseOps(
                        phrases.iter().map(|f| mk(f, format!("{f}{b}"))).collect(),
                    )));
                }
            }
        }
        return Ok(None);
    }
    // 位置を省いた続きは、直前の位置の列挙（「第九十四条第一項及び第三項中「A」を「B」に、「C」を「D」に改める」）全部に当てる
    // 条・項の中の表の行: 「第三十八条の表第七十条第二項の項中「A」を「B」に改め」（別表は上で）
    static TROW: OnceLock<Regex> = OnceLock::new();
    let trow = TROW.get_or_init(|| re(r"^(?P<loc>.+?)の表(?:(?P<row>.+?)の項)?$"));
    if let Some(c) = loc_part.and_then(|l| trow.captures(l)) {
        let at = loc(&c["loc"], ante)?;
        // 行を言わない「第二十四条第一項の表中「A」を「B」に改める」は表の全部（row = ""）
        let row = c
            .name("row")
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        // 続き（「「C」を「D」に」）と「同表」はこの表
        let path = if row.is_empty() {
            String::new()
        } else {
            format!("{row}の項")
        };
        ante.tedit = Some((TableRef::InArticle(at.clone()), path));
        return Ok(phrase_tail(rest, &phrases, |from, to| Op::ReplaceTableRow {
            at: at.clone(),
            row: row.clone(),
            from: from.clone(),
            to,
        })
        .map(PhraseOps));
    }
    // 「第百八十五条（見出しを含む。）中「A」を「B」に改める」: 見出しも
    let (loc_part, with_caption) = match loc_part {
        Some(l) => match l.strip_suffix("（見出しを含む。）") {
            Some(base) => (Some(base), true),
            None => (Some(l), false),
        },
        None => (None, false),
    };
    // 「第八十五条の二（見出しを含む。）及び第百二条第二項中「A」を削る」: 見出しも改める条
    let mut caption_arts: Vec<ArticleNum> = Vec::new();
    let listed_owned;
    let loc_part = match loc_part {
        Some(l) if l.contains("（見出しを含む。）") => {
            for tok in l
                .split("並びに")
                .flat_map(|x| x.split("及び"))
                .flat_map(|x| x.split('、'))
            {
                if let Some(base) = tok.strip_suffix("（見出しを含む。）") {
                    let mut probe = ante.clone();
                    caption_arts.push(loc(base, &mut probe)?.article);
                }
            }
            listed_owned = l.replace("（見出しを含む。）", "");
            Some(listed_owned.as_str())
        }
        l => l,
    };
    // 「目次、第三章の章名及び第七条中」「第一条並びに第二条の見出し及び同条第一項中」: 目次・章名・見出しを分けて
    let mut extra_ops: Vec<Box<dyn Fn(&String, String) -> Op>> = Vec::new();
    let special_owned;
    let loc_part = match loc_part {
        Some(l)
            if (l.contains("及び") || l.contains('、') || l.contains("並びに"))
                && (l.contains("目次") || l.contains("名") || l.contains("見出し")) =>
        {
            let mut rest: Vec<String> = Vec::new();
            for tok in l
                .split("並びに")
                .flat_map(|x| x.split("及び"))
                .flat_map(|x| x.split('、'))
            {
                if tok == "目次" {
                    extra_ops.push(Box::new(|f: &String, t: String| Op::ReplaceToc {
                        from: f.clone(),
                        to: t,
                    }));
                } else if let Some(base) = ["の章名", "の節名", "の款名", "の編名", "の目名"]
                    .iter()
                    .find_map(|x| tok.strip_suffix(x))
                {
                    let path = match base.strip_prefix('同') {
                        Some(r) => {
                            let k = r.chars().next().unwrap_or('章');
                            let mut p = ante_container_upto(ante, k);
                            p.extend(container_path(&r[k.len_utf8()..]));
                            p
                        }
                        None => container_path(base),
                    };
                    ante.container = path.clone();
                    extra_ops.push(Box::new(move |f: &String, t: String| {
                        Op::ReplaceContainerTitle {
                            path: path.clone(),
                            from: f.clone(),
                            to: t,
                        }
                    }));
                } else if let Some(base) = tok
                    .strip_suffix("の前の見出し")
                    .or_else(|| tok.strip_suffix("の見出し"))
                {
                    let l = loc(base, ante)?;
                    extra_ops.push(Box::new(move |f: &String, t: String| {
                        caption_op(
                            l.clone(),
                            CaptionEdit::Replace {
                                from: f.clone(),
                                to: t,
                            },
                        )
                        .in_suppl(l.suppl)
                    }));
                } else {
                    rest.push(tok.to_string());
                }
            }
            if extra_ops.is_empty() {
                Some(l)
            } else {
                // 残りの位置が無ければ None（名前の付いた位置の操作だけ）
                special_owned = rest.join("及び");
                Some(special_owned.as_str())
            }
        }
        l => l,
    };
    if !extra_ops.is_empty() {
        // 目次・章名・見出しの操作を並べ、最後に残りの位置の字句の操作
        let mut v = Vec::new();
        for mk in &extra_ops {
            match phrase_tail(rest, &phrases, |f, t| mk(f, t)) {
                Some(w) => v.extend(w),
                None => return Ok(None),
            }
        }
        let l = loc_part.unwrap_or("");
        if !l.is_empty() {
            match parse_phrase_op(&format!("{l}中{full_rest}"), ante)? {
                Some(PhraseOps(w)) => v.extend(w),
                None => return Ok(None),
            }
        }
        return Ok(Some(PhraseOps(v)));
    }
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
            let mut v: Vec<Op> = caption_arts
                .iter()
                .flat_map(|a| {
                    phrases.iter().map(move |from| Op::ReplaceCaption {
                        article: a.clone(),
                        from: from.clone(),
                        to: String::new(),
                    })
                })
                .collect();
            v.extend(ats.iter().flat_map(|at| {
                phrases.iter().map(move |from| Op::Replace {
                    at: at.clone(),
                    from: from.clone(),
                    to: String::new(),
                })
            }));
            return Ok(Some(PhraseOps(v)));
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
    // 古い「「A」の上に「B」を加える」: A の前に B
    if let Some(r) = rest.strip_prefix("の上に") {
        if let Some((b, tail)) = take_quoted(r) {
            if matches!(tail, "を" | "を加え" | "を加える") {
                return Ok(Some(PhraseOps(
                    ats.iter()
                        .flat_map(|at| {
                            phrases.iter().map(|f| Op::Replace {
                                at: at.clone(),
                                from: f.clone(),
                                to: format!("{b}{f}"),
                            })
                        })
                        .collect(),
                )));
            }
        }
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

/// 表の在りかと、その中の位置（字面）に分ける。「別表第四表名称の欄」→（別表第四表, 名称の欄）、
/// 「同表中第十二号」→（直前の表, 第十二号）、「第十三条第一項の表中A及びBの項」→（第十三条第一項の表, A及びBの項）
fn split_table_target(
    target: &str,
    ante: &mut Ante,
) -> Result<Option<(TableRef, String)>, ParseError> {
    static NAME: OnceLock<Regex> = OnceLock::new();
    let name = NAME.get_or_init(|| {
        re(r"^(?P<t>(?:附則)?別表(?:第?[一二三四五六七八九十百千]+(?:号表|表)?(?:[のノ][一二三四五六七八九十]+)?(?:[(（][Ａ-ＺA-Z][)）])?|[甲乙丙丁戊己庚辛][号表])?(?:(?:及び|、|から)(?:別表)?第?[一二三四五六七八九十百千]+(?:号表|表)?(?:まで)?)*)(?P<rest>.*)$")
    });
    let path_of = |r: &str| {
        r.trim_start_matches('、')
            .trim_start_matches('中')
            .trim_start_matches('の')
            .trim_end_matches('中')
            .to_string()
    };
    // 「同号を同表の第八号とし」「同号の前に次の三号を加える」: 直前の表の中の号
    if (target == "同号" || target == "その") && ante.tedit.is_some() {
        return Ok(ante.tedit.clone());
    }
    // 「同号（二）イ」: 直前の表の号の中。「同欄」: 直前の表の欄
    if (target.starts_with("同号") || target.starts_with("同欄"))
        && ante.tedit.is_some()
        && !has_article_table(target)
    {
        return Ok(ante.tedit.clone().map(|(t, _)| (t, target.to_string())));
    }
    // 「同項の次に次のように加える」「同項第六号中」: 直前の表の行（の中）
    // 号の中の表（「第十一条第一項第三号の表」）の後の「同項」は条の項
    let item_table = matches!(&ante.tedit, Some((TableRef::InArticle(at), _)) if at.item.is_some());
    if let Some(rest) = target.strip_prefix("同項").filter(|_| !item_table) {
        let hit = ante
            .tedit
            .clone()
            .and_then(|(t, p)| {
                // 行（「…の項」）か、表の中の項（「第二八・〇四項」）
                let row = match p.find("の項") {
                    Some(i) => &p[..i + "の項".len()],
                    None if p.ends_with('項') => p.as_str(),
                    None => return None,
                };
                Some((t, format!("{row}{}", path_of(rest))))
            })
            // 別表の行の先行詞（「同表の一の項を同表の一の二の項とし、同項の前に次のように加える」）
            .or_else(|| {
                ante.appdx
                    .clone()
                    .filter(|(_, r)| !r.is_empty())
                    .map(|(t, r)| (TableRef::Appdx(t), format!("{r}の項{}", path_of(rest))))
            });
        if hit.is_some() {
            return Ok(hit);
        }
    }
    // 「同注１(h)」: 直前の表の注
    if let Some(r) = target.strip_prefix("同注") {
        return Ok(ante.tedit.clone().map(|(t, _)| (t, format!("注{r}"))));
    }
    if let Some(r) = target.strip_prefix("同表") {
        let table = match (&ante.tedit, &ante.appdx) {
            (Some((t, _)), _) => t.clone(),
            (None, Some((t, _))) => TableRef::Appdx(t.clone()),
            _ => return Err(ParseError::NoAntecedent(target.to_string())),
        };
        return Ok(Some((table, path_of(r))));
    }
    // 「備考を次のように改める」: 直前の表の備考
    if target.starts_with("備考") {
        let table = match (&ante.tedit, &ante.appdx) {
            (Some((t, _)), _) => t.clone(),
            (None, Some((t, _))) => TableRef::Appdx(t.clone()),
            _ => return Ok(None),
        };
        return Ok(Some((table, target.to_string())));
    }
    // 「付表第二第二号の第二欄」「附録第一号算式」: 付表・附録も表と同じく
    for head in ["付表", "附録", "付録"] {
        if let Some(r) = target.strip_prefix(head) {
            static NUM: OnceLock<Regex> = OnceLock::new();
            let num = NUM.get_or_init(|| re(r"^(?P<n>第?{N}(?:号)?)?(?P<rest>.*)$"));
            let c = num.captures(r).expect("付表");
            let n = c.name("n").map(|m| m.as_str()).unwrap_or("");
            return Ok(Some((
                TableRef::Appdx(format!("{head}{n}")),
                path_of(&c["rest"]),
            )));
        }
    }
    // 「様式第二号」: 様式も表と同じく字面の位置で
    if target.starts_with("様式") || target.starts_with("附則様式") {
        static FORM: OnceLock<Regex> = OnceLock::new();
        let form = FORM
            .get_or_init(|| re(r"^(?P<t>(?:附則)?様式(?:第{N}(?:号)?(?:の{N})*)?)(?P<rest>.*)$"));
        let c = form.captures(target).expect("様式");
        return Ok(Some((
            TableRef::Appdx(c["t"].to_string()),
            path_of(&c["rest"]),
        )));
    }
    if target.starts_with("別表") || target.starts_with("附則別表") {
        let c = name.captures(target).expect("別表");
        return Ok(Some((
            TableRef::Appdx(c["t"].to_string()),
            path_of(&c["rest"]),
        )));
    }
    // 「表中」: 条の無い本則の表。「政府製造たばこ価格表中」: 名の付いた表
    if target == "表" || target.starts_with("表中") {
        let at = Loc::new(
            ArticleNum::Single {
                base: 0,
                branch: vec![],
            },
            None,
        );
        return Ok(Some((
            TableRef::InArticle(at),
            path_of(&target["表".len()..]),
        )));
    }
    static NAMED: OnceLock<Regex> = OnceLock::new();
    let named = NAMED.get_or_init(|| {
        re(r"^(?:(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*|同条)?(?:第{N}項|同項)?)の)?(?P<name>[^の「」第同]+表)(?P<rest>.*)$")
    });
    if let Some(c) = named.captures(target) {
        if !c["name"].ends_with("の表") {
            return Ok(Some(
                match c.name("loc").map(|m| m.as_str()).filter(|l| !l.is_empty()) {
                    Some(l) => (
                        TableRef::InArticle(loc(l, ante)?),
                        format!("{}{}", &c["name"], path_of(&c["rest"])),
                    ),
                    None => (TableRef::Appdx(c["name"].to_string()), path_of(&c["rest"])),
                },
            ));
        }
    }
    // 「第二条体育の日の項」: 表の形の条の行
    static ART_ROW: OnceLock<Regex> = OnceLock::new();
    let art_row = ART_ROW.get_or_init(|| {
        re(r"^(?P<loc>第{N}条(?:の{N})*|同条)(?P<row>[^第の同中、「」][^「」、]*?の項.*)$")
    });
    if let Some(c) = art_row.captures(target) {
        let at = loc(&c["loc"], ante)?;
        return Ok(Some((TableRef::InArticle(at), c["row"].to_string())));
    }
    // 「第四条第一項の式」: 条・項の中の式も表と同じく
    if let Some(a) = target.strip_suffix("の式") {
        let at = loc(a, ante)?;
        return Ok(Some((TableRef::InArticle(at), "式".to_string())));
    }
    if let Some(i) = target.find("の表") {
        let at = loc(&target[..i], ante)?;
        return Ok(Some((
            TableRef::InArticle(at),
            path_of(&target[i + "の表".len()..]),
        )));
    }
    Ok(None)
}

/// 「第百三十二条の前の見出し及び同条」「第六条及び同条の前の見出し」「附則第四項の前の見出し及び同項から附則第九項まで」:
/// 条・項と一緒に見出しを削る・改める（見出しは内容の側にある）ので、位置は条・項だけにする
fn without_caption_mention(l: &str) -> String {
    static A: OnceLock<Regex> = OnceLock::new();
    static B: OnceLock<Regex> = OnceLock::new();
    let a = A.get_or_init(|| re(r"^(?P<x>.+?)の(?:前の)?見出し及び(?:同条|同項)(?P<rest>.*)$"));
    let b = B.get_or_init(|| re(r"^(?P<x>.+?)及び同(?:条|項)の(?:前の)?見出し$"));
    if let Some(c) = a.captures(l) {
        return format!("{}{}", &c["x"], &c["rest"]);
    }
    if let Some(c) = b.captures(l) {
        return c["x"].to_string();
    }
    // 「第十一条（見出しを含む。）を次のように改める」: 内容に見出しがある
    l.replace("（見出しを含む。）", "")
}

/// 「第三号及び第四号」「第一号及び第一号の二」「第六号から第八号まで」: 号の列挙で終わる
fn item_list_tail(l: &str) -> bool {
    static T: OnceLock<Regex> = OnceLock::new();
    T.get_or_init(|| re(r"第{N}号(?:の{N})*(?:まで)?$"))
        .is_match(l)
}

/// 「第十三条第一項の表」「同項の表の第一号」: 条・項・号の中の表を言う（「次の表」「表以外の部分」は違う）
fn has_article_table(s: &str) -> bool {
    // 「第二条体育の日の項」: 表の形の条（国民の祝日に関する法律）の行
    static ROW: OnceLock<Regex> = OnceLock::new();
    if ROW
        .get_or_init(|| re(r"^(?:第{N}条(?:の{N})*|同条)[^第の同中、「」][^「」、]*?の項"))
        .is_match(s)
    {
        return true;
    }
    s.match_indices("の表").any(|(i, _)| {
        let prev = s[..i].chars().last();
        let next = &s[i + "の表".len()..];
        prev.is_some_and(|c| "条項号一二三四五六七八九十百千".contains(c) || KANA.contains(c))
            && !next.starts_with("以外")
    })
}

/// 位置の列挙から並んで出る、内容を受ける操作の受けた行の数（それ以外は None）
fn content_len(op: &Op) -> Option<usize> {
    match op {
        Op::AppendSentence { text, .. }
        | Op::AppendParagraph { text, .. }
        | Op::InsertParagraphAfter { text, .. }
        | Op::AppendItem { text, .. } => Some(text.len()),
        _ => None,
    }
}

/// 「第三章第二節から第四節まで」→ 第三章第二節・第三節・第四節（範囲でなければその容器）
fn container_range(t: &str) -> Vec<ContainerPath> {
    let Some((a, b)) = t.split_once("から") else {
        return vec![container_path(t)];
    };
    let pa = container_path(a);
    let pb = container_path(b.trim_end_matches("まで"));
    let (Some((ka, na)), Some((_, nb))) = (pa.last().cloned(), pb.last().cloned()) else {
        return vec![pa];
    };
    let (Ok(x), Ok(y)) = (na.parse::<u32>(), nb.parse::<u32>()) else {
        return vec![pa, pb];
    };
    let parent = &pa[..pa.len() - 1];
    (x..=y)
        .map(|n| {
            let mut p = parent.to_vec();
            p.push((ka, n.to_string()));
            p
        })
        .collect()
}

/// 「第四章及び第五章」「第二章の二」「第三章第二節から第四節まで」: 容器の列挙
fn is_container_list(l: &str) -> bool {
    let l = &l.replace("中第", "第");
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| {
        re(r"^(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+(?:(?:及び|、|から)(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+(?:まで)?)*$")
    })
    .is_match(l)
}

/// 「同表」の指す別表（別表の行の先行詞か、表の中の位置の先行詞）
fn same_appdx(ante: &Ante) -> Option<String> {
    match (&ante.appdx, &ante.tedit) {
        (Some((t, _)), _) => Some(t.clone()),
        (None, Some((TableRef::Appdx(t), _))) => Some(t.clone()),
        _ => None,
    }
}

/// 見出しの操作。位置に項があれば項の見出し、無ければ条の見出し
fn caption_op(l: Loc, edit: CaptionEdit) -> Op {
    if l.paragraph.is_some() {
        return Op::ParagraphCaption { at: l, edit };
    }
    let article = l.article;
    match edit {
        CaptionEdit::Replace { from, to } => Op::ReplaceCaption { article, from, to },
        CaptionEdit::Set(text) => Op::SetCaption { article, text },
        CaptionEdit::Attach(text) => Op::AttachCaption { article, text },
        CaptionEdit::Delete => Op::DeleteCaption { article },
    }
}

/// 「を削り」「を「B」に改め」「の下に「B」を加え」の尾を読み、字句ごとに `mk(字句, 置き換え後)` の操作にする
fn phrase_tail(
    rest: &str,
    phrases: &[String],
    mk: impl Fn(&String, String) -> Op,
) -> Option<Vec<Op>> {
    if let Some(r) = rest.strip_prefix("を") {
        if r == "削り" || r == "削る" {
            return Some(phrases.iter().map(|f| mk(f, String::new())).collect());
        }
        if let Some((b, tail)) = take_quoted(r) {
            if matches!(tail, "に" | "に改め" | "に改める") {
                return Some(phrases.iter().map(|f| mk(f, b.clone())).collect());
            }
        }
    }
    if let Some(r) = rest.strip_prefix("の下に") {
        if let Some((b, tail)) = take_quoted(r) {
            if matches!(tail, "を" | "を加え" | "を加える") {
                return Some(phrases.iter().map(|f| mk(f, format!("{f}{b}"))).collect());
            }
        }
    }
    // 古い「「A」の上に「B」を加える」: A の前に B
    if let Some(r) = rest.strip_prefix("の上に") {
        if let Some((b, tail)) = take_quoted(r) {
            if matches!(tail, "を" | "を加え" | "を加える") {
                return Some(phrases.iter().map(|f| mk(f, format!("{b}{f}"))).collect());
            }
        }
    }
    None
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

/// 直前の容器のうち、`kind`（「節」）までの部分（「同節第一款」の「同節」）
fn ante_container_upto(ante: &Ante, kind: char) -> Vec<(lawean_source::ContainerKind, String)> {
    use lawean_source::ContainerKind::*;
    let want = match kind {
        '編' => Part,
        '章' => Chapter,
        '節' => Section,
        '款' => Subsection,
        _ => Division,
    };
    let mut out = Vec::new();
    for (k, n) in &ante.container {
        out.push((*k, n.clone()));
        if *k == want {
            break;
        }
    }
    out
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

/// 古い改め文の書き方を今の書き方に揃える（「」の外だけ。「」の中は被改正法の字句なのでそのまま）:
/// 「第二十七条ノ二」→「第二十七条の二」、「但書」→「ただし書」、「左の一項」「左のように」→「次の…」、半角の句読点
pub fn normalize_instruction(line: &str) -> String {
    // 「十二条の二中」（行頭の第の落ち）
    static NO_DAI_HEAD: OnceLock<Regex> = OnceLock::new();
    let no_dai_head = NO_DAI_HEAD.get_or_init(|| re(r"^({N}条)"));
    let line = no_dai_head.replace(line, "第${1}").to_string();
    let line = line.as_str();
    // 古い法律の目次は「目録」。「この法律中（別に定める場合を除き、）」は本則の全部
    let line = match line.strip_prefix("目録") {
        Some(r) => format!("目次{r}"),
        None => line.to_string(),
    };
    // 「目次中第二章中「A」を「B」に」「目次第二章第一節第三款中」: 目次の中の位置は目次の字句として読む
    static TOC_PATH: OnceLock<Regex> = OnceLock::new();
    let toc_path =
        TOC_PATH.get_or_init(|| re(r"^目次(?:中)?(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+中「"));
    let line = match toc_path.find(&line) {
        Some(m) => format!("目次中「{}", &line[m.end()..]),
        None => line,
    };
    let line = match line
        .strip_prefix("この法律中別に定める場合を除き、")
        .or_else(|| line.strip_prefix("この法律中"))
    {
        Some(r) if r.starts_with('「') => format!("本則中{r}"),
        _ => line,
    };
    let line = line
        .replace('｡', "。")
        .replace('､', "、")
        .replace('｢', "「")
        .replace('｣', "」");
    let mut out = String::with_capacity(line.len());
    let mut depth = 0i32;
    let chars: Vec<char> = line.chars().collect();
    let digits = "一二三四五六七八九十百千";
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '「' => depth += 1,
            '」' => depth = (depth - 1).max(0),
            _ => {}
        }
        if depth == 0 {
            // 条・項・号の後の「ノ」+数 →「の」
            if c == 'ノ'
                && i > 0
                && ("条項号編章節款目".contains(chars[i - 1]) || digits.contains(chars[i - 1]))
                && chars.get(i + 1).is_some_and(|n| digits.contains(*n))
            {
                out.push('の');
                i += 1;
                continue;
            }
            let rest: String = chars[i..chars.len().min(i + 5)].iter().collect();
            if rest.starts_with("但書") {
                out.push_str("ただし書");
                i += 2;
                continue;
            }
            if rest.starts_with("左の") {
                out.push_str("次の");
                i += 2;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    let out = map_unquoted(&out, |t| {
        static THRU: OnceLock<Regex> = OnceLock::new();
        static AND: OnceLock<Regex> = OnceLock::new();
        // 「第二項乃至第四項」→「第二項から第四項まで」
        let thru = THRU.get_or_init(|| re(r"乃至(第{N}(?:条|項|号)(?:の{N})*)"));
        // 古い「及」「並ニ」
        let and = AND.get_or_init(|| re(r"及(第|同|附則|別表)"));
        let t = thru.replace_all(t, "から${1}まで");
        let t = and.replace_all(&t, "及び${1}");
        // 「第二十八ノ二」（条の落ちた枝番）、「第三条第一項第一号（ハ）」（括弧の細目）
        static NO_JO: OnceLock<Regex> = OnceLock::new();
        let no_jo = NO_JO.get_or_init(|| re(r"(^|[^条項号表])第({N})[ノの]({N})を"));
        let t = no_jo.replace_all(&t, "${1}第${2}条の${3}を");
        // 「第六ノ六ノ二号」→「第六号の六の二」
        static ITEM_BRANCH: OnceLock<Regex> = OnceLock::new();
        let item_branch = ITEM_BRANCH.get_or_init(|| re(r"第({N})((?:[ノの]{N})+)号"));
        let t = item_branch.replace_all(&t, |c: &regex::Captures| {
            format!("第{}号{}", &c[1], c[2].replace('ノ', "の"))
        });
        // 「同条第三項を第二項に改め」: 番号の付け替え
        static RENUM_NI: OnceLock<Regex> = OnceLock::new();
        let renum_ni = RENUM_NI.get_or_init(|| {
            re(r"(第{N}(?:項|号|条)(?:の{N})*)を((?:同条|同項)?第{N}(?:項|号|条)(?:の{N})*)に改め(る)?")
        });
        let t = renum_ni.replace_all(&t, |c: &regex::Captures| {
            let end = if c.get(3).is_some() {
                "とする"
            } else {
                "とし"
            };
            format!("{}を{}{end}", &c[1], &c[2])
        });
        // 「第五章を六章とし」（第の落ち）、「第四条第二頂」
        // 「同条十二号」（第の落ち）
        static NO_NO: OnceLock<Regex> = OnceLock::new();
        let no_no = NO_NO.get_or_init(|| re(r"(条の{N})見出し"));
        let t = no_no.replace_all(&t, "${1}の見出し");
        static SAME_NO_DAI: OnceLock<Regex> = OnceLock::new();
        let same_no_dai = SAME_NO_DAI.get_or_init(|| re(r"(同条|同項)({N})(号|項)"));
        let t = same_no_dai.replace_all(&t, "${1}第${2}${3}");
        static MADE: OnceLock<Regex> = OnceLock::new();
        let made = MADE.get_or_init(|| re(r"まで({N}(?:項|号|条)ずつ繰り)"));
        let t = made.replace_all(&t, "までを${1}");
        static AND_DAI: OnceLock<Regex> = OnceLock::new();
        // 「第九十四条及び九十五条」（第の落ち）: 前も番号のときだけ（「見出し及び二条を加える」は数）
        let and_dai =
            AND_DAI.get_or_init(|| re(r"(第{N}(?:条|項|号)(?:の{N})*)及び({N})(条|項|号)"));
        let t = and_dai.replace_all(&t, "${1}及び第${2}${3}");
        static NO_DAI: OnceLock<Regex> = OnceLock::new();
        let no_dai = NO_DAI.get_or_init(|| re(r"を({N})(章|節|款)と"));
        let t = no_dai.replace_all(&t, "を第${1}${2}と");
        let t = t.replace("項頂", "項").replace("条第二頂", "条第二項");
        static TYPO_KO: OnceLock<Regex> = OnceLock::new();
        let typo_ko = TYPO_KO.get_or_init(|| re(r"第({N})頂"));
        let t = typo_ko.replace_all(&t, "第${1}項");
        static PAREN_SUB: OnceLock<Regex> = OnceLock::new();
        let paren_sub = PAREN_SUB.get_or_init(|| re(r"号[（(]([{K}])[）)]"));
        let t = paren_sub.replace_all(&t, "号${1}");
        // 「加う」「改む」
        static OLD_VERB: OnceLock<Regex> = OnceLock::new();
        let old_verb = OLD_VERB.get_or_init(|| re(r"(加|改)([うむ])$"));
        let t = old_verb.replace_all(&t, |c: &regex::Captures| {
            if &c[1] == "加" {
                "加える".to_string()
            } else {
                "改める".to_string()
            }
        });
        // 「第三十八条の三第一項「A」を…」の落ちた「中」
        let t = if t.starts_with('第')
            && t.ends_with(['項', '号'])
            && !t.contains('を')
            && !t.contains('に')
        {
            format!("{t}中")
        } else {
            t.to_string()
        };
        // 行の折り返しの名残の空白（「」の外の字下げ）
        static HE: OnceLock<Regex> = OnceLock::new();
        let he = HE.get_or_init(|| re(r"(^|[をの中号ニホ、及び])へ([をとかの中]|$)"));
        let t = he.replace_all(&t, "${1}ヘ${2}").to_string();
        t.replace("までの中", "まで中")
            .replace("を次のように改正する", "を次のように改める")
            .replace("までの各条中", "まで中")
            .replace("号へ中", "号ヘ中")
            .replace("号へを", "号ヘを")
            .replace("から同条第", "から第")
            .replace("から同項第", "から第")
            // 古い書き方
            .replace("づつ", "ずつ")
            .replace("を削除し", "を削り")
            .replace("を削除する", "を削る")
            .replace("に、次の", "に次の")
            .replace("として、", "として")
            .replace("中見出し", "の見出し")
            .replace("章の標題", "章の章名")
            .replace("章標題", "章の章名")
            .replace("繰下げ", "繰り下げ")
            .replace("繰上げ", "繰り上げ")
            .replace("加へ", "加え")
            .replace("中、", "中")
            .replace("項表以外の部分", "項の表以外の部分")
            .replace("までの各号", "まで")
            .replace("までの各項", "まで")
            .replace("その次に次のように加え", "同号の次に次のように加え")
            .replace("その前に次のように加え", "同号の前に次のように加え")
            .replace('，', "、")
            .replace("条の第", "条第")
            .replace("本則中、別に定める場合を除き、", "本則中")
            .replace("の前の次の", "の前に次の")
            .replace("のうち「", "中「")
            .replace("見出中", "見出し中")
            .replace("（同条の前の見出しを含まないものとする。）", "")
            .replace("（同条の前の見出しを除く。）", "")
            .replace("の次に一条を加え", "の次に次の一条を加え")
            .replace("に一項を加え", "に次の一項を加え")
            .replace("イの(", "イ(")
            .replace("ロの(", "ロ(")
            .replace("ハの(", "ハ(")
            .replace("同条の第", "同条第")
            .replace("項項", "項")
            .replace("の欄の", "の欄中")
            .replace("節の標題", "節の節名")
            .replace("の各号列記以外の部分", "各号列記以外の部分")
            .replace("並に", "並びに")
            .replace(['\u{3000}', ' '], "")
    });
    // 古い「「A」を、「B」に改め」の読点
    static WO_COMMA: OnceLock<Regex> = OnceLock::new();
    let wo_comma = WO_COMMA.get_or_init(|| re(r"」を、「([^「」]*)」に"));
    wo_comma
        .replace_all(&out, "」を「${1}」に")
        .replace("」の下に、「", "」の下に「")
        .replace("」の次に「", "」の下に「")
        .replace("」改め", "」に改め")
        .replace("」にに改め", "」に改め")
        .replace("」加え", "」を加え")
        .replace("」ヲ「", "」を「")
        .replace("」をを加え", "」を加え")
        .replace("」加う。", "」を加える。")
}

/// 「」の外だけを `f` で書き換える
fn map_unquoted(s: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    let mut span = String::new();
    for c in s.chars() {
        match c {
            '「' => {
                if depth == 0 {
                    out.push_str(&f(&span));
                    span.clear();
                }
                depth += 1;
                out.push(c);
            }
            '」' if depth > 0 => {
                depth -= 1;
                out.push(c);
            }
            _ if depth > 0 => out.push(c),
            _ => span.push(c),
        }
    }
    out.push_str(&f(&span));
    out
}

pub fn parse_instruction(line: &str) -> Result<Vec<Op>, ParseError> {
    parse_instruction_with(line, &mut Ante::empty())
}

/// 前の文の位置を引き継いで読む（古い改め文は文をまたいで「同条第二項中…」と書く）
fn parse_instruction_with(line: &str, ante_in: &mut Ante) -> Result<Vec<Op>, ParseError> {
    // 字句に「」が入って切れ目がずれる文は、動詞の後の「、」で切り直して読む。
    // それでも読めなければ、字句の閉じ括弧を後ろに続く語（「を「」「に改め」…）で決め、字句の中の「」を伏せて読む
    let saved = ante_in.clone();
    let first = match parse_instruction_split(line, ante_in, false) {
        Ok(v) => return Ok(v),
        Err(e) => e,
    };
    let mut retry = saved.clone();
    if let Ok(v) = parse_instruction_split(line, &mut retry, true) {
        *ante_in = retry;
        return Ok(v);
    }
    for masked in mask_inner_quotes(&normalize_instruction(line), 64) {
        if !masked.contains([MASK_O, MASK_C]) {
            continue;
        }
        for strict in [false, true] {
            let mut retry = saved.clone();
            if let Ok(mut v) = parse_instruction_split(&masked, &mut retry, strict) {
                for op in &mut v {
                    op.map_strings(&unmask);
                }
                *ante_in = retry;
                return Ok(v);
            }
        }
    }
    Err(first)
}

/// 字句の中の「」を伏せる字（私用領域）
const MASK_O: char = '\u{E030}';
const MASK_C: char = '\u{E031}';

fn unmask(s: &str) -> String {
    s.replace(MASK_O, "「").replace(MASK_C, "」")
}

/// 字句（「」で囲んだ部分）の閉じ括弧を、後ろに続く語で決めた読み方の候補（先に閉じるものから、`limit` 個まで）。
/// 字句の中の「」は伏せる。置き換える前の字句（「中「」「、「」の後）は「を「」「を削る」「の下に「」…の前で、
/// 置き換えた後の字句（「を「」「の下に「」の後）は「に改め、」「を加える」…の前で閉じる。
/// 「「附則第五条第二項」と」を「…」: 最初の「」」の後は「と」なので字句の中、次の「」」の後が「を「」なので閉じ
fn mask_inner_quotes(line: &str, limit: usize) -> Vec<String> {
    static FROM: OnceLock<Regex> = OnceLock::new();
    static TO: OnceLock<Regex> = OnceLock::new();
    let from_end = FROM.get_or_init(|| {
        Regex::new(r"^(?:を「|を削(?:り|る)(?:、|。|$)|の(?:下|上|次)に「|、「|及び「|並びに「|とあるのは「|を同)").unwrap()
    });
    let to_end = TO.get_or_init(|| {
        Regex::new(r"^(?:に改め(?:、|る?(?:。|$))|に(?:、|。|$)|を加え(?:、|る?(?:。|$))|を(?:、|。|$)|と、「|と(?:、|。|$))").unwrap()
    });
    fn go(
        chars: &[char],
        i: usize,
        acc: &mut String,
        out: &mut Vec<String>,
        limit: usize,
        from_end: &Regex,
        to_end: &Regex,
    ) {
        if out.len() >= limit {
            return;
        }
        // 次の「まで（字句の外）
        let mut k = i;
        while k < chars.len() && chars[k] != '「' {
            k += 1;
        }
        // 字句の外に閉じ括弧が残る読み方は誤り（前の字句を早く閉じすぎた）
        if chars[i..k].contains(&'」') {
            return;
        }
        let len = acc.len();
        acc.extend(&chars[i..k]);
        if k >= chars.len() {
            out.push(acc.clone());
            acc.truncate(len);
            return;
        }
        let before: String = chars[k.saturating_sub(4)..k].iter().collect();
        let is_to = before.ends_with('を')
            || before.ends_with("下に")
            || before.ends_with("上に")
            || before.ends_with("次に")
            || before.ends_with("あるのは");
        let pat = if is_to { to_end } else { from_end };
        let mut any = false;
        for j in k + 1..chars.len() {
            if chars[j] != '」' {
                continue;
            }
            let after: String = chars[j + 1..].iter().collect();
            if !pat.is_match(&after) {
                continue;
            }
            any = true;
            let l2 = acc.len();
            acc.push('「');
            for &x in &chars[k + 1..j] {
                acc.push(match x {
                    '「' => MASK_O,
                    '」' => MASK_C,
                    x => x,
                });
            }
            acc.push('」');
            go(chars, j + 1, acc, out, limit, from_end, to_end);
            acc.truncate(l2);
            if out.len() >= limit {
                break;
            }
        }
        if !any {
            // 閉じ方が決まらない「は字句の外の字として残す
            acc.push('「');
            go(chars, k + 1, acc, out, limit, from_end, to_end);
        }
        acc.truncate(len);
    }
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    go(
        &chars,
        0,
        &mut String::new(),
        &mut out,
        limit,
        from_end,
        to_end,
    );
    out
}

fn parse_instruction_split(
    line: &str,
    ante_in: &mut Ante,
    strict: bool,
) -> Result<Vec<Op>, ParseError> {
    let normalized = normalize_instruction(line);
    let line = normalized.as_str();
    static RULES: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            ("toc", r"^目次中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            // 見出しは「中」の規則より先に（「第四十二条の見出し中「A」を「B」に改め」）
            ("caption_replace", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出し中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            // 見出しの中の字句の追加・削除: 「第九条の見出し中「A」の下に「B」を加え」「同条の見出し中「A」を削り」
            ("caption_insert", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出し中「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            ("caption_delete_phrase", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出し中「(?P<a>.+?)」を削(?:り|る)$"),
            ("container_title", r"^(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名中「(?P<a>.+?)」(?:を(?:「(?P<b>.+?)」に(?:改め(?:る)?)?|削(?:り|る))|の下に「(?P<c>.+?)」を加え(?:る)?)$"),
            ("container_title_quoted", r"^(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名を「(?P<a>[^「」]+)」に改め(?:る)?$"),
            // 改正法の改正規定そのもの: 「第十八条中X法第四十二条の三の改正規定を次のように改める」「同改正規定の次に次のように加える」
            ("amend_edit", r"^(?P<t>[^「」]*?改正規定(?:（[^）]*）)?(?:のうち[^「」]*?(?:に係る部分)?)?)(?P<act>を次のように改め(?:る)?|を削(?:り|る)|の次に次の(?:ように|改正規定を)加え(?:る)?|に次のように加え(?:る)?)$"),
            ("amend_append", r"^(?P<t>(?:附則)?第{N}条(?:の{N})*)に次の改正規定を加え(?:る)?$"),
            // 「同条を第四章第三節中第三十六条とする」「第四章の二第三節を第四章の三第一節とする」「同条第二項を第百十三条の二の六とする」
            ("move_article", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条)を(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中第(?P<q>{N})条(?P<qb>(?:の{N})*)と(?:し|する)$"),
            ("move_container", r"^(?P<from>(?:第{N}(?:編|章|節|款|目)(?:の{N})*){2,})を(?P<to>(?:第{N}(?:編|章|節|款|目)(?:の{N})*){2,})と(?:し|する)$"),
            ("move_para", r"^(?P<loc>(?:(?:附則)?第{N}条(?:の{N})*|同条)?第{N}項|同項)を(?P<to>(?:附則)?第{N}条(?:の{N})*(?:第{N}項)?)と(?:し|する)$"),
            // 種類の違う位置をまとめて: 「第二編第三章の章名及び第八十一条から第八十八条までを次のように改める」
            ("replace_structure", r"^(?P<scope>[^「」]*?(?:(?:編|章|節|款|目)名|ただし書|見出し)(?:、|及び|並びに)[^「」]+?)を次のように改め(?:る)?$"),
            ("caption_whole", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出しを次のように改め(?:る)?$"),
            // 「第四十条の見出し、第一項及び第三項中「委員長」を「会長」に改め」: 見出しと項の両方
            ("caption_and_phrase", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出し(?:及び|、)(?P<rest>[^「」]+?)中(?P<tail>「.+)$"),
            ("caption_set_bare", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出しを(?P<a>（[^「」（）]+）)に改め(?:る)?$"),
            ("caption_set", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出しを「(?P<a>.+?)」に(?:改め(?:る)?)?$"),
            ("caption_attach", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の前に見出しとして「(?P<a>.+?)」を[付附](?:し|する)$"),
            ("caption_delete", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)の(?:前の)?見出しを削(?:り|る)$"),
            // 「改める」「加える」が付かない形は、同じ文の中で「、」で連なる列挙の途中（「A」を「B」に、「C」を「D」に改める）
            ("replace", r"^(?P<loc>.+?)中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("insert_phrase", r"^(?P<loc>.+?)中「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            // 位置を省いた続き: 「第X中「A」の下に「B」を加え、「C」を「D」に改める」の後半。直前の位置を引き継ぐ
            ("replace_cont", r"^「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            ("insert_phrase_cont", r"^「(?P<a>.+?)」の下に「(?P<b>.+?)」を(?:加え(?:る)?)?$"),
            // 号ずれ（項の中）。「第N項中第A号を第B号とし」「第A号を同条第B号とし」「同号の次に次のK号を加える」
            ("renumber_item", r"^(?P<loc>.+?中|(?:附則)?(?:第{N}条(?:の{N})*|同条)(?:第{N}項|同項)?|同項|(?:附則)?第{N}項)?(?:第(?P<p>{N})号(?P<pb>(?:の{N})*)|(?P<same>同号))を(?:同条|同項)?(?:第{N}項)?第(?P<q>{N})号(?P<qb>(?:の{N})*)と(?:し|する)$"),
            ("shift_items", r"^(?P<loc>.+?中|同条|同項|第{N}条(?:の{N})*(?:第{N}項)?)?(?:第(?P<p>{N})号から第(?P<q>{N})号まで|第(?P<p2>{N})号以下|(?P<list>第{N}号(?:(?:及び|、)第{N}号)+))を(?:順次)?(?P<k>{N})号ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            // 号の下のイロハ: 「同号ロを同号ハとし」「同号中ヘをトとし」「ハからホまでをニからヘまでとし」「同号イの次に次のように加える」
            ("renumber_sub", r"^(?P<loc>.+?中|同号|第{N}条(?:の{N})*(?:第{N}項)?第{N}号(?:の{N})*|同項第{N}号(?:の{N})*|同条第{N}項第{N}号(?:の{N})*)?(?P<a>{S})を(?:同号)?(?P<b>{S})と(?:し|する)$"),
            ("shift_sub", r"^(?P<loc>.+?中|同号|第{N}条(?:の{N})*(?:第{N}項)?第{N}号(?:の{N})*|同項第{N}号(?:の{N})*)?(?P<a>[{K}])から(?P<b>[{K}])までを(?:同号)?(?P<c>[{K}])から(?P<d>[{K}])までと(?:し|する)$"),
            ("insert_sub_after", r"^(?P<loc>.+?中|同号|第{N}条(?:の{N})*(?:第{N}項)?第{N}号(?:の{N})*|同項第{N}号(?:の{N})*)?(?P<a>{S})の次に次のように加え(?:る)?$"),
            // 「第二項の次に第三項として次の一項を加える」「同条に第二項として次の一項を加える」「第七号として次の一号を加える」
            // 「同項に第一号及び第二号として次のように加える」: 加えた後の番号を言う形
            // 「第一条を第一条の二とし、第一条として次の一条を加える」: 付け替えた条の前に
            ("art_add_as", r"^第(?P<n>{N})条(?P<nb>(?:の{N})*)として次の{N}条を加え(?:る)?$"),
            // 「(2)を(3)とし」: 細目の下の番号の付け替え
            ("renumber_subsub", r"^(?:同号[{K}]?(?:中)?)?(?P<a>[（(][０-９0-9一二三四五六七八九十]+[）)])を(?:同号[{K}]?)?(?P<b>[（(][０-９0-9一二三四五六七八九十]+[）)])と(?:し|する)$"),
            // 「第二十五条の四の次に次のように加える」+ 条・款など: 条の後ろに構造を置く
            ("structure_after_art", r"^(?P<loc>第{N}条(?:の{N})*|同条)の次に次のように加え(?:る)?$"),
            ("add_as", r"^(?P<loc>.*?)(?:の(?P<side>次|前)に|に)?第(?P<n>{N})(?P<u>項|号)(?P<nb>(?:の{N})*)(?:(?:及び|、|乃至|から)第{N}(?:項|号)(?:の{N})*(?:まで)?)*として次の(?:{N}(?:項|号)を|ように)加え(?:る)?$"),
            ("insert_item_after", r"^(?P<loc>.+?)の(?P<side>次|前)に次の{N}号を加え(?:る)?$"),
            ("append_item", r"^(?P<loc>.+?)に次の(?:{N}号|各号|号)(?:及び{N}項)?を加え(?:る)?$"),
            ("insert_item_first", r"^(?P<loc>.+?)に第一号として次の{N}号を加え(?:る)?$"),
            // 「同号に次のように加える」+ イロハ: 号の下の列記を足す
            ("append_subitems", r"^(?P<loc>.+?号(?:の{N})*(?:{S})?)に次のように加え(?:る)?$"),
            // 「同条第二項に項番号を付する」: 1 項だけだった条の項に番号を付ける（番号は変わらない）
            ("number_para", r"^(?P<loc>.+?第{N}項|同項)(?:から第(?P<q>{N})項まで|及び(?:同条)?第(?P<q2>{N})項)?に項番号を付(?:し|する)$"),
            ("delete_suppl_note", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条)の付記を削(?:り|る)$"),
            ("set_suppl_note", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条)(?:の付記を次のように改め(?:る)?|に付記として次のように加え(?:る)?)$"),
            ("shift_branch_items", r"^(?P<loc>.+?中|同条|同項)?第(?P<b>{N})号の(?P<p>{N})から第(?P<b2>{N})号の(?P<q>{N})までを(?P<k>{N})号ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("renumber_items_each", r"^(?P<loc>.+?中|同条|同項)?第(?P<a>{N})号及び第(?P<b>{N})号をそれぞれ第(?P<c>{N})号及び第(?P<d>{N})号と(?:し|する)$"),
            ("append_structure", r"^本則に次の{N}条及び{N}(?:編|章|節)を加え(?:る)?$"),
            // 「同号イからニまでを次のように改める」「同号イ及びロを削る」: 号の下のイロハの列挙
            ("subitems_edit", r"^(?P<loc>.*?号(?:の{N})*)?(?P<a>{S})(?:から(?P<b>{S})まで|(?P<list>(?:(?:、|及び){S})+))を(?P<act>次のように改め(?:る)?|削(?:り|る))$"),
            ("renumber_para", r"^(?P<loc>.+?)中第(?P<p>{N})項を第(?P<q>{N})項と(?:し|する)$"),
            // 「同条を同条第二項とし」: 項番号の無い 1 項だけの条の本文を第二項に
            ("renumber_whole_para", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条|附則)を(?:同条|附則)?第(?P<q>{N})項と(?:し|する)$"),
            ("insert_para_first", r"^(?P<loc>第{N}条(?:の{N})*|同条)に第一項として次の一項を加え(?:る)?$"),
            // 「同条中第三項を第五項とし、第二項を第四項とし」の続き（条は直前のもの）
            ("renumber_para_cont", r"^第(?P<p>{N})項を第(?P<q>{N})項と(?:し|する)$"),
            ("shift_paras", r"^(?:(?P<loc>.+?)中|(?P<loc2>同条|附則|第{N}条(?:の{N})*))?(?:(?P<pre>附則)?第(?P<p>{N})項から(?:附則)?第(?P<q>{N})項まで|(?:(?P<pre2>附則)?第(?P<p2>{N})項|(?P<same>同項))以下|第(?P<l1>{N})項及び第(?P<l2>{N})項)を(?:順次)?(?P<k>{N})項ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("renumber_para_same", r"^(?P<loc>(?:.+?中)?同項|.+?第{N}項)を(?:同条|附則)?第(?P<q>{N})項と(?:し|する)$"),
            // 条ずれ: 「第六十一条を第六十四条とする」「同条を第六十三条とし」
            ("renumber_art", r"^(?:本則中|(?P<sup>附則)中|(?P<ctx>(?:同編|同章|同節|同款|第{N}(?:編|章|節|款|目)(?:の{N})*)(?:第{N}(?:編|章|節|款|目)(?:の{N})*)*)中)?(?P<loc>附則第{N}条(?:の{N})*|第{N}条(?:の{N})*|同条)を(?P<qs>附則)?第(?P<q>{N})条(?P<qb>(?:の{N})*)(?:と(?:し|する)|に改め(?:る)?)$"),
            // 「第二号を第一号とし、以下順次一号ずつ繰り上げる」「第四条を削り、以下一条ずつ繰り上げる」: 直前の付け替え・削除の次から（「第Z号まで」が無ければ最後まで）
            ("shift_rest", r"^以下(?:(?:附則|同条|同項)?第(?P<z>{N})(?:条|項|号|編|章|節|款|目)(?:の(?P<zb>{N}))?まで(?:を)?)?(?:各(?:号|項|条)を)?(?:順次)?(?:(?P<k>{N})(?P<u>条|項|号|編|章|節|款|目)ずつ)?繰り(?P<dir>上げ|下げ)(?:る)?$"),
            ("shift_containers", r"^(?:(?P<pre>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?第(?P<p>{N})(?P<k>編|章|節|款|目)から第(?P<q>{N})(?:編|章|節|款|目)までを(?P<n>{N})(?:編|章|節|款|目)ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            // 「第十二条の次に章名として「第三章　審議会」を加え」
            ("heading_quoted", r"^(?P<loc>第{N}条(?:の{N})*|同条)の(?P<side>前|次)に(?:編|章|節|款|目)名として「(?P<a>[^「」]+)」を(?:加え(?:る)?|付(?:し|する))$"),
            // 「ヘの前に次のように加える」
            ("insert_sub_before", r"^(?P<loc>.+?号(?:の{N})*)?(?P<a>{S})の前に次のように加え(?:る)?$"),
            // 「第五章に第一節として次の一節を加える」
            ("container_add_as", r"^(?P<path>同編|同章|同節|同款|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)に第(?P<n>{N})(?P<k>編|章|節|款|目)(?:(?:及び|から)第{N}(?:編|章|節|款|目)(?:まで)?)?として次の{N}(?:編|章|節|款|目)を加え(?:る)?$"),
            ("shift_arts", r"^(?:(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+中)?(?P<pre>附則)?第(?P<p>{N})条から(?:附則)?第(?P<q>{N})条までを(?P<k>{N})条ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            ("shift_branch_arts", r"^第(?P<b>{N})条の(?P<p>{N})から第(?P<b2>{N})条の(?P<q>{N})までを(?P<k>{N})条ずつ繰り(?P<dir>下げ|上げ)(?:る)?$"),
            // 「同項の前に次の一項を加える」
            ("insert_para_before", r"^(?P<loc>.+?)の前に次の{N}項を加え(?:る)?$"),
            ("insert_para_after", r"^(?P<loc>.+?)の次に次の(?:見出し及び)?{N}項を加え(?:る)?$"),
            ("append_para", r"^(?P<loc>.+?)に次の(?:見出し及び)?{N}項を加え(?:る)?$"),
            ("append_art", r"^(?P<path>本則|同編|同章|同節|同款|同目|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)に次の(?:見出し及び)?{N}条を加え(?:る)?$"),
            // 「同条第三項を同条とする」: 残った 1 項を項番号の無い条の本文に（第一項に）
            ("para_to_article", r"^(?P<loc>.+?第{N}項|同項)を同条と(?:し|する)$"),
            // 「第二条に見出しとして「（X）」を加え」
            ("caption_attach2", r"^(?P<loc>(?:附則)?(?:第{N}条(?:の{N})*)?(?:第{N}項)?|同条|同項)に見出しとして「(?P<a>.+?)」を(?:加え(?:る)?|附(?:し|する)|付(?:し|する))$"),
            ("append_suppl_arts", r"^附則に次の(?:見出し及び)?{N}条(?:及び{N}表)?を加え(?:る)?$"),
            ("append_table", r"^(?P<loc>.+?)に次の表を加え(?:る)?$"),
            ("delete_appdx", r"^(?P<list>別表(?:第[一二三四五六七八九十百千]+)?(?:(?:及び|、)別表(?:第[一二三四五六七八九十百千]+)?)*)を削(?:り|る)$"),
            // 別表の行: 全部改正・削除・番号の付け替え・行の追加・別表の追加・行の中の細目
            ("appdx_row_whole", r"^(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<row>.+?)の項を次のように改め(?:る)?$"),
            ("appdx_rows_delete", r"^(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<rows>.+?の項(?:(?:及び|、)(?:同表(?:の|中)?)?.+?の項)*)を削(?:り|る)$"),
            ("appdx_row_renumber", r"^(?:(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?)?(?:(?P<from>.+?)の項|同項)を(?:同表(?:の|中)?)?(?P<to>.+?)の項と(?:し|する)$"),
            ("appdx_rows_insert", r"^(?:(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?)?(?:(?P<after>.+?)の項|同項)の次に次のように加え(?:る)?$"),
            ("append_appdx", r"^(?:附則の次に次の別表|附則の次に(?:附則)?別表として次の{N}表|附則の次に次の{N}表|本則に次の別表)を加え(?:る)?$"),
            ("rename_appdx", r"^(?P<from>別表(?:第[一二三四五六七八九十百千]+)?|同表)を(?P<to>別表(?:第[一二三四五六七八九十百千]+)?)と(?:し|する)$"),
            ("insert_appdx_after", r"^(?P<after>別表(?:第[一二三四五六七八九十百千]+)?|同表)の次に次の{N}表を加え(?:る)?$"),
            ("appdx_row_sub_delete", r"^(?:(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<row>.+?)の項中)?(?P<sub>[イロハニホヘトチリヌルヲワカヨタレソツネナラム])を削(?:り|る)$"),
            ("appdx_row_sub_renumber", r"^(?:(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<row>.+?)の項中)?(?P<a>[イロハニホヘトチリヌルヲワカヨタレソツネナラム])を(?P<b>[イロハニホヘトチリヌルヲワカヨタレソツネナラム])と(?:し|する)$"),
            ("append_containers", r"^(?P<path>本則|同編|同章|同節|同款|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)に次の{N}(?:編|章|節|款|目)(?:及び{N}(?:編|章|節|款|目))*を加え(?:る)?$"),
            ("insert_arts_before", r"^(?:(?:同編|同章|同節|同款|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?P<loc>附則第{N}条(?:の{N})*|第{N}条(?:の{N})*|同条)の前に次の(?:(?:{N}条|見出し)(?:、|及び))*{N}条を加え(?:る)?$"),
            // 「第八条の次に次の二章を加える」: 条の後ろに容器
            // 「第十五条の次に第四章及び第五章として次のように加える」
            ("containers_after_art_as", r"^(?P<loc>第{N}条(?:の{N})*|同条)の次に第{N}(?:編|章|節|款|目)(?:(?:及び|から)第{N}(?:編|章|節|款|目)(?:まで)?)?として次のように加え(?:る)?$"),
            ("containers_after_art", r"^(?:(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+中)?(?P<loc>第{N}条(?:の{N})*|同条)の次に次の{N}(?:編|章|節|款|目)(?:及び{N}(?:編|章|節|款|目))*を加え(?:る)?$"),
            // 「第一条の前に次の章名を加える」「題名の次に次の目次及び章名を附する」
            ("headings_before", r"^(?:(?:同編|同章|同節|同款|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?:(?P<loc>第{N}条(?:の{N})*|同条)の(?P<side>前|次)|題名の次)に次の(?P<what>[^「」を]*?(?:編|章|節|款|目)名[^「」を]*?|{N}条(?:、|及び|並びに)[^「」を]*?{N}(?:編|章|節|款|目)[^「」を]*?)を(?:加え(?:る)?|付(?:し|する)|附(?:し|する))$"),
            // 「第二章の次に次の二章を加える」「第一章中第五節の次に次の二節を加える」「第五節の次に…」（章は直前のもの）
            ("insert_containers_after", r"^(?:(?P<pre>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+|(?:同編|同章|同節|同款|同目)(?:第{N}(?:編|章|節|款|目)(?:の{N})*)*)の(?P<side>次|前)に次の(?:{N}条及び)?{N}(?:編|章|節|款|目)(?:(?:及び|並びに)[^「」を]*?)?を加え(?:る)?$"),
            // 「第三章を第五章とする」「第一章中第八節を第十節とし」「第六節を第八節とし」
            ("renumber_container", r"^(?:(?P<pre>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?P<path>(?:同編|同章|同節|同款)?(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+|同編|同章|同節|同款|同目)を(?:同章|同節|同編|同款)?第(?P<q>{N})(?:編|章|節|款|目)(?P<qb>(?:の{N})*)と(?:し|する)$"),
            // 表の中の位置への操作（上の別表の行の規則に当たらないもの）: 位置は字面のまま
            ("append_appdx_as", r"^(?:附則の次に)?別表として次のように加え(?:る)?$"),
            ("append_before_suppl", r"^附則の前に次の{N}条を加え(?:る)?$"),
            ("table_edit", r"^(?P<target>(?:別表|同表|附則別表|様式|附則様式|付表|附録|付録|備考)[^「」]*?|[^「」]+?の表[^「」]*?|表[^「」]*?|[^「」第同附別]+?表[^「」]*?|同号[^「」]*?|同欄[^「」]*?|同注[^「」]*?|同項|その|第[^「」]+?の式|第{N}条(?:の{N})*[^第の同中、「」][^「」、]*?の項)(?P<act>を次のように改め(?:る)?|を削(?:り|る)|の(?P<side>次|前)に次の[^「」]+を加え(?:る)?|の(?P<side2>次|前)に次のように加え(?:る)?|に[^「」]*?として次のように加え(?:る)?|に[^「」]*?として次の[^「」]+を加え(?:る)?|に次の[^「」]+を加え(?:る)?|に次のように加え(?:る)?|を(?P<to>[^「」]+?)と(?:し|する)|を(?P<k>{N})(?:号|項|条)ずつ繰り(?P<dir>下げ|上げ)(?:る)?)$"),
            ("title_and_toc", r"^(?:題名及び目次|目次及び題名)を次のように改め(?:る)?$"),
            ("delete_toc", r"^目次(?:及び(?P<rest>.+))?を削(?:り|る)$"),
            ("delete_title", r"^題名(?P<toc>及び目次(?:（[^）]*）)?)?を削(?:り|る)$"),
            // 「附則を附則第一条とし」「附則第一項を附則第一条とし」: 項だけの附則を条に
            ("suppl_para_to_art", r"^(?:附則(?:第(?P<p>{N})項)?|(?P<same>同項))を附則第(?P<q>{N})条と(?:し|する)$"),
            // 「同号にイとして次のように加える」「同号ロの次にハとして次のように加える」
            ("add_sub_as", r"^(?P<loc>.+?号(?:の{N})*(?:{S})?)(?:(?P<after>{S})の次)?に(?:同号)?(?P<k>{S})(?:(?:及び|から){S}(?:まで)?)?として次のように加え(?:る)?$"),
            // 「附則第一項の項番号を削る」: 残った 1 項の番号を外す（項番号の無い本文に）
            ("delete_para_num", r"^(?P<loc>.*?第{N}項|同項)(?:及び(?:同条)?第(?P<q>{N})項)?の(?P<cap>見出し及び)?項番号を削(?:り|る)$"),
            ("delete_art_title", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条)の(?P<cap>見出し及び)?条名を削(?:り|る)$"),
            // 「第七十一条の付記中「A」を「B」に改める」
            ("suppl_note", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条)の付記中「(?P<a>.+?)」を「(?P<b>.+?)」に(?:改め(?:る)?)?$"),
            // 「第四章の章名及び同章第一節の節名を次のように改める」
            ("set_container_titles", r"^(?P<a>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名及び(?P<b>(?:同(?:編|章|節|款))?(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名を次のように改め(?:る)?$"),
            ("set_title", r"^題名を次のように改め(?:る)?$"),
            ("set_title_quoted", r"^題名を「(?P<a>[^「」]+)」に改め(?:る)?$"),
            // 題名の無い古い法律に題名を付ける
            ("attach_title", r"^(?:この法律に)?(?:次|左)の題名(?P<toc>及び目次)?を[附付]する$"),
            ("set_toc", r"^題名の次に次の目次を[付附]する$"),
            ("replace_pairs", r"^(?P<scope>.+?)中次の表の上欄に掲げる字句を同表の下欄に掲げる字句に改め(?:る)?$"),
            // 「同項及び同条第四項を同条第四項及び第五項とし」
            ("renumber_paras_pair", r"^(?P<a>同項|(?:同条)?第{N}項)及び(?:同条)?第(?P<b>{N})項を(?:同条)?第(?P<c>{N})項及び第(?P<d>{N})項と(?:し|する)$"),
            ("insert_arts_before_container", r"^(?P<path>同編|同章|同節|同款|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の前に次の(?:{N}条|[^「」を]+)を加え(?:る)?$"),
            ("replace_toc_whole", r"^目次を次のように改め(?:る)?$"),
            ("set_container_title", r"^(?P<path>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)の(?:編|章|節|款|目)名を次のように改め(?:る)?$"),
            ("replace_whole", r"^(?P<loc>.+?)を次のように改め(?:る)?$"),
            ("delete", r"^(?P<loc>.+?)を削(?:り|る)$"),
            ("insert_arts_after", r"^(?:本則中|(?:同編|同章|同節|同款|同目|(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中)?(?P<loc>附則第{N}条(?:の{N})*|第{N}条(?:の{N})*|同条)の次に次の(?:(?:{N}条|見出し)(?:、|及び))*(?P<k>{N})条を加え(?:る)?$"),
            ("append_sentence", r"^(?P<loc>.+?)(?:の下)?に(?:後段として次のように|ただし書として次のように|次のただし書(?:及び各号)?を|次の後段を|次のように後段を)加え(?:る)?$"),
            // 「同条に次の見出しを加え」+「（見出し）」
            ("caption_content", r"^(?P<loc>(?:附則)?第{N}条(?:の{N})*|同条)に次の見出しを(?:加え(?:る)?|付(?:し|する))$"),
            // 「本則を本則第一項とし」: 条の無い本則の 1 項に番号
            ("main_para_number", r"^本則を本則第(?P<q>{N})項と(?:し|する)$"),
            // 「次の但書を加える」: 直前の位置に
            ("append_proviso_here", r"^次のただし書(?:及び{N}項)?を加え(?:る)?$"),
            // 「第六項及び第七項をそれぞれ第三項及び第四項とする」
            ("renumber_paras_each", r"^(?P<loc>.*?)第(?P<a>{N})項及び第(?P<b>{N})項をそれぞれ第(?P<c>{N})項及び第(?P<d>{N})項と(?:し|する)$"),
            ("append_appdx_after_art", r"^第{N}条(?:の{N})*の次に次の別表を加え(?:る)?$"),
        ]
        .iter()
        .map(|(n, s)| (*n, re(s)))
        .collect()
    });
    let mut ante = std::mem::replace(ante_in, Ante::empty());
    let mut ops = Vec::new();
    // 「第十二条中「及び法制局」及び第三項を削り」「「第七章ノ二　…」及び第百五条ノ二から第百五条ノ四までを削る」:
    // 字句と位置を並べて削る。字句を削る断片と位置を削る断片に分ける
    static MIXED_DEL: OnceLock<Regex> = OnceLock::new();
    let mixed_del = MIXED_DEL.get_or_init(|| {
        re(r"^(?P<q>(?:[^「」]*「[^「」]*」)+)及び(?P<l>(?:第|同|ただし書|本文|前段|後段)[^「」]*)を(?P<v>削(?:り|る))$")
    });
    let segs: Vec<String> = split_segments_with(line, strict)
        .into_iter()
        .flat_map(|seg| match mixed_del.captures(&seg) {
            Some(c) => vec![
                format!("{}を{}", &c["q"], &c["v"]),
                format!("{}を{}", &c["l"], &c["v"]),
            ],
            None => vec![seg],
        })
        // 「「A」を「B」に、改める」の読点の後の動詞だけの断片は前の断片の終わり
        .filter(|seg| !matches!(seg.trim(), "改め" | "改める"))
        .collect();
    if std::env::var("LAWEAN_DEBUG_SEGS").is_ok() {
        for (i, x) in segs.iter().enumerate() {
            eprintln!("seg {i}: {x}");
        }
    }
    // 断片ごとの操作の始まり（読めない断片が出たとき、前の断片とつないで読み直すのに使う）
    // 断片ごとの ops の先頭（読めない断片を前後とつないで読み直すときに、そこまで戻す）。
    // 飛ばした断片は None
    let mut op_start: Vec<Option<usize>> = vec![None; segs.len()];
    let mut skip_until = 0usize;
    for (si, seg) in segs.iter().enumerate() {
        if si < skip_until {
            continue;
        }
        op_start[si] = Some(ops.len());
        ante.fresh = si == 0;
        let seg = seg.trim();
        // 「第四章中第四十条の見出しを…」: 条を言う前の容器は先行詞にだけ（条の番号は法律で一意）
        static CPREFIX: OnceLock<Regex> = OnceLock::new();
        let cprefix = CPREFIX.get_or_init(|| {
            re(r"^(?P<same>同編|同章|同節|同款|同目)?(?P<c>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)*)中(?P<rest>(?:第{N}条|第{N}(?:編|章|節|款|目)|同(?:編|章|節|款|目)[^中]).*)$")
        });
        let cstripped;
        let seg = match cprefix.captures(seg) {
            Some(c) if c.name("same").is_some() || !c["c"].is_empty() => {
                // 「同節第二款中第百四十二条の八の次に…」: 直前の容器の中
                let mut path = match c.name("same") {
                    Some(m) => ante_container_upto(&ante, m.as_str().chars().nth(1).unwrap()),
                    None => Vec::new(),
                };
                path.extend(container_path(&c["c"]));
                ante.container = path;
                cstripped = c["rest"].to_string();
                cstripped.as_str()
            }
            _ => seg,
        };
        // 字句そのものに「」に、「」が入る読替え規定の書き換えは「、」で切れてしまう。
        // 読めない断片は、前の断片（字句の操作）とつないで、改め・加え・削りで終わるところまで緩く読み直す
        let try_merge = |ops: &mut Vec<Op>, ante: &mut Ante| -> Option<usize> {
            // 今の断片から前に遡って（今の断片だけで後ろとつなぐ場合も含む）
            for start in (0..=si).rev() {
                let Some(from_op) = op_start[start] else {
                    break;
                };
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
                    let mut a2 = ante.clone();
                    // 「「A」を「B」に、「C」を「D」に改め」の列挙: 「」に、「」で区切ってから、各片を緩く読む
                    // （B や D の中の「」が釣り合わなくても、最初の「」を「」で A と B が分かれる）
                    if let Some(v) = parse_phrase_list_loose(&merged, &mut a2) {
                        ops.truncate(from_op);
                        ops.extend(v);
                        return Some(end + 1);
                    }
                    // 字句の中の「改め」で終わる切れ目でなかったかもしれない: 先まで延ばして読み直す
                }
            }
            None
        };
        // 字句の置換・追加・削除は正規表現でなく手で読む（置換先に「」が入れ子になることがある）。
        // ただし「第百条第三項及び第百三条第四項中」の複数位置は下の規則で展開する。見出しの中は別
        let loc_part = seg.split("中「").next().unwrap_or("");
        let listed =
            (loc_part.contains("及び") || loc_part.contains('、') || loc_part.contains("まで"))
                && !loc_part.starts_with("題名及び")
                && !loc_part.starts_with("本則及び")
                && !loc_part.starts_with("本則（")
                && !(loc_part.starts_with('第') && loc_part.ends_with("を除く。）"))
                && !loc_part.contains("のうち")
                && !loc_part.ends_with("付記")
                && !loc_part.contains("改正規定")
                && !(ante.tedit.is_some()
                    && (loc_part.starts_with("同号")
                        || loc_part.starts_with("同表")
                        || loc_part.starts_with("同注")
                        || loc_part.starts_with(['(', '（'])))
                && !has_article_table(loc_part)
                && !(loc_part.starts_with("同項")
                    && ante.tedit.as_ref().is_some_and(|(_, p)| p.contains("の項")))
                && !loc_part.starts_with("別表")
                && !loc_part.starts_with("附則別表")
                && !loc_part.starts_with("様式")
                && !loc_part.starts_with("付表")
                && !(loc_part.ends_with('表') && !loc_part.contains('条'))
                && !loc_part.starts_with("同表")
                && !loc_part.contains("の表");
        if (!listed || seg.starts_with('「') || seg.ends_with("削り") || seg.ends_with("削る"))
            && (!loc_part.contains("見出し")
                || loc_part.contains("（見出しを含む。）")
                || loc_part.contains("及び")
                || loc_part.contains("並びに"))
            && (!loc_part.ends_with("名")
                || loc_part == "題名"
                || loc_part.contains("及び")
                || loc_part.contains('、'))
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
        // 直前の位置が表の中なら、「同号」はその表の号
        let in_table = ante.tedit.is_some()
            && (seg.starts_with("同号")
                || seg.starts_with("その次")
                || seg.starts_with("同欄")
                || seg.starts_with("同注")
                || seg.starts_with("同項")
                    && !matches!(&ante.tedit, Some((TableRef::InArticle(at), _)) if at.item.is_some())
                    && ante
                        .tedit
                        .as_ref()
                        .is_some_and(|(_, p)| p.contains("の項") || p.ends_with('項')))
            || ante.appdx.as_ref().is_some_and(|(_, r)| !r.is_empty())
                && seg.starts_with("同項の")
                && !seg.contains('「');
        // 別表・同表の中の位置（「同表中第十号を第十一号とし」「別表第一第一号の次に次の一号を加える」）は表の規則だけで読む
        let pre_quote = seg.split('「').next().unwrap_or("");
        // 条の中の表（「第一条の表中Xの項の次に次のように加える」）は表の規則だけで読む（別表の行の規則は別表のもの）
        let article_table_seg = has_article_table(pre_quote);
        let table_seg = seg.starts_with("同表")
            || seg.starts_with("別表")
            || seg.starts_with("附則別表")
            || article_table_seg;
        // 「同表中第十号を第十一号とし、第九号を第十号とし」の続き（条・項を言わない号）は直前の表
        let table_cont = ante.tedit.is_some()
            && !seg.contains('条')
            && !seg.contains('「')
            && (seg.starts_with('第') && !seg.contains('項')
                || seg.starts_with(['(', '（'])
                || seg.chars().next().is_some_and(|c| {
                    KANA.contains(c) || c.is_ascii_digit() || ('０'..='９').contains(&c)
                })
                || !seg.starts_with(['第', '同', '附', '別']) && seg.contains("の項"));
        let cont_seg;
        let seg = if table_cont {
            cont_seg = format!("同表中{seg}");
            cont_seg.as_str()
        } else {
            seg
        };
        for (name, r) in rules.iter() {
            if (in_table || table_cont) && *name != "table_edit" {
                continue;
            }
            let article_table = seg.starts_with("同表")
                && matches!(ante.tedit, Some((TableRef::InArticle(_), _)))
                && ante.appdx.is_none();
            if article_table && *name != "table_edit" {
                continue;
            }
            if article_table_seg && *name != "table_edit" {
                continue;
            }
            if table_seg
                && !(name.starts_with("appdx")
                    || name.ends_with("appdx")
                    || name.ends_with("appdx_as")
                    || *name == "table_edit"
                    || *name == "delete_appdx"
                    || *name == "insert_appdx_after")
            {
                continue;
            }
            let Some(c) = r.captures(seg) else { continue };
            if std::env::var("LAWEAN_DEBUG_RULES").is_ok() {
                eprintln!("rule {name}: {seg}");
            }
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
                    } else if tok.starts_with("別表") || tok.starts_with("附則別表") {
                        // 「第八条第五号及び別表第二第八号中」: 別表の中の位置
                        let Some((table, path)) = split_table_target(tok, &mut ante)? else {
                            return Err(ParseError::Unrecognized(seg.to_string()));
                        };
                        ops.push(Op::TableEdit {
                            table,
                            path,
                            action: TableAction::Phrase {
                                from: from.clone(),
                                to: to.clone(),
                            },
                        });
                    } else if let Some(base) = tok
                        .strip_suffix("の付記")
                        .or_else(|| tok.strip_suffix("の各付記"))
                    {
                        for at in expand_locs(base, &mut ante)? {
                            ops.push(Op::ReplaceSupplNote {
                                article: at.article,
                                from: from.clone(),
                                to: to.clone(),
                            });
                        }
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
                    } else if let Some(base) = tok
                        .strip_suffix("の前の見出し")
                        .or_else(|| tok.strip_suffix("の見出し"))
                    {
                        let l = loc(base, &mut ante)?;
                        ops.push(
                            caption_op(
                                l.clone(),
                                CaptionEdit::Replace {
                                    from: from.clone(),
                                    to: to.clone(),
                                },
                            )
                            .in_suppl(l.suppl),
                        );
                    } else if let Some(base) = ["の章名", "の節名", "の款名", "の編名", "の目名"]
                        .iter()
                        .find_map(|s| tok.strip_suffix(s))
                    {
                        let path = container_path(base);
                        ante.container = path.clone();
                        ops.push(Op::ReplaceContainerTitle {
                            path,
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
                "caption_replace" => caption_op(
                    loc(&g("loc"), &mut ante)?,
                    CaptionEdit::Replace {
                        from: g("a"),
                        to: g("b"),
                    },
                ),
                "caption_insert" => caption_op(
                    loc(&g("loc"), &mut ante)?,
                    CaptionEdit::Replace {
                        from: g("a"),
                        to: format!("{}{}", g("a"), g("b")),
                    },
                ),
                "caption_delete_phrase" => caption_op(
                    loc(&g("loc"), &mut ante)?,
                    CaptionEdit::Replace {
                        from: g("a"),
                        to: String::new(),
                    },
                ),
                "containers_after_art" | "containers_after_art_as" => {
                    Op::InsertContainersAfterArticle {
                        after: loc(&g("loc"), &mut ante)?.article,
                        text: Vec::new(),
                    }
                }
                "headings_before" => Op::InsertHeadingsBefore {
                    before: match g("loc").as_str() {
                        "" => None,
                        l => Some(loc(l, &mut ante)?.article),
                    },
                    after: g("side") == "次",
                    with_toc: g("what").contains("目次"),
                    text: Vec::new(),
                },
                "amend_edit" | "amend_append" => {
                    let t = g("t");
                    let target = match t.strip_prefix("同改正規定") {
                        Some(r) => format!(
                            "{}{r}",
                            ante.amend.clone().unwrap_or_else(|| "同改正規定".into())
                        ),
                        None => t.clone(),
                    };
                    ante.amend = Some(target.clone());
                    let act = g("act");
                    let action = if *name == "amend_append" {
                        TableAction::Append { text: Vec::new() }
                    } else if act.starts_with("を次のように改め") {
                        TableAction::Replace { text: Vec::new() }
                    } else if act.starts_with("を削") {
                        TableAction::Delete
                    } else if act.starts_with("に次") {
                        TableAction::Append { text: Vec::new() }
                    } else {
                        TableAction::InsertAfter { text: Vec::new() }
                    };
                    Op::AmendmentEdit { target, action }
                }
                "move_article" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let to = art_num(&format!("第{}条{}", g("q"), g("qb")));
                    let path = container_path(&g("path"));
                    ante.article = Some(to.clone());
                    ante.container = path.clone();
                    Op::MoveArticle {
                        from: l.article,
                        path,
                        to,
                    }
                }
                "move_container" => {
                    let from = container_path(&g("from"));
                    let to = container_path(&g("to"));
                    ante.container = to.clone();
                    // 同じ容器の中なら番号の付け替え
                    if from.len() == to.len()
                        && from[..from.len() - 1] == to[..to.len() - 1]
                        && from.last().map(|x| x.0) == to.last().map(|x| x.0)
                    {
                        Op::RenumberContainer {
                            path: from,
                            to: to.last().unwrap().1.clone(),
                        }
                    } else {
                        Op::MoveContainer { from, to }
                    }
                }
                "move_para" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let Some(ParaRef::Num(p)) = l.paragraph else {
                        return Err(ParseError::NoAntecedent(seg.to_string()));
                    };
                    Op::MoveParagraph {
                        article: l.article,
                        paragraph: p,
                        to: g("to"),
                    }
                }
                "replace_structure" => Op::ReplaceStructure {
                    scope: g("scope"),
                    text: Vec::new(),
                },
                "caption_whole" => {
                    caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Set(String::new()))
                }
                "caption_and_phrase" => {
                    // 読めなければ（「第百五十八条の見出し、第百五十九条の見出し及び…中」など）位置の列挙の規則で
                    let mut probe = ante.clone();
                    let Ok(l) = loc(&g("loc"), &mut probe) else {
                        matched = false;
                        continue;
                    };
                    let tail = g("tail");
                    let Some((a, r)) = take_quoted(&tail) else {
                        matched = false;
                        continue;
                    };
                    let Some(caps) = phrase_tail(r, &[a], |from, to| {
                        caption_op(
                            l.clone(),
                            CaptionEdit::Replace {
                                from: from.clone(),
                                to,
                            },
                        )
                    }) else {
                        matched = false;
                        continue;
                    };
                    let Ok(Some(PhraseOps(rest))) =
                        parse_phrase_op(&format!("{}中{tail}", g("rest")), &mut probe)
                    else {
                        matched = false;
                        continue;
                    };
                    ante = probe;
                    for op in caps {
                        ops.push(op.in_suppl(l.suppl));
                    }
                    let (last, init) = rest.split_last().expect("phrase ops");
                    for op in init {
                        ops.push(op.clone().in_suppl(ante.suppl));
                    }
                    last.clone()
                }
                "caption_set_bare" => {
                    caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Set(g("a")))
                }
                "caption_set" => caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Set(g("a"))),
                "caption_attach" => {
                    caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Attach(g("a")))
                }
                "caption_delete" => caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Delete),
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
                    // 別表の行の中の細目（「別表第一の一一の二の項中ニをハとし」）
                    static APPDX_ROW_LOC: OnceLock<Regex> = OnceLock::new();
                    let appdx_row_loc = APPDX_ROW_LOC.get_or_init(|| {
                        re(r"^(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<row>.+?)の項$")
                    });
                    if *name == "renumber_sub" {
                        if let Some(c) = appdx_row_loc.captures(l) {
                            let table = match &c["table"] {
                                "同表" => match &ante.appdx {
                                    Some((t, _)) => t.clone(),
                                    None => return Err(ParseError::NoAntecedent(seg.to_string())),
                                },
                                t => t.to_string(),
                            };
                            let row = c["row"].to_string();
                            ante.appdx = Some((table.clone(), row.clone()));
                            ante.article = None;
                            ops.push(Op::RenumberAppdxRowSub {
                                table,
                                row,
                                from: g("a"),
                                to: g("b"),
                            });
                            break;
                        }
                    }
                    // 「ニをハとし」（別表の行の先行詞）
                    if l.is_empty() && ante.article.is_none() {
                        if let Some((table, row)) = ante.appdx.clone() {
                            if *name == "renumber_sub" {
                                ops.push(Op::RenumberAppdxRowSub {
                                    table,
                                    row,
                                    from: g("a"),
                                    to: g("b"),
                                });
                                break;
                            }
                        }
                    }
                    let mut at = if l.is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(l, &mut ante)?
                    };
                    // 「同号イ中(3)を(4)とし」: 括弧だけの細目は位置の細目（無ければ直前の細目）の中
                    let ctx = at.sub.clone().or_else(|| ante.sub.clone());
                    let sub_of = |x: &str| full_sub(x, ctx.as_deref());
                    at.sub = None;
                    at.part = None;
                    if at.item.is_none() {
                        return Err(ParseError::NoAntecedent(seg.to_string()));
                    }
                    match *name {
                        "renumber_sub" => Op::RenumberSubitem {
                            at,
                            from: sub_of(&g("a")),
                            to: sub_of(&g("b")),
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
                            after: sub_of(&g("a")),
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
                    // 「第二号以下を」: 最後まで。「第二号及び第三号を」: 続いた号の列挙
                    let (from, to) = if !g("p2").is_empty() {
                        (num("p2"), u32::MAX)
                    } else if !g("list").is_empty() {
                        let ns: Vec<u32> = g("list")
                            .split(['及', '、'])
                            .map(|x| x.trim_start_matches('び'))
                            .filter_map(|x| {
                                kanji_to_u32(x.trim_start_matches('第').trim_end_matches('号'))
                            })
                            .collect();
                        let (lo, hi) = (*ns.iter().min().unwrap(), *ns.iter().max().unwrap());
                        if (hi - lo + 1) as usize != ns.len() {
                            return Err(ParseError::Unrecognized(seg.to_string()));
                        }
                        (lo, hi)
                    } else {
                        (num("p"), num("q"))
                    };
                    Op::ShiftItems {
                        at: Loc { item: None, ..at },
                        from,
                        to,
                        by: if g("dir") == "下げ" { k } else { -k },
                    }
                }
                "caption_content" => {
                    caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Set(String::new()))
                }
                "main_para_number" => Op::RenumberParagraph {
                    article: ArticleNum::Single {
                        base: 0,
                        branch: vec![],
                    },
                    from: ParaRef::Num(1),
                    to: num("q"),
                },
                "replace_pairs" => Op::ReplacePairs {
                    scope: g("scope"),
                    text: Vec::new(),
                },
                "renumber_paras_pair" => {
                    let article = ante
                        .article
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    let a = if g("a") == "同項" {
                        ante.paragraph
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?
                    } else {
                        kanji_to_u32(
                            g("a")
                                .trim_start_matches("同条")
                                .trim_start_matches('第')
                                .trim_end_matches('項'),
                        )
                        .ok_or_else(|| ParseError::Unrecognized(seg.to_string()))?
                    };
                    // 後ろの項から付け替える（番号が重ならないように）
                    ops.push(
                        Op::RenumberParagraph {
                            article: article.clone(),
                            from: ParaRef::Num(num("b")),
                            to: num("d"),
                        }
                        .in_suppl(ante.suppl),
                    );
                    Op::RenumberParagraph {
                        article,
                        from: ParaRef::Num(a),
                        to: num("c"),
                    }
                }
                "insert_arts_before_container" => {
                    let path = match g("path").as_str() {
                        p if p.starts_with('同') => {
                            let k = p.chars().nth(1).unwrap_or('章');
                            ante_container_upto(&ante, k)
                        }
                        p => container_path(p),
                    };
                    Op::InsertContainersBefore {
                        path,
                        text: Vec::new(),
                    }
                }
                "set_suppl_note" => Op::SetSupplNote {
                    article: loc(&g("loc"), &mut ante)?.article,
                    text: Vec::new(),
                },
                "shift_branch_items" => {
                    if num("b") != num("b2") {
                        return Err(ParseError::Unrecognized(seg.to_string()));
                    }
                    let at = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(g("loc").trim_end_matches('中'), &mut ante)?
                    };
                    Op::ShiftBranchItems {
                        at: Loc { item: None, ..at },
                        base: num("b"),
                        from: num("p"),
                        to: num("q"),
                        by: num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 },
                    }
                }
                "renumber_items_each" => {
                    let at = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(g("loc").trim_end_matches('中'), &mut ante)?
                    };
                    let at = Loc { item: None, ..at };
                    // 後ろの号から付け替える
                    ops.push(
                        Op::RenumberItem {
                            at: at.clone(),
                            from: num("b").to_string(),
                            to: num("d").to_string(),
                        }
                        .in_suppl(ante.suppl),
                    );
                    Op::RenumberItem {
                        at,
                        from: num("a").to_string(),
                        to: num("c").to_string(),
                    }
                }
                "append_structure" => Op::AppendContainers {
                    path: Vec::new(),
                    text: Vec::new(),
                },
                "delete_suppl_note" => Op::DeleteSupplNote {
                    article: loc(&g("loc"), &mut ante)?.article,
                },
                "number_para" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let Some(ParaRef::Num(p)) = l.paragraph else {
                        return Err(ParseError::Unrecognized(seg.to_string()));
                    };
                    // 「同条第二項から第六項までに項番号を付する」: 各項に
                    let last = [g("q"), g("q2")]
                        .iter()
                        .find(|x| !x.is_empty())
                        .and_then(|x| kanji_to_u32(x))
                        .unwrap_or(p);
                    for k in p..last {
                        ops.push(
                            Op::RenumberParagraph {
                                article: l.article.clone(),
                                from: ParaRef::Num(k),
                                to: k,
                            }
                            .in_suppl(ante.suppl),
                        );
                    }
                    let p = last;
                    Op::RenumberParagraph {
                        article: l.article,
                        from: ParaRef::Num(p),
                        to: p,
                    }
                }
                "subitems_edit" => {
                    let at = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(g("loc").trim_end_matches('中'), &mut ante)?
                    };
                    let ctx = at.sub.clone().or_else(|| ante.sub.clone());
                    let a = full_sub(&g("a"), ctx.as_deref());
                    let subs: Vec<String> = if g("b").is_empty() {
                        std::iter::once(a.clone())
                            .chain(
                                g("list")
                                    .split(['、', '及'])
                                    .map(|x| x.trim_start_matches('び'))
                                    .filter(|x| !x.is_empty())
                                    .map(|x| full_sub(x, Some(&a))),
                            )
                            .collect()
                    } else if KANA.contains(g("a").as_str()) && KANA.contains(g("b").as_str()) {
                        (kana_index(&g("a"))..=kana_index(&g("b")))
                            .map(kana_of)
                            .collect()
                    } else {
                        vec![a.clone(), full_sub(&g("b"), Some(&a))]
                    };
                    let at = Loc { sub: None, ..at };
                    Op::SubitemsEdit {
                        at,
                        subs,
                        text: if g("act").starts_with('削') {
                            None
                        } else {
                            Some(Vec::new())
                        },
                    }
                }
                "art_add_as" => {
                    let n = art_num(&format!("第{}条{}", g("n"), g("nb")));
                    // 直前にその番号の条を付け替えていれば、付け替えた後の条の前
                    let moved = ops.iter().rev().find_map(|o| match o {
                        Op::RenumberArticle {
                            from,
                            to,
                            suppl: false,
                        } if *from == n => Some(to.clone()),
                        _ => None,
                    });
                    match moved {
                        Some(before) => Op::InsertArticleBefore {
                            before,
                            text: Vec::new(),
                            suppl: false,
                        },
                        None => return Err(ParseError::NoAntecedent(seg.to_string())),
                    }
                }
                "renumber_subsub" => {
                    let at = ante_loc(&ante, seg)?;
                    Op::RenumberSubitem {
                        at,
                        from: g("a"),
                        to: g("b"),
                    }
                }
                "structure_after_art" => Op::InsertHeadingsBefore {
                    before: Some(loc(&g("loc"), &mut ante)?.article),
                    after: true,
                    with_toc: false,
                    text: Vec::new(),
                },
                "add_as" => {
                    let l = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(&g("loc"), &mut ante)?
                    };
                    let n = num("n");
                    let branched = !g("nb").is_empty();
                    if g("u") == "号" {
                        let at = Loc {
                            item: None,
                            ..l.clone()
                        };
                        let after = match g("side").as_str() {
                            "次" => l.item.clone(),
                            "前" => {
                                let before = l
                                    .item
                                    .clone()
                                    .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                                ops.push(
                                    Op::InsertItemBefore {
                                        at,
                                        before,
                                        text: Vec::new(),
                                    }
                                    .in_suppl(ante.suppl),
                                );
                                break;
                            }
                            _ if branched => Some(n.to_string()),
                            _ if n > 1 => Some((n - 1).to_string()),
                            _ => None,
                        };
                        match after {
                            Some(after) => Op::InsertItemAfter {
                                at,
                                after,
                                text: Vec::new(),
                            },
                            None => Op::InsertItemFirst {
                                at,
                                text: Vec::new(),
                            },
                        }
                    } else {
                        let after = match g("side").as_str() {
                            "次" => l.paragraph.clone(),
                            "前" => match l.paragraph.clone() {
                                Some(ParaRef::Num(p)) if p > 1 => Some(ParaRef::Num(p - 1)),
                                Some(_) => None,
                                None => return Err(ParseError::NoAntecedent(seg.to_string())),
                            },
                            _ if branched => Some(ParaRef::Num(n)),
                            _ if n > 1 => Some(ParaRef::Num(n - 1)),
                            _ => None,
                        };
                        match after {
                            Some(after) => Op::InsertParagraphAfter {
                                article: l.article,
                                after,
                                text: Vec::new(),
                            },
                            None => Op::InsertParagraphFirst {
                                article: l.article,
                                text: Vec::new(),
                            },
                        }
                    }
                }
                "insert_item_after" if g("loc").contains("及び") || g("loc").contains('、') => {
                    // 「第百六十四条第一項第三号及び第百六十五条第一項第三号の次に次の一号を加える」: 位置ごとに同じ内容
                    let locs = expand_locs(&g("loc"), &mut ante)?;
                    let mut v: Vec<Op> = Vec::new();
                    for l in locs {
                        let after = l
                            .item
                            .clone()
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                        v.push(Op::InsertItemAfter {
                            at: Loc { item: None, ..l },
                            after,
                            text: Vec::new(),
                        });
                    }
                    let last = v
                        .pop()
                        .ok_or_else(|| ParseError::Unrecognized(seg.to_string()))?;
                    for op in v {
                        ops.push(op.in_suppl(ante.suppl));
                    }
                    last
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
                "insert_item_first" => Op::InsertItemFirst {
                    at: loc(&g("loc"), &mut ante)?,
                    text: Vec::new(),
                },
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
                "renumber_whole_para" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    ante.paragraph = Some(num("q"));
                    Op::RenumberParagraph {
                        article: l.article,
                        from: ParaRef::Num(1),
                        to: num("q"),
                    }
                }
                "insert_para_first" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    Op::InsertParagraphFirst {
                        article: l.article,
                        text: Vec::new(),
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
                    // 位置: 「第五条中」「同条」「附則中」「附則第三項から附則第五項まで」、無ければ直前の条
                    let l = [g("loc"), g("loc2"), g("pre"), g("pre2")]
                        .into_iter()
                        .find(|x| !x.is_empty());
                    let article = match l {
                        Some(l) => loc(&l, &mut ante)?.article,
                        None => ante
                            .article
                            .clone()
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?,
                    };
                    let (from, to) = if !g("same").is_empty() {
                        let p = ante
                            .paragraph
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                        (p, u32::MAX)
                    } else if !g("p2").is_empty() {
                        (num("p2"), u32::MAX)
                    } else if !g("l1").is_empty() {
                        // 「第四項及び第五項を一項ずつ繰り上げ」
                        if num("l2") != num("l1") + 1 {
                            return Err(ParseError::Unrecognized(seg.to_string()));
                        }
                        (num("l1"), num("l2"))
                    } else {
                        (num("p"), num("q"))
                    };
                    let by = num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 };
                    Op::ShiftParagraphs {
                        article,
                        from,
                        to,
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
                    // 「第六章の二中第五十五条の三を…とし、同章を第六章の三とする」: 容器の先行詞
                    if g("ctx").starts_with('第') {
                        ante.container = container_path(&g("ctx"));
                    }
                    let l = if g("sup").is_empty() {
                        loc(&g("loc"), &mut ante)?
                    } else {
                        // 「附則中第七条を第八条とし」
                        loc(&format!("附則{}", g("loc")), &mut ante)?
                    };
                    let to = art_num(&format!("第{}条{}", g("q"), g("qb")));
                    // 以後の「同条」は新しい番号
                    ante.article = Some(to.clone());
                    Op::RenumberArticle {
                        from: l.article,
                        to,
                        suppl: l.suppl || g("qs") == "附則",
                    }
                }
                "append_appdx_as" | "append_appdx_after_art" => {
                    Op::AppendAppdx { text: Vec::new() }
                }
                "append_proviso_here" => Op::AppendSentence {
                    at: ante_loc(&ante, seg)?,
                    text: Vec::new(),
                },
                "renumber_paras_each" => {
                    let article = if g("loc").is_empty() {
                        ante.article
                            .clone()
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?
                    } else {
                        loc(g("loc").trim_end_matches('中'), &mut ante)?.article
                    };
                    ops.push(
                        Op::RenumberParagraph {
                            article: article.clone(),
                            from: ParaRef::Num(num("a")),
                            to: num("c"),
                        }
                        .in_suppl(ante.suppl),
                    );
                    Op::RenumberParagraph {
                        article,
                        from: ParaRef::Num(num("b")),
                        to: num("d"),
                    }
                }
                "append_before_suppl" => Op::AppendArticle {
                    path: Vec::new(),
                    text: Vec::new(),
                },
                "table_edit" => {
                    // 表でなければ（直前が表でない「同号」など）他の規則で読む
                    let mut probe = ante.clone();
                    let Ok(Some((table, path))) = split_table_target(&g("target"), &mut probe)
                    else {
                        matched = false;
                        continue;
                    };
                    ante = probe;
                    let act = g("act");
                    let action = if act.starts_with("を次のように改め") {
                        TableAction::Replace { text: Vec::new() }
                    } else if act.starts_with("を削") {
                        TableAction::Delete
                    } else if g("side") == "次" || g("side2") == "次" {
                        TableAction::InsertAfter { text: Vec::new() }
                    } else if g("side") == "前" || g("side2") == "前" {
                        TableAction::InsertBefore { text: Vec::new() }
                    } else if act.starts_with("に次")
                        || act.ends_with("として次のように加え")
                        || act.ends_with("として次のように加える")
                    {
                        TableAction::Append { text: Vec::new() }
                    } else if !g("k").is_empty() {
                        TableAction::Shift {
                            by: num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 },
                        }
                    } else {
                        TableAction::Renumber { to: g("to") }
                    };
                    ante.appdx = None;
                    ante.tedit = Some((table.clone(), path.clone()));
                    Op::TableEdit {
                        table,
                        path,
                        action,
                    }
                }
                "title_and_toc" => Op::SetTitleAndToc { text: Vec::new() },
                "delete_toc" => {
                    if !g("rest").is_empty() {
                        // 「目次及び第一章の章名を削る」
                        let rest = g("rest");
                        let Some(path) = rest
                            .strip_suffix("の章名")
                            .or_else(|| rest.strip_suffix("の編名"))
                        else {
                            return Err(ParseError::Unrecognized(seg.to_string()));
                        };
                        ops.push(Op::DeleteToc);
                        Op::DeleteContainerTitle {
                            path: container_path(path),
                        }
                    } else {
                        Op::DeleteToc
                    }
                }
                "delete_title" => {
                    if !g("toc").is_empty() {
                        ops.push(Op::DeleteToc);
                    }
                    Op::DeleteTitle
                }
                "suppl_para_to_art" => {
                    let to = ArticleNum::Single {
                        base: num("q"),
                        branch: vec![],
                    };
                    let from = if !g("same").is_empty() {
                        ante.paragraph
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?
                    } else if g("p").is_empty() {
                        1
                    } else {
                        num("p")
                    };
                    ante.article = Some(to.clone());
                    ante.suppl = true;
                    Op::ParagraphToArticle { from, to }
                }
                "add_sub_as" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let first_paren = |k: &str| {
                        let inner = k.trim_start_matches(['(', '（']);
                        inner.starts_with(['1', '１', '一', 'i', 'ｉ'])
                    };
                    match g("after").as_str() {
                        // 「同号イに(1)として次のように加える」: 細目の下に列記を足す
                        "" if g("k").starts_with(['(', '（']) && first_paren(&g("k")) => {
                            Op::AppendItem {
                                at: l,
                                text: Vec::new(),
                            }
                        }
                        "" if g("k").starts_with(['(', '（']) => Op::InsertSubitemAfter {
                            at: Loc {
                                sub: None,
                                ..l.clone()
                            },
                            after: full_sub(&g("k"), l.sub.as_deref()),
                            text: Vec::new(),
                        },
                        "" if g("k") == "イ" => Op::AppendItem {
                            at: l,
                            text: Vec::new(),
                        },
                        "" => {
                            let prev = kana_of(kana_index(&g("k")) - 1);
                            Op::InsertSubitemAfter {
                                at: l,
                                after: prev,
                                text: Vec::new(),
                            }
                        }
                        a => Op::InsertSubitemAfter {
                            at: l,
                            after: a.to_string(),
                            text: Vec::new(),
                        },
                    }
                }
                "delete_para_num" => {
                    let l = loc(&g("loc").replace("附則中", "附則"), &mut ante)?;
                    let from = l
                        .paragraph
                        .clone()
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    if !g("cap").is_empty() {
                        ops.push(caption_op(l.clone(), CaptionEdit::Delete).in_suppl(l.suppl));
                    }
                    // 「同項及び同条第三項の項番号を削り」: 二つの項の番号を外す（項番号の無い段落に）
                    if !g("q").is_empty() {
                        ops.push(
                            Op::RenumberParagraph {
                                article: l.article.clone(),
                                from: ParaRef::Num(num("q")),
                                to: 1,
                            }
                            .in_suppl(l.suppl),
                        );
                    }
                    Op::RenumberParagraph {
                        article: l.article,
                        from,
                        to: 1,
                    }
                }
                "delete_art_title" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    if !g("cap").is_empty() {
                        ops.push(caption_op(l.clone(), CaptionEdit::Delete).in_suppl(l.suppl));
                    }
                    Op::DeleteArticleTitle { article: l.article }
                }
                "suppl_note" => Op::ReplaceSupplNote {
                    article: loc(&g("loc"), &mut ante)?.article,
                    from: g("a"),
                    to: g("b"),
                },
                "set_container_titles" => {
                    let a = container_path(&g("a"));
                    let b = match g("b").strip_prefix('同') {
                        // 「同章第一節」: 直前（a）の章の中
                        Some(r) => {
                            let r = r.trim_start_matches(['編', '章', '節', '款']);
                            let mut p = a.clone();
                            p.truncate(1);
                            p.extend(container_path(r));
                            p
                        }
                        None => container_path(&g("b")),
                    };
                    ante.container = b.clone();
                    Op::SetContainerTitles {
                        paths: vec![a, b],
                        text: Vec::new(),
                    }
                }
                "set_title_quoted" => Op::SetTitle { text: vec![g("a")] },
                "attach_title" if !g("toc").is_empty() => {
                    // 「次の題名及び目次を付する」: 題名の行と「目次」以下の行
                    ops.push(Op::SetTitle { text: Vec::new() });
                    Op::SetToc {
                        text: Vec::new(),
                        replace: false,
                    }
                }
                "attach_title" => Op::SetTitle { text: Vec::new() },
                "shift_rest" => {
                    let to = if g("z").is_empty() {
                        u32::MAX
                    } else {
                        num("z")
                    };
                    let base = |n: &str| n.split('_').next().and_then(|b| b.parse::<u32>().ok());
                    let (prev, suppl) = match ops.last() {
                        Some(Op::Suppl(inner)) => ((**inner).clone(), true),
                        Some(op) => (op.clone(), false),
                        None => return Err(ParseError::NoAntecedent(seg.to_string())),
                    };
                    let bad = || ParseError::Unrecognized(seg.to_string());
                    // 「以下順次繰り上げる」: ずらす数を言わなければ直前の付け替えと同じだけ（削ったなら 1 つ）
                    let sign = if g("dir") == "下げ" { 1 } else { -1 };
                    let k = if g("k").is_empty() {
                        let d = match &prev {
                            Op::RenumberItem { from, to, .. } => {
                                base(to).zip(base(from)).map(|(t, f)| t.abs_diff(f))
                            }
                            Op::RenumberParagraph {
                                from: ParaRef::Num(f),
                                to: t,
                                ..
                            } => Some(t.abs_diff(*f)),
                            Op::RenumberArticle {
                                from:
                                    ArticleNum::Single {
                                        base: f,
                                        branch: fb,
                                    },
                                to:
                                    ArticleNum::Single {
                                        base: t,
                                        branch: tb,
                                    },
                                ..
                            } => Some(if f == t {
                                fb.first()
                                    .zip(tb.first())
                                    .map(|(a, b)| a.abs_diff(*b))
                                    .unwrap_or(1)
                            } else {
                                t.abs_diff(*f)
                            }),
                            _ => Some(1),
                        };
                        d.filter(|d| *d > 0).ok_or_else(bad)? as i32 * sign
                    } else {
                        num("k") as i32 * sign
                    };
                    let unit = match (g("u").as_str(), &prev) {
                        ("", Op::RenumberItem { .. }) => "号",
                        ("", Op::RenumberParagraph { .. }) => "項",
                        ("", Op::RenumberArticle { .. }) => "条",
                        ("", Op::Delete { at }) if at.item.is_some() => "号",
                        ("", Op::Delete { at }) if at.paragraph.is_some() => "項",
                        ("", Op::Delete { .. }) => "条",
                        ("", Op::RenumberContainer { .. }) => "容器",
                        ("編" | "章" | "節" | "款" | "目", _) => "容器",
                        (u, _) => u,
                    }
                    .to_string();
                    let op = match (unit.as_str(), prev) {
                        // 「第六章を第七章とし、以下順次一章ずつ繰り下げ」
                        ("容器", Op::RenumberContainer { path, .. }) => {
                            let Some(((kind, num), parent)) = path.split_last() else {
                                return Err(bad());
                            };
                            let n: u32 = num.parse().map_err(|_| bad())?;
                            Op::ShiftContainers {
                                path: parent.to_vec(),
                                kind: *kind,
                                from: n + 1,
                                to,
                                by: k,
                            }
                        }
                        (
                            "容器",
                            Op::DeleteContainers {
                                path, kind, to: t, ..
                            },
                        ) => Op::ShiftContainers {
                            path,
                            kind,
                            from: t + 1,
                            to,
                            by: k,
                        },
                        // 「第二十八条の三を第二十八条の二とし、以下第二十八条の六まで一条ずつ繰り上げる」: 枝番
                        (
                            "条",
                            Op::RenumberArticle {
                                from: ArticleNum::Single { base: b, branch },
                                suppl: false,
                                ..
                            },
                        ) if branch.len() == 1 => Op::ShiftBranchArticles {
                            base: b,
                            from: branch[0] + 1,
                            to: if g("zb").is_empty() {
                                u32::MAX
                            } else {
                                num("zb")
                            },
                            by: k,
                        },
                        ("号", Op::RenumberItem { at, from, .. }) => Op::ShiftItems {
                            from: base(&from).ok_or_else(bad)? + 1,
                            at,
                            to,
                            by: k,
                        },
                        ("号", Op::Delete { at }) if at.item.is_some() => Op::ShiftItems {
                            from: base(at.item.as_deref().unwrap()).ok_or_else(bad)? + 1,
                            at: Loc { item: None, ..at },
                            to,
                            by: k,
                        },
                        (
                            "項",
                            Op::RenumberParagraph {
                                article,
                                from: ParaRef::Num(p),
                                ..
                            },
                        ) => Op::ShiftParagraphs {
                            article,
                            from: p + 1,
                            to,
                            by: k,
                        },
                        ("項", Op::Delete { at })
                            if at.item.is_none() && at.paragraph.is_some() =>
                        {
                            let Some(ParaRef::Num(p)) = at.paragraph else {
                                unreachable!()
                            };
                            Op::ShiftParagraphs {
                                article: at.article,
                                from: p + 1,
                                to,
                                by: k,
                            }
                        }
                        (
                            "条",
                            Op::RenumberArticle {
                                from: ArticleNum::Single { base, branch },
                                suppl: false,
                                ..
                            },
                        ) if branch.is_empty() => Op::ShiftArticles {
                            from: base + 1,
                            to,
                            by: k,
                        },
                        ("条", Op::Delete { at }) if at.paragraph.is_none() && !at.suppl => {
                            match at.article {
                                ArticleNum::Single { base, branch } if branch.is_empty() => {
                                    Op::ShiftArticles {
                                        from: base + 1,
                                        to,
                                        by: k,
                                    }
                                }
                                _ => return Err(bad()),
                            }
                        }
                        _ => return Err(bad()),
                    };
                    if suppl {
                        Op::Suppl(Box::new(op))
                    } else {
                        op
                    }
                }
                "shift_containers" => {
                    let by = num("n") as i32 * if g("dir") == "下げ" { 1 } else { -1 };
                    let kind = match g("k").as_str() {
                        "編" => lawean_source::ContainerKind::Part,
                        "章" => lawean_source::ContainerKind::Chapter,
                        "節" => lawean_source::ContainerKind::Section,
                        "款" => lawean_source::ContainerKind::Subsection,
                        _ => lawean_source::ContainerKind::Division,
                    };
                    Op::ShiftContainers {
                        path: container_path(&g("pre")),
                        kind,
                        from: num("p"),
                        to: num("q"),
                        by,
                    }
                }
                "heading_quoted" => Op::InsertHeadingsBefore {
                    before: Some(loc(&g("loc"), &mut ante)?.article),
                    after: g("side") == "次",
                    with_toc: false,
                    text: vec![g("a")],
                },
                "insert_sub_before" => {
                    let at = if g("loc").is_empty() {
                        ante_loc(&ante, seg)?
                    } else {
                        loc(&g("loc"), &mut ante)?
                    };
                    let k = kana_index(&g("a"));
                    if k > 1 {
                        Op::InsertSubitemAfter {
                            at: Loc { sub: None, ..at },
                            after: kana_of(k - 1),
                            text: Vec::new(),
                        }
                    } else {
                        Op::AppendItem {
                            at: Loc { sub: None, ..at },
                            text: Vec::new(),
                        }
                    }
                }
                "container_add_as" => {
                    let parent = match g("path").as_str() {
                        p if p.starts_with('同') => {
                            let k = p.chars().nth(1).unwrap_or('章');
                            ante_container_upto(&ante, k)
                        }
                        p => container_path(p),
                    };
                    let kind = match g("k").as_str() {
                        "編" => lawean_source::ContainerKind::Part,
                        "章" => lawean_source::ContainerKind::Chapter,
                        "節" => lawean_source::ContainerKind::Section,
                        "款" => lawean_source::ContainerKind::Subsection,
                        _ => lawean_source::ContainerKind::Division,
                    };
                    let n = num("n");
                    // 同じ文で番号を変えた容器があれば、その変えた後の番号の前に
                    let moved = ops.iter().rev().find_map(|o| match o {
                        Op::RenumberContainer { path, to }
                            if path.last() == Some(&(kind, n.to_string())) =>
                        {
                            Some(to.clone())
                        }
                        _ => None,
                    });
                    let mut path = parent;
                    path.push((kind, moved.unwrap_or_else(|| n.to_string())));
                    Op::InsertContainersBefore {
                        path,
                        text: Vec::new(),
                    }
                }
                "shift_branch_arts" => {
                    if num("b") != num("b2") {
                        return Err(ParseError::Unrecognized(seg.to_string()));
                    }
                    let by = num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 };
                    Op::ShiftBranchArticles {
                        base: num("b"),
                        from: num("p"),
                        to: num("q"),
                        by,
                    }
                }
                "shift_arts" => {
                    let by = num("k") as i32 * if g("dir") == "下げ" { 1 } else { -1 };
                    let op = Op::ShiftArticles {
                        from: num("p"),
                        to: num("q"),
                        by,
                    };
                    // 「附則第三条から第十三条までを一条ずつ繰り下げ」: 原始附則の条
                    if g("pre").is_empty() {
                        op
                    } else {
                        Op::Suppl(Box::new(op))
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
                "insert_para_before" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    match l.paragraph {
                        Some(ParaRef::Num(p)) if p > 1 => Op::InsertParagraphAfter {
                            article: l.article,
                            after: ParaRef::Num(p - 1),
                            text: Vec::new(),
                        },
                        Some(_) => Op::InsertParagraphFirst {
                            article: l.article,
                            text: Vec::new(),
                        },
                        None => return Err(ParseError::NoAntecedent(seg.to_string())),
                    }
                }
                "para_to_article" => {
                    let l = loc(&g("loc"), &mut ante)?;
                    let from = l
                        .paragraph
                        .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?;
                    Op::RenumberParagraph {
                        article: l.article,
                        from,
                        to: 1,
                    }
                }
                "caption_attach2" => {
                    caption_op(loc(&g("loc"), &mut ante)?, CaptionEdit::Attach(g("a")))
                }
                "append_art" => Op::AppendArticle {
                    path: match g("path").as_str() {
                        // 「同節に次の一条を加える」: 直前の容器（の節まで）
                        p if p.starts_with('同') => {
                            ante_container_upto(&ante, p.chars().nth(1).unwrap_or('章'))
                        }
                        p => container_path(p),
                    },
                    text: Vec::new(),
                },
                "append_suppl_arts" => Op::AppendSupplArticles { text: Vec::new() },
                "append_table" => Op::AppendTable {
                    at: loc(&g("loc"), &mut ante)?,
                    text: Vec::new(),
                },
                "appdx_row_whole"
                | "appdx_rows_delete"
                | "appdx_row_renumber"
                | "appdx_rows_insert"
                | "appdx_row_sub_delete"
                | "appdx_row_sub_renumber" => {
                    // 別表と行の先行詞（「同表」「同項」）
                    let table = match g("table").as_str() {
                        "" | "同表" => match &ante.appdx {
                            Some((t, _)) => t.clone(),
                            // 別表でない（条の中の表の行、号の下のイロハ）: 他の規則で読む
                            None if ante.tedit.is_some() || ante.article.is_some() => {
                                matched = false;
                                continue;
                            }
                            None => return Err(ParseError::NoAntecedent(seg.to_string())),
                        },
                        t => t.to_string(),
                    };
                    let row_of = |r: &str, ante: &Ante| -> Result<String, ParseError> {
                        if r == "同" || r.is_empty() {
                            return ante
                                .appdx
                                .as_ref()
                                .map(|(_, x)| x.clone())
                                .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()));
                        }
                        Ok(r.to_string())
                    };
                    match *name {
                        "appdx_row_whole" => {
                            let row = row_of(&g("row"), &ante)?;
                            ante.appdx = Some((table.clone(), row.clone()));
                            ante.article = None;
                            Op::ReplaceAppdxRowWhole {
                                table,
                                row,
                                text: Vec::new(),
                            }
                        }
                        "appdx_rows_delete" => {
                            let rows: Vec<String> = g("rows")
                                .split("及び")
                                .flat_map(|x| x.split('、'))
                                .filter_map(|x| {
                                    x.trim()
                                        .trim_start_matches("同表")
                                        .trim_start_matches(['の', '中'])
                                        .strip_suffix("の項")
                                        .map(str::to_string)
                                })
                                .collect();
                            if rows.is_empty() {
                                return Err(ParseError::Unrecognized(seg.to_string()));
                            }
                            ante.appdx = Some((table.clone(), rows[0].clone()));
                            ante.article = None;
                            Op::DeleteAppdxRows { table, rows }
                        }
                        "appdx_row_renumber" => {
                            // 「同項を同表の九の項とし」: from は直前の行
                            let from = row_of(
                                g("from")
                                    .trim_start_matches("同表")
                                    .trim_start_matches(['の', '中']),
                                &ante,
                            )?;
                            let to = g("to")
                                .trim_start_matches("同表")
                                .trim_start_matches(['の', '中'])
                                .to_string();
                            ante.appdx = Some((table.clone(), to.clone()));
                            ante.article = None;
                            Op::RenumberAppdxRow { table, from, to }
                        }
                        "appdx_rows_insert" => {
                            // 「同項の次に次のように加える」: 直前の行
                            let after = row_of(&g("after"), &ante)?;
                            ante.appdx = Some((table.clone(), after.clone()));
                            ante.article = None;
                            Op::InsertAppdxRowsAfter {
                                table,
                                after,
                                text: Vec::new(),
                            }
                        }
                        "appdx_row_sub_delete" => {
                            let row = row_of(&g("row"), &ante)?;
                            ante.appdx = Some((table.clone(), row.clone()));
                            ante.article = None;
                            Op::DeleteAppdxRowSub {
                                table,
                                row,
                                sub: g("sub"),
                            }
                        }
                        _ => {
                            let row = row_of(&g("row"), &ante)?;
                            ante.appdx = Some((table.clone(), row.clone()));
                            ante.article = None;
                            Op::RenumberAppdxRowSub {
                                table,
                                row,
                                from: g("a"),
                                to: g("b"),
                            }
                        }
                    }
                }
                "append_appdx" => Op::AppendAppdx { text: Vec::new() },
                "rename_appdx" => {
                    let from = match g("from").as_str() {
                        "同表" => same_appdx(&ante)
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?,
                        t => t.to_string(),
                    };
                    let to = g("to");
                    ante.appdx = Some((to.clone(), String::new()));
                    ante.article = None;
                    Op::RenameAppdx { from, to }
                }
                "insert_appdx_after" => {
                    let after = match g("after").as_str() {
                        "同表" => same_appdx(&ante)
                            .ok_or_else(|| ParseError::NoAntecedent(seg.to_string()))?,
                        t => t.to_string(),
                    };
                    Op::InsertAppdxAfter {
                        after,
                        text: Vec::new(),
                    }
                }
                "delete_appdx" => Op::DeleteAppdx {
                    tables: g("list")
                        .split("及び")
                        .flat_map(|x| x.split('、'))
                        .map(str::to_string)
                        .collect(),
                },
                "append_containers" => Op::AppendContainers {
                    path: match g("path").as_str() {
                        "本則" => Vec::new(),
                        // 「同章に次の一節を加える」: 直前の容器（の章まで）
                        p if p.starts_with('同') => {
                            let depth = match p {
                                "同編" => 1,
                                "同章" => ante_container_upto(&ante, '章').len(),
                                "同節" => ante_container_upto(&ante, '節').len(),
                                _ => ante_container_upto(&ante, '款').len(),
                            };
                            let mut v = ante.container.clone();
                            v.truncate(depth.max(1));
                            if v.is_empty() {
                                return Err(ParseError::NoAntecedent(seg.to_string()));
                            }
                            v
                        }
                        p => container_path(p),
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
                "container_title" => {
                    let path = container_path(&g("path"));
                    ante.container = path.clone();
                    ante.article = None;
                    Op::ReplaceContainerTitle {
                        path,
                        from: g("a"),
                        to: if g("c").is_empty() {
                            g("b")
                        } else {
                            format!("{}{}", g("a"), g("c"))
                        },
                    }
                }
                "container_title_quoted" => {
                    let path = container_path(&g("path"));
                    ante.container = path.clone();
                    ante.article = None;
                    Op::SetContainerTitle {
                        path,
                        text: vec![g("a")],
                    }
                }
                "set_title" => Op::SetTitle { text: Vec::new() },
                "set_toc" => Op::SetToc {
                    text: Vec::new(),
                    replace: false,
                },
                "replace_toc_whole" => Op::SetToc {
                    text: Vec::new(),
                    replace: true,
                },
                "set_container_title" => Op::SetContainerTitle {
                    path: container_path(&g("path")),
                    text: Vec::new(),
                },
                "insert_containers_after" => {
                    // 「同節第一款の次に」: 直前の容器のうち「節」までを取り、その中の「第一款」
                    let path = match g("path").strip_prefix('同') {
                        Some(rest) => {
                            let kind = rest.chars().next().unwrap_or('章');
                            let mut p = ante_container_upto(&ante, kind);
                            p.extend(container_path(
                                rest.trim_start_matches(['編', '章', '節', '款', '目']),
                            ));
                            p
                        }
                        None => container_path_with_ante(&g("pre"), &g("path"), &mut ante),
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
                    // 「同節を同章第二節とし」: 直前の容器。「同款第七目の二」は直前の容器の中
                    let path = if g("path").starts_with('同') && g("path").chars().count() > 2 {
                        container_path_with_ante(&g("pre"), &g("path"), &mut ante)
                    } else if g("path").starts_with('同') {
                        if ante.container.is_empty() {
                            return Err(ParseError::NoAntecedent(seg.to_string()));
                        }
                        ante.container.clone()
                    } else {
                        container_path_with_ante(&g("pre"), &g("path"), &mut ante)
                    };
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
                    let l = without_caption_mention(&g("loc"));
                    let l = match l.split_once("及び") {
                        Some((first, _)) if l.contains('項') && !l.ends_with('号') => {
                            first.to_string()
                        }
                        // 「第九条第三項から第五項までを次のように改める」も同じ（内容の番号で各項に当てる）
                        _ => match l.split_once("から") {
                            Some((first, last))
                                if last.ends_with("項まで") && first.ends_with('項') =>
                            {
                                first.to_string()
                            }
                            _ => l,
                        },
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
                    } else if let Some((k, rest)) = ["同編", "同章", "同節", "同款"]
                        .iter()
                        .find_map(|h| {
                            l.strip_prefix(h)
                                .map(|r| (h.chars().nth(1).unwrap(), r.trim_start_matches('中')))
                        })
                        .filter(|(_, r)| r.starts_with('第') && is_container_list(r))
                    {
                        // 「同節中第三款を次のように改める」: 直前の容器の中
                        let mut path = ante_container_upto(&ante, k);
                        path.extend(container_path(rest));
                        Op::ReplaceContainers {
                            paths: vec![path],
                            text: Vec::new(),
                        }
                    } else if l.starts_with('第') && is_container_list(&l) {
                        // 「第四章及び第五章を次のように改める」
                        let paths = l
                            .split("及び")
                            .flat_map(|x| x.split('、'))
                            .map(|t| t.replace("中第", "第"))
                            .flat_map(|t| container_range(&t))
                            .collect();
                        Op::ReplaceContainers {
                            paths,
                            text: Vec::new(),
                        }
                    } else if item_list_tail(&l)
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
                    } else if l.contains("から")
                        && l.ends_with("まで")
                        && !l.contains('項')
                        && !l.contains('号')
                        && !l.ends_with(['章', '節', '款', '編', '目'])
                    {
                        // 「第七条から第九条までを次のように改める」: 条の範囲をまとめて
                        let articles = expand_locs(&l, &mut ante)?
                            .into_iter()
                            .map(|x| x.article)
                            .collect();
                        Op::ReplaceArticles {
                            articles,
                            text: Vec::new(),
                        }
                    } else if l.contains("及び")
                        && !l.contains('項')
                        && !l.contains('号')
                        && l.rsplit("及び")
                            .next()
                            .is_some_and(|t| t.starts_with('第') && t.contains('条'))
                    {
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
                    let mut l = without_caption_mention(&g("loc"));
                    // 別表の行の中の細目（「別表第一の一一の二の項中ハを削り」「同表の…の項中ハを削り」）
                    static APPDX_SUB_DEL: OnceLock<Regex> = OnceLock::new();
                    let appdx_sub_del = APPDX_SUB_DEL.get_or_init(|| {
                        re(r"^(?P<table>別表(?:第[一二三四五六七八九十百千]+)?|同表)(?:の|中)?(?P<row>.+?)の項中(?P<sub>[{K}])$")
                    });
                    if let Some(c) = appdx_sub_del.captures(&l) {
                        let table = match &c["table"] {
                            "同表" => match &ante.appdx {
                                Some((t, _)) => t.clone(),
                                None => return Err(ParseError::NoAntecedent(seg.to_string())),
                            },
                            t => t.to_string(),
                        };
                        let row = c["row"].to_string();
                        ante.appdx = Some((table.clone(), row.clone()));
                        ante.article = None;
                        ops.push(Op::DeleteAppdxRowSub {
                            table,
                            row,
                            sub: c["sub"].to_string(),
                        });
                        break;
                    }
                    // 「ハを削り」（別表の行の先行詞）
                    if l.chars().count() == 1 && KANA.contains(&l) && ante.article.is_none() {
                        if let Some((table, row)) = ante.appdx.clone() {
                            ops.push(Op::DeleteAppdxRowSub {
                                table,
                                row,
                                sub: l.clone(),
                            });
                            break;
                        }
                    }
                    // 「第二章第三節中第四款を削り」: 容器の中の容器
                    static CCTX: OnceLock<Regex> = OnceLock::new();
                    let cctx = CCTX.get_or_init(|| {
                        re(r"^(?P<c>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+)中(?P<r>(?:第{N}(?:編|章|節|款|目)(?:の{N})*)+(?:から.+まで)?)$")
                    });
                    if let Some(c) = cctx.captures(&l) {
                        l = format!("{}{}", &c["c"], &c["r"]);
                    }
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
                    // 「第二章の二を削り」: 枝番の容器
                    static CBRANCH: OnceLock<Regex> = OnceLock::new();
                    let cbranch = CBRANCH.get_or_init(|| {
                        re(r"^(?:第{N}(?:編|章|節|款|目)(?:の{N})*)*第{N}(?:編|章|節|款|目)(?:の{N})+$")
                    });
                    let structural = tokens.iter().any(|t| {
                        t.starts_with("別表")
                            || t.ends_with("付記")
                            || t.ends_with('名')
                            || t.ends_with("見出し")
                            || crange.is_match(t)
                            || csingle.is_match(t)
                            || cbranch.is_match(t)
                            || t.starts_with("同章")
                            || t.starts_with("同編")
                            || t.contains("まで")
                    }) || tokens.len() > 1;
                    if structural {
                        let mut last_container: Vec<(lawean_source::ContainerKind, String)> =
                            Vec::new();
                        for t in tokens {
                            // 「第二十六条の前の見出し、同条から第二十六条の三まで…を削る」: 見出しも
                            // 「同項後段及び同条の付記を削る」
                            if let Some(base) = t.strip_suffix("の付記") {
                                let l = loc(base, &mut ante)?;
                                ops.push(Op::DeleteSupplNote { article: l.article });
                                continue;
                            }
                            if let Some((base, part)) = [
                                ("後段", SentencePart::Back),
                                ("前段", SentencePart::Front),
                                ("ただし書", SentencePart::Proviso),
                            ]
                            .iter()
                            .find_map(|(sfx, p)| t.strip_suffix(sfx).map(|b| (b, *p)))
                            {
                                let at = loc(base, &mut ante)?;
                                ops.push(Op::DeleteSentencePart { at, part }.in_suppl(ante.suppl));
                                continue;
                            }
                            // 「第六条から第十二条まで及び別表を削る」
                            if t.starts_with("別表") || t.starts_with("附則別表") {
                                ops.push(Op::DeleteAppdx {
                                    tables: vec![t.to_string()],
                                });
                                continue;
                            }
                            if let Some(base) = t
                                .strip_suffix("の前の見出し")
                                .or_else(|| t.strip_suffix("の見出し"))
                            {
                                let l = loc(base, &mut ante)?;
                                ops.push(
                                    caption_op(l.clone(), CaptionEdit::Delete).in_suppl(l.suppl),
                                );
                                continue;
                            }
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
                            } else if cbranch.is_match(t) {
                                let path = container_path(t);
                                last_container = path.clone();
                                ops.push(Op::DeleteContainer { path });
                            } else if let Some(rest) = t
                                .strip_prefix("同編")
                                .filter(|r| csingle.is_match(r) || r.ends_with('名'))
                            {
                                // 「第二編の編名、同編第一章及び第二章並びに同編第三章の章名を削る」
                                let mut path = last_container.clone();
                                path.truncate(1);
                                if let Some(base) = rest.strip_suffix("の章名") {
                                    path.extend(container_path(base));
                                    ops.push(Op::DeleteContainerTitle { path });
                                } else {
                                    let c = csingle.captures(rest).unwrap();
                                    let from = kanji_to_u32(&c["p"]).unwrap_or(0);
                                    ops.push(Op::DeleteContainers {
                                        path,
                                        kind: lawean_source::ContainerKind::Chapter,
                                        from,
                                        to: from,
                                    });
                                }
                            } else if let Some(rest) =
                                t.strip_prefix("同章").filter(|r| csingle.is_match(r))
                            {
                                // 「第三章の章名及び同章第一節を削る」: 直前の章の中の節
                                let c = csingle.captures(rest).unwrap();
                                let kind = match &c["k"] {
                                    "節" => lawean_source::ContainerKind::Section,
                                    "款" => lawean_source::ContainerKind::Subsection,
                                    _ => lawean_source::ContainerKind::Division,
                                };
                                let mut path = last_container.clone();
                                path.truncate(1);
                                let from = kanji_to_u32(&c["p"]).unwrap_or(0);
                                ops.push(Op::DeleteContainers {
                                    path,
                                    kind,
                                    from,
                                    to: from,
                                });
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
                _ => {
                    // 「第百五十条第三項、…及び第二百四十四条に次のただし書を加える」: 位置ごとに同じ内容
                    let l = g("loc");
                    if l.contains("及び") || l.contains('、') {
                        let locs = expand_locs(&l, &mut ante)?;
                        let Some((last, init)) = locs.split_last() else {
                            return Err(ParseError::Unrecognized(seg.to_string()));
                        };
                        for at in init {
                            ops.push(
                                Op::AppendSentence {
                                    at: at.clone(),
                                    text: Vec::new(),
                                }
                                .in_suppl(ante.suppl),
                            );
                        }
                        Op::AppendSentence {
                            at: last.clone(),
                            text: Vec::new(),
                        }
                    } else {
                        Op::AppendSentence {
                            at: loc(&l, &mut ante)?,
                            text: Vec::new(),
                        }
                    }
                }
            };
            ops.push(op.in_suppl(ante.suppl));
            break;
        }
        // 表の操作でない断片の後は、表の先行詞を持ち越さない（「同号」「第N号」が表の中を指さないように）
        if matched {
            let from = op_start[si].unwrap_or(ops.len());
            let table_op = |o: &Op| {
                matches!(
                    o,
                    Op::TableEdit { .. }
                        | Op::ReplaceTableRow { .. }
                        | Op::ReplaceAppdxRow { .. }
                        | Op::ReplaceAppdxRowWhole { .. }
                        | Op::DeleteAppdxRows { .. }
                        | Op::RenumberAppdxRow { .. }
                        | Op::InsertAppdxRowsAfter { .. }
                        | Op::DeleteAppdxRowSub { .. }
                        | Op::RenumberAppdxRowSub { .. }
                )
            };
            if ops.len() > from && !ops[from..].iter().any(table_op) {
                ante.tedit = None;
            }
        }
        if !matched {
            if let Some(next) = try_merge(&mut ops, &mut ante) {
                skip_until = next;
                continue;
            }
            return Err(ParseError::Unrecognized(seg.to_string()));
        }
    }
    *ante_in = ante;
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

    /// 読替え規定の書き換え（字句そのものに「」が入って釣り合わない）で、最初の断片から後ろとつないで読み直す。
    /// 令5-53: 「第六十三条第二項中「）」とあるのは」を「）」とあるのは、」に改め、「、「電子調書」とあるのは…」を削る」
    #[test]
    fn loose_merge_starts_at_the_first_segment() {
        let ops = parse_instruction(
            "第六十三条第二項中「）」とあるのは」を「）」とあるのは、」に改め、「、「電子調書」とあるのは「調書」と」を削る。",
        )
        .unwrap();
        assert_eq!(ops.len(), 2, "{ops:?}");
        assert!(matches!(&ops[0], Op::Replace { from, to, .. }
            if from == "）」とあるのは" && to == "）」とあるのは、"));
        assert!(matches!(&ops[1], Op::Replace { from, to, .. }
            if from == "、「電子調書」とあるのは「調書」と" && to.is_empty()));
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
