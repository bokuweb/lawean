//! 改正単位を Source IR のリビジョンに適用する（溶け込み）。
//! 対象が無ければ失敗する（発射台の不一致）。適用後は XML を経由して再パースし、stable_id を振り直す。

use crate::op::*;
use lawean_source::xml::{Element, Node};
use lawean_source::*;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApplyError {
    #[error("第{0}条が無い")]
    ArticleNotFound(String),
    #[error("第{article}条第{paragraph}項が無い")]
    ParagraphNotFound { article: String, paragraph: u32 },
    #[error("第{0}章が無い")]
    ChapterNotFound(u32),
    #[error("{at} に「{phrase}」が無い")]
    PhraseNotFound { at: String, phrase: String },
    #[error("目次に「{0}」が無い")]
    TocPhraseNotFound(String),
    #[error("第{article}条の項番号が連続しない: {labels:?}")]
    ParagraphNumbering { article: String, labels: Vec<u32> },
    #[error("追加する条文の形式が読めない: {0}")]
    BadContent(String),
    #[error("re-parse: {0}")]
    Reparse(String),
}

/// 改正単位を適用した新しいリビジョンを返す
pub fn apply_unit(
    doc: &LegalDocument,
    unit: &AmendUnit,
    new_version_id: &str,
) -> Result<LegalDocument, ApplyError> {
    let mut doc = doc.clone();
    for ins in &unit.instructions {
        apply_instruction(&mut doc, ins)?;
    }
    check_numbering(&doc)?;
    refresh(&mut doc, new_version_id)
}

