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
    collapse_untitled(&mut doc.main_provision);
    check_numbering(&doc)?;
    refresh(&mut doc, new_version_id)
}

/// 1 文の中の項番号は文の始まりの番号（改正前）で解釈する
fn apply_instruction(doc: &mut LegalDocument, ins: &Instruction) -> Result<(), ApplyError> {
    // 条ごとに、文の始まりの項ラベル
    let mut snapshots: BTreeMap<String, Vec<Option<u32>>> = BTreeMap::new();
    // この文で加えた字句（後の置換はその中を指さない）
    let mut inserted: Vec<String> = Vec::new();
    // 別表の行は文の始まりの上欄で引く（同じ文の最初の置換で上欄が変わっても、後の置換は同じ行）
    let mut appdx_rows: BTreeMap<(String, String), usize> = BTreeMap::new();
    for op in &ins.ops {
        match op {
            Op::ReplaceToc { from, to } => replace_toc(doc, from, to)?,
            Op::Replace { at, from, to } => {
                for at in expand_range(doc, at) {
                    let art = loc_article_mut(doc, &at)?;
                    let idx = para_index(art, &at.paragraph, &mut snapshots)?;
                    let n = replace_in_article_part(
                        art,
                        idx,
                        at.item.as_deref(),
                        at.sub.as_deref(),
                        at.part,
                        from,
                        &mark(to),
                        &inserted,
                    );
                    if n == 0 {
                        return Err(ApplyError::PhraseNotFound {
                            at: loc_name(&at),
                            phrase: from.clone(),
                        });
                    }
                }
                inserted.push(to.clone());
            }
            Op::InsertAfterPhrase { at, anchor, text } => {
                let art = loc_article_mut(doc, at)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?;
                let n = replace_in_article_part(
                    art,
                    idx,
                    at.item.as_deref(),
                    at.sub.as_deref(),
                    at.part,
                    anchor,
                    &format!("{anchor}{}", mark(text)),
                    &inserted,
                );
                if n == 0 {
                    return Err(ApplyError::PhraseNotFound {
                        at: loc_name(at),
                        phrase: anchor.clone(),
                    });
                }
                inserted.push(text.clone());
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
            Op::InsertParagraphFirst { article, text } => {
                let art = article_mut(doc, article)?;
                snapshot(art, &mut snapshots);
                for (k, p) in parse_paragraphs(text)?.into_iter().enumerate() {
                    let pos = nth_paragraph_child(art, k);
                    art.children.insert(pos, ArticleChild::Paragraph(p));
                    snapshots
                        .get_mut(&article.to_num_string())
                        .unwrap()
                        .insert(k, None);
                }
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
            Op::AppendContainers { path, text } => {
                let new = parse_containers(text)?;
                let slot = if path.is_empty() {
                    &mut doc.main_provision
                } else {
                    &mut container_mut(doc, path)?.children
                };
                slot.extend(new.into_iter().map(Provision::Container));
            }
            Op::InsertArticleBefore {
                before,
                text,
                suppl,
            } => {
                if *suppl {
                    return Err(ApplyError::BadContent(
                        "附則の条の前への挿入は未対応".into(),
                    ));
                }
                article_mut(doc, before)?;
                // 前に置く = 直前の条の後ろに置く。先頭ならその列の先頭に
                let arts = parse_articles(text)?;
                insert_articles_before(&mut doc.main_provision, before, arts)?;
            }
            Op::AppendSupplArticles { text } => append_suppl_articles(doc, text)?,
            Op::InsertContainersBefore { path, text } => {
                let new = parse_containers(text)?;
                insert_containers_at(doc, path, new, false)?;
            }
            Op::RenumberContainer { path, to } => renumber_container(doc, path, to)?,
            Op::InsertArticleAfter { after, text, suppl } if *suppl => {
                insert_suppl_articles_after(doc, after, text)?;
            }
            Op::InsertArticleAfter { after, text, .. } => {
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
            Op::RenumberArticle { from, to, suppl } if *suppl => {
                let art = suppl_article_mut(doc, from)?;
                art.num = to.clone();
                art.title = Some(vec![Inline::Text(article_label(to))]);
            }
            Op::RenumberArticle { from, to, .. } => renumber_article(doc, from, to)?,
            Op::ShiftArticles { from, to, by } => shift_articles(doc, *from, *to, *by)?,
            Op::ReplaceContainerTitle { path, from, to } => {
                replace_container_title(doc, path, from, to)?;
            }
            Op::SetContainerTitle { path, text } => {
                let c = container_mut(doc, path)?;
                c.title = Some(vec![Inline::Text(text.join("").trim().to_string())]);
            }
            Op::DeleteContainerTitle { path } => delete_container_title(doc, path)?,
            Op::DeleteContainers {
                path,
                kind,
                from,
                to,
            } => delete_containers(doc, path, *kind, *from, *to)?,
            Op::ReplaceArticles { articles, text } => {
                replace_articles(doc, articles, text)?;
            }
            Op::ReplaceContainers { paths, text } => {
                let new = parse_containers(text)?;
                replace_containers(doc, paths, new)?;
            }
            Op::ReplaceAppdxRow {
                table,
                row,
                sub,
                from,
                to,
            } => replace_appdx_row(doc, table, row, sub.as_deref(), from, to, &mut appdx_rows)?,
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
            Op::SetCaption { article, text } | Op::AttachCaption { article, text } => {
                let art = article_mut(doc, article)?;
                art.caption = Some(vec![Inline::Text(text.clone())]);
            }
            Op::DeleteCaption { article } => {
                article_mut(doc, article)?.caption = None;
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
                // 2 行目以降が号なら、号を入れ替える（「ただし書を次のように改める」+「一　…」）
                if text.len() > 1 && split_item_title(&text[1], KANJI_ITEM).is_some() {
                    p.children.retain(|c| !matches!(c, ParagraphChild::Item(_)));
                    insert_items_after(p, None, &text[1..])?;
                } else if text.len() > 1 {
                    // 読替え表（「次の表の上欄に掲げる…」）。段落の表として持つ
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
                append_sentence(p, text)?;
            }
            Op::SetToc { text } => set_toc(doc, text),
            Op::SetTitle { text } => {
                let t = text.join("").trim().to_string();
                doc.title = Some(LawTitle {
                    text: vec![Inline::Text(t)],
                    attrs: doc
                        .title
                        .as_ref()
                        .map(|x| x.attrs.clone())
                        .unwrap_or_default(),
                });
            }
            Op::ReplaceItem { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                replace_item(
                    paragraph_mut(art, idx),
                    at.item.as_deref().unwrap_or(""),
                    text,
                )?;
            }
            Op::RenumberItem { at, from, to } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                renumber_item(paragraph_mut(art, idx), from, to)?;
            }
            Op::ShiftItems { at, from, to, by } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                shift_items(paragraph_mut(art, idx), *from, *to, *by)?;
            }
            Op::InsertItemAfter { at, after, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                insert_items_after(paragraph_mut(art, idx), Some(after), text)?;
            }
            Op::InsertItemBefore { at, before, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                insert_items_before(paragraph_mut(art, idx), before, text)?;
            }
            Op::AppendItem { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                match &at.item {
                    Some(item) => append_subitems(paragraph_mut(art, idx), item, text)?,
                    None => insert_items_after(paragraph_mut(art, idx), None, text)?,
                }
            }
            Op::ReplaceItems { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                let p = paragraph_mut(art, idx);
                p.children.retain(|c| !matches!(c, ParagraphChild::Item(_)));
                insert_items_after(p, None, text)?;
            }
            Op::ReplaceTableRow { at, row, from, to } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                replace_table_row(paragraph_mut(art, idx), row, from, to, &inserted)?;
            }
            Op::AppendTable { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                let table = build_table(text);
                paragraph_mut(art, idx)
                    .children
                    .push(ParagraphChild::Raw(Element {
                        name: "TableStruct".into(),
                        attrs: vec![],
                        children: vec![Node::Element(table)],
                    }));
            }
            Op::DeleteAppdx { tables } => delete_appendices(doc, tables)?,
            Op::ReplaceAppdxRowWhole { table, row, text } => {
                replace_appdx_row_whole(doc, table, row, text)?
            }
            Op::DeleteAppdxRows { table, rows } => delete_appdx_rows(doc, table, rows)?,
            Op::RenumberAppdxRow { table, from, to } => renumber_appdx_row(doc, table, from, to)?,
            Op::InsertAppdxRowsAfter { table, after, text } => {
                insert_appdx_rows_after(doc, table, after, text)?
            }
            Op::AppendAppdx { text } => append_appdx(doc, text, None)?,
            Op::InsertAppdxAfter { after, text } => append_appdx(doc, text, Some(after))?,
            Op::RenameAppdx { from, to } => rename_appdx(doc, from, to)?,
            Op::DeleteAppdxRowSub { table, row, sub } => {
                delete_appdx_row_sub(doc, table, row, sub)?
            }
            Op::RenumberAppdxRowSub {
                table,
                row,
                from,
                to,
            } => renumber_appdx_row_sub(doc, table, row, from, to)?,

            Op::InsertItemFirst { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                insert_items_first(paragraph_mut(art, idx), text)?;
            }
            Op::ReplaceItemSet {
                at, items, text, ..
            } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                replace_item_set(paragraph_mut(art, idx), items, text)?;
            }
            Op::RenumberSubitem { at, from, to } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                renumber_subitem(
                    paragraph_mut(art, idx),
                    at.item.as_deref().unwrap_or(""),
                    from,
                    to,
                )?;
            }
            Op::ShiftSubitems { at, from, to, by } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                shift_subitems(
                    paragraph_mut(art, idx),
                    at.item.as_deref().unwrap_or(""),
                    from,
                    to,
                    *by,
                )?;
            }
            Op::InsertSubitemAfter { at, after, text } => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                insert_subitems_after(
                    paragraph_mut(art, idx),
                    at.item.as_deref().unwrap_or(""),
                    after,
                    text,
                )?;
            }
            Op::ReplaceParagraph { at, text } => {
                let art = article_mut(doc, &at.article)?;
                let new = parse_paragraphs(text)?;
                let single = new.len() == 1;
                for q in new {
                    // 複数の項（「第二項及び第三項を次のように改める」）は内容の番号で当てる
                    let pr = if single {
                        at.paragraph.clone()
                    } else {
                        Some(ParaRef::Num(q.num.parse().unwrap_or(1)))
                    };
                    let idx = para_index(art, &pr, &mut snapshots)?.unwrap_or(0);
                    let p = paragraph_mut(art, idx);
                    p.sentences = q.sentences;
                    p.children = q.children;
                }
            }
            Op::ReplaceArticle { article, text } => {
                let a = parse_article(text)?;
                let art = article_mut(doc, article)?;
                // 見出しの行が無ければ今の見出しのまま（見出しは「第N条の見出しを削り」で別に扱う。令3-44 第5条）
                if a.caption.is_some() {
                    art.caption = a.caption;
                }
                art.title = a.title;
                art.children = a.children;
            }
            Op::Delete { at } if at.item.is_some() => {
                let art = article_mut(doc, &at.article)?;
                let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                delete_item(paragraph_mut(art, idx), at.item.as_deref().unwrap_or(""))?;
            }
            Op::Delete { at } if matches!(at.article, ArticleNum::Range { .. }) => {
                // 「第百四条から第百五条の二までを削る」
                for a in expand_range(doc, at) {
                    remove_article(&mut doc.main_provision, &a.article);
                }
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
    strip_marks_doc(doc);
    Ok(())
}

