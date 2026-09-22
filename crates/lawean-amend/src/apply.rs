//! 改正単位を Source IR のリビジョンに適用する（溶け込み）。
//! 対象が無ければ失敗する（発射台の不一致）。適用後は XML を経由して再パースし、stable_id を振り直す。

use crate::op::*;
use lawean_resolve::numeral::kanji_to_u32;
use lawean_source::xml::{Element, Node};
use lawean_source::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
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
                for at in expand_range(doc, at) {
                    let art = article_mut(doc, &at.article)?;
                    let idx = para_index(art, &at.paragraph, &mut snapshots)?;
                    let n =
                        replace_in_article_part(art, idx, at.item.as_deref(), at.part, from, to);
                    if n == 0 {
                        return Err(ApplyError::PhraseNotFound {
                            at: loc_name(&at),
                            phrase: from.clone(),
                        });
                    }
                }
            }
            Op::InsertAfterPhrase { at, anchor, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?;
                let n = replace_in_article_part(
                    art,
                    idx,
                    at.item.as_deref(),
                    at.part,
                    anchor,
                    &format!("{anchor}{text}"),
                );
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
                for p in parse_paragraphs(text)? {
                    art.children.push(ArticleChild::Paragraph(p));
                    snapshots
                        .get_mut(&article.to_num_string())
                        .unwrap()
                        .push(None);
                }
            }
            Op::InsertParagraphAfter {
                article,
                after,
                text,
            } => {
                let art = article_mut(doc, article)?;
                let idx = para_index(art, &Some(after.clone()), &mut snapshots)?.unwrap();
                for (k, p) in parse_paragraphs(text)?.into_iter().enumerate() {
                    let pos = nth_paragraph_child(art, idx + k) + 1;
                    art.children.insert(pos, ArticleChild::Paragraph(p));
                    snapshots
                        .get_mut(&article.to_num_string())
                        .unwrap()
                        .insert(idx + k + 1, None);
                }
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
            Op::AppendArticle { path, text } => {
                let arts = parse_articles(text)?;
                let c = container_mut(doc, path)?;
                c.children.extend(arts.into_iter().map(Provision::Article));
            }
            Op::InsertContainersAfter { path, text } => {
                let new = parse_containers(text)?;
                insert_containers_after(doc, path, new)?;
            }
            Op::RenumberContainer { path, to } => renumber_container(doc, path, *to)?,
            Op::InsertArticleAfter { after, text } => {
                article_mut(doc, after)?;
                let mut anchor = after.clone();
                for a in parse_articles(text)? {
                    let num = a.num.clone();
                    if !insert_article_after(&mut doc.main_provision, &anchor, a) {
                        return Err(ApplyError::ArticleNotFound(anchor.to_num_string()));
                    }
                    anchor = num;
                }
            }
            Op::RenumberArticle { from, to } => renumber_article(doc, from, to)?,
            Op::ShiftArticles { from, to, by } => shift_articles(doc, *from, *to, *by)?,
            Op::ReplaceContainerTitle { path, from, to } => {
                replace_container_title(doc, path, from, to)?;
            }
            Op::ReplaceCaption { article, from, to } => {
                let art = article_mut(doc, article)?;
                let cur = art
                    .caption
                    .as_ref()
                    .map(|c| inline_text(c))
                    .unwrap_or_default();
                if !cur.contains(from.as_str()) {
                    return Err(ApplyError::PhraseNotFound {
                        at: format!("{}の見出し", article_label(article)),
                        phrase: from.clone(),
                    });
                }
                art.caption = Some(vec![Inline::Text(cur.replace(from.as_str(), to))]);
            }
            Op::SetCaption { article, text } => {
                let art = article_mut(doc, article)?;
                art.caption = Some(vec![Inline::Text(text.clone())]);
            }
            Op::DeleteSentencePart { at, part } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                delete_sentence_part(paragraph_mut(art, idx), *part)?;
            }
            Op::ReplaceSentencePart { at, part, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                let first = text.first().cloned().unwrap_or_default();
                let p = paragraph_mut(art, idx);
                replace_sentence_part(p, *part, &first)?;
                // 2 行目以降は読替え表（「次の表の上欄に掲げる…」）。段落の表として持つ
                if text.len() > 1 {
                    p.children.retain(
                        |c| !matches!(c, ParagraphChild::Raw(e) if e.name == "TableStruct"),
                    );
                    p.children
                        .push(ParagraphChild::Raw(build_table(&text[1..])));
                }
            }
            Op::AppendSentence { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                let p = paragraph_mut(art, idx);
                let n = p.sentences.len();
                for (k, s) in make_sentences(&text.join("")).into_iter().enumerate() {
                    let mut s = s;
                    s.num = Some((n + k + 1).to_string());
                    p.sentences.push(s);
                }
            }
            Op::ReplaceParagraph { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                let mut new = parse_paragraphs(text)?;
                if new.len() != 1 {
                    return Err(ApplyError::BadContent(
                        "項の全部改正の内容が 1 項でない".into(),
                    ));
                }
                let p = paragraph_mut(art, idx);
                let new = new.remove(0);
                p.sentences = new.sentences;
                p.children = new.children;
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
                        // 「第N条を削る」: 条そのものを取り除く（番号の繰り上げは改め文が別に指示する）。
                        // 番号を残して内容を「削除」にするのは「第N条を次のように改める。第N条　削除」の形
                        remove_article(&mut doc.main_provision, &at.article);
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 位置の解決

pub(crate) fn loc_name(l: &Loc) -> String {
    let mut s = match &l.paragraph {
        Some(ParaRef::Num(n)) => format!("第{}条第{n}項", l.article.to_num_string()),
        None => format!("第{}条", l.article.to_num_string()),
    };
    if let Some(i) = &l.item {
        s.push_str(&format!("第{i}号"));
    }
    s
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

/// 条 `after` を含む列（本則直下か章・節の中）を見つけ、その直後に条を挿入する。見つかれば true
pub(crate) fn insert_article_after(
    ps: &mut Vec<Provision>,
    after: &ArticleNum,
    a: Article,
) -> bool {
    fn container_of<'a>(
        ps: &'a mut Vec<Provision>,
        after: &ArticleNum,
    ) -> Option<&'a mut Vec<Provision>> {
        if ps
            .iter()
            .any(|p| matches!(p, Provision::Article(x) if &x.num == after))
        {
            return Some(ps);
        }
        for p in ps.iter_mut() {
            if let Provision::Container(c) = p {
                if let Some(v) = container_of(&mut c.children, after) {
                    return Some(v);
                }
            }
        }
        None
    }
    match container_of(ps, after) {
        Some(v) => {
            let i = v
                .iter()
                .position(|p| matches!(p, Provision::Article(x) if &x.num == after))
                .unwrap();
            v.insert(i + 1, Provision::Article(a));
            true
        }
        None => false,
    }
}