/// 1 文の中の項番号は文の始まりの番号（改正前）で解釈する
fn apply_instruction(doc: &mut LegalDocument, ins: &Instruction) -> Result<(), ApplyError> {
    // 条ごとに、文の始まりの項ラベル
    let mut snapshots: BTreeMap<String, Vec<Option<u32>>> = BTreeMap::new();
    for op in &ins.ops {
        match op {
            Op::ReplaceToc { from, to } => replace_toc(doc, from, to)?,
            Op::Replace { at, from, to } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?;
                let n = replace_in_article(art, idx, from, to);
                if n == 0 {
                    return Err(ApplyError::PhraseNotFound {
                        at: loc_name(at),
                        phrase: from.clone(),
                    });
                }
            }
            Op::InsertAfterPhrase { at, anchor, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?;
                let n = replace_in_article(art, idx, anchor, &format!("{anchor}{text}"));
                if n == 0 {
                    return Err(ApplyError::PhraseNotFound {
                        at: loc_name(at),
                        phrase: anchor.clone(),
                    });
                }
            }
            Op::AppendParagraph { article, text } => {
                let art = article_mut(doc, article)?;
                snapshot(art, &mut snapshots);
                let p = parse_paragraph(text)?;
                art.children.push(ArticleChild::Paragraph(p));
                snapshots
                    .get_mut(&article.to_num_string())
                    .unwrap()
                    .push(None);
            }
            Op::InsertParagraphAfter {
                article,
                after,
                text,
            } => {
                let art = article_mut(doc, article)?;
                let idx = para_index(art, &Some(after.clone()), &mut snapshots)?.unwrap();
                let p = parse_paragraph(text)?;
                let pos = nth_paragraph_child(art, idx) + 1;
                art.children.insert(pos, ArticleChild::Paragraph(p));
                snapshots
                    .get_mut(&article.to_num_string())
                    .unwrap()
                    .insert(idx + 1, None);
            }
            Op::RenumberParagraph { article, from, to } => {
                let art = article_mut(doc, article)?;
                let idx = para_index(art, &Some(from.clone()), &mut snapshots)?.unwrap();
                set_label(paragraph_mut(art, idx), *to);
            }
            Op::ShiftParagraphs {
                article,
                from,
                to,
                by,
            } => {
                let art = article_mut(doc, article)?;
                snapshot(art, &mut snapshots);
                let snap = snapshots[&article.to_num_string()].clone();
                for (i, orig) in snap.iter().enumerate() {
                    if let Some(n) = orig {
                        if *n >= *from && *n <= *to {
                            set_label(paragraph_mut(art, i), (*n as i32 + by) as u32);
                        }
                    }
                }
            }
            Op::AppendArticle { chapter, text } => {
                let a = parse_article(text)?;
                let ch = chapter_mut(doc, *chapter)?;
                ch.children.push(Provision::Article(a));
            }
            Op::ReplaceArticle { article, text } => {
                let a = parse_article(text)?;
                let art = article_mut(doc, article)?;
                art.caption = a.caption;
                art.title = a.title;
                art.children = a.children;
            }
            Op::Delete { at } => {
                let art = article_mut(doc, &at.article)?;
                match para_index(art, &at.paragraph, &mut snapshots)? {
                    Some(idx) => {
                        let pos = nth_paragraph_child(art, idx);
                        art.children.remove(pos);
                        snapshots
                            .get_mut(&at.article.to_num_string())
                            .unwrap()
                            .remove(idx);
                    }
                    None => {
                        // 条の削除: 見出し・本文を「削除」にする（条番号は残す。法制執務の慣行）
                        art.caption = None;
                        art.children = vec![ArticleChild::Paragraph(parse_paragraph(&[
                            "削除".to_string()
                        ])?)];
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 位置の解決

fn loc_name(l: &Loc) -> String {
    match &l.paragraph {
        Some(ParaRef::Num(n)) => format!("第{}条第{n}項", l.article.to_num_string()),
        None => format!("第{}条", l.article.to_num_string()),
    }
}

fn find_article<'a>(ps: &'a mut [Provision], num: &ArticleNum) -> Option<&'a mut Article> {
    for p in ps {
        match p {
            Provision::Article(a) if &a.num == num => return Some(a),
            Provision::Container(c) => {
                if let Some(a) = find_article(&mut c.children, num) {
                    return Some(a);
                }
            }
            _ => {}
        }
    }
    None
}

fn article_mut<'a>(
    doc: &'a mut LegalDocument,
    num: &ArticleNum,
) -> Result<&'a mut Article, ApplyError> {
    find_article(&mut doc.main_provision, num)
        .ok_or_else(|| ApplyError::ArticleNotFound(num.to_num_string()))
}

fn chapter_mut(doc: &mut LegalDocument, n: u32) -> Result<&mut Container, ApplyError> {
    fn go(ps: &mut [Provision], n: u32) -> Option<&mut Container> {
        for p in ps {
            if let Provision::Container(c) = p {
                if c.kind == ContainerKind::Chapter && c.num.as_deref() == Some(&n.to_string()) {
                    return Some(c);
                }
                if let Some(x) = go(&mut c.children, n) {
                    return Some(x);
                }
            }
        }
        None
    }
    go(&mut doc.main_provision, n).ok_or(ApplyError::ChapterNotFound(n))
}

fn paragraphs(art: &Article) -> Vec<&Paragraph> {
    art.children
        .iter()
        .filter_map(|c| match c {
            ArticleChild::Paragraph(p) => Some(p),
            _ => None,
        })
        .collect()
}

fn paragraph_mut(art: &mut Article, idx: usize) -> &mut Paragraph {
    art.children
        .iter_mut()
        .filter_map(|c| match c {
            ArticleChild::Paragraph(p) => Some(p),
            _ => None,
        })
        .nth(idx)
        .unwrap()
}

/// idx 番目の Paragraph が children の何番目か
fn nth_paragraph_child(art: &Article, idx: usize) -> usize {
    art.children
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, ArticleChild::Paragraph(_)))
        .nth(idx)
        .map(|(i, _)| i)
        .unwrap()
}

fn label(p: &Paragraph) -> u32 {
    p.num.parse().unwrap_or(0)
}

fn snapshot(art: &Article, snapshots: &mut BTreeMap<String, Vec<Option<u32>>>) {
    snapshots
        .entry(art.num.to_num_string())
        .or_insert_with(|| paragraphs(art).iter().map(|p| Some(label(p))).collect());
}