// ---------------------------------------------------------------- 位置の解決

/// 位置の本文（条・項・号・細目。位置が無ければ空）。「加える字句が既に入っているか」の判定など
pub fn loc_text(doc: &LegalDocument, at: &Loc) -> String {
    fn find<'a>(ps: &'a [Provision], n: &ArticleNum) -> Option<&'a Article> {
        ps.iter().find_map(|p| match p {
            Provision::Article(a) if &a.num == n => Some(a),
            Provision::Container(c) => find(&c.children, n),
            _ => None,
        })
    }
    let Some(art) = find(&doc.main_provision, &at.article) else {
        return String::new();
    };
    let paras = paragraphs(art);
    let idx = match at.paragraph {
        Some(ParaRef::Num(n)) => paras
            .iter()
            .position(|p| p.num == n.to_string())
            .or(Some(n as usize - 1)),
        None => None,
    };
    paras
        .iter()
        .enumerate()
        .filter(|(i, _)| idx.is_none_or(|j| j == *i))
        .map(|(_, p)| para_text(p))
        .collect()
}

pub fn loc_name(l: &Loc) -> String {
    let mut s = match &l.paragraph {
        Some(ParaRef::Num(n)) => format!("第{}条第{n}項", l.article.to_num_string()),
        None => format!("第{}条", l.article.to_num_string()),
    };
    if l.suppl {
        s.insert_str(0, "附則");
    }
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

/// 条 `before` を含む列を見つけ、その直前に条を並べる
pub(crate) fn insert_articles_before(
    ps: &mut Vec<Provision>,
    before: &ArticleNum,
    arts: Vec<Article>,
) -> Result<(), ApplyError> {
    fn go(ps: &mut Vec<Provision>, before: &ArticleNum, arts: &mut Option<Vec<Article>>) {
        if let Some(i) = ps
            .iter()
            .position(|p| matches!(p, Provision::Article(x) if &x.num == before))
        {
            if let Some(v) = arts.take() {
                for (k, a) in v.into_iter().enumerate() {
                    ps.insert(i + k, Provision::Article(a));
                }
            }
            return;
        }
        for p in ps.iter_mut() {
            if let Provision::Container(c) = p {
                go(&mut c.children, before, arts);
                if arts.is_none() {
                    return;
                }
            }
        }
    }
    let mut slot = Some(arts);
    go(ps, before, &mut slot);
    if slot.is_some() {
        return Err(ApplyError::ArticleNotFound(before.to_num_string()));
    }
    Ok(())
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

/// 原始附則（AmendLawNum の無い附則）の条
/// 「附則に次の二条を加える」: 原始附則の末尾に条を足す（見出しの行「（罰則）」は最初の条の見出しに）
pub(crate) fn append_suppl_articles(
    doc: &mut LegalDocument,
    text: &[String],
) -> Result<(), ApplyError> {
    let arts = parse_articles(text)?;
    let sp = doc
        .suppl_provisions
        .iter_mut()
        .find(|s| s.amend_law_num.is_none())
        .ok_or_else(|| ApplyError::ArticleNotFound("附則".into()))?;
    sp.children.extend(
        arts.into_iter()
            .map(|a| SupplChild::Provision(Provision::Article(a))),
    );
    Ok(())
}

/// 「附則第一条の次に次の一条を加える」: 原始附則の条の後ろに条を挿す
pub(crate) fn insert_suppl_articles_after(
    doc: &mut LegalDocument,
    after: &ArticleNum,
    text: &[String],
) -> Result<(), ApplyError> {
    let sp = doc
        .suppl_provisions
        .iter_mut()
        .find(|s| s.amend_law_num.is_none())
        .ok_or_else(|| ApplyError::ArticleNotFound(format!("附則{}", after.to_num_string())))?;
    let mut anchor = after.clone();
    for a in parse_articles(text)? {
        let num = a.num.clone();
        let i = sp
            .children
            .iter()
            .position(
                |c| matches!(c, SupplChild::Provision(Provision::Article(x)) if x.num == anchor),
            )
            .ok_or_else(|| {
                ApplyError::ArticleNotFound(format!("附則{}", anchor.to_num_string()))
            })?;
        sp.children
            .insert(i + 1, SupplChild::Provision(Provision::Article(a)));
        anchor = num;
    }
    Ok(())
}

pub(crate) fn suppl_article_mut<'a>(
    doc: &'a mut LegalDocument,
    num: &ArticleNum,
) -> Result<&'a mut Article, ApplyError> {
    for sp in doc
        .suppl_provisions
        .iter_mut()
        .filter(|s| s.amend_law_num.is_none())
    {
        for c in sp.children.iter_mut() {
            let SupplChild::Provision(p) = c else {
                continue;
            };
            let hit = match p {
                Provision::Article(a) => &a.num == num,
                Provision::Container(c) => find_article(&mut c.children, num).is_some(),
                _ => false,
            };
            if hit {
                return match p {
                    Provision::Article(a) => Ok(a),
                    Provision::Container(c) => Ok(find_article(&mut c.children, num).unwrap()),
                    _ => unreachable!(),
                };
            }
        }
    }
    Err(ApplyError::ArticleNotFound(format!(
        "附則{}",
        num.to_num_string()
    )))
}

/// 位置に応じて本則か原始附則の条
pub(crate) fn loc_article_mut<'a>(
    doc: &'a mut LegalDocument,
    at: &Loc,
) -> Result<&'a mut Article, ApplyError> {
    if at.suppl {
        suppl_article_mut(doc, &at.article)
    } else {
        article_mut(doc, &at.article)
    }
}

/// 「第一章第八節」のように外側から辿った容器
pub(crate) fn container_mut<'a>(
    doc: &'a mut LegalDocument,
    path: &[(ContainerKind, String)],
) -> Result<&'a mut Container, ApplyError> {
    fn find<'a>(
        ps: &'a mut [Provision],
        path: &[(ContainerKind, String)],
    ) -> Option<&'a mut Container> {
        // 題名を消した（併合待ちの）容器と番号が重なることがあるので、題名のある方を先に探す
        if find_with(ps, path, true).is_some() {
            return find_with(ps, path, true);
        }
        find_with(ps, path, false)
    }
    fn find_with<'a>(
        ps: &'a mut [Provision],
        path: &[(ContainerKind, String)],
        titled_only: bool,
    ) -> Option<&'a mut Container> {
        let (kind, n) = path.first()?;
        for p in ps {
            if let Provision::Container(c) = p {
                if c.kind == *kind
                    && c.num.as_deref() == Some(n.as_str())
                    && (!titled_only || c.title.is_some())
                {
                    return if path.len() == 1 {
                        Some(c)
                    } else {
                        find_with(&mut c.children, &path[1..], titled_only)
                    };
                }
                // 章の下の節など: 外側の容器を飛ばして探す
                if let Some(x) = find_with(&mut c.children, path, titled_only) {
                    return Some(x);
                }
            }
        }
        None
    }
    find(&mut doc.main_provision, path)
        .ok_or_else(|| ApplyError::BadContent(format!("{}が無い", container_label(path))))
}

/// 「2_2」→「二章の二」の「二」「の二」
fn container_num_label(n: &str) -> (String, String) {
    let mut parts = n.split('_').filter_map(|x| x.parse::<u32>().ok());
    let head = parts
        .next()
        .map(lawean_resolve::numeral::to_kanji)
        .unwrap_or_else(|| n.to_string());
    let tail: String = parts
        .map(|b| format!("の{}", lawean_resolve::numeral::to_kanji(b)))
        .collect();
    (head, tail)
}