pub(crate) fn remove_article(ps: &mut Vec<Provision>, num: &ArticleNum) -> bool {
    let before = ps.len();
    ps.retain(|p| !matches!(p, Provision::Article(a) if &a.num == num));
    if ps.len() != before {
        return true;
    }
    for p in ps.iter_mut() {
        if let Provision::Container(c) = p {
            if remove_article(&mut c.children, num) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn article_mut<'a>(
    doc: &'a mut LegalDocument,
    num: &ArticleNum,
) -> Result<&'a mut Article, ApplyError> {
    find_article(&mut doc.main_provision, num)
        .ok_or_else(|| ApplyError::ArticleNotFound(num.to_num_string()))
}

/// 「第一章第八節」のように外側から辿った容器
pub(crate) fn container_mut<'a>(
    doc: &'a mut LegalDocument,
    path: &[(ContainerKind, u32)],
) -> Result<&'a mut Container, ApplyError> {
    fn find<'a>(
        ps: &'a mut [Provision],
        path: &[(ContainerKind, u32)],
    ) -> Option<&'a mut Container> {
        let (kind, n) = path.first()?;
        for p in ps {
            if let Provision::Container(c) = p {
                if c.kind == *kind && c.num.as_deref() == Some(&n.to_string()) {
                    return if path.len() == 1 {
                        Some(c)
                    } else {
                        find(&mut c.children, &path[1..])
                    };
                }
                // 章の下の節など: 外側の容器を飛ばして探す
                if let Some(x) = find(&mut c.children, path) {
                    return Some(x);
                }
            }
        }
        None
    }
    find(&mut doc.main_provision, path)
        .ok_or_else(|| ApplyError::BadContent(format!("{}が無い", container_label(path))))
}

