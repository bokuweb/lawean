//! `Op` / `Instruction` / `AmendUnit` → 改め文。`lawean-amend::parse` の逆で、往復（parse ∘ render = id）をテストで保証する。
//! 構造化記述で改正を書き、改め文を生成するための出口（ADR-0012）。

use crate::numeral::to_kanji;
use lawean_amend::*;
use lawean_source::ArticleNum;

pub fn article_label(n: &ArticleNum) -> String {
    match n {
        ArticleNum::Single { base, branch } => {
            let mut s = format!("第{}条", to_kanji(*base));
            for b in branch {
                s.push_str(&format!("の{}", to_kanji(*b)));
            }
            s
        }
        ArticleNum::Range { from, to } => {
            format!("{}から{}まで", article_label(from), article_label(to))
        }
        ArticleNum::Other(s) => s.clone(),
    }
}

fn loc_label(l: &Loc) -> String {
    let mut s = match &l.paragraph {
        Some(ParaRef::Num(n)) => format!("{}第{}項", article_label(&l.article), to_kanji(*n)),
        None => article_label(&l.article),
    };
    if let Some(item) = &l.item {
        // 「3_2」→「第三号の二」
        let mut parts = item.split('_').filter_map(|x| x.parse::<u32>().ok());
        if let Some(first) = parts.next() {
            s.push_str(&format!("第{}号", to_kanji(first)));
            for b in parts {
                s.push_str(&format!("の{}", to_kanji(b)));
            }
        }
    }
    s
}

/// 1 操作を、文の途中（`last = false`）または文末（`last = true`）の形にする
fn segment(op: &Op, last: bool) -> String {
    let end = |mid: &str, fin: &str| {
        if last {
            fin.to_string()
        } else {
            mid.to_string()
        }
    };
    match op {
        Op::ReplaceToc { from, to } => {
            format!("目次中「{from}」を「{to}」に{}", end("改め", "改める"))
        }
        Op::Replace { at, from, to } => format!(
            "{}中「{from}」を「{to}」に{}",
            loc_label(at),
            end("改め", "改める")
        ),
        Op::InsertAfterPhrase { at, anchor, text } => format!(
            "{}中「{anchor}」の下に「{text}」を{}",
            loc_label(at),
            end("加え", "加える")
        ),
        Op::AppendParagraph { article, .. } => format!(
            "{}に次の一項を{}",
            article_label(article),
            end("加え", "加える")
        ),
        Op::InsertParagraphAfter {
            article,
            after: ParaRef::Num(n),
            ..
        } => {
            format!(
                "{}第{}項の次に次の一項を{}",
                article_label(article),
                to_kanji(*n),
                end("加え", "加える")
            )
        }
        Op::AppendArticle { chapter, .. } => format!(
            "第{}章に次の一条を{}",
            to_kanji(*chapter),
            end("加え", "加える")
        ),
        Op::ReplaceArticle { article, .. } => format!(
            "{}を次のように{}",
            article_label(article),
            end("改め", "改める")
        ),
        Op::InsertArticleAfter { after, .. } => format!(
            "{}の次に次の一条を{}",
            article_label(after),
            end("加え", "加える")
        ),
        Op::AppendSentence { at, .. } => format!(
            "{}に後段として次のように{}",
            loc_label(at),
            end("加え", "加える")
        ),
        Op::RenumberParagraph {
            article,
            from: ParaRef::Num(p),
            to,
        } => {
            format!(
                "{}中第{}項を第{}項と{}",
                article_label(article),
                to_kanji(*p),
                to_kanji(*to),
                end("し", "する")
            )
        }
        Op::ShiftParagraphs {
            article,
            from,
            to,
            by,
        } => format!(
            "{}中第{}項から第{}項までを{}項ずつ繰り{}",
            article_label(article),
            to_kanji(*from),
            to_kanji(*to),
            to_kanji(by.unsigned_abs()),
            if *by > 0 {
                end("下げ", "下げる")
            } else {
                end("上げ", "上げる")
            }
        ),
        Op::Delete { at } => format!("{}を{}", loc_label(at), end("削り", "削る")),
    }
}

fn content_of(op: &Op) -> &[String] {
    match op {
        Op::AppendParagraph { text, .. }
        | Op::InsertParagraphAfter { text, .. }
        | Op::AppendArticle { text, .. }
        | Op::InsertArticleAfter { text, .. }
        | Op::AppendSentence { text, .. }
        | Op::ReplaceArticle { text, .. } => text,
        _ => &[],
    }
}

/// 1 文（複数操作は「、」で連ねる）。追加する条文はインデント 1 で続ける
pub fn render_instruction(ins: &Instruction) -> String {
    let n = ins.ops.len();
    let mut s = String::from("\u{3000}\u{3000}");
    for (i, op) in ins.ops.iter().enumerate() {
        if i > 0 {
            s.push('、');
        }
        s.push_str(&segment(op, i + 1 == n));
    }
    s.push_str("。\n");
    for op in &ins.ops {
        for line in content_of(op) {
            // 見出し「（…）」はインデント 2、それ以外は 1（衆議院の体裁）
            let indent = if line.starts_with('（') {
                "\u{3000}\u{3000}"
            } else {
                "\u{3000}"
            };
            s.push_str(indent);
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

pub fn render_unit(u: &AmendUnit) -> String {
    let mut s = format!(
        "{}\u{3000}{}の一部を次のように改正する。\n",
        u.article_of_amending_law, u.target_title
    );
    for ins in &u.instructions {
        s.push_str(&render_instruction(ins));
    }
    s
}

pub fn render_units(units: &[AmendUnit]) -> String {
    units.iter().map(render_unit).collect::<Vec<_>>().join("")
}