pub(crate) fn container_label(path: &[(ContainerKind, String)]) -> String {
    path.iter()
        .map(|(k, n)| {
            let (head, tail) = container_num_label(n);
            format!(
                "第{head}{}{tail}",
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
    path: &[(ContainerKind, String)],
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
/// 項の文。`only_item` があればその号（とその下の細目）だけ、さらに `only_sub`（「ロ」）があればその細目だけ
fn sentences_mut<'a>(
    p: &'a mut Paragraph,
    only_item: Option<&str>,
    only_sub: Option<&str>,
) -> Vec<&'a mut Sentence> {
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
                Some(_) if only_sub.is_some() => {
                    for c in &mut i.children {
                        if let ItemChild::Subitem(s) = c {
                            if s.title.as_ref().map(|t| inline_text(t)).as_deref() == only_sub {
                                item(s, &mut out);
                            }
                        }
                    }
                }
                _ => item(i, &mut out),
            }
        }
    }
    out
}

/// 同じ文（改め文の 1 文）で加えた字句の印。置換で入れた字句を `MARK_O`…`MARK_C` で囲み、文の終わりに外す。
/// 後の置換は印の中（加えた字句）を指さない（改正規定は改正前の字句を指す）。印は e-Gov の本文に現れない私用領域の文字
pub(crate) const MARK_O: char = '\u{E000}';
pub(crate) const MARK_C: char = '\u{E001}';

/// 加えた字句に印を付ける（`replace_protected` に渡す置換先）
pub(crate) fn mark(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    format!("{MARK_O}{s}{MARK_C}")
}

/// 印を外す
pub(crate) fn strip_marks(s: &str) -> String {
    if !s.contains(MARK_O) && !s.contains(MARK_C) {
        return s.to_string();
    }
    s.chars().filter(|c| *c != MARK_O && *c != MARK_C).collect()
}

/// 文書の全部の文から印を外す（改め文の 1 文の終わりに）
pub(crate) fn strip_marks_doc(doc: &mut LegalDocument) {
    fn sentences(s: &mut Sentence) {
        for inl in &mut s.text {
            if let Inline::Text(t) = inl {
                if t.contains(MARK_O) || t.contains(MARK_C) {
                    *t = strip_marks(t);
                }
            }
        }
    }
    fn item(i: &mut Item) {
        match &mut i.body {
            ItemBody::Sentences(ss) => ss.iter_mut().for_each(sentences),
            ItemBody::Columns(cs) => cs
                .iter_mut()
                .for_each(|c| c.sentences.iter_mut().for_each(sentences)),
            _ => {}
        }
        for c in &mut i.children {
            if let ItemChild::Subitem(x) = c {
                item(x);
            }
        }
    }
    fn element(e: &mut Element) {
        for c in &mut e.children {
            match c {
                Node::Text(t) => {
                    if t.contains(MARK_O) || t.contains(MARK_C) {
                        *t = strip_marks(t);
                    }
                }
                Node::Element(x) => element(x),
            }
        }
    }
    fn paragraph(p: &mut Paragraph) {
        p.sentences.iter_mut().for_each(sentences);
        for c in &mut p.children {
            match c {
                ParagraphChild::Item(i) => item(i),
                ParagraphChild::Raw(e) => element(e),
            }
        }
    }
    fn article(a: &mut Article) {
        for c in &mut a.children {
            if let ArticleChild::Paragraph(p) = c {
                paragraph(p);
            }
        }
    }
    fn provisions(ps: &mut [Provision]) {
        for p in ps {
            match p {
                Provision::Article(a) => article(a),
                Provision::Container(c) => provisions(&mut c.children),
                Provision::Paragraph(p) => paragraph(p),
                Provision::Raw(_) => {}
            }
        }
    }
    provisions(&mut doc.main_provision);
    for sp in &mut doc.suppl_provisions {
        for c in &mut sp.children {
            match c {
                SupplChild::Provision(p) => provisions(std::slice::from_mut(p)),
                SupplChild::Paragraph(p) => paragraph(p),
                SupplChild::Raw(_) => {}
            }
        }
    }
}

/// `t` の中の `from` を `to` に置き換え、置換数を返す。`to` は加えた字句なら `mark` で印を付けて渡す。
/// 同じ文で先に加えた字句（印の中）は置き換えない（改正規定は改正前の字句を指す）。
/// 元の字句に無く、加えた字句の中にだけあるなら、それを指しているので置き換える。
/// 「第五条の二」は「第五条の二十二」の頭には当たらない（数の途中で切らない）
fn replace_protected(t: &str, from: &str, to: &str, _protect: &[String]) -> (String, usize) {
    if from.is_empty() {
        return (t.to_string(), 0);
    }
    let mut guarded: Vec<(usize, usize)> = Vec::new();
    let mut open: Option<usize> = None;
    for (i, ch) in t.char_indices() {
        match ch {
            MARK_O => open = Some(i),
            MARK_C => {
                if let Some(a) = open.take() {
                    guarded.push((a, i + ch.len_utf8()));
                }
            }
            _ => {}
        }
    }
    let ends_with_numeral = from
        .chars()
        .last()
        .is_some_and(|c| "一二三四五六七八九十百千".contains(c));
    let hits: Vec<usize> = t
        .match_indices(from)
        .map(|(i, _)| i)
        .filter(|i| {
            !ends_with_numeral
                || !t[i + from.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| "一二三四五六七八九十百千".contains(c))
        })
        .collect();
    let outside: Vec<usize> = hits
        .iter()
        .copied()
        .filter(|i| !guarded.iter().any(|(a, b)| a <= i && i + from.len() <= *b))
        .collect();
    let targets = if outside.is_empty() { hits } else { outside };
    if targets.is_empty() {
        return (t.to_string(), 0);
    }
    let mut out = String::with_capacity(t.len());
    let mut pos = 0;
    let mut n = 0;
    for i in targets {
        if i < pos {
            continue;
        }
        out.push_str(&t[pos..i]);
        out.push_str(to);
        pos = i + from.len();
        n += 1;
    }
    out.push_str(&t[pos..]);
    (out, n)
}

/// `part` があれば、その文（ただし書・本文・前段・後段・各号列記以外の部分）だけで置き換える。
/// `protect` は同じ文で先に加えた字句（その中は置き換えない）
#[allow(clippy::too_many_arguments)]
pub(crate) fn replace_in_article_part(
    art: &mut Article,
    idx: Option<usize>,
    item: Option<&str>,
    sub: Option<&str>,
    part: Option<SentencePart>,
    from: &str,
    to: &str,
    protect: &[String],
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
                (Some(_), _) => sentences_mut(p, item, sub),
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
                n += replace_in_sentence(ss[k], from, to, protect);
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
        for s in sentences_mut(paragraph_mut(art, i), item, sub) {
            n += replace_in_sentence(s, from, to, protect);
        }
    }
    n
}

