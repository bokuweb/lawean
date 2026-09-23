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

fn part_label(p: SentencePart) -> &'static str {
    match p {
        SentencePart::Front => "前段",
        SentencePart::Back => "後段",
        SentencePart::Proviso => "ただし書",
        SentencePart::Main => "本文",
        SentencePart::Chapeau => "各号列記以外の部分",
    }
}

/// 「3_2」→「三の二」
fn item_kanji(num: &str) -> String {
    let mut parts = num.split('_').filter_map(|x| x.parse::<u32>().ok());
    let mut s = parts.next().map(to_kanji).unwrap_or_default();
    for b in parts {
        s.push('の');
        s.push_str(&to_kanji(b));
    }
    s
}

/// 内容の行のうち号（漢数字＋全角空白）の数
fn count_items(text: &[String]) -> u32 {
    text.iter()
        .filter(|l| {
            l.split_once('\u{3000}').is_some_and(|(t, _)| {
                !t.is_empty() && t.chars().all(|c| "一二三四五六七八九十".contains(c))
            })
        })
        .count()
        .max(1) as u32
}

/// 「第二章の二」（番号は「2_2」）
fn cont_label(k: lawean_source::ContainerKind, n: &str) -> String {
    let mut parts = n.split('_').filter_map(|x| x.parse::<u32>().ok());
    let head = parts.next().map(to_kanji).unwrap_or_default();
    let tail: String = parts.map(|b| format!("の{}", to_kanji(b))).collect();
    format!("第{head}{}{tail}", kind_label(k))
}

fn path_label(path: &[(lawean_source::ContainerKind, String)]) -> String {
    path.iter().map(|(k, n)| cont_label(*k, n)).collect()
}

fn kind_label(k: lawean_source::ContainerKind) -> &'static str {
    use lawean_source::ContainerKind::*;
    match k {
        Part => "編",
        Chapter => "章",
        Section => "節",
        Subsection => "款",
        Division => "目",
    }
}

/// 別表の行の位置。行が数（「一一の二」）なら「別表第一の一一の二の項」、法令名なら「別表第二宅地建物取引業法（…）の項」
fn appdx_loc(table: &str, row: &str) -> String {
    let numeric = row
        .chars()
        .next()
        .is_some_and(|c| "一二三四五六七八九十〇百千".contains(c));
    if numeric {
        format!("{table}の{row}の項")
    } else {
        format!("{table}{row}の項")
    }
}

fn loc_label(l: &Loc) -> String {
    let mut s = match &l.paragraph {
        Some(ParaRef::Num(n)) => format!("{}第{}項", article_label(&l.article), to_kanji(*n)),
        None => article_label(&l.article),
    };
    if l.suppl {
        s.insert_str(0, "附則");
    }
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
    if let Some(k) = &l.sub {
        s.push_str(k);
    }
    if let Some(p) = l.part {
        s.push_str(part_label(p));
    }
    s
}

/// 繰り下げ・繰り上げの範囲。`to` が `u32::MAX` なら「以下」（「第二号を第一号とし、以下…」の最後まで）
fn shift_range(from: u32, to: u32, unit: &str) -> String {
    if to == u32::MAX {
        format!("第{}{unit}以下", to_kanji(from))
    } else {
        format!("第{}{unit}から第{}{unit}まで", to_kanji(from), to_kanji(to))
    }
}