/// 文の始まりの番号で項を引く。None は条全体
fn para_index(
    art: &Article,
    r: &Option<ParaRef>,
    snapshots: &mut BTreeMap<String, Vec<Option<u32>>>,
) -> Result<Option<usize>, ApplyError> {
    snapshot(art, snapshots);
    let Some(ParaRef::Num(n)) = r else {
        return Ok(None);
    };
    let snap = &snapshots[&art.num.to_num_string()];
    snap.iter()
        .position(|o| *o == Some(*n))
        .map(Some)
        .ok_or_else(|| ApplyError::ParagraphNotFound {
            article: art.num.to_num_string(),
            paragraph: *n,
        })
}

// ---------------------------------------------------------------- テキスト操作

fn sentences_mut(p: &mut Paragraph) -> Vec<&mut Sentence> {
    fn item<'a>(i: &'a mut Item, out: &mut Vec<&'a mut Sentence>) {
        match &mut i.body {
            ItemBody::Sentences(ss) => out.extend(ss.iter_mut()),
            ItemBody::Columns(cs) => cs
                .iter_mut()
                .for_each(|c| out.extend(c.sentences.iter_mut())),
            _ => {}
        }
        for c in &mut i.children {
            if let ItemChild::Subitem(s) = c {
                item(s, out);
            }
        }
    }
    let mut out: Vec<&mut Sentence> = p.sentences.iter_mut().collect();
    for c in &mut p.children {
        if let ParagraphChild::Item(i) = c {
            item(i, &mut out);
        }
    }
    out
}

/// 条（idx=None）または項の中の全出現を置換し、置換数を返す
fn replace_in_article(art: &mut Article, idx: Option<usize>, from: &str, to: &str) -> usize {
    let mut n = 0;
    let count = paragraphs(art).len();
    for i in 0..count {
        if idx.is_some_and(|j| j != i) {
            continue;
        }
        for s in sentences_mut(paragraph_mut(art, i)) {
            for inl in &mut s.text {
                if let Inline::Text(t) = inl {
                    n += t.matches(from).count();
                    *t = t.replace(from, to);
                }
            }
        }
    }
    n
}

fn replace_toc(doc: &mut LegalDocument, from: &str, to: &str) -> Result<(), ApplyError> {
    fn go(e: &mut Element, from: &str, to: &str) -> usize {
        let mut n = 0;
        for c in &mut e.children {
            match c {
                Node::Text(t) => {
                    n += t.matches(from).count();
                    *t = t.replace(from, to);
                }
                Node::Element(x) => n += go(x, from, to),
            }
        }
        n
    }
    match doc.toc.as_mut().map(|t| go(t, from, to)) {
        Some(n) if n > 0 => Ok(()),
        _ => Err(ApplyError::TocPhraseNotFound(from.into())),
    }
}

fn fullwidth(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap())
        .collect()
}

fn set_label(p: &mut Paragraph, n: u32) {
    p.num = n.to_string();
    p.num_text = Some(if n == 1 {
        Vec::new()
    } else {
        vec![Inline::Text(fullwidth(n))]
    });
}

// ---------------------------------------------------------------- 追加する条文の構築

/// 「。」で文に分ける。「」（）の中では分けない
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut q, mut p) = (0i32, 0i32);
    for c in text.chars() {
        cur.push(c);
        match c {
            '「' => q += 1,
            '」' => q -= 1,
            '（' => p += 1,
            '）' => p -= 1,
            '。' if q == 0 && p == 0 => out.push(std::mem::take(&mut cur)),
            _ => {}
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn make_sentences(text: &str) -> Vec<Sentence> {
    let parts = split_sentences(text);
    let proviso_at = parts.iter().position(|s| s.starts_with("ただし、"));
    parts
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let function = match proviso_at {
                Some(j) if i + 1 == j => SentenceFunction::Main,
                Some(j) if i == j => SentenceFunction::Proviso,
                _ => SentenceFunction::Unspecified,
            };
            Sentence {
                stable_id: StableId(String::new()),
                num: Some((i + 1).to_string()),
                function,
                text: vec![Inline::Text(t.clone())],
                attrs: vec![("WritingMode".into(), "vertical".into())],
            }
        })
        .collect()
}