/// 1 文の中の字句の置換。まず素の字句（`Inline::Text`）の中で探し、無ければルビ（`<Ruby>と<Rt>ヽ</Rt></Ruby>`。傍点・ふりがな）を
/// またいで平文で探す（旅館業法 第5条第2号「とばく、」→「賭博」。改め文はルビを書かないので、ルビを落とした本文に当てる）
fn replace_in_sentence(s: &mut Sentence, from: &str, to: &str, protect: &[String]) -> usize {
    let mut n = 0;
    for inl in &mut s.text {
        if let Inline::Text(t) = inl {
            let (nt, c) = replace_protected(t, from, to, protect);
            n += c;
            *t = nt;
        }
    }
    if n == 0 && s.text.iter().any(|i| matches!(i, Inline::Raw(_))) {
        let flat = s.plain_text();
        let (nt, c) = replace_protected(&flat, from, to, protect);
        if c > 0 {
            s.text = vec![Inline::Text(nt)];
            n += c;
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
                    let (nt, c) = replace_protected(t, from, to, &[]);
                    n += c;
                    *t = nt;
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
            let (new, _) = replace_protected(&flat, &f, &t, &[]);
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

const KANJI_ITEM: &str = "一二三四五六七八九十の";
const KANA_SUBITEM: &str = "イロハニホヘトチリヌルヲワカヨタレソツネナラム";

/// 「一　本文」「イ　本文」→ (番号の字, 本文)
fn split_item_title<'a>(line: &'a str, letters: &str) -> Option<(&'a str, &'a str)> {
    let (t, body) = line.split_once('\u{3000}')?;
    (!t.is_empty() && t.chars().all(|c| letters.contains(c))).then_some((t, body))
}

/// 「(1)　本文」「（１）　本文」→ ("(1)", 本文)
fn split_paren_number(line: &str) -> Option<(&str, &str)> {
    let (t, body) = line.split_once('\u{3000}')?;
    let inner = t
        .strip_prefix('(')
        .and_then(|x| x.strip_suffix(')'))
        .or_else(|| t.strip_prefix('（').and_then(|x| x.strip_suffix('）')))?;
    (!inner.is_empty()
        && inner
            .chars()
            .all(|c| c.is_ascii_digit() || ('０'..='９').contains(&c)))
    .then_some((t, body))
}

fn make_item(depth: u8, n: usize, title: &str, body: &str) -> Item {
    // 号の番号は題から（「九の二」→ 9_2）。イロハなど数でなければ位置から
    let num_from_title: Option<String> = {
        let parts: Vec<Option<u32>> = title.split('の').map(kanji_to_u32).collect();
        if !parts.is_empty() && parts.iter().all(|p| p.is_some()) && title != "の" {
            Some(
                parts
                    .iter()
                    .map(|p| p.unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join("_"),
            )
        } else {
            None
        }
    };
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
        num: Some(num_from_title.unwrap_or_else(|| n.to_string())),
        title: Some(vec![Inline::Text(title.to_string())]),
        body,
        attrs: Vec::new(),
        children: Vec::new(),
    }
}

/// 「後段として次のように加える」「次のただし書を加える」: 最初の行を文として足し、続く「一　…」は号として足す
pub(crate) fn append_sentence(p: &mut Paragraph, text: &[String]) -> Result<(), ApplyError> {
    let Some(first) = text.first() else {
        return Err(ApplyError::BadContent("empty".into()));
    };
    let n = p.sentences.len();
    for (k, s) in make_sentences(first).into_iter().enumerate() {
        let mut s = s;
        s.num = Some((n + k + 1).to_string());
        p.sentences.push(s);
    }
    if text.len() > 1 {
        insert_items_after(p, None, &text[1..])?;
    }
    Ok(())
}

/// 号の全部改正: 「三　本文」（続くイロハを含む）で、番号 `num`（「3_2」）の号を差し替える
pub(crate) fn replace_item(
    p: &mut Paragraph,
    num: &str,
    lines: &[String],
) -> Result<(), ApplyError> {
    // 内容を仮の項として読み、その最初の号を取る
    let mut tmp = vec!["仮".to_string()];
    tmp.extend(lines.iter().cloned());
    let parsed = parse_paragraphs(&tmp)?;
    let new = parsed
        .into_iter()
        .next()
        .and_then(|q| {
            q.children.into_iter().find_map(|c| match c {
                ParagraphChild::Item(i) => Some(i),
                _ => None,
            })
        })
        .ok_or_else(|| ApplyError::BadContent("号の内容が読めない".into()))?;
    let want = num.split('_').collect::<Vec<_>>().join("_");
    let slot = p.children.iter_mut().find_map(|c| match c {
        ParagraphChild::Item(i) if i.num.as_deref() == Some(want.as_str()) => Some(i),
        _ => None,
    });
    let Some(slot) = slot else {
        return Err(ApplyError::BadContent(format!("第{num}号が無い")));
    };
    // 番号と id は元のまま。題・本文・下の号を入れ替える
    slot.title = new.title;
    slot.body = new.body;
    slot.children = new.children;
    Ok(())
}

/// 挙げた号（「第三号及び第四号」）だけを、内容の号（番号の順に対応）で差し替える。番号と id は元のまま
pub(crate) fn replace_item_set(
    p: &mut Paragraph,
    nums: &[String],
    lines: &[String],
) -> Result<(), ApplyError> {
    let mut tmp = vec!["仮".to_string()];
    tmp.extend(lines.iter().cloned());
    let parsed = parse_paragraphs(&tmp)?;
    let new_items: Vec<Item> = parsed
        .into_iter()
        .next()
        .map(|q| {
            q.children
                .into_iter()
                .filter_map(|c| match c {
                    ParagraphChild::Item(i) => Some(i),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    if new_items.len() != nums.len() {
        return Err(ApplyError::BadContent(format!(
            "号の数が合わない（{} 号を改めるのに内容は {} 号）",
            nums.len(),
            new_items.len()
        )));
    }
    for (num, new) in nums.iter().zip(new_items) {
        let slot = p.children.iter_mut().find_map(|c| match c {
            ParagraphChild::Item(i) if i.num.as_deref() == Some(num.as_str()) => Some(i),
            _ => None,
        });
        let Some(slot) = slot else {
            return Err(ApplyError::BadContent(format!("第{num}号が無い")));
        };
        slot.title = new.title;
        slot.body = new.body;
        slot.children = new.children;
    }
    Ok(())
}

/// 号の番号（「3_2」）→「三の二」
fn item_title(num: &str) -> String {
    let mut parts = num.split('_').filter_map(|x| x.parse::<u32>().ok());
    let mut s = parts
        .next()
        .map(lawean_resolve::numeral::to_kanji)
        .unwrap_or_default();
    for b in parts {
        s.push('の');
        s.push_str(&lawean_resolve::numeral::to_kanji(b));
    }
    s
}

fn items_mut(p: &mut Paragraph) -> Vec<&mut Item> {
    p.children
        .iter_mut()
        .filter_map(|c| match c {
            ParagraphChild::Item(i) => Some(i),
            _ => None,
        })
        .collect()
}

/// 「第A号を第B号とし」: 番号と題だけ変える
pub(crate) fn renumber_item(p: &mut Paragraph, from: &str, to: &str) -> Result<(), ApplyError> {
    let i = items_mut(p)
        .into_iter()
        .find(|i| i.num.as_deref() == Some(from))
        .ok_or_else(|| ApplyError::BadContent(format!("第{from}号が無い")))?;
    i.num = Some(to.to_string());
    i.title = Some(vec![Inline::Text(item_title(to))]);
    Ok(())
}

/// 「第A号から第B号までをK号ずつ繰り下げ」（枝番も基数ごと）
pub(crate) fn shift_items(
    p: &mut Paragraph,
    from: u32,
    to: u32,
    by: i32,
) -> Result<(), ApplyError> {
    let mut targets: Vec<(u32, String)> = items_mut(p)
        .into_iter()
        .filter_map(|i| {
            let n = i.num.clone()?;
            let base: u32 = n.split('_').next()?.parse().ok()?;
            (base >= from && base <= to).then_some((base, n))
        })
        .collect();
    if by > 0 {
        targets.sort_by_key(|a| std::cmp::Reverse(a.0));
    } else {
        targets.sort();
    }
    for (base, n) in targets {
        let rest: String = n.split('_').skip(1).map(|x| format!("_{x}")).collect();
        let new = format!("{}{rest}", base as i32 + by);
        renumber_item(p, &n, &new)?;
    }
    Ok(())
}

/// 「第N号の次に次のK号を加える」（`after` が None なら末尾に）
pub(crate) fn insert_items_after(
    p: &mut Paragraph,
    after: Option<&str>,
    lines: &[String],
) -> Result<(), ApplyError> {
    let mut tmp = vec!["仮".to_string()];
    tmp.extend(lines.iter().cloned());
    let parsed = parse_paragraphs(&tmp)?;
    let new: Vec<Item> = parsed
        .into_iter()
        .next()
        .map(|q| {
            q.children
                .into_iter()
                .filter_map(|c| match c {
                    ParagraphChild::Item(i) => Some(i),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    if new.is_empty() {
        return Err(ApplyError::BadContent("号の内容が読めない".into()));
    }
    let pos = match after {
        Some(a) => {
            p.children
                .iter()
                .position(|c| matches!(c, ParagraphChild::Item(i) if i.num.as_deref() == Some(a)))
                .ok_or_else(|| ApplyError::BadContent(format!("第{a}号が無い")))?
                + 1
        }
        None => p.children.len(),
    };
    for (k, i) in new.into_iter().enumerate() {
        p.children.insert(pos + k, ParagraphChild::Item(i));
    }
    Ok(())
}

/// 「同号に次のように加える」+ イロハの行: 号の下の列記を末尾に足す
pub(crate) fn append_subitems(
    p: &mut Paragraph,
    num: &str,
    lines: &[String],
) -> Result<(), ApplyError> {
    let mut tmp = vec!["仮".to_string(), "一\u{3000}仮".to_string()];
    tmp.extend(lines.iter().cloned());
    let parsed = parse_paragraphs(&tmp)?;
    let subs: Vec<ItemChild> = parsed
        .into_iter()
        .next()
        .and_then(|q| {
            q.children.into_iter().find_map(|c| match c {
                ParagraphChild::Item(i) => Some(i.children),
                _ => None,
            })
        })
        .unwrap_or_default();
    if subs.is_empty() {
        return Err(ApplyError::BadContent("イロハの内容が読めない".into()));
    }
    let slot = items_mut(p)
        .into_iter()
        .find(|i| i.num.as_deref() == Some(num))
        .ok_or_else(|| ApplyError::BadContent(format!("第{num}号が無い")))?;
    let base = slot
        .children
        .iter()
        .filter(|c| matches!(c, ItemChild::Subitem(_)))
        .count();
    for (k, c) in subs.into_iter().enumerate() {
        let c = match c {
            ItemChild::Subitem(mut s) => {
                s.num = Some((base + k + 1).to_string());
                ItemChild::Subitem(s)
            }
            other => other,
        };
        slot.children.push(c);
    }
    Ok(())
}

fn subitems_mut<'a>(p: &'a mut Paragraph, num: &str) -> Result<&'a mut Item, ApplyError> {
    items_mut(p)
        .into_iter()
        .find(|i| i.num.as_deref() == Some(num))
        .ok_or_else(|| ApplyError::BadContent(format!("第{num}号が無い")))
}

fn sub_title(i: &Item) -> String {
    i.title.as_ref().map(|t| inline_text(t)).unwrap_or_default()
}

/// 「同号ロを同号ハとし」: 細目の記号（題）と番号を変える
pub(crate) fn renumber_subitem(
    p: &mut Paragraph,
    num: &str,
    from: &str,
    to: &str,
) -> Result<(), ApplyError> {
    let slot = subitems_mut(p, num)?;
    let s = slot
        .children
        .iter_mut()
        .find_map(|c| match c {
            ItemChild::Subitem(s) if sub_title(s) == from => Some(s),
            _ => None,
        })
        .ok_or_else(|| ApplyError::BadContent(format!("第{num}号{from}が無い")))?;
    s.title = Some(vec![Inline::Text(to.to_string())]);
    s.num = Some(crate::parse::kana_index(to).to_string());
    Ok(())
}

/// 「ハからホまでをニからヘまでとし」: 記号の範囲を `by` だけずらす（重ならない順に）
pub(crate) fn shift_subitems(
    p: &mut Paragraph,
    num: &str,
    from: &str,
    to: &str,
    by: i32,
) -> Result<(), ApplyError> {
    let (a, b) = (crate::parse::kana_index(from), crate::parse::kana_index(to));
    let mut order: Vec<u32> = (a..=b).collect();
    if by > 0 {
        order.reverse();
    }
    for k in order {
        let kf = crate::parse::kana_of(k);
        let kt = crate::parse::kana_of((k as i32 + by) as u32);
        renumber_subitem(p, num, &kf, &kt)?;
    }
    Ok(())
}

/// 「同号イの次に次のように加える」+「ロ　本文」: 細目の挿入（番号は記号の順に付け直す）
pub(crate) fn insert_subitems_after(
    p: &mut Paragraph,
    num: &str,
    after: &str,
    lines: &[String],
) -> Result<(), ApplyError> {
    let mut tmp = vec!["仮".to_string(), "一\u{3000}仮".to_string()];
    tmp.extend(lines.iter().cloned());
    let parsed = parse_paragraphs(&tmp)?;
    let subs: Vec<ItemChild> = parsed
        .into_iter()
        .next()
        .and_then(|q| {
            q.children.into_iter().find_map(|c| match c {
                ParagraphChild::Item(i) => Some(i.children),
                _ => None,
            })
        })
        .unwrap_or_default();
    if subs.is_empty() {
        return Err(ApplyError::BadContent("イロハの内容が読めない".into()));
    }
    let slot = subitems_mut(p, num)?;
    let pos = slot
        .children
        .iter()
        .position(|c| matches!(c, ItemChild::Subitem(s) if sub_title(s) == after))
        .ok_or_else(|| ApplyError::BadContent(format!("第{num}号{after}が無い")))?;
    for (k, c) in subs.into_iter().enumerate() {
        slot.children.insert(pos + 1 + k, c);
    }
    for c in &mut slot.children {
        if let ItemChild::Subitem(s) = c {
            let t = sub_title(s);
            s.num = Some(crate::parse::kana_index(&t).to_string());
        }
    }
    Ok(())
}

/// 「同項に第一号として次の一号を加える」: 項の先頭（最初の号の前）に号を置く
pub(crate) fn insert_items_first(p: &mut Paragraph, lines: &[String]) -> Result<(), ApplyError> {
    let first = items_mut(p)
        .into_iter()
        .find_map(|i| i.num.clone())
        .ok_or_else(|| ApplyError::BadContent("号の無い項".into()))?;
    insert_items_before(p, &first, lines)
}

/// 「同号の前に次の一号を加える」
pub(crate) fn insert_items_before(
    p: &mut Paragraph,
    before: &str,
    lines: &[String],
) -> Result<(), ApplyError> {
    let pos = p
        .children
        .iter()
        .position(|c| matches!(c, ParagraphChild::Item(i) if i.num.as_deref() == Some(before)))
        .ok_or_else(|| ApplyError::BadContent(format!("第{before}号が無い")))?;
    let n_before = p.children.len();
    insert_items_after(p, None, lines)?;
    let added: Vec<ParagraphChild> = p.children.drain(n_before..).collect();
    for (k, c) in added.into_iter().enumerate() {
        p.children.insert(pos + k, c);
    }
    Ok(())
}

/// 「同項第N号を削る」
pub(crate) fn delete_item(p: &mut Paragraph, num: &str) -> Result<(), ApplyError> {
    let pos = p
        .children
        .iter()
        .position(|c| matches!(c, ParagraphChild::Item(i) if i.num.as_deref() == Some(num)))
        .ok_or_else(|| ApplyError::BadContent(format!("第{num}号が無い")))?;
    p.children.remove(pos);
    Ok(())
}

/// 「２　本文」「一　号」「イ　イロハ」…の行の列を項の列にする。数字で始まる行が項の頭、
/// 漢数字＋全角空白は号、片仮名＋全角空白はその号の下のイロハ。先頭の行に番号が無ければ第1項
pub fn parse_paragraphs(lines: &[String]) -> Result<Vec<Paragraph>, ApplyError> {
    let mut out: Vec<Paragraph> = Vec::new();
    // 項・号・イロハのどれでもない行（番号が無い）は読替え表のセル。最後の項に表として付ける
    let mut cells: Vec<String> = Vec::new();
    for l in lines {
        if !out.is_empty()
            && split_leading_number(l).0.is_none()
            && split_item_title(l, KANJI_ITEM).is_none()
            && split_item_title(l, KANA_SUBITEM).is_none()
            && split_paren_number(l).is_none()
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
        // 「(1)　本文」: イロハの下の列記（Subitem2）
        if let Some((t, body)) = split_paren_number(l).filter(|_| !out.is_empty()) {
            let last_sub = out
                .last_mut()
                .unwrap()
                .children
                .iter_mut()
                .rev()
                .find_map(|c| match c {
                    ParagraphChild::Item(i) => Some(i),
                    _ => None,
                })
                .and_then(|i| {
                    i.children.iter_mut().rev().find_map(|c| match c {
                        ItemChild::Subitem(s) => Some(s),
                        _ => None,
                    })
                });
            let Some(sub) = last_sub else {
                return Err(ApplyError::BadContent(l.clone()));
            };
            let n = sub.children.len() + 1;
            // e-Gov の表記「（１）」に合わせる
            let title: String = t
                .chars()
                .map(|c| match c {
                    '(' => '（',
                    ')' => '）',
                    '0'..='9' => char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap(),
                    c => c,
                })
                .collect();
            sub.children
                .push(ItemChild::Subitem(make_item(2, n, &title, body)));
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
            // 「第六十八条の三」も、「第百二条及び第百三条」（削除の範囲）も読む
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
    // 「二章の二」: 種類の字で番号と枝番に分ける
    let (kind_pos, kind_ch) = t.char_indices().find(|(_, c)| "編章節款目".contains(*c))?;
    let kind = match kind_ch {
        '編' => ContainerKind::Part,
        '章' => ContainerKind::Chapter,
        '節' => ContainerKind::Section,
        '款' => ContainerKind::Subsection,
        _ => ContainerKind::Division,
    };
    let n = kanji_to_u32(&t[..kind_pos])?;
    let mut num = n.to_string();
    let rest = &t[kind_pos + kind_ch.len_utf8()..];
    if !rest.is_empty() {
        for b in rest.split('の').filter(|x| !x.is_empty()) {
            num.push('_');
            num.push_str(&kanji_to_u32(b)?.to_string());
        }
    }
    Some((kind, num, title.to_string()))
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

/// 「第二条及び第三条を次のように改める」+ 条文（条ごと。「第百二条及び第百三条　削除」なら範囲の番号の 1 条）:
/// 最初の条の位置に新しい条を並べ、残りの旧条を取り除く
pub(crate) fn replace_articles(
    doc: &mut LegalDocument,
    articles: &[ArticleNum],
    text: &[String],
) -> Result<(), ApplyError> {
    let new = parse_articles(text)?;
    let first = articles
        .first()
        .ok_or_else(|| ApplyError::BadContent("条が無い".into()))?;
    // 旧条は先に取り除く（最初の条だけ、その位置に新しい条を置く）
    for n in &articles[1..] {
        remove_article(&mut doc.main_provision, n);
    }
    let mut anchor = first.clone();
    let mut first_done = false;
    for a in new {
        if !first_done {
            let art = article_mut(doc, first)?;
            art.caption = a.caption;
            art.title = a.title;
            art.num = a.num.clone();
            art.children = a.children;
            anchor = a.num;
            first_done = true;
        } else {
            let num = a.num.clone();
            if !insert_article_after(&mut doc.main_provision, &anchor, a) {
                return Err(ApplyError::ArticleNotFound(anchor.to_num_string()));
            }
            anchor = num;
        }
    }
    Ok(())
}

/// 「第四章及び第五章を次のように改める」+ 章の内容: 最初の章の位置に新しい章を並べ、旧章を取り除く
pub(crate) fn replace_containers(
    doc: &mut LegalDocument,
    paths: &[Vec<(ContainerKind, String)>],
    new: Vec<Container>,
) -> Result<(), ApplyError> {
    let first = paths
        .first()
        .ok_or_else(|| ApplyError::BadContent("章が無い".into()))?;
    insert_containers_at(doc, first, new, false)?;
    for path in paths {
        let (kind, n) = path
            .last()
            .cloned()
            .ok_or_else(|| ApplyError::BadContent("空の位置".into()))?;
        // 番号が同じ新しい章と区別するため、古い方（後ろにある方）を取り除く
        let list: &mut Vec<Provision> = if path.len() == 1 {
            &mut doc.main_provision
        } else {
            &mut container_mut(doc, &path[..path.len() - 1])?.children
        };
        let positions: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p, Provision::Container(c) if c.kind == kind && c.num.as_deref() == Some(n.as_str())))
            .map(|(i, _)| i)
            .collect();
        if let Some(&last) = positions.last() {
            if positions.len() > 1 {
                list.remove(last);
            }
        }
    }
    Ok(())
}

/// 別表の行の字句を改める。表は `AppdxTableTitle` が `table`（「別表第二」）で始まるもの、
/// 行は上欄が `row` の `TableRow`（`rowspan` や上欄の空欄で続く行も同じ項）。`sub` があればその細目の文だけ。
/// 同じ文（改め文の 1 文）の中では行を一度だけ引く（最初の置換で上欄が変わっても、後の置換は同じ行）
#[allow(clippy::too_many_arguments)]
pub(crate) fn replace_appdx_row(
    doc: &mut LegalDocument,
    table: &str,
    row: &str,
    sub: Option<&str>,
    from: &str,
    to: &str,
    rows: &mut BTreeMap<(String, String), usize>,
) -> Result<(), ApplyError> {
    fn replace_in(e: &mut Element, from: &str, to: &str) -> usize {
        let mut n = 0;
        for c in &mut e.children {
            match c {
                Node::Text(t) => {
                    let (nt, k) = replace_protected(t, from, to, &[]);
                    n += k;
                    *t = nt;
                }
                Node::Element(x) => n += replace_in(x, from, to),
            }
        }
        n
    }
    // 細目（「イ」「イ(イ)」）の見出し: 衆議院の本文は半角括弧、e-Gov は全角
    let sub_key = sub.map(|k| match k.find(['(', '（']) {
        Some(i) => format!(
            "（{}）",
            k[i..]
                .trim_start_matches(['(', '（'])
                .trim_end_matches([')', '）'])
        ),
        None => k.to_string(),
    });
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    let key = (table.to_string(), strip_ws(row));
    let (at, len) = match rows.get(&key) {
        Some(i) => {
            let len = row_group_len(body, *i);
            (*i, len)
        }
        None => {
            let g = row_group(body, row)
                .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{row}」の項が無い")))?;
            rows.insert(key, g.0);
            g
        }
    };
    let mut n = 0;
    for c in body[at..at + len].iter_mut() {
        let Node::Element(r) = c else { continue };
        match &sub_key {
            None => n += replace_in(r, from, to),
            Some(k) => {
                fn walk(e: &mut Element, k: &str, from: &str, to: &str, n: &mut usize) {
                    if e.name == "Sentence" {
                        if strip_ws(&e.text()).starts_with(k) {
                            let mut m = 0;
                            for c in &mut e.children {
                                match c {
                                    Node::Text(t) => {
                                        let (nt, x) = replace_protected(t, from, to, &[]);
                                        m += x;
                                        *t = nt;
                                    }
                                    Node::Element(y) => {
                                        let mut inner = 0;
                                        walk_all(y, from, to, &mut inner);
                                        m += inner;
                                    }
                                }
                            }
                            *n += m;
                        }
                        return;
                    }
                    for c in &mut e.children {
                        if let Node::Element(x) = c {
                            walk(x, k, from, to, n);
                        }
                    }
                }
                fn walk_all(e: &mut Element, from: &str, to: &str, n: &mut usize) {
                    for c in &mut e.children {
                        match c {
                            Node::Text(t) => {
                                let (nt, k) = replace_protected(t, from, to, &[]);
                                *n += k;
                                *t = nt;
                            }
                            Node::Element(x) => walk_all(x, from, to, n),
                        }
                    }
                }
                walk(r, k, from, to, &mut n);
            }
        }
    }
    if n == 0 {
        return Err(ApplyError::PhraseNotFound {
            at: format!("{table}{row}の項{}", sub.unwrap_or("")),
            phrase: from.to_string(),
        });
    }
    Ok(())
}

/// `at` の行から始まる項の行数（`rowspan`、無ければ上欄が空の続き）
fn row_group_len(body: &[Node], at: usize) -> usize {
    let rows: Vec<(usize, &Element)> = body
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            Node::Element(x) if x.name == "TableRow" => Some((i, x)),
            _ => None,
        })
        .collect();
    let Some(hit) = rows.iter().position(|(i, _)| *i == at) else {
        return 1;
    };
    let mut span = rows[hit]
        .1
        .children
        .iter()
        .find_map(|c| match c {
            Node::Element(x) if x.name == "TableColumn" => Some(
                x.attrs
                    .iter()
                    .find(|(k, _)| k == "rowspan")
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(1),
            ),
            _ => None,
        })
        .unwrap_or(1);
    if span == 1 {
        while hit + span < rows.len() && row_key(rows[hit + span].1).is_empty() {
            span += 1;
        }
    }
    let end = rows
        .get(hit + span - 1)
        .map(|(i, _)| *i)
        .unwrap_or(rows[rows.len() - 1].0);
    end - at + 1
}

// ---------------------------------------------------------------- 別表の行

/// 別表（`AppdxTable`）を題で引く。「別表第一」は題が「別表第一（第三条関係）」でも当たる
pub(crate) fn appdx_mut<'a>(
    doc: &'a mut LegalDocument,
    table: &str,
) -> Result<&'a mut Element, ApplyError> {
    doc.appendices
        .iter_mut()
        .find(|ap| {
            let title = ap
                .children
                .iter()
                .find_map(|c| match c {
                    Node::Element(x) if x.name.ends_with("Title") => Some(strip_ws(&x.text())),
                    _ => None,
                })
                .unwrap_or_default();
            title == table
                || title.starts_with(&format!("{table}（"))
                || title.starts_with(&format!("{table}\u{3000}"))
        })
        .ok_or_else(|| ApplyError::BadContent(format!("{table}が無い")))
}

/// `TableRow` を持つ列（`Table` の children）を返す
fn table_body_mut(e: &mut Element) -> Option<&mut Vec<Node>> {
    if e.children
        .iter()
        .any(|c| matches!(c, Node::Element(x) if x.name == "TableRow"))
    {
        return Some(&mut e.children);
    }
    for c in &mut e.children {
        if let Node::Element(x) = c {
            if let Some(v) = table_body_mut(x) {
                return Some(v);
            }
        }
    }
    None
}

fn row_key(r: &Element) -> String {
    r.children
        .iter()
        .find_map(|c| match c {
            Node::Element(x) if x.name == "TableColumn" => Some(strip_ws(&x.text())),
            _ => None,
        })
        .unwrap_or_default()
}

/// 上欄が `key` の行（`rowspan` で続く行も含む）の位置と長さ。番号は完全一致を先に、無ければ頭の一致
fn row_group(body: &[Node], key: &str) -> Option<(usize, usize)> {
    let rows: Vec<(usize, &Element)> = body
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            Node::Element(x) if x.name == "TableRow" => Some((i, x)),
            _ => None,
        })
        .collect();
    let key = strip_ws(key);
    let hit = rows
        .iter()
        .position(|(_, r)| row_key(r) == key)
        .or_else(|| rows.iter().position(|(_, r)| row_key(r).starts_with(&key)))?;
    // 最初の欄の rowspan がこの行の数
    let span = rows[hit]
        .1
        .children
        .iter()
        .find_map(|c| match c {
            Node::Element(x) if x.name == "TableColumn" => Some(
                x.attrs
                    .iter()
                    .find(|(k, _)| k == "rowspan")
                    .and_then(|(_, v)| v.parse::<usize>().ok())
                    .unwrap_or(1),
            ),
            _ => None,
        })
        .unwrap_or(1);
    // rowspan が無ければ、上欄が空の続きの行までがこの項（e-Gov は欄を省く形と rowspan の形の両方がある）
    let mut span = span;
    if span == 1 {
        while hit + span < rows.len() && row_key(rows[hit + span].1).is_empty() {
            span += 1;
        }
    }
    let start = rows[hit].0;
    let end = rows
        .get(hit + span - 1)
        .map(|(i, _)| *i)
        .unwrap_or(rows[rows.len() - 1].0);
    Some((start, end - start + 1))
}

/// 行の内容（「八」「再審の訴えの提起」「四千円」…）から `TableRow` を組む。`cols` 欄ずつ 1 行に
/// （余りは 1 行にまとめる。別表の最後の「この表の各項の…」のような欄をまたぐ行）
fn appdx_rows_of(text: &[String], cols: usize) -> Vec<Node> {
    fn column(t: &str) -> Node {
        Node::Element(Element {
            name: "TableColumn".into(),
            attrs: vec![],
            children: vec![Node::Element(Element {
                name: "Sentence".into(),
                attrs: vec![("Num".into(), "1".into())],
                children: if t.is_empty() {
                    vec![]
                } else {
                    vec![Node::Text(t.to_string())]
                },
            })],
        })
    }
    let cols = cols.max(1);
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let n = cols.min(text.len() - i);
        out.push(Node::Element(Element {
            name: "TableRow".into(),
            attrs: vec![],
            children: text[i..i + n].iter().map(|c| column(c.trim())).collect(),
        }));
        i += n;
    }
    out
}

/// その行（`at`）の欄の数
fn row_cols(body: &[Node], at: usize) -> usize {
    match body.get(at) {
        Some(Node::Element(r)) => r
            .children
            .iter()
            .filter(|c| matches!(c, Node::Element(x) if x.name == "TableColumn"))
            .count(),
        _ => 3,
    }
}

/// 「別表第一の八の項を次のように改める」: 行（rowspan で続く行も含む）を差し替える
pub(crate) fn replace_appdx_row_whole(
    doc: &mut LegalDocument,
    table: &str,
    row: &str,
    text: &[String],
) -> Result<(), ApplyError> {
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    let (at, len) = row_group(body, row)
        .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{row}」の項が無い")))?;
    let cols = row_cols(body, at);
    let new = appdx_rows_of(text, cols);
    body.splice(at..at + len, new);
    Ok(())
}

/// 「別表第一中九の項及び一〇の項を削り」
pub(crate) fn delete_appdx_rows(
    doc: &mut LegalDocument,
    table: &str,
    rows: &[String],
) -> Result<(), ApplyError> {
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    for row in rows {
        let (at, len) = row_group(body, row)
            .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{row}」の項が無い")))?;
        body.drain(at..at + len);
    }
    Ok(())
}

/// 「同項を同表の九の項とし」: 上欄の番号を付け替える
pub(crate) fn renumber_appdx_row(
    doc: &mut LegalDocument,
    table: &str,
    from: &str,
    to: &str,
) -> Result<(), ApplyError> {
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    let (at, _) = row_group(body, from)
        .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{from}」の項が無い")))?;
    let Node::Element(r) = &mut body[at] else {
        return Err(ApplyError::BadContent("行が無い".into()));
    };
    let col = r
        .children
        .iter_mut()
        .find_map(|c| match c {
            Node::Element(x) if x.name == "TableColumn" => Some(x),
            _ => None,
        })
        .ok_or_else(|| ApplyError::BadContent("上欄が無い".into()))?;
    set_column_text(col, to);
    Ok(())
}

/// 欄の文を差し替える（`Sentence` の中身だけ）
fn set_column_text(col: &mut Element, text: &str) {
    for c in &mut col.children {
        if let Node::Element(x) = c {
            if x.name == "Sentence" {
                x.children = vec![Node::Text(text.to_string())];
                return;
            }
        }
    }
    col.children = vec![Node::Element(Element {
        name: "Sentence".into(),
        attrs: vec![("Num".into(), "1".into())],
        children: vec![Node::Text(text.to_string())],
    })];
}

/// 「四十四の三の項の次に次のように加える」
pub(crate) fn insert_appdx_rows_after(
    doc: &mut LegalDocument,
    table: &str,
    after: &str,
    text: &[String],
) -> Result<(), ApplyError> {
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    let (at, len) = row_group(body, after)
        .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{after}」の項が無い")))?;
    let cols = row_cols(body, at);
    let new = appdx_rows_of(text, cols);
    body.splice(at + len..at + len, new);
    Ok(())
}

/// 「同表を別表第三とし」: 別表の題の「別表第二」の部分を付け替える
pub(crate) fn rename_appdx(
    doc: &mut LegalDocument,
    from: &str,
    to: &str,
) -> Result<(), ApplyError> {
    let ap = appdx_mut(doc, from)?;
    let title = ap
        .children
        .iter_mut()
        .find_map(|c| match c {
            Node::Element(x) if x.name.ends_with("Title") => Some(x),
            _ => None,
        })
        .ok_or_else(|| ApplyError::BadContent(format!("{from}に題が無い")))?;
    let cur = strip_ws(&title.text());
    let rest = cur.strip_prefix(from).unwrap_or("");
    title.children = vec![Node::Text(format!("{to}{rest}"))];
    Ok(())
}

/// 「附則の次に次の別表を加える」「別表第一の次に次の一表を加える」+ 「別表第二（第三条関係）」と行の内容: 別表を足す。
/// `after` があればその別表の次に、無ければ末尾に
pub(crate) fn append_appdx(
    doc: &mut LegalDocument,
    text: &[String],
    after: Option<&str>,
) -> Result<(), ApplyError> {
    let Some((title, cells)) = text.split_first() else {
        return Err(ApplyError::BadContent("別表の内容が無い".into()));
    };
    // 題は「別表第二（第三条、第四条関係）」: e-Gov は題（`AppdxTableTitle`）と関係条（`RelatedArticleNum`）に分ける
    let title = title.trim();
    let (name, related) = match title.find('（') {
        Some(i) => (&title[..i], Some(&title[i..])),
        None => (title, None),
    };
    let mut children = vec![Node::Element(Element {
        name: "AppdxTableTitle".into(),
        attrs: vec![("WritingMode".into(), "vertical".into())],
        children: vec![Node::Text(name.to_string())],
    })];
    if let Some(r) = related {
        children.push(Node::Element(Element {
            name: "RelatedArticleNum".into(),
            attrs: vec![],
            children: vec![Node::Text(r.to_string())],
        }));
    }
    children.push(Node::Element(Element {
        name: "TableStruct".into(),
        attrs: vec![],
        children: vec![Node::Element(Element {
            name: "Table".into(),
            attrs: vec![("WritingMode".into(), "vertical".into())],
            children: appdx_rows_of(cells, 3),
        })],
    }));
    let ap = Element {
        name: "AppdxTable".into(),
        attrs: vec![],
        children,
    };
    // 並び（`body_order`）の枠も。`after` の別表の次に入れるなら、後ろの枠の番号をずらす
    let at = match after {
        None => doc.appendices.len(),
        Some(t) => {
            let title_of = |ap: &Element| {
                ap.children
                    .iter()
                    .find_map(|c| match c {
                        Node::Element(x) if x.name.ends_with("Title") => Some(strip_ws(&x.text())),
                        _ => None,
                    })
                    .unwrap_or_default()
            };
            doc.appendices
                .iter()
                .position(|ap| {
                    let ti = title_of(ap);
                    ti == t
                        || ti.starts_with(&format!("{t}（"))
                        || ti.starts_with(&format!("{t}\u{3000}"))
                })
                .map(|i| i + 1)
                .ok_or_else(|| ApplyError::BadContent(format!("{t}が無い")))?
        }
    };
    doc.appendices.insert(at, ap);
    for s in &mut doc.body_order {
        if let BodySlot::Appendix(i) = s {
            if *i >= at {
                *i += 1;
            }
        }
    }
    let pos = doc
        .body_order
        .iter()
        .position(|s| matches!(s, BodySlot::Appendix(i) if *i == at + 1))
        .unwrap_or(doc.body_order.len());
    doc.body_order.insert(pos, BodySlot::Appendix(at));
    Ok(())
}

/// 別表の行の中の細目（「イ　…」「ロ　…」の文）を引く
fn appdx_row_sentences<'a>(
    doc: &'a mut LegalDocument,
    table: &str,
    row: &str,
) -> Result<Vec<&'a mut Element>, ApplyError> {
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    let (at, len) = row_group(body, row)
        .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{row}」の項が無い")))?;
    let mut out = Vec::new();
    for c in body[at..at + len].iter_mut() {
        if let Node::Element(r) = c {
            fn walk<'a>(e: &'a mut Element, out: &mut Vec<&'a mut Element>) {
                if e.name == "Sentence" {
                    out.push(e);
                    return;
                }
                for c in &mut e.children {
                    if let Node::Element(x) = c {
                        walk(x, out);
                    }
                }
            }
            walk(r, &mut out);
        }
    }
    Ok(out)
}