pub(crate) fn container_label(path: &[(ContainerKind, u32)]) -> String {
    path.iter()
        .map(|(k, n)| {
            format!(
                "第{}{}",
                lawean_resolve::numeral::to_kanji(*n),
                match k {
                    ContainerKind::Part => "編",
                    ContainerKind::Chapter => "章",
                    ContainerKind::Section => "節",
                    ContainerKind::Subsection => "款",
                    ContainerKind::Division => "目",
                }
            )
        })
        .collect()
}

/// 容器の題名の字句を改める
pub(crate) fn replace_container_title(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, u32)],
    from: &str,
    to: &str,
) -> Result<(), ApplyError> {
    let label = container_label(path);
    let c = container_mut(doc, path)?;
    let cur = c.title.as_ref().map(|t| inline_text(t)).unwrap_or_default();
    if !cur.contains(from) {
        return Err(ApplyError::PhraseNotFound {
            at: format!("{label}の題名"),
            phrase: from.to_string(),
        });
    }
    c.title = Some(vec![Inline::Text(cur.replace(from, to))]);
    Ok(())
}

pub(crate) fn paragraphs(art: &Article) -> Vec<&Paragraph> {
    art.children
        .iter()
        .filter_map(|c| match c {
            ArticleChild::Paragraph(p) => Some(p),
            _ => None,
        })
        .collect()
}

pub(crate) fn paragraph_mut(art: &mut Article, idx: usize) -> &mut Paragraph {
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
pub(crate) fn nth_paragraph_child(art: &Article, idx: usize) -> usize {
    art.children
        .iter()
        .enumerate()
        .filter(|(_, c)| matches!(c, ArticleChild::Paragraph(_)))
        .nth(idx)
        .map(|(i, _)| i)
        .unwrap()
}

pub(crate) fn label(p: &Paragraph) -> u32 {
    p.num.parse().unwrap_or(0)
}

pub(crate) fn snapshot(art: &Article, snapshots: &mut BTreeMap<String, Vec<Option<u32>>>) {
    snapshots
        .entry(art.num.to_num_string())
        .or_insert_with(|| paragraphs(art).iter().map(|p| Some(label(p))).collect());
}

/// 文の始まりの番号で項を引く。None は条全体
pub(crate) fn para_index(
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

/// 項の中の文。`only_item` があればその号（とその下の号）の文だけ
fn sentences_mut<'a>(p: &'a mut Paragraph, only_item: Option<&str>) -> Vec<&'a mut Sentence> {
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
    let mut out: Vec<&mut Sentence> = if only_item.is_none() {
        p.sentences.iter_mut().collect()
    } else {
        Vec::new()
    };
    for c in &mut p.children {
        if let ParagraphChild::Item(i) = c {
            match only_item {
                Some(n) if i.num.as_deref() != Some(n) => {}
                _ => item(i, &mut out),
            }
        }
    }
    out
}