/// 表の中の位置の前置き（別表の名と位置の間は「中」）
fn sep(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else {
        format!("中{path}")
    }
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
        Op::ParagraphCaption { at, edit } => match edit {
            CaptionEdit::Replace { from, to } if to.is_empty() => {
                format!("{}の見出し中「{from}」を{}", loc_label(at), end("削り", "削る"))
            }
            CaptionEdit::Replace { from, to } => format!(
                "{}の見出し中「{from}」を「{to}」に{}",
                loc_label(at),
                end("改め", "改める")
            ),
            CaptionEdit::Set(t) => format!("{}の見出しを「{t}」に{}", loc_label(at), end("改め", "改める")),
            CaptionEdit::Attach(t) => {
                format!("{}の前に見出しとして「{t}」を{}", loc_label(at), end("付し", "付する"))
            }
            CaptionEdit::Delete => format!("{}の見出しを{}", loc_label(at), end("削り", "削る")),
        },
        Op::InsertContainersAfterArticle { after, .. } => {
            format!("{}の次に次のように{}", article_label(after), end("加え", "加える"))
        }
        Op::DeleteToc => format!("目次を{}", end("削り", "削る")),
        Op::ReplacePairs { scope, .. } => format!(
            "{scope}中次の表の上欄に掲げる字句を同表の下欄に掲げる字句に{}",
            end("改め", "改める")
        ),
        Op::SubitemsEdit { at, subs, text } => format!(
            "{}{}を{}",
            loc_label(at),
            subs.join("及び"),
            match text {
                Some(_) => format!("次のように{}", end("改め", "改める")),
                None => end("削り", "削る"),
            }
        ),
        Op::DeleteTitle => format!("題名を{}", end("削り", "削る")),
        Op::ParagraphToArticle { from, to } => format!(
            "附則第{}項を附則{}と{}",
            to_kanji(*from),
            article_label(to),
            end("し", "する")
        ),
        Op::DeleteContainer { path } => format!("{}を{}", path_label(path), end("削り", "削る")),
        Op::ShiftContainers {
            path,
            kind,
            from,
            to,
            by,
        } => {
            let unit = kind_label(*kind);
            let range = if *to == u32::MAX {
                format!("第{}{unit}以下", to_kanji(*from))
            } else {
                format!("第{}{unit}から第{}{unit}まで", to_kanji(*from), to_kanji(*to))
            };
            let pre = if path.is_empty() {
                String::new()
            } else {
                format!("{}中", path_label(path))
            };
            format!(
                "{pre}{range}を{}{unit}ずつ繰り{}",
                to_kanji(by.unsigned_abs()),
                if *by > 0 {
                    end("下げ", "下げる")
                } else {
                    end("上げ", "上げる")
                }
            )
        }
        Op::ShiftBranchArticles {
            base,
            from,
            to,
            by,
        } => format!(
            "第{b}条の{}から第{b}条の{}までを{}条ずつ繰り{}",
            to_kanji(*from),
            to_kanji(*to),
            to_kanji(by.unsigned_abs()),
            if *by > 0 {
                end("下げ", "下げる")
            } else {
                end("上げ", "上げる")
            },
            b = to_kanji(*base)
        ),
        Op::ReplaceInContainer {
            path,
            except,
            from,
            to,
        } => {
            let scope = if path.is_empty() {
                "本則".to_string()
            } else {
                path_label(path)
            };
            let ex = if except.is_empty() {
                String::new()
            } else {
                format!(
                    "（{}を除く。）",
                    except.iter().map(loc_label).collect::<Vec<_>>().join("、")
                )
            };
            if to.is_empty() {
                format!("{scope}{ex}中「{from}」を{}", end("削り", "削る"))
            } else {
                format!("{scope}{ex}中「{from}」を「{to}」に{}", end("改め", "改める"))
            }
        }
        Op::SetContainerTitles { paths, .. } => format!(
            "{}を次のように{}",
            paths
                .iter()
                .map(|p| format!(
                    "{}の{}名",
                    path_label(p),
                    p.last().map(|(k, _)| kind_label(*k)).unwrap_or("")
                ))
                .collect::<Vec<_>>()
                .join("及び"),
            end("改め", "改める")
        ),
        Op::DeleteArticleTitle { article } => {
            format!("{}の条名を{}", article_label(article), end("削り", "削る"))
        }
        Op::ReplaceSupplNote { article, from, to } => format!(
            "{}の付記中「{from}」を「{to}」に{}",
            article_label(article),
            end("改め", "改める")
        ),
        Op::ReplaceInAmendment {
            article,
            target,
            from,
            to,
        } => format!(
            "{}のうち、{target}中「{from}」を「{to}」に{}",
            article_label(article),
            end("改め", "改める")
        ),
        Op::InsertHeadingsBefore {
            before,
            after,
            with_toc,
            ..
        } => {
            let what = if *with_toc { "目次及び章名" } else { "章名" };
            let side = if *after { "次" } else { "前" };
            match before {
                Some(a) => format!(
                    "{}の{side}に次の{what}を{}",
                    article_label(a),
                    end("加え", "加える")
                ),
                None => format!("題名の次に次の{what}を{}", end("付し", "付する")),
            }
        }
        Op::TableEdit {
            table,
            path,
            action,
        } => {
            let t = match table {
                TableRef::Appdx(t) => t.clone(),
                TableRef::InArticle(at) => format!("{}の表", loc_label(at)),
                TableRef::Preamble => "前文".to_string(),
            };
            let p = path.clone();
            match action {
                TableAction::Phrase { from, to } if to.is_empty() => {
                    format!("{t}{}中「{from}」を{}", sep(&p), end("削り", "削る"))
                }
                TableAction::Phrase { from, to } => {
                    format!("{t}{}中「{from}」を「{to}」に{}", sep(&p), end("改め", "改める"))
                }
                TableAction::Replace { .. } => {
                    format!("{t}{}を次のように{}", sep(&p), end("改め", "改める"))
                }
                TableAction::Delete => format!("{t}{}を{}", sep(&p), end("削り", "削る")),
                TableAction::InsertAfter { .. } => {
                    format!("{t}{}の次に次のように{}", sep(&p), end("加え", "加える"))
                }
                TableAction::InsertBefore { .. } => {
                    format!("{t}{}の前に次のように{}", sep(&p), end("加え", "加える"))
                }
                TableAction::Append { .. } => {
                    format!("{t}{}に次のように{}", sep(&p), end("加え", "加える"))
                }
                TableAction::Renumber { to } => {
                    format!("{t}{}を{to}と{}", sep(&p), end("し", "する"))
                }
                TableAction::Shift { by } => format!(
                    "{t}{}を{}ずつ繰り{}",
                    sep(&p),
                    to_kanji(by.unsigned_abs()),
                    if *by > 0 {
                        end("下げ", "下げる")
                    } else {
                        end("上げ", "上げる")
                    }
                ),
            }
        }
        // 原始附則: 中の操作の位置に「附則」を冠する（項だけの附則は仮の第0条）
        Op::Suppl(inner) => {
            let s = segment(inner, last);
            if s.starts_with("第0条") {
                s.replacen("第0条", "附則", 1)
            } else {
                format!("附則{s}")
            }
        }
        Op::ReplaceAll { from, to } if to.is_empty() => {
            format!("本則中「{from}」を{}", end("削り", "削る"))
        }
        Op::ReplaceAll { from, to } => {
            format!("本則中「{from}」を「{to}」に{}", end("改め", "改める"))
        }
        Op::ReplaceToc { from, to } if to.is_empty() => {
            format!("目次中「{from}」を{}", end("削り", "削る"))
        }
        Op::ReplaceToc { from, to } => {
            format!("目次中「{from}」を「{to}」に{}", end("改め", "改める"))
        }
        Op::Replace { at, from, to } if to.is_empty() => {
            format!("{}中「{from}」を{}", loc_label(at), end("削り", "削る"))
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
        Op::InsertParagraphFirst { article, .. } => format!(
            "{}に第一項として次の一項を{}",
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
        Op::InsertContainersAfter { path, text } | Op::InsertContainersBefore { path, text } => {
            let side = if matches!(op, Op::InsertContainersBefore { .. }) {
                "前"
            } else {
                "次"
            };
            let kind = path.last().map(|(k, _)| kind_label(*k)).unwrap_or("章");
            let n = text
                .iter()
                .filter(|l| {
                    l.trim_start_matches('\u{3000}').starts_with('第')
                        && l.split_once('\u{3000}')
                            .is_some_and(|(t, _)| t.ends_with(kind))
                })
                .count()
                .max(1);
            format!(
                "{}の{side}に次の{}{kind}を{}",
                path_label(path),
                to_kanji(n as u32),
                end("加え", "加える")
            )
        }
        Op::RenumberContainer { path, to } => format!(
            "{}を第{}{}と{}",
            path_label(path),
            item_kanji(to),
            path.last().map(|(k, _)| kind_label(*k)).unwrap_or("章"),
            end("し", "する")
        ),
        Op::AppendSupplArticles { text } => format!(
            "附則に次の{}{}条を{}",
            if text.first().is_some_and(|l| l.starts_with('（')) {
                "見出し及び"
            } else {
                ""
            },
            to_kanji(
                text.iter()
                    .filter(|l| l.starts_with('第') && l.contains('\u{3000}'))
                    .count() as u32
            ),
            end("加え", "加える")
        ),
        Op::AppendContainers { path, text } => {
            // 加える容器の種類は最初の行から（「第二章の二　…」→ 章）
            let kind = text
                .iter()
                .find_map(|l| {
                    l.trim_start_matches('\u{3000}')
                        .split_once('\u{3000}')
                        .map(|(t, _)| t.chars().last().unwrap_or('章'))
                })
                .unwrap_or('章');
            format!(
                "{}に次の{}{kind}を{}",
                if path.is_empty() {
                    "本則".to_string()
                } else {
                    path.iter()
                        .map(|(k, n)| cont_label(*k, n))
                        .collect::<String>()
                },
                to_kanji(
                    text.iter()
                        .filter(|l| {
                            l.trim_start_matches('\u{3000}').starts_with('第')
                                && l.split_once('\u{3000}')
                                    .is_some_and(|(t, _)| t.ends_with(kind))
                        })
                        .count() as u32
                ),
                end("加え", "加える")
            )
        }
        Op::InsertArticleBefore {
            before,
            text,
            suppl,
        } => format!(
            "{}{}の前に次の{}条を{}",
            if *suppl { "附則" } else { "" },
            article_label(before),
            to_kanji(
                text.iter()
                    .filter(|l| l.starts_with('第') && l.contains('\u{3000}'))
                    .count() as u32
            ),
            end("加え", "加える")
        ),
        Op::AppendArticle { path, text } => format!(
            "{}に次の{}条を{}",
            path.iter()
                .map(|(k, n)| cont_label(*k, n))
                .collect::<String>(),
            to_kanji(
                text.iter()
                    .filter(|l| l.starts_with('第') && l.contains('\u{3000}'))
                    .count()
                    .max(1) as u32
            ),
            end("加え", "加える")
        ),
        Op::ReplaceTitle { from, to } if to.is_empty() => {
            format!("題名中「{from}」を{}", end("削り", "削る"))
        }
        Op::ReplaceTitle { from, to } => {
            format!("題名中「{from}」を「{to}」に{}", end("改め", "改める"))
        }
        Op::SetTitle { .. } => format!("題名を次のように{}", end("改め", "改める")),
        Op::SetToc { replace: true, .. } => format!("目次を次のように{}", end("改め", "改める")),
        Op::SetToc { .. } => "題名の次に次の目次を付する".to_string(),
        Op::DeleteContainerTitle { path } => format!(
            "{}の{}名を{}",
            path_label(path),
            path.last().map(|(k, _)| kind_label(*k)).unwrap_or(""),
            end("削り", "削る")
        ),
        Op::DeleteContainers {
            path,
            kind,
            from,
            to,
        } => format!(
            "{}第{}{}から第{}{}までを{}",
            path_label(path),
            to_kanji(*from),
            kind_label(*kind),
            to_kanji(*to),
            kind_label(*kind),
            end("削り", "削る")
        ),
        Op::ReplaceAppdxRow {
            table,
            row,
            sub,
            from,
            to,
        } => format!(
            "{}{}中「{from}」を{}",
            appdx_loc(table, row),
            sub.clone().unwrap_or_default(),
            if to.is_empty() {
                end("削り", "削る").to_string()
            } else {
                format!("「{to}」に{}", end("改め", "改める"))
            }
        ),
        Op::ReplaceContainers { paths, .. } => format!(
            "{}を次のように{}",
            paths
                .iter()
                .map(|p| path_label(p))
                .collect::<Vec<_>>()
                .join("及び"),
            end("改め", "改める")
        ),
        Op::ReplaceArticles { articles, .. } => format!(
            "{}を次のように{}",
            articles
                .iter()
                .map(article_label)
                .collect::<Vec<_>>()
                .join("及び"),
            end("改め", "改める")
        ),
        Op::SetContainerTitle { path, .. } => format!(
            "{}の{}名を次のように{}",
            path_label(path),
            path.last().map(|(k, _)| kind_label(*k)).unwrap_or(""),
            end("改め", "改める")
        ),
        Op::RenumberItem { at, from, to } => format!(
            "{}中第{}号を第{}号と{}",
            loc_label(at),
            item_kanji(from),
            item_kanji(to),
            end("し", "する")
        ),
        Op::ShiftItems { at, from, to, by } => format!(
            "{}中{}を{}号ずつ繰り{}",
            loc_label(at),
            shift_range(*from, *to, "号"),
            to_kanji(by.unsigned_abs()),
            if *by > 0 {
                end("下げ", "下げる")
            } else {
                end("上げ", "上げる")
            }
        ),
        Op::InsertItemBefore { at, before, text } => format!(
            "{}第{}号の前に次の{}号を{}",
            loc_label(at),
            item_kanji(before),
            to_kanji(count_items(text)),
            end("加え", "加える")
        ),
        Op::InsertItemAfter { at, after, text } => format!(
            "{}第{}号の次に次の{}号を{}",
            loc_label(at),
            item_kanji(after),
            to_kanji(count_items(text)),
            end("加え", "加える")
        ),
        Op::InsertItemFirst { at, text } => format!(
            "{}に第一号として次の{}号を{}",
            loc_label(at),
            to_kanji(count_items(text)),
            end("加え", "加える")
        ),
        Op::ReplaceItems { at, .. } => {
            format!("{}各号を次のように{}", loc_label(at), end("改め", "改める"))
        }
        Op::ReplaceTableRow { at, row, from, to } if row.is_empty() && to.is_empty() => {
            format!("{}の表中「{from}」を{}", loc_label(at), end("削り", "削る"))
        }
        Op::ReplaceTableRow { at, row, from, to } if row.is_empty() => format!(
            "{}の表中「{from}」を「{to}」に{}",
            loc_label(at),
            end("改め", "改める")
        ),
        Op::ReplaceTableRow { at, row, from, to } if to.is_empty() => format!(
            "{}の表{row}の項中「{from}」を{}",
            loc_label(at),
            end("削り", "削る")
        ),
        Op::ReplaceTableRow { at, row, from, to } => format!(
            "{}の表{row}の項中「{from}」を「{to}」に{}",
            loc_label(at),
            end("改め", "改める")
        ),
        Op::AppendTable { at, .. } => {
            format!("{}に次の表を{}", loc_label(at), end("加え", "加える"))
        }
        Op::DeleteAppdx { tables } => format!("{}を{}", tables.join("及び"), end("削り", "削る")),
        Op::ReplaceAppdxRowWhole { table, row, .. } => {
            format!("{table}の{row}の項を次のように{}", end("改め", "改める"))
        }
        Op::DeleteAppdxRows { table, rows } => format!(
            "{table}中{}を{}",
            rows.iter()
                .map(|r| format!("{r}の項"))
                .collect::<Vec<_>>()
                .join("及び"),
            end("削り", "削る")
        ),
        Op::RenumberAppdxRow { table, from, to } => {
            format!("{table}中{from}の項を{to}の項と{}", end("し", "する"))
        }
        Op::InsertAppdxRowsAfter { table, after, .. } => format!(
            "{}の次に次のように{}",
            appdx_loc(table, after),
            end("加え", "加える")
        ),
        Op::AppendAppdx { .. } => format!("附則の次に次の別表を{}", end("加え", "加える")),
        Op::RenameAppdx { from, to } => format!("{from}を{to}と{}", end("し", "する")),
        Op::InsertAppdxAfter { after, .. } => {
            format!("{after}の次に次の一表を{}", end("加え", "加える"))
        }
        Op::DeleteAppdxRowSub { table, row, sub } => {
            format!("{}中{sub}を{}", appdx_loc(table, row), end("削り", "削る"))
        }
        Op::RenumberAppdxRowSub {
            table,
            row,
            from,
            to,
        } => format!(
            "{}中{from}を{to}と{}",
            appdx_loc(table, row),
            end("し", "する")
        ),
        Op::RenumberSubitem { at, from, to } => {
            format!("{}{from}を{to}と{}", loc_label(at), end("し", "する"))
        }
        Op::ShiftSubitems { at, from, to, by } => {
            let (a, b) = (lawean_amend::kana_index(from), lawean_amend::kana_index(to));
            format!(
                "{}{from}から{to}までを{}から{}までと{}",
                loc_label(at),
                lawean_amend::kana_of((a as i32 + by) as u32),
                lawean_amend::kana_of((b as i32 + by) as u32),
                end("し", "する")
            )
        }
        Op::InsertSubitemAfter { at, after, .. } => format!(
            "{}{after}の次に次のように{}",
            loc_label(at),
            end("加え", "加える")
        ),
        Op::ReplaceItemSet {
            at, items, range, ..
        } => {
            let list = if *range && items.len() >= 2 {
                format!(
                    "第{}号から第{}号まで",
                    item_kanji(&items[0]),
                    item_kanji(&items[items.len() - 1])
                )
            } else {
                let labels: Vec<String> = items
                    .iter()
                    .map(|i| format!("第{}号", item_kanji(i)))
                    .collect();
                match labels.len() {
                    0 | 1 => labels.concat(),
                    n => format!("{}及び{}", labels[..n - 1].join("、"), labels[n - 1]),
                }
            };
            format!(
                "{}{list}を次のように{}",
                loc_label(at),
                end("改め", "改める")
            )
        }
        Op::AppendItem { at, text } if at.item.is_some() => {
            format!("{}に次のように{}", loc_label(at), end("加え", "加える"))
        }
        Op::AppendItem { at, text } => format!(
            "{}に次の{}号を{}",
            loc_label(at),
            to_kanji(count_items(text)),
            end("加え", "加える")
        ),
        Op::ReplaceItem { at, .. } => {
            format!("{}を次のように{}", loc_label(at), end("改め", "改める"))
        }
        Op::ReplaceParagraph { at, .. } => {
            format!("{}を次のように{}", loc_label(at), end("改め", "改める"))
        }
        Op::ReplaceArticle { article, .. } => format!(
            "{}を次のように{}",
            article_label(article),
            end("改め", "改める")
        ),
        Op::InsertArticleAfter { after, text, suppl } => format!(
            "{}{}の次に次の{}条を{}",
            if *suppl { "附則" } else { "" },
            article_label(after),
            to_kanji(
                text.iter()
                    .filter(|l| l.starts_with('第') && l.contains('\u{3000}'))
                    .count()
                    .max(1) as u32
            ),
            end("加え", "加える")
        ),
        Op::AppendSentence { at, text } => format!(
            "{}に{}{}",
            loc_label(at),
            if text.first().is_some_and(|t| t.starts_with("ただし")) {
                "次のただし書を"
            } else {
                "後段として次のように"
            },
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
            "{}中{}を{}項ずつ繰り{}",
            article_label(article),
            shift_range(*from, *to, "項"),
            to_kanji(by.unsigned_abs()),
            if *by > 0 {
                end("下げ", "下げる")
            } else {
                end("上げ", "上げる")
            }
        ),
        Op::Delete { at } => format!("{}を{}", loc_label(at), end("削り", "削る")),
        Op::RenumberArticle { from, to, suppl } => format!(
            "{}{}を{}{}と{}",
            if *suppl { "附則" } else { "" },
            article_label(from),
            if *suppl { "附則" } else { "" },
            article_label(to),
            end("し", "する")
        ),
        Op::ShiftArticles { from, to, by } => format!(
            "{}を{}条ずつ繰り{}",
            shift_range(*from, *to, "条"),
            to_kanji(by.unsigned_abs()),
            if *by > 0 {
                end("下げ", "下げる")
            } else {
                end("上げ", "上げる")
            }
        ),
        Op::ReplaceContainerTitle { path, from, to } => format!(
            "{}の{}名中「{from}」を{}",
            path.iter()
                .map(|(k, n)| cont_label(*k, n))
                .collect::<String>(),
            path.last().map(|(k, _)| kind_label(*k)).unwrap_or(""),
            if to.is_empty() {
                end("削り", "削る").to_string()
            } else {
                format!("「{to}」に{}", end("改め", "改める"))
            }
        ),
        Op::ReplaceCaption { article, from, to } if to.is_empty() => format!(
            "{}の見出し中「{from}」を{}",
            article_label(article),
            end("削り", "削る")
        ),
        Op::ReplaceCaption { article, from, to } => format!(
            "{}の見出し中「{from}」を「{to}」に{}",
            article_label(article),
            end("改め", "改める")
        ),
        Op::SetCaption { article, text } => format!(
            "{}の見出しを「{text}」に{}",
            article_label(article),
            end("改め", "改める")
        ),
        Op::AttachCaption { article, text } => format!(
            "{}の前に見出しとして「{text}」を{}",
            article_label(article),
            end("付し", "付する")
        ),
        Op::DeleteCaption { article } => format!(
            "{}の見出しを{}",
            article_label(article),
            end("削り", "削る")
        ),
        Op::ReplaceSentencePart { at, part, .. } => format!(
            "{}{}を次のように{}",
            loc_label(at),
            part_label(*part),
            end("改め", "改める")
        ),
        Op::DeleteSentencePart { at, part } => format!(
            "{}{}を{}",
            loc_label(at),
            part_label(*part),
            end("削り", "削る")
        ),
    }
}

fn content_of(op: &Op) -> &[String] {
    match op {
        Op::AppendParagraph { text, .. }
        | Op::InsertParagraphAfter { text, .. }
        | Op::InsertParagraphFirst { text, .. }
        | Op::AppendArticle { text, .. }
        | Op::InsertContainersAfter { text, .. }
        | Op::InsertContainersBefore { text, .. }
        | Op::InsertArticleAfter { text, .. }
        | Op::AppendSentence { text, .. }
        | Op::ReplaceArticle { text, .. }
        | Op::ReplaceParagraph { text, .. }
        | Op::ReplaceItem { text, .. }
        | Op::InsertItemAfter { text, .. }
        | Op::InsertItemBefore { text, .. }
        | Op::AppendItem { text, .. }
        | Op::InsertItemFirst { text, .. }
        | Op::ReplaceItems { text, .. }
        | Op::ReplaceItemSet { text, .. }
        | Op::InsertSubitemAfter { text, .. }
        | Op::AppendSupplArticles { text, .. }
        | Op::AppendContainers { text, .. }
        | Op::InsertArticleBefore { text, .. }
        | Op::AppendTable { text, .. }
        | Op::ReplaceAppdxRowWhole { text, .. }
        | Op::InsertAppdxRowsAfter { text, .. }
        | Op::AppendAppdx { text, .. }
        | Op::InsertAppdxAfter { text, .. }
        | Op::SetTitle { text, .. }
        | Op::SetToc { text, .. }
        | Op::SetContainerTitle { text, .. }
        | Op::ReplaceArticles { text, .. }
        | Op::ReplaceContainers { text, .. }
        | Op::ReplaceSentencePart { text, .. } => text,
        Op::TableEdit {
            action:
                TableAction::Replace { text }
                | TableAction::InsertAfter { text }
                | TableAction::InsertBefore { text }
                | TableAction::Append { text },
            ..
        } => text,
        Op::Suppl(inner) => content_of(inner),
        Op::SubitemsEdit {
            text: Some(text), ..
        }
        | Op::ReplacePairs { text, .. } => text,
        Op::InsertContainersAfterArticle { text, .. }
        | Op::InsertHeadingsBefore { text, .. }
        | Op::SetContainerTitles { text, .. } => text,
        _ => &[],
    }
}

/// 1 文（複数操作は「、」で連ねる）。追加する条文はインデント 1 で続ける
pub fn render_instruction(ins: &Instruction) -> String {
    let n = ins.ops.len();
    let mut s = String::from("\u{3000}\u{3000}");
    let phrase_loc = |op: &Op| match op {
        Op::Replace { at, .. } | Op::InsertAfterPhrase { at, .. } => Some(loc_label(at)),
        Op::ReplaceToc { .. } => Some("目次".to_string()),
        Op::ReplaceAppdxRow {
            table, row, sub, ..
        } => Some(format!(
            "{}{}",
            appdx_loc(table, row),
            sub.clone().unwrap_or_default()
        )),
        _ => None,
    };
    for (i, op) in ins.ops.iter().enumerate() {
        if i > 0 {
            s.push('、');
        }
        let mut seg = segment(op, i + 1 == n);
        // 同じ位置に続く字句の操作は位置を繰り返さない（「A」を「B」に、「C」を「D」に改め）。
        // 次も同じ位置の同じ種類の置換なら「改め」「加え」も省く
        let same_loc =
            i > 0 && phrase_loc(op).is_some() && phrase_loc(op) == phrase_loc(&ins.ops[i - 1]);
        let next_same = i + 1 < n
            && phrase_loc(op).is_some()
            && phrase_loc(&ins.ops[i + 1]) == phrase_loc(op)
            && std::mem::discriminant(op) == std::mem::discriminant(&ins.ops[i + 1]);
        if next_same {
            for v in ["に改め", "を加え"] {
                if let Some(base) = seg.strip_suffix(v) {
                    seg = format!("{base}{}", &v[..v.find(['改', '加']).unwrap()]);
                    break;
                }
            }
        }
        if same_loc {
            if let Some(k) = seg.find("中「") {
                s.push_str(&seg[k + '中'.len_utf8()..]);
                continue;
            }
        }
        s.push_str(&seg);
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