/// 「同表の一一の二の項中ハを削り」
pub(crate) fn delete_appdx_row_sub(
    doc: &mut LegalDocument,
    table: &str,
    row: &str,
    sub: &str,
) -> Result<(), ApplyError> {
    let ap = appdx_mut(doc, table)?;
    let body =
        table_body_mut(ap).ok_or_else(|| ApplyError::BadContent(format!("{table}に表が無い")))?;
    let (at, len) = row_group(body, row)
        .ok_or_else(|| ApplyError::BadContent(format!("{table}に「{row}」の項が無い")))?;
    let mut n = 0;
    for c in body[at..at + len].iter_mut() {
        if let Node::Element(r) = c {
            fn walk(e: &mut Element, sub: &str, n: &mut usize) {
                let before = e.children.len();
                e.children.retain(|c| {
                    !matches!(c, Node::Element(x) if x.name == "Sentence"
                        && strip_ws(&x.text()).starts_with(sub))
                });
                *n += before - e.children.len();
                for c in &mut e.children {
                    if let Node::Element(x) = c {
                        walk(x, sub, n);
                    }
                }
            }
            walk(r, sub, &mut n);
        }
    }
    if n == 0 {
        return Err(ApplyError::BadContent(format!(
            "{table}「{row}」の項に{sub}が無い"
        )));
    }
    Ok(())
}