/// 条（idx=None）または項の中の全出現を置換し、置換数を返す。`item` があればその号の中だけ
/// `part` があれば、その文（ただし書・本文・前段・後段・各号列記以外の部分）だけで置き換える
pub(crate) fn replace_in_article_part(
    art: &mut Article,
    idx: Option<usize>,
    item: Option<&str>,
    part: Option<SentencePart>,
    from: &str,
    to: &str,
) -> usize {
    if let Some(part) = part {
        let mut n = 0;
        let count = paragraphs(art).len();
        for i in 0..count {
            if idx.is_some_and(|j| j != i) {
                continue;
            }
            let p = paragraph_mut(art, i);
            // 号があればその号の文、無ければ項の文（各号列記以外の部分）の中で、部分を選ぶ
            let mut ss: Vec<&mut Sentence> = match (item, part) {
                (Some(_), _) => sentences_mut(p, item),
                (None, _) => p.sentences.iter_mut().collect(),
            };
            let mains: Vec<usize> = ss
                .iter()
                .enumerate()
                .filter(|(_, s)| s.function != SentenceFunction::Proviso)
                .map(|(k, _)| k)
                .collect();
            let targets: Vec<usize> = match part {
                SentencePart::Main => mains.clone(),
                SentencePart::Chapeau => (0..ss.len()).collect(),
                SentencePart::Front => mains.first().copied().into_iter().collect(),
                SentencePart::Back => mains
                    .get(1)
                    .copied()
                    .or_else(|| mains.last().copied())
                    .into_iter()
                    .collect(),
                SentencePart::Proviso => ss
                    .iter()
                    .position(|s| s.function == SentenceFunction::Proviso)
                    .into_iter()
                    .collect(),
            };
            for k in targets {
                for inl in &mut ss[k].text {
                    if let Inline::Text(t) = inl {
                        n += t.matches(from).count();
                        *t = t.replace(from, to);
                    }
                }
            }
        }
        return n;
    }
    let mut n = 0;
    let count = paragraphs(art).len();
    for i in 0..count {
        if idx.is_some_and(|j| j != i) {
            continue;
        }
        for s in sentences_mut(paragraph_mut(art, i), item) {
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

pub(crate) fn replace_toc(doc: &mut LegalDocument, from: &str, to: &str) -> Result<(), ApplyError> {
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
        Some(_) => {
            // 字句が節や条の範囲をまたぐ（「第六節　管理組合法人（第四十七条−第五十六条の七）第七節　…」）ときは
            // 目次の要素の切れ目に掛かるので、空白を除いた平文で置き換え、目次は平文 1 つの要素にする
            // （検査は目次を平文で比べる。XML の目次の構造は溶け込み後の章・節から作り直す前提）
            let flat = toc_text(doc).unwrap_or_default();
            // 条の範囲のダッシュは衆議院の本文が「−」（U+2212）、e-Gov が「―」（U+2015）
            let dash = |s: &str| s.replace(['−', '—', '－'], "―");
            let (f, t) = (dash(&strip_ws(from)), dash(&strip_ws(to)));
            if f.is_empty() || !flat.contains(&f) {
                return Err(ApplyError::TocPhraseNotFound(from.into()));
            }
            let new = flat.replace(&f, &t);
            doc.toc = Some(Element {
                name: "TOC".into(),
                attrs: vec![],
                children: vec![Node::Text(new)],
            });
            Ok(())
        }
        None => Err(ApplyError::TocPhraseNotFound(from.into())),
    }
}

fn fullwidth(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap())
        .collect()
}

pub(crate) fn set_label(p: &mut Paragraph, n: u32) {
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

pub(crate) fn make_sentences(text: &str) -> Vec<Sentence> {
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
pub(crate) fn parse_paragraph(lines: &[String]) -> Result<Paragraph, ApplyError> {
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

const KANJI_ITEM: &str = "一二三四五六七八九十";
const KANA_SUBITEM: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラム";

/// 「一　本文」「イ　本文」→ (番号の字, 本文)
fn split_item_title<'a>(line: &'a str, letters: &str) -> Option<(&'a str, &'a str)> {
    let (t, body) = line.split_once('\u{3000}')?;
    (!t.is_empty() && t.chars().all(|c| letters.contains(c))).then_some((t, body))
}

fn make_item(depth: u8, n: usize, title: &str, body: &str) -> Item {
    // 「場合　定める者」のように全角空白で 2 欄に分かれる号は Column（定義規定・区分の号）
    let body = match body.split_once('\u{3000}') {
        Some((a, b)) => ItemBody::Columns(
            [a, b]
                .iter()
                .enumerate()
                .map(|(k, t)| Column {
                    stable_id: StableId(String::new()),
                    num: Some((k + 1).to_string()),
                    sentences: make_sentences(t.trim()),
                    attrs: Vec::new(),
                })
                .collect(),
        ),
        None => ItemBody::Sentences(make_sentences(body.trim())),
    };
    Item {
        stable_id: StableId(String::new()),
        depth,
        num: Some(n.to_string()),
        title: Some(vec![Inline::Text(title.to_string())]),
        body,
        attrs: Vec::new(),
        children: Vec::new(),
    }
}

/// 「２　本文」「一　号」「イ　イロハ」…の行の列を項の列にする。数字で始まる行が項の頭、
/// 漢数字＋全角空白は号、片仮名＋全角空白はその号の下のイロハ。先頭の行に番号が無ければ第1項
pub(crate) fn parse_paragraphs(lines: &[String]) -> Result<Vec<Paragraph>, ApplyError> {
    let mut out: Vec<Paragraph> = Vec::new();
    // 項・号・イロハのどれでもない行（番号が無い）は読替え表のセル。最後の項に表として付ける
    let mut cells: Vec<String> = Vec::new();
    for l in lines {
        if !out.is_empty()
            && split_leading_number(l).0.is_none()
            && split_item_title(l, KANJI_ITEM).is_none()
            && split_item_title(l, KANA_SUBITEM).is_none()
        {
            cells.push(l.clone());
            continue;
        }
        if !cells.is_empty() {
            let cells_now = std::mem::take(&mut cells);
            out.last_mut()
                .unwrap()
                .children
                .push(ParagraphChild::Raw(build_table(&cells_now)));
        }
        if let Some((t, body)) = split_item_title(l, KANJI_ITEM).filter(|_| !out.is_empty()) {
            let p = out.last_mut().unwrap();
            let n = p
                .children
                .iter()
                .filter(|c| matches!(c, ParagraphChild::Item(_)))
                .count()
                + 1;
            p.children
                .push(ParagraphChild::Item(make_item(0, n, t, body)));
            continue;
        }
        if let Some((t, body)) = split_item_title(l, KANA_SUBITEM).filter(|_| !out.is_empty()) {
            let last_item =
                out.last_mut()
                    .unwrap()
                    .children
                    .iter_mut()
                    .rev()
                    .find_map(|c| match c {
                        ParagraphChild::Item(i) => Some(i),
                        _ => None,
                    });
            let Some(i) = last_item else {
                return Err(ApplyError::BadContent(l.clone()));
            };
            let n = i.children.len() + 1;
            i.children
                .push(ItemChild::Subitem(make_item(1, n, t, body)));
            continue;
        }
        out.push(parse_paragraph(std::slice::from_ref(l))?);
    }
    if !cells.is_empty() {
        let last = out
            .last_mut()
            .ok_or_else(|| ApplyError::BadContent("empty".into()))?;
        last.children.push(ParagraphChild::Raw(build_table(&cells)));
    }
    if out.is_empty() {
        return Err(ApplyError::BadContent("empty".into()));
    }
    Ok(out)
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
pub(crate) fn parse_article(lines: &[String]) -> Result<Article, ApplyError> {
    let mut caption = None;
    let mut title = None;
    let mut num = None;
    let mut body_lines: Vec<String> = Vec::new();
    for l in lines {
        if l.starts_with('（') && title.is_none() {
            caption = Some(vec![Inline::Text(l.clone())]);
        } else if let Some(rest) = l.strip_prefix('第').filter(|_| title.is_none()) {
            let (t, body) = rest
                .split_once('\u{3000}')
                .ok_or_else(|| ApplyError::BadContent(l.clone()))?;
            let t = format!("第{t}");
            // 「第六十八条の三」も読む
            num = Some(crate::parse::art_num(&t));
            title = Some(vec![Inline::Text(t)]);
            body_lines.push(body.to_string());
        } else {
            body_lines.push(l.clone());
        }
    }
    let paragraphs = parse_paragraphs(&body_lines)?;
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

/// 「第N章　題名」「第一節　題名」「第一款」「第一目」の行
fn container_line(l: &str) -> Option<(ContainerKind, String, String)> {
    let (t, title) = l.split_once('\u{3000}')?;
    let t = t.strip_prefix('第')?;
    let kind = match t.chars().last()? {
        '編' => ContainerKind::Part,
        '章' => ContainerKind::Chapter,
        '節' => ContainerKind::Section,
        '款' => ContainerKind::Subsection,
        '目' => ContainerKind::Division,
        _ => return None,
    };
    let n = kanji_to_u32(&t[..t.len() - t.chars().last()?.len_utf8()])?;
    Some((kind, n.to_string(), title.to_string()))
}

fn container_depth(k: ContainerKind) -> u8 {
    match k {
        ContainerKind::Part => 0,
        ContainerKind::Chapter => 1,
        ContainerKind::Section => 2,
        ContainerKind::Subsection => 3,
        ContainerKind::Division => 4,
    }
}

/// 容器の番号を変える（題名の「第N章」も）
pub(crate) fn renumber_container(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, u32)],
    to: u32,
) -> Result<(), ApplyError> {
    let c = container_mut(doc, path)?;
    c.num = Some(to.to_string());
    if let Some(t) = &mut c.title {
        let label = inline_text(t);
        let rest = label.trim_start_matches(|ch: char| ch != '\u{3000}');
        *t = vec![Inline::Text(format!(
            "{}{rest}",
            container_label(&[(c.kind, to)])
        ))];
    }
    Ok(())
}

/// `path` の容器の直後に容器を並べる
pub(crate) fn insert_containers_after(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, u32)],
    new: Vec<Container>,
) -> Result<(), ApplyError> {
    let label = container_label(path);
    let (last_kind, last_n) = *path
        .last()
        .ok_or_else(|| ApplyError::BadContent("空の位置".into()))?;
    // 親の子リストを探す（外側の指定があればその中で）
    let parent: &mut Vec<Provision> = if path.len() == 1 {
        &mut doc.main_provision
    } else {
        &mut container_mut(doc, &path[..path.len() - 1])?.children
    };
    fn find_list(
        ps: &mut Vec<Provision>,
        kind: ContainerKind,
        n: u32,
    ) -> Option<&mut Vec<Provision>> {
        if ps.iter().any(|p| matches!(p, Provision::Container(c) if c.kind == kind && c.num.as_deref() == Some(&n.to_string()))) {
            return Some(ps);
        }
        for p in ps.iter_mut() {
            if let Provision::Container(c) = p {
                if let Some(l) = find_list(&mut c.children, kind, n) {
                    return Some(l);
                }
            }
        }
        None
    }
    let list = find_list(parent, last_kind, last_n)
        .ok_or_else(|| ApplyError::BadContent(format!("{label}が無い")))?;
    let pos = list
        .iter()
        .position(|p| matches!(p, Provision::Container(c) if c.kind == last_kind && c.num.as_deref() == Some(&last_n.to_string())))
        .unwrap();
    for (k, c) in new.into_iter().enumerate() {
        list.insert(pos + 1 + k, Provision::Container(c));
    }
    Ok(())
}

/// 「次の二章を加える」「次の二節を加える」の内容: 章・節・款・目の題名の行で入れ子の容器を作り、条はその中に置く。
/// いちばん外側の容器の列を返す
pub(crate) fn parse_containers(lines: &[String]) -> Result<Vec<Container>, ApplyError> {
    fn new_container(kind: ContainerKind, num: String, title: String) -> Container {
        let label = lawean_resolve::numeral::to_kanji(num.parse().unwrap_or(0));
        Container {
            stable_id: StableId(String::new()),
            kind,
            num: Some(num),
            title: Some(vec![Inline::Text(format!(
                "第{label}{}\u{3000}{title}",
                match kind {
                    ContainerKind::Part => "編",
                    ContainerKind::Chapter => "章",
                    ContainerKind::Section => "節",
                    ContainerKind::Subsection => "款",
                    ContainerKind::Division => "目",
                }
            ))]),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }
    // 容器のスタックと、今の容器に入れる条の行
    let mut stack: Vec<Container> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    fn flush(stack: &mut [Container], pending: &mut Vec<String>) -> Result<(), ApplyError> {
        if pending.is_empty() {
            return Ok(());
        }
        let arts = parse_articles(pending)?;
        pending.clear();
        let top = stack
            .last_mut()
            .ok_or_else(|| ApplyError::BadContent("章の題名が無い".into()))?;
        top.children
            .extend(arts.into_iter().map(Provision::Article));
        Ok(())
    }
    // 閉じた最外の容器
    let mut done: Vec<Container> = Vec::new();
    fn close_to(stack: &mut Vec<Container>, depth: u8, done: &mut Vec<Container>) {
        while let Some(top) = stack.last() {
            if container_depth(top.kind) < depth {
                break;
            }
            let c = stack.pop().unwrap();
            match stack.last_mut() {
                Some(parent) => parent.children.push(Provision::Container(c)),
                None => done.push(c),
            }
        }
    }
    for l in lines {
        if let Some((kind, num, title)) = container_line(l) {
            flush(&mut stack, &mut pending)?;
            close_to(&mut stack, container_depth(kind), &mut done);
            stack.push(new_container(kind, num, title));
        } else {
            pending.push(l.clone());
        }
    }
    flush(&mut stack, &mut pending)?;
    close_to(&mut stack, 0, &mut done);
    if done.is_empty() {
        return Err(ApplyError::BadContent("章・節の題名が無い".into()));
    }
    Ok(done)
}

/// 「次の二条を加える」の内容を条ごとに分ける（「（見出し）」の行と「第N条　…」の行で区切る）
pub(crate) fn parse_articles(lines: &[String]) -> Result<Vec<Article>, ApplyError> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut pending_caption: Option<String> = None;
    for l in lines {
        if l.starts_with('（') {
            pending_caption = Some(l.clone());
            continue;
        }
        let is_title = l.starts_with('第')
            && l.split_once('\u{3000}')
                .is_some_and(|(t, _)| t.ends_with('条') || t.contains("条の"));
        if is_title || groups.is_empty() {
            let mut g = Vec::new();
            if let Some(c) = pending_caption.take() {
                g.push(c);
            }
            g.push(l.clone());
            groups.push(g);
        } else {
            groups.last_mut().unwrap().push(l.clone());
        }
    }
    groups.iter().map(|g| parse_article(g)).collect()
}

/// 条番号を変える（見出しの「第N条」も）
pub(crate) fn renumber_article(
    doc: &mut LegalDocument,
    from: &ArticleNum,
    to: &ArticleNum,
) -> Result<(), ApplyError> {
    let art = article_mut(doc, from)?;
    art.num = to.clone();
    art.title = Some(vec![Inline::Text(article_label(to))]);
    Ok(())
}

/// 位置の条が範囲（「第三十一条から第三十三条まで」）なら、発射台にあるその範囲の条ごとの位置に展開する
pub(crate) fn expand_range(doc: &LegalDocument, at: &Loc) -> Vec<Loc> {
    match &at.article {
        ArticleNum::Range { from, to } => crate::numbering::article_nums(doc)
            .into_iter()
            .filter(|n| matches!(n, ArticleNum::Single { .. }) && n >= from && n <= to)
            .map(|n| Loc {
                article: n,
                paragraph: at.paragraph.clone(),
                item: at.item.clone(),
                part: None,
            })
            .collect(),
        _ => vec![at.clone()],
    }
}

/// 「第百四十二条の四」
pub fn article_label(n: &ArticleNum) -> String {
    use lawean_resolve::numeral::to_kanji;
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
        other => format!("第{}条", other.to_num_string()),
    }
}