/// 「２　本文」または「本文」（第1項）
fn parse_paragraph(lines: &[String]) -> Result<Paragraph, ApplyError> {
    let Some(first) = lines.first() else {
        return Err(ApplyError::BadContent("empty".into()));
    };
    let (n, body) = split_leading_number(first);
    let mut p = Paragraph {
        stable_id: StableId(String::new()),
        num: String::new(),
        caption: None,
        num_text: None,
        sentences: make_sentences(body.trim()),
        attrs: Vec::new(),
        children: Vec::new(),
    };
    set_label(&mut p, n.unwrap_or(1));
    Ok(p)
}

/// 「２　本文」→ (Some(2), 本文)
fn split_leading_number(line: &str) -> (Option<u32>, &str) {
    let mut chars = line.char_indices();
    let mut n = 0u32;
    let mut end = 0;
    let mut seen = false;
    for (i, c) in chars.by_ref() {
        if let Some(d) = c
            .to_digit(10)
            .or_else(|| ('０'..='９').contains(&c).then(|| c as u32 - '０' as u32))
        {
            n = n * 10 + d;
            seen = true;
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    if !seen {
        return (None, line);
    }
    (Some(n), line[end..].trim_start_matches(['\u{3000}', ' ']))
}

/// 「（見出し）」「第N条　本文」「２　本文」…
fn parse_article(lines: &[String]) -> Result<Article, ApplyError> {
    let mut caption = None;
    let mut title = None;
    let mut num = None;
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    for l in lines {
        if l.starts_with('（') && title.is_none() {
            caption = Some(vec![Inline::Text(l.clone())]);
        } else if let Some(rest) = l.strip_prefix('第').filter(|_| title.is_none()) {
            let (t, body) = rest
                .split_once('\u{3000}')
                .ok_or_else(|| ApplyError::BadContent(l.clone()))?;
            let t = format!("第{t}");
            let base = lawean_resolve::numeral::kanji_to_u32(
                t.trim_start_matches('第').trim_end_matches('条'),
            )
            .ok_or_else(|| ApplyError::BadContent(l.clone()))?;
            num = Some(ArticleNum::Single {
                base,
                branch: vec![],
            });
            title = Some(vec![Inline::Text(t)]);
            paragraphs.push(parse_paragraph(&[body.to_string()])?);
        } else {
            paragraphs.push(parse_paragraph(std::slice::from_ref(l))?);
        }
    }
    Ok(Article {
        stable_id: StableId(String::new()),
        num: num.ok_or_else(|| ApplyError::BadContent(lines.join("/")))?,
        caption,
        title,
        attrs: Vec::new(),
        children: paragraphs
            .into_iter()
            .map(ArticleChild::Paragraph)
            .collect(),
    })
}

// ---------------------------------------------------------------- 事後検査と再パース

fn check_numbering(doc: &LegalDocument) -> Result<(), ApplyError> {
    fn go(ps: &[Provision]) -> Result<(), ApplyError> {
        for p in ps {
            match p {
                Provision::Container(c) => go(&c.children)?,
                Provision::Article(a) => {
                    let labels: Vec<u32> = paragraphs(a).iter().map(|p| label(p)).collect();
                    if labels.iter().enumerate().any(|(i, l)| *l != i as u32 + 1) {
                        return Err(ApplyError::ParagraphNumbering {
                            article: a.num.to_num_string(),
                            labels,
                        });
                    }
                }
                Provision::Raw(_) => {}
            }
        }
        Ok(())
    }
    go(&doc.main_provision)
}

/// XML を経由して再パースし、stable_id を振り直す
fn refresh(doc: &mut LegalDocument, new_version_id: &str) -> Result<LegalDocument, ApplyError> {
    let xml = emit_law(doc);
    let opts = ParseOptions {
        law_id: doc.law_id.clone(),
        version_id: Some(new_version_id.into()),
    };
    parse_law(&xml, opts).map_err(|e| ApplyError::Reparse(e.to_string()))
}

// ---------------------------------------------------------------- 項番号の対応

/// 改正単位を適用したとき、ある条の項番号が 旧 → 新 でどう対応するか。
/// 各項に印（属性）を付けて適用し、印を追って読む。削られた項は含まれない
pub fn paragraph_mapping(
    doc: &LegalDocument,
    unit: &AmendUnit,
    article: &ArticleNum,
) -> Result<BTreeMap<u32, u32>, ApplyError> {
    let mut tagged = doc.clone();
    {
        let art = article_mut(&mut tagged, article)?;
        let n = paragraphs(art).len();
        for i in 0..n {
            let p = paragraph_mut(art, i);
            let l = label(p);
            p.attrs.push(("lawean-orig".into(), l.to_string()));
        }
    }
    let applied = apply_unit(&tagged, unit, "mapping")?;
    let mut applied = applied;
    let art = article_mut(&mut applied, article)?;
    let mut map = BTreeMap::new();
    for p in paragraphs(art) {
        if let Some((_, v)) = p.attrs.iter().find(|(k, _)| k == "lawean-orig") {
            map.insert(v.parse().unwrap_or(0), label(p));
        }
    }
    Ok(map)
}

// ---------------------------------------------------------------- 比較用スナップショット

/// 本則の 条番号 → [(項ラベル, 本文)]。空白を除いた平文。目次は "TOC" キー
pub fn snapshot_main(doc: &LegalDocument) -> BTreeMap<String, Vec<(u32, String)>> {
    fn strip(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }
    fn para_text(p: &Paragraph) -> String {
        let mut s: String = p.sentences.iter().map(|x| x.plain_text()).collect();
        fn item(i: &Item, s: &mut String) {
            if let Some(t) = &i.title {
                s.push_str(&inline_text(t));
            }
            match &i.body {
                ItemBody::Sentences(ss) => ss.iter().for_each(|x| s.push_str(&x.plain_text())),
                ItemBody::Columns(cs) => cs
                    .iter()
                    .for_each(|c| c.sentences.iter().for_each(|x| s.push_str(&x.plain_text()))),
                _ => {}
            }
            for c in &i.children {
                if let ItemChild::Subitem(x) = c {
                    item(x, s);
                }
            }
        }
        for c in &p.children {
            if let ParagraphChild::Item(i) = c {
                item(i, &mut s);
            }
        }
        strip(&s)
    }
    let mut out = BTreeMap::new();
    fn go(ps: &[Provision], out: &mut BTreeMap<String, Vec<(u32, String)>>) {
        for p in ps {
            match p {
                Provision::Container(c) => go(&c.children, out),
                Provision::Article(a) => {
                    out.insert(
                        a.num.to_num_string(),
                        paragraphs(a)
                            .iter()
                            .map(|p| (label(p), para_text(p)))
                            .collect(),
                    );
                }
                Provision::Raw(_) => {}
            }
        }
    }
    go(&doc.main_provision, &mut out);
    if let Some(t) = &doc.toc {
        out.insert("TOC".into(), vec![(0, strip(&t.text()))]);
    }
    out
}

/// 2 つのスナップショットの差分を人が読める行にする
pub fn diff_snapshots(
    a: &BTreeMap<String, Vec<(u32, String)>>,
    b: &BTreeMap<String, Vec<(u32, String)>>,
) -> Vec<String> {
    let mut out = Vec::new();
    for k in a
        .keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        match (a.get(k), b.get(k)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                for i in 0..x.len().max(y.len()) {
                    let (l, r) = (x.get(i), y.get(i));
                    if l != r {
                        out.push(format!(
                            "art {k} #{i}: {:?} != {:?}",
                            l.map(|(n, t)| format!("{n}:{}", &t[..t.len().min(60)])),
                            r.map(|(n, t)| format!("{n}:{}", &t[..t.len().min(60)]))
                        ));
                    }
                }
            }
            (Some(_), None) => out.push(format!("art {k}: only in left")),
            (None, Some(_)) => out.push(format!("art {k}: only in right")),
            (None, None) => {}
        }
    }
    out
}