/// 「ニをハとし」: 別表の行の中の細目の記号を付け替える
pub(crate) fn renumber_appdx_row_sub(
    doc: &mut LegalDocument,
    table: &str,
    row: &str,
    from: &str,
    to: &str,
) -> Result<(), ApplyError> {
    let ss = appdx_row_sentences(doc, table, row)?;
    for s in ss {
        if !strip_ws(&s.text()).starts_with(from) {
            continue;
        }
        for c in &mut s.children {
            if let Node::Text(t) = c {
                if let Some(rest) = t.trim_start().strip_prefix(from) {
                    *t = format!("{to}{rest}");
                    return Ok(());
                }
            }
        }
    }
    Err(ApplyError::BadContent(format!(
        "{table}「{row}」の項に{from}が無い"
    )))
}

/// 「別表第一及び別表第二を削る」: 別表を（本文の並び `body_order` の枠ごと）消す
pub(crate) fn delete_appendices(
    doc: &mut LegalDocument,
    tables: &[String],
) -> Result<(), ApplyError> {
    for t in tables {
        let idx = doc
            .appendices
            .iter()
            .position(|ap| {
                let title = ap
                    .children
                    .iter()
                    .find_map(|c| match c {
                        Node::Element(x) if x.name.ends_with("Title") => Some(strip_ws(&x.text())),
                        _ => None,
                    })
                    .unwrap_or_default();
                title == *t
                    || title.starts_with(&format!("{t}（"))
                    || title.starts_with(&format!("{t}\u{3000}"))
            })
            .ok_or_else(|| ApplyError::BadContent(format!("{t}が無い")))?;
        doc.appendices.remove(idx);
        doc.body_order
            .retain(|s| !matches!(s, BodySlot::Appendix(i) if *i == idx));
        for s in &mut doc.body_order {
            if let BodySlot::Appendix(i) = s {
                if *i > idx {
                    *i -= 1;
                }
            }
        }
    }
    Ok(())
}