/// 範囲の条をまとめて動かす（繰り下げは番号の大きい方から、繰り上げは小さい方から）
pub(crate) fn shift_articles(
    doc: &mut LegalDocument,
    from: u32,
    to: u32,
    by: i32,
) -> Result<(), ApplyError> {
    let mut nums: Vec<ArticleNum> = crate::numbering::article_nums(doc)
        .into_iter()
        .filter(|n| matches!(n, ArticleNum::Single { base, .. } if *base >= from && *base <= to))
        .collect();
    if by > 0 {
        nums.reverse();
    }
    for n in nums {
        let ArticleNum::Single { base, branch } = &n else {
            continue;
        };
        let target = ArticleNum::Single {
            base: (*base as i32 + by) as u32,
            branch: branch.clone(),
        };
        renumber_article(doc, &n, &target)?;
    }
    Ok(())
}

/// 項の本文のうち前段／後段（ただし書きを除く本文の最初／最後の文）を差し替える。差し替え後の本文を返す
pub(crate) fn replace_sentence_part(
    p: &mut Paragraph,
    part: SentencePart,
    text: &str,
) -> Result<(), ApplyError> {
    let mains: Vec<usize> = p
        .sentences
        .iter()
        .enumerate()
        .filter(|(_, s)| s.function != SentenceFunction::Proviso)
        .map(|(i, _)| i)
        .collect();
    let idx = sentence_part_index(p, part)
        .ok_or_else(|| ApplyError::BadContent("前段・後段・ただし書が無い".into()))?;
    let _ = mains;
    p.sentences[idx].text = vec![Inline::Text(text.to_string())];
    Ok(())
}