/// 項の中の表（読替え表）の、上欄が `row` の行の字句を置き換える
pub(crate) fn replace_table_row(
    p: &mut Paragraph,
    row: &str,
    from: &str,
    to: &str,
    protect: &[String],
) -> Result<(), ApplyError> {
    fn all_rows<'a>(e: &'a mut Element, out: &mut Vec<&'a mut Element>) {
        if e.name == "TableRow" {
            out.push(e);
            return;
        }
        for c in &mut e.children {
            if let Node::Element(x) = c {
                all_rows(x, out);
            }
        }
    }
    fn replace_in(e: &mut Element, from: &str, to: &str, protect: &[String]) -> usize {
        let mut n = 0;
        for c in &mut e.children {
            match c {
                Node::Text(t) => {
                    let (nt, k) = replace_protected(t, from, to, protect);
                    n += k;
                    *t = nt;
                }
                Node::Element(x) => n += replace_in(x, from, to, protect),
            }
        }
        n
    }
    let key = strip_ws(row);
    let mut all: Vec<&mut Element> = Vec::new();
    for c in &mut p.children {
        if let ParagraphChild::Raw(e) = c {
            all_rows(e, &mut all);
        }
    }
    let hit = all.into_iter().find(|r| {
        r.children
            .iter()
            .find_map(|c| match c {
                Node::Element(x) if x.name == "TableColumn" => Some(strip_ws(&x.text())),
                _ => None,
            })
            .is_some_and(|t| t == key)
    });
    let Some(r) = hit else {
        return Err(ApplyError::BadContent(format!("表に「{row}」の項が無い")));
    };
    if replace_in(r, from, to, protect) == 0 {
        return Err(ApplyError::PhraseNotFound {
            at: format!("表{row}の項"),
            phrase: from.to_string(),
        });
    }
    Ok(())
}