/// 前段・後段（ただし書きを除く本文の最初／最後の文）・ただし書の位置
pub(crate) fn sentence_part_index(p: &Paragraph, part: SentencePart) -> Option<usize> {
    let mains: Vec<usize> = p
        .sentences
        .iter()
        .enumerate()
        .filter(|(_, s)| s.function != SentenceFunction::Proviso)
        .map(|(i, _)| i)
        .collect();
    match part {
        SentencePart::Front | SentencePart::Main | SentencePart::Chapeau => mains.first().copied(),
        SentencePart::Back => mains.get(1).copied().or_else(|| mains.last().copied()),
        SentencePart::Proviso => p
            .sentences
            .iter()
            .position(|s| s.function == SentenceFunction::Proviso),
    }
}

/// 後段・ただし書を削る
pub(crate) fn delete_sentence_part(
    p: &mut Paragraph,
    part: SentencePart,
) -> Result<(), ApplyError> {
    let idx = sentence_part_index(p, part)
        .ok_or_else(|| ApplyError::BadContent("前段・後段・ただし書が無い".into()))?;
    if p.sentences.len() <= 1 {
        return Err(ApplyError::BadContent(
            "文が 1 つしか無い項の一部は削れない".into(),
        ));
    }
    p.sentences.remove(idx);
    for (k, s) in p.sentences.iter_mut().enumerate() {
        s.num = Some((k + 1).to_string());
    }
    Ok(())
}