/// 「第三章の章名を削る」: 題名を消す。番号は残す（続く「第三章第二節から第五節までを削る」が指す）。
/// 単位の最後に `collapse_untitled` が、題名の無い容器を前の同じ種類の容器に併合する（無ければ親に広げる）
pub(crate) fn delete_container_title(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, String)],
) -> Result<(), ApplyError> {
    let c = container_mut(doc, path)?;
    c.title = None;
    Ok(())
}

/// 題名の無い容器を前の同じ種類の容器の末尾に併合する（前が無ければ親のその位置に広げる）
pub fn collapse_untitled(ps: &mut Vec<Provision>) {
    // まず中を
    for p in ps.iter_mut() {
        if let Provision::Container(c) = p {
            collapse_untitled(&mut c.children);
        }
    }
    let mut i = 0;
    while i < ps.len() {
        let untitled = matches!(&ps[i], Provision::Container(c) if c.title.is_none());
        if !untitled {
            i += 1;
            continue;
        }
        let Provision::Container(removed) = ps.remove(i) else {
            unreachable!()
        };
        let prev = (0..i)
            .rev()
            .find(|k| matches!(&ps[*k], Provision::Container(c) if c.kind == removed.kind));
        match prev {
            Some(k) => {
                if let Provision::Container(c) = &mut ps[k] {
                    c.children.extend(removed.children);
                }
            }
            None => {
                for (k, ch) in removed.children.into_iter().enumerate() {
                    ps.insert(i + k, ch);
                }
            }
        }
    }
}

/// 「第三章第二節から第五節までを削る」
pub(crate) fn delete_containers(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, String)],
    kind: ContainerKind,
    from: u32,
    to: u32,
) -> Result<(), ApplyError> {
    let list: &mut Vec<Provision> = if path.is_empty() {
        &mut doc.main_provision
    } else {
        &mut container_mut(doc, path)?.children
    };
    let before = list.len();
    list.retain(|p| {
        !matches!(p, Provision::Container(c) if c.kind == kind
            && c.num.as_deref().and_then(|x| x.parse::<u32>().ok()).is_some_and(|x| x >= from && x <= to))
    });
    if list.len() == before {
        return Err(ApplyError::BadContent(format!(
            "{}第{}〜第{}{}が無い",
            container_label(path),
            from,
            to,
            container_label(&[(kind, "0".to_string())])
                .chars()
                .last()
                .unwrap_or('章')
        )));
    }
    Ok(())
}

/// 容器の番号を変える（題名の「第N章」も）
pub(crate) fn renumber_container(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, String)],
    to: &str,
) -> Result<(), ApplyError> {
    let c = container_mut(doc, path)?;
    c.num = Some(to.to_string());
    if let Some(t) = &mut c.title {
        let label = inline_text(t);
        let rest = label.trim_start_matches(|ch: char| ch != '\u{3000}');
        *t = vec![Inline::Text(format!(
            "{}{rest}",
            container_label(&[(c.kind, to.to_string())])
        ))];
    }
    Ok(())
}

/// `path` の容器の直後に容器を並べる
pub(crate) fn insert_containers_after(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, String)],
    new: Vec<Container>,
) -> Result<(), ApplyError> {
    insert_containers_at(doc, path, new, true)
}

/// `path` の容器の直後（`after`）または直前に容器を並べる
pub(crate) fn insert_containers_at(
    doc: &mut LegalDocument,
    path: &[(ContainerKind, String)],
    new: Vec<Container>,
    after: bool,
) -> Result<(), ApplyError> {
    let label = container_label(path);
    let (last_kind, last_n) = path
        .last()
        .cloned()
        .ok_or_else(|| ApplyError::BadContent("空の位置".into()))?;
    // 親の子リストを探す（外側の指定があればその中で）
    let parent: &mut Vec<Provision> = if path.len() == 1 {
        &mut doc.main_provision
    } else {
        &mut container_mut(doc, &path[..path.len() - 1])?.children
    };
    fn find_list<'a>(
        ps: &'a mut Vec<Provision>,
        kind: ContainerKind,
        n: &str,
    ) -> Option<&'a mut Vec<Provision>> {
        if ps.iter().any(|p| matches!(p, Provision::Container(c) if c.kind == kind && c.num.as_deref() == Some(n))) {
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
    let list = find_list(parent, last_kind, &last_n)
        .ok_or_else(|| ApplyError::BadContent(format!("{label}が無い")))?;
    let pos = list
        .iter()
        .position(|p| matches!(p, Provision::Container(c) if c.kind == last_kind && c.num.as_deref() == Some(last_n.as_str())))
        .unwrap();
    let base = if after { pos + 1 } else { pos };
    for (k, c) in new.into_iter().enumerate() {
        list.insert(base + k, Provision::Container(c));
    }
    Ok(())
}

/// 「次の二章を加える」「次の二節を加える」の内容: 章・節・款・目の題名の行で入れ子の容器を作り、条はその中に置く。
/// いちばん外側の容器の列を返す
pub(crate) fn parse_containers(lines: &[String]) -> Result<Vec<Container>, ApplyError> {
    fn new_container(kind: ContainerKind, num: String, title: String) -> Container {
        let (head, tail) = container_num_label(&num);
        Container {
            stable_id: StableId(String::new()),
            kind,
            num: Some(num),
            title: Some(vec![Inline::Text(format!(
                "第{head}{}{tail}\u{3000}{title}",
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
                suppl: false,
                sub: None,
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
    strip_ws(&strip_marks(&s))
}

/// 「題名の次に次の目次を付する」の行から e-Gov の形の目次を組む:
/// 「目次」→ TOCLabel、「第一章　総則（第一条）」→ TOCChapter（ChapterTitle + ArticleRange）、「附則」→ TOCSupplProvision。
/// 節・款は章の中に入れる（TOCSection / TOCSubsection）。範囲の「−」は e-Gov の「―」に
pub(crate) fn build_toc(lines: &[String]) -> Element {
    fn text_el(name: &str, t: &str) -> Node {
        Node::Element(Element {
            name: name.into(),
            attrs: vec![],
            children: vec![Node::Text(t.to_string())],
        })
    }
    fn close_to(stack: &mut Vec<(usize, Element)>, toc: &mut Element, depth: usize) {
        while stack.last().is_some_and(|(d, _)| *d >= depth) {
            let (_, e) = stack.pop().unwrap();
            match stack.last_mut() {
                Some((_, parent)) => parent.children.push(Node::Element(e)),
                None => toc.children.push(Node::Element(e)),
            }
        }
    }
    let mut toc = Element {
        name: "TOC".into(),
        attrs: vec![],
        children: vec![],
    };
    // 章の下に節、節の下に款
    let mut stack: Vec<(usize, Element)> = Vec::new();
    for raw in lines {
        let line = raw.trim().replace(['−', '－'], "―");
        if line.is_empty() {
            continue;
        }
        if line == "目次" {
            toc.children.push(text_el("TOCLabel", "目次"));
            continue;
        }
        if line == "附則" {
            close_to(&mut stack, &mut toc, 0);
            toc.children.push(Node::Element(Element {
                name: "TOCSupplProvision".into(),
                attrs: vec![],
                children: vec![text_el("SupplProvisionLabel", "附則")],
            }));
            continue;
        }
        // 「第三章の二　臨床研修（第十六条の二―第十六条の六）」→ 題と範囲
        let (title, range) = match line.find('（') {
            Some(i) => (line[..i].to_string(), Some(line[i..].to_string())),
            None => (line.clone(), None),
        };
        let label = title.split('　').next().unwrap_or("");
        let kind = label
            .trim_start_matches('第')
            .trim_start_matches(|c: char| "一二三四五六七八九十百の".contains(c))
            .chars()
            .next();
        let (name, tname, depth) = match kind {
            Some('編') => ("TOCPart", "PartTitle", 0),
            Some('章') => ("TOCChapter", "ChapterTitle", 1),
            Some('節') => ("TOCSection", "SectionTitle", 2),
            Some('款') => ("TOCSubsection", "SubsectionTitle", 3),
            _ => ("TOCDivision", "DivisionTitle", 4),
        };
        let num = toc_container_num(label);
        close_to(&mut stack, &mut toc, depth);
        let mut e = Element {
            name: name.into(),
            attrs: vec![("Num".into(), num)],
            children: vec![text_el(tname, &title)],
        };
        if let Some(r) = range {
            e.children.push(text_el("ArticleRange", &r));
        }
        stack.push((depth, e));
    }
    close_to(&mut stack, &mut toc, 0);
    toc
}

/// 目次を付ける（題名の次 = 前文・本則の前）。目次の無かった法律なら本文の並び（`body_order`）にも目次の枠を足す
pub(crate) fn set_toc(doc: &mut LegalDocument, lines: &[String]) {
    doc.toc = Some(build_toc(lines));
    if !doc.body_order.iter().any(|s| matches!(s, BodySlot::Toc)) {
        let at = doc
            .body_order
            .iter()
            .position(|s| matches!(s, BodySlot::Preamble | BodySlot::MainProvision))
            .unwrap_or(doc.body_order.len());
        doc.body_order.insert(at, BodySlot::Toc);
    }
}

/// 「第三章の二」→ "3_2"
fn toc_container_num(label: &str) -> String {
    label
        .trim_start_matches('第')
        .trim_end_matches(['編', '章', '節', '款', '目'])
        .split('の')
        .filter_map(kanji_to_u32)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("_")
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