/// 改め文に平らに並んだ表のセルを行にまとめる。読替え表の上欄は条項の参照なので、
/// 「第N条…」で始まるセルが行の先頭、それ以外で始まる場合は上欄が空の続き行（2 セル）とみなす
pub(crate) fn build_table(cells: &[String]) -> Element {
    fn sentence(t: &str) -> Node {
        Node::Element(Element {
            name: "Sentence".into(),
            attrs: vec![("Num".into(), "1".into())],
            children: if t.is_empty() {
                vec![]
            } else {
                vec![Node::Text(t.to_string())]
            },
        })
    }
    fn column(t: &str) -> Node {
        Node::Element(Element {
            name: "TableColumn".into(),
            attrs: vec![],
            children: vec![sentence(t)],
        })
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut i = 0;
    while i < cells.len() {
        let head = cells[i].starts_with('第') || rows.is_empty();
        let n = if head { 3 } else { 2 };
        let mut row: Vec<String> = if head { vec![] } else { vec![String::new()] };
        row.extend(cells[i..(i + n).min(cells.len())].iter().cloned());
        while row.len() < 3 {
            row.push(String::new());
        }
        rows.push(row);
        i += n;
    }
    Element {
        name: "TableStruct".into(),
        attrs: vec![],
        children: vec![Node::Element(Element {
            name: "Table".into(),
            attrs: vec![],
            children: rows
                .iter()
                .map(|r| {
                    Node::Element(Element {
                        name: "TableRow".into(),
                        attrs: vec![],
                        children: r.iter().map(|c| column(c)).collect(),
                    })
                })
                .collect(),
        })],
    }
}

// ---------------------------------------------------------------- 事後検査と再パース

pub(crate) fn check_numbering(doc: &LegalDocument) -> Result<(), ApplyError> {
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
                Provision::Paragraph(_) | Provision::Raw(_) => {}
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

/// 空白を除く。Lean 側と突き合わせる本文の正規化はこれだけ
pub(crate) fn strip_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 項の本文（文 + 号・欄の文）を空白を除いた平文に。Rust の突き合わせと Lean への出力で同じものを使う
pub fn para_text(p: &Paragraph) -> String {
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
    strip_ws(&s)
}

/// 目次の平文（空白を除く）
pub fn toc_text(doc: &LegalDocument) -> Option<String> {
    doc.toc.as_ref().map(|t| strip_ws(&t.text()))
}

/// 本則の 条番号 → [(項ラベル, 本文)]。空白を除いた平文。目次は "TOC" キー
pub fn snapshot_main(doc: &LegalDocument) -> BTreeMap<String, Vec<(u32, String)>> {
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
                Provision::Paragraph(p) => out
                    .entry("main".into())
                    .or_default()
                    .push((label(p), para_text(p))),
                Provision::Raw(_) => {}
            }
        }
    }
    go(&doc.main_provision, &mut out);
    if let Some(t) = toc_text(doc) {
        out.insert("TOC".into(), vec![(0, t)]);
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
