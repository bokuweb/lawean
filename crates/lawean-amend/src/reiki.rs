//! 例規（条例・規則、横書き）の改め文を、法律の改め文の書き方に写す（`parse_units` で読むため）。
//!
//! 例規の改め文は算用数字（「第34条の７第２項中」「次の１条を加える」）で、号を「(1)」「⑴」と書く。
//! 法律の書き方に写すのは位置と数だけで、「」の中の字句と、加える条文の本文は変えない（改正前の例規の条文と突き合わせるため）:
//!
//! - 改正の文と柱書き: 「」の外の数字を漢数字にする
//! - 加える条文: 行頭の番号だけを写す。条（「第５条の２　」）は漢数字、号（「(3)　」「⑶」）は「三　」。項（「２　」）はそのまま
//! - 柱書きに改正する条例の条が無い（単独の一部改正条例「X条例（…）の一部を次のように改正する。」）ときは「第一条　」を補う
use regex::Regex;
use std::sync::OnceLock;

/// 数（1〜9999）を漢数字に
fn kanji(n: u32) -> String {
    const D: [&str; 10] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    if n == 0 {
        return "〇".into();
    }
    let mut s = String::new();
    for (unit, name) in [(1000, "千"), (100, "百"), (10, "十")] {
        let d = n / unit % 10;
        if d > 0 {
            if d > 1 {
                s.push_str(D[d as usize]);
            }
            s.push_str(name);
        }
    }
    s.push_str(D[(n % 10) as usize]);
    s
}

fn digits(s: &str) -> Option<u32> {
    let half: String = s
        .chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            _ => c,
        })
        .collect();
    half.parse().ok()
}

/// 「」の外の数字の並びを漢数字にする（「」の中は例規の条文の字句なので、そのまま）
fn kanji_outside_quotes(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if !run.is_empty() {
            match digits(run) {
                Some(n) if n < 10000 => out.push_str(&kanji(n)),
                _ => out.push_str(run),
            }
            run.clear();
        }
    };
    let chars: Vec<char> = line.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let digit = c.is_ascii_digit() || ('０'..='９').contains(&c);
        if digit && depth == 0 {
            run.push(c);
            continue;
        }
        // 表の番号の行（「３７の項」「２の部」）: 例規の表の番号の欄は算用数字なので写さない
        let rest: String = chars[i..].iter().take(2).collect();
        if !run.is_empty() && (rest == "の項" || rest == "の部") {
            out.push_str(&run);
            run.clear();
        }
        flush(&mut run, &mut out);
        match c {
            '「' => depth += 1,
            '」' => depth = depth.saturating_sub(1),
            _ => {}
        }
        out.push(c);
    }
    flush(&mut run, &mut out);
    out
}

/// 改正の文か（柱書きの後の行で、改め文の終わり方をして、位置から書き出す）
fn is_instruction(t: &str) -> bool {
    static END: OnceLock<Regex> = OnceLock::new();
    let end = END.get_or_init(|| {
        Regex::new(r"(改める|加える|削る|とする|付する|繰り上げる|繰り下げる|移す|置く)。$")
            .unwrap()
    });
    static HEAD: OnceLock<Regex> = OnceLock::new();
    let head = HEAD.get_or_init(|| {
        Regex::new(r"^(第[0-9０-９]|同|附則|付則|別表|別記|様式|目次|題名|本則|前文|次に|表|備考)")
            .unwrap()
    });
    // 加える条（「第２０条の３　…とする。」）は改め文ではない。改め文は「第N条中」「第N条を」のように続ける
    static ARTICLE_LINE: OnceLock<Regex> = OnceLock::new();
    let article_line = ARTICLE_LINE
        .get_or_init(|| Regex::new(r"^第[0-9０-９]+条(?:の[0-9０-９]+)*\u{3000}").unwrap());
    end.is_match(t) && head.is_match(t) && !article_line.is_match(t)
}

/// 加える条文の行頭の番号を法律の書き方に: 条は漢数字、号（「(3)」「⑶」「（３）」）は「三」
fn content_line(t: &str) -> String {
    // 別表の題名（「別表第２（第１３条関係）」）は、組み立てた例規の別表（`document`）と同じく番号を漢数字に
    static APPDX: OnceLock<Regex> = OnceLock::new();
    let appdx = APPDX
        .get_or_init(|| Regex::new(r"^(別表|様式|別記)第[0-9０-９]+(?:の[0-9０-９]+)*").unwrap());
    if let Some(m) = appdx.find(t) {
        if t.chars().count() < 60 {
            return format!("{}{}", kanji_outside_quotes(m.as_str()), &t[m.end()..]);
        }
    }
    static ARTICLE: OnceLock<Regex> = OnceLock::new();
    let article = ARTICLE
        .get_or_init(|| Regex::new(r"^第[0-9０-９]+条(?:の[0-9０-９]+)*(?:\u{3000}|$)").unwrap());
    if let Some(m) = article.find(t) {
        return format!("{}{}", kanji_outside_quotes(m.as_str()), &t[m.end()..]);
    }
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let item = ITEM.get_or_init(|| {
        Regex::new(r"^(?:\(([0-9０-９]+)\)|（([0-9０-９]+)）|([⑴-⒇]))\u{3000}?").unwrap()
    });
    if let Some(c) = item.captures(t) {
        let n = match (c.get(1).or(c.get(2)), c.get(3)) {
            (Some(d), _) => digits(d.as_str()),
            (None, Some(circled)) => circled
                .as_str()
                .chars()
                .next()
                .map(|ch| ch as u32 - '⑴' as u32 + 1),
            _ => None,
        };
        if let Some(n) = n {
            return format!(
                "{}\u{3000}{}",
                kanji(n),
                &t[c.get(0).map_or(0, |m| m.end())..]
            );
        }
    }
    t.to_string()
}

/// 例規の改め文（柱書きから、1 行 1 段落）を、`parse_units` で読める法律の改め文の書き方に写す
pub fn to_law_style(lines: &[&str]) -> String {
    static HEADER: OnceLock<Regex> = OnceLock::new();
    let header = HEADER.get_or_init(|| {
        Regex::new(r"^(?:(第[0-9０-９]+条)\u{3000}?|[0-9０-９]+\u{3000})?(.+の一部を次のように改正する。)$").unwrap()
    });
    let mut out = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let t = raw.trim_start_matches(['\u{3000}', ' ']).trim_end();
        if i == 0 {
            if let Some(c) = header.captures(t) {
                let art = c.get(1).map_or("第一条".to_string(), |m| {
                    kanji_outside_quotes(m.as_str())
                });
                out.push(format!("{art}\u{3000}{}", kanji_outside_quotes(&c[2])));
                continue;
            }
        }
        if is_instruction(t) {
            // 自治体によって附則を「付則」と書く（北九州市）
            out.push(kanji_outside_quotes(t).replace("付則", "附則"));
        } else {
            out.push(content_line(t));
        }
    }
    out.join("\n")
}

/// 例規の条文の行と表（構造付き plain text の `paragraph` / `table`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(String),
    Table(Vec<Vec<String>>),
}

/// 数字・括弧・位取りの幅を揃える（公布された改め文は全角、例規集は 2 桁以上を半角で書く）。
/// 改め文と改正前・改正後の条文の両方にかけて、字句を突き合わせられるようにする
pub fn canonical(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0'..='9' => char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap_or(c),
            '(' => '（',
            ')' => '）',
            ',' => '，',
            '.' => '．',
            _ => c,
        })
        .collect()
}

const KATAKANA: &str =
    "アイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワヲン";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 本文を「。」で文に分ける（「」と（）の中では分けない）
fn split_sentences(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in t.chars() {
        cur.push(c);
        match c {
            '「' | '（' => depth += 1,
            '」' | '）' => depth -= 1,
            '。' if depth <= 0 => out.push(std::mem::take(&mut cur)),
            _ => {}
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// e-Gov の形の文: 「ただし」で始まる文から後はただし書（`Function="proviso"`）、前は本文。
/// ただし書の無い 2 文以上は前段・後段（番号だけ）
fn sentence(tag: &str, t: &str) -> String {
    let parts = split_sentences(t);
    let proviso = parts.iter().position(|p| p.starts_with("ただし"));
    let mut x = format!("<{tag}>");
    for (i, p) in parts.iter().enumerate() {
        let function = match proviso {
            Some(at) if i >= at => " Function=\"proviso\"",
            Some(_) => " Function=\"main\"",
            None => "",
        };
        x.push_str(&format!(
            "<Sentence Num=\"{}\"{function}>{}</Sentence>",
            i + 1,
            esc(p)
        ));
    }
    if parts.is_empty() {
        x.push_str("<Sentence></Sentence>");
    }
    x.push_str(&format!("</{tag}>"));
    x
}

fn table_xml(rows: &[Vec<String>]) -> String {
    let mut x = String::from("<TableStruct><Table>");
    for r in rows {
        x.push_str("<TableRow>");
        for c in r {
            x.push_str(&format!(
                "<TableColumn><Sentence>{}</Sentence></TableColumn>",
                esc(c)
            ));
        }
        x.push_str("</TableRow>");
    }
    x.push_str("</Table></TableStruct>");
    x
}

fn from_kanji(s: &str) -> u32 {
    let mut n = 0;
    let mut cur = 0;
    for c in s.chars() {
        if c == '十' {
            n += if cur == 0 { 10 } else { cur * 10 };
            cur = 0;
        } else {
            cur = "〇一二三四五六七八九"
                .chars()
                .position(|k| k == c)
                .unwrap_or(0) as u32;
        }
    }
    n + cur
}

/// 条・項・号・細目の入れ子を閉じながら e-Gov の形の要素を組む
#[derive(Default)]
struct Builder {
    out: String,
    /// 開いている要素（外から）: Article / Paragraph / Item / Subitem1 / Subitem2
    open: Vec<&'static str>,
    caption: Option<String>,
    para_num: u32,
}

impl Builder {
    fn close_to(&mut self, depth: usize) {
        while self.open.len() > depth {
            let tag = self.open.pop().unwrap_or_default();
            self.out.push_str(&format!("</{tag}>"));
        }
    }

    fn depth_of(&self, tag: &str) -> usize {
        self.open
            .iter()
            .position(|t| *t == tag)
            .unwrap_or(self.open.len())
    }

    fn article(&mut self, num: &str, title: &str, body: &str) {
        self.close_to(0);
        self.out.push_str(&format!("<Article Num=\"{num}\">"));
        if let Some(c) = self.caption.take() {
            self.out
                .push_str(&format!("<ArticleCaption>{}</ArticleCaption>", esc(&c)));
        }
        self.out
            .push_str(&format!("<ArticleTitle>{}</ArticleTitle>", esc(title)));
        self.open.push("Article");
        self.para_num = 0;
        self.paragraph(None, body);
    }

    fn paragraph(&mut self, label: Option<&str>, body: &str) {
        let d = self.depth_of("Paragraph");
        self.close_to(d);
        self.para_num += 1;
        // 条の無い本則・附則の項の見出し（「（経過措置）」+「２　…」）は項の見出し
        let caption = if self.open.is_empty() {
            self.caption
                .take()
                .map(|c| format!("<ParagraphCaption>{}</ParagraphCaption>", esc(&c)))
                .unwrap_or_default()
        } else {
            String::new()
        };
        self.out.push_str(&format!(
            "<Paragraph Num=\"{}\">{caption}<ParagraphNum>{}</ParagraphNum>{}",
            self.para_num,
            esc(label.unwrap_or("")),
            sentence("ParagraphSentence", body)
        ));
        self.open.push("Paragraph");
    }

    fn ensure_paragraph(&mut self) {
        if !self.open.contains(&"Paragraph") {
            self.paragraph(None, "");
        }
    }

    fn item(&mut self, n: u32, body: &str) {
        self.ensure_paragraph();
        let d = self.depth_of("Item");
        self.close_to(d);
        self.out.push_str(&format!(
            "<Item Num=\"{n}\"><ItemTitle>{}</ItemTitle>{}",
            kanji(n),
            sentence("ItemSentence", body)
        ));
        self.open.push("Item");
    }

    fn subitem(&mut self, level: u8, n: u32, title: &str, body: &str) {
        let (tag, parent) = if level == 1 {
            ("Subitem1", "Item")
        } else {
            ("Subitem2", "Subitem1")
        };
        if !self.open.contains(&parent) {
            // 上の段が無い（号の無い条の「ア」など）: 本文の行として続ける
            self.text(&format!("{title}\u{3000}{body}"));
            return;
        }
        let d = self.depth_of(tag);
        self.close_to(d);
        self.out.push_str(&format!(
            "<{tag} Num=\"{n}\"><{tag}Title>{}</{tag}Title>{}",
            esc(title),
            sentence(&format!("{tag}Sentence"), body)
        ));
        self.open.push(tag);
    }

    /// 番号の無い行（表の見出し・備考の類）: いまの段落に続ける
    fn text(&mut self, t: &str) {
        self.ensure_paragraph();
        let d = self.depth_of("Paragraph") + 1;
        self.close_to(d);
        self.out.push_str(&format!(
            "<List><ListSentence><Sentence>{}</Sentence></ListSentence></List>",
            esc(t)
        ));
    }

    fn table(&mut self, rows: &[Vec<String>]) {
        self.ensure_paragraph();
        let d = self.depth_of("Paragraph") + 1;
        self.close_to(d);
        self.out.push_str(&table_xml(rows));
    }
}

/// 「第５条の２」の番号（「5_2」）と、法律の書き方の見出し（「第五条の二」）
fn article_num(nums: &str) -> (String, String) {
    let parts: Vec<u32> = nums.split('の').filter_map(digits).collect();
    let num = parts
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("_");
    let mut title = String::from("第");
    for (i, n) in parts.iter().enumerate() {
        if i == 0 {
            title.push_str(&format!("{}条", kanji(*n)));
        } else {
            title.push_str(&format!("の{}", kanji(*n)));
        }
    }
    (num, title)
}

/// 例規の条文（構造付き plain text。先頭は題名）から、e-Gov の形の法令を組んで読む。
/// 行頭の番号は法律の書き方に揃える（条「第五条の二」、号「一」）。字句は `canonical` で幅を揃える
pub fn document(blocks: &[Block]) -> Result<lawean_source::LegalDocument, String> {
    static ARTICLE: OnceLock<Regex> = OnceLock::new();
    let article = ARTICLE
        .get_or_init(|| Regex::new(r"^第([０-９]+)条((?:の[０-９]+)*)(?:\u{3000}(.*))?$").unwrap());
    static CAPTION: OnceLock<Regex> = OnceLock::new();
    let caption = CAPTION.get_or_init(|| Regex::new(r"^（[^（）]{1,60}）$").unwrap());
    static PARA: OnceLock<Regex> = OnceLock::new();
    let para = PARA.get_or_init(|| Regex::new(r"^([０-９]+)\u{3000}(.*)$").unwrap());
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let item = ITEM.get_or_init(|| {
        Regex::new(r"^(?:（([０-９]+)）|([一二三四五六七八九十]+))\u{3000}(.*)$").unwrap()
    });
    static SUB1: OnceLock<Regex> = OnceLock::new();
    let sub1 = SUB1.get_or_init(|| Regex::new(r"^([ア-ン])\u{3000}(.*)$").unwrap());
    static SUB2: OnceLock<Regex> = OnceLock::new();
    let sub2 = SUB2.get_or_init(|| Regex::new(r"^(（[ア-ン]）)\u{3000}(.*)$").unwrap());
    static APPDX: OnceLock<Regex> = OnceLock::new();
    let appdx_title = APPDX.get_or_init(|| Regex::new(r"^(別表|様式|別記)").unwrap());
    static NUMBERED: OnceLock<Regex> = OnceLock::new();
    let numbered =
        NUMBERED.get_or_init(|| Regex::new(r"^(別表|様式|別記)第[０-９]+(?:の[０-９]+)*").unwrap());

    let mut blocks = blocks.iter().map(|b| match b {
        Block::Paragraph(t) => Block::Paragraph(canonical(t)),
        Block::Table(rows) => Block::Table(
            rows.iter()
                .map(|r| r.iter().map(|c| canonical(c)).collect())
                .collect(),
        ),
    });
    let title = match blocks.next() {
        Some(Block::Paragraph(t)) => t,
        _ => return Err("no title".into()),
    };
    #[derive(PartialEq)]
    enum Part {
        Toc,
        Main,
        Suppl,
        Appdx,
    }
    let mut part = Part::Main;
    let mut toc = String::new();
    let mut main = Builder::default();
    let mut suppl = Builder::default();
    let mut appdx = String::new();
    let mut appdx_open = false;
    for b in blocks {
        let t = match b {
            Block::Table(rows) => {
                match part {
                    Part::Appdx => appdx.push_str(&table_xml(&rows)),
                    Part::Suppl => suppl.table(&rows),
                    _ => main.table(&rows),
                }
                continue;
            }
            Block::Paragraph(t) => t,
        };
        if t == "目次" {
            part = Part::Toc;
            continue;
        }
        if part == Part::Toc {
            if article.is_match(&t) || caption.is_match(&t) {
                part = Part::Main;
            } else {
                toc.push_str(&format!(
                    "<TOCChapter><ChapterTitle>{}</ChapterTitle></TOCChapter>",
                    esc(&t)
                ));
                continue;
            }
        }
        let plain = t.replace('\u{3000}', "");
        if plain == "附則" || plain == "付則" {
            part = Part::Suppl;
            continue;
        }
        // 別表・様式の題名（短い行）から先は別表
        if appdx_title.is_match(&t) && t.chars().count() < 60 {
            if appdx_open {
                appdx.push_str("</AppdxTable>");
            }
            // 表題の番号は法律の書き方（「別表第１（第２条関係）」→「別表第一（第２条関係）」。改め文の位置と照らす）
            let title = match numbered.find(&t) {
                Some(m) => format!("{}{}", kanji_outside_quotes(m.as_str()), &t[m.end()..]),
                None => t.clone(),
            };
            appdx.push_str(&format!(
                "<AppdxTable><AppdxTableTitle>{}</AppdxTableTitle>",
                esc(&title)
            ));
            appdx_open = true;
            part = Part::Appdx;
            continue;
        }
        if part == Part::Appdx {
            appdx.push_str(&format!(
                "<Remarks><Sentence>{}</Sentence></Remarks>",
                esc(&t)
            ));
            continue;
        }
        let first_in_suppl = part == Part::Suppl && suppl.out.is_empty();
        let b = if part == Part::Suppl {
            &mut suppl
        } else {
            &mut main
        };
        if caption.is_match(&t) {
            b.close_to(0);
            b.caption = Some(t);
        } else if let Some(c) = article.captures(&t) {
            let (num, title) = article_num(&format!("{}{}", &c[1], &c[2]));
            b.article(&num, &title, c.get(3).map_or("", |m| m.as_str()));
        } else if let Some(c) = item.captures(&t) {
            let n = c
                .get(1)
                .and_then(|m| digits(m.as_str()))
                .or_else(|| c.get(2).map(|m| from_kanji(m.as_str())))
                .unwrap_or(0);
            b.item(n, &c[3]);
        } else if let Some(c) = para.captures(&t) {
            b.paragraph(Some(&c[1]), &c[2]);
        } else if let Some(c) = sub2.captures(&t) {
            let n = KATAKANA
                .chars()
                .position(|k| c[1].contains(k))
                .map_or(0, |p| p as u32 + 1);
            b.subitem(2, n, &c[1], &c[2]);
        } else if let Some(c) = sub1.captures(&t) {
            let n = KATAKANA
                .chars()
                .position(|k| c[1].starts_with(k))
                .map_or(0, |p| p as u32 + 1);
            b.subitem(1, n, &c[1], &c[2]);
        } else if first_in_suppl {
            // 附則の最初の項（番号の無い「この条例は、…から施行する。」）
            b.paragraph(None, &t);
        } else {
            b.text(&t);
        }
    }
    main.close_to(0);
    suppl.close_to(0);
    if appdx_open {
        appdx.push_str("</AppdxTable>");
    }
    let mut xml = format!(
        "<Law Era=\"Reiwa\" Year=\"1\" Num=\"1\" LawType=\"Ordinance\" Lang=\"ja\"><LawNum></LawNum><LawBody><LawTitle>{}</LawTitle>",
        esc(&title)
    );
    if !toc.is_empty() {
        xml.push_str(&format!("<TOC><TOCLabel>目次</TOCLabel>{toc}</TOC>"));
    }
    xml.push_str(&format!("<MainProvision>{}</MainProvision>", main.out));
    if !suppl.out.is_empty() {
        xml.push_str(&format!(
            "<SupplProvision><SupplProvisionLabel>附　則</SupplProvisionLabel>{}</SupplProvision>",
            suppl.out
        ));
    }
    xml.push_str(&appdx);
    xml.push_str("</LawBody></Law>");
    lawean_source::parse_law_xml(&xml).map_err(|e| format!("{e}"))
}

/// 例規の当てた結果の突き合わせ: 本則・原始附則の条と項の本文、目次、別表の本文
fn snapshot(
    doc: &lawean_source::LegalDocument,
) -> std::collections::BTreeMap<String, Vec<(u32, String)>> {
    let mut out = crate::snapshot_main(doc);
    if let Some(sp) = doc.suppl_provisions.first() {
        let mut d = doc.clone();
        d.toc = None;
        d.main_provision = sp
            .children
            .iter()
            .map(|c| match c {
                lawean_source::SupplChild::Provision(p) => p.clone(),
                lawean_source::SupplChild::Paragraph(p) => {
                    lawean_source::Provision::Paragraph(p.clone())
                }
                lawean_source::SupplChild::Raw(e) => lawean_source::Provision::Raw(e.clone()),
            })
            .collect();
        for (k, v) in crate::snapshot_main(&d) {
            out.insert(format!("附則{k}"), v);
        }
    }
    for (i, ap) in doc.appendices.iter().enumerate() {
        let text: String = ap.text().chars().filter(|c| !c.is_whitespace()).collect();
        out.insert(format!("別表{i}"), vec![(0, text)]);
    }
    out
}

/// 当てて確かめられなかった理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckFailure {
    /// 条文を e-Gov の形に起こせない
    Document(String),
    /// 改め文を読めない
    Parse(String),
    /// 読めたが当てられない（`ApplyError::Unsupported` は `unsupported`）
    Apply { unsupported: bool, detail: String },
    /// 当てた結果が改正後と違う（最初に違う所の前後）
    Mismatch(String),
}

impl CheckFailure {
    pub fn kind(&self) -> &'static str {
        match self {
            CheckFailure::Document(_) => "doc_error",
            CheckFailure::Parse(_) => "parse_error",
            CheckFailure::Apply {
                unsupported: true, ..
            } => "unsupported",
            CheckFailure::Apply { .. } => "apply_error",
            CheckFailure::Mismatch(_) => "mismatch",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            CheckFailure::Document(d) | CheckFailure::Parse(d) | CheckFailure::Mismatch(d) => d,
            CheckFailure::Apply { detail, .. } => detail,
        }
    }
}

/// 例規の改め文（柱書きから）を改正前の条文に当て、改正後の条文と一致するかを確かめる
pub fn check(old: &[Block], new: &[Block], aratamebun: &[&str]) -> Result<(), CheckFailure> {
    let old = document(old).map_err(CheckFailure::Document)?;
    let new = document(new).map_err(CheckFailure::Document)?;
    let lines: Vec<String> = aratamebun.iter().map(|l| canonical(l)).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let units = match crate::parse_units(&to_law_style(&lines)) {
        Ok(u) if !u.is_empty() => u,
        Ok(_) => return Err(CheckFailure::Parse("no units".into())),
        Err(e) => return Err(CheckFailure::Parse(e.to_string())),
    };
    let mut doc = old;
    for u in &units {
        doc = crate::apply_unit(&doc, u, "applied").map_err(|e| CheckFailure::Apply {
            unsupported: matches!(e, crate::ApplyError::Unsupported(_)),
            detail: match e {
                crate::ApplyError::Unsupported(w) => w,
                e => e.to_string(),
            },
        })?;
    }
    let (got, want) = (snapshot(&doc), snapshot(&new));
    if got == want {
        return Ok(());
    }
    let text = |v: Option<&Vec<(u32, String)>>| {
        v.map(|x| {
            x.iter()
                .map(|(n, t)| format!("[{n}]{t}"))
                .collect::<String>()
        })
        .unwrap_or_default()
    };
    let key = got
        .keys()
        .chain(want.keys())
        .find(|k| got.get(*k) != want.get(*k))
        .cloned()
        .unwrap_or_default();
    let (g, w) = (text(got.get(&key)), text(want.get(&key)));
    let at = g
        .chars()
        .zip(w.chars())
        .position(|(a, b)| a != b)
        .unwrap_or(g.chars().count().min(w.chars().count()));
    let around = |s: &str| {
        s.chars()
            .skip(at.saturating_sub(12))
            .take(36)
            .collect::<String>()
    };
    Err(CheckFailure::Mismatch(format!(
        "{key}: {} ≠ {}",
        around(&g),
        around(&w)
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(old: &[&str], aratamebun: &[&str]) -> lawean_source::LegalDocument {
        let doc = document(
            &old.iter()
                .map(|t| Block::Paragraph(t.to_string()))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let lines: Vec<String> = aratamebun.iter().map(|l| canonical(l)).collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let units = crate::parse_units(&to_law_style(&lines)).unwrap();
        units
            .iter()
            .fold(doc, |d, u| crate::apply_unit(&d, u, "applied").unwrap())
    }

    fn texts(doc: &lawean_source::LegalDocument) -> Vec<String> {
        crate::snapshot_main(doc)
            .into_values()
            .flat_map(|v| v.into_iter().map(|(_, t)| t))
            .collect()
    }

    #[test]
    fn proviso_of_a_paragraph_is_found() {
        let doc = apply(
            &[
                "X条例",
                "第４条　市長は、許可する。ただし、第19条第１項第３号の場合は、この限りでない。",
            ],
            &[
                "X条例（平成17年X市条例第１号）の一部を次のように改正する。",
                "第４条ただし書中「第１９条第１項第３号」を「第１９条第３号」に改める。",
            ],
        );
        assert!(texts(&doc)
            .iter()
            .any(|t| t.contains("第１９条第３号の場合")));
    }

    #[test]
    fn caption_of_a_supplementary_paragraph_is_amended() {
        let doc = apply(
            &[
                "X条例",
                "第１条　この条例は、Xについて定める。",
                "附　則",
                "この条例は、平成17年１月１日から施行する。",
                "（附則第15条第33項の特例）",
                "２　附則第15条第33項の規定は、なお効力を有する。",
            ],
            &[
                "X条例（平成17年X市条例第１号）の一部を次のように改正する。",
                "附則第２項（見出しを含む。）中「附則第１５条第３３項」を「附則第１５条第３２項」に改める。",
            ],
        );
        let suppl = format!("{:?}", doc.suppl_provisions);
        assert!(suppl.contains("附則第１５条第３２項の特例"), "{suppl}");
        assert!(!suppl.contains("第３３項"), "{suppl}");
    }

    #[test]
    fn appendix_title_is_found_by_its_law_style_number() {
        let doc = document(&[
            Block::Paragraph("X条例".into()),
            Block::Paragraph("第１条　手数料は、別表第１のとおりとする。".into()),
            Block::Paragraph("別表第１（第１条関係）".into()),
            Block::Table(vec![
                vec!["区分".into(), "金額".into()],
                vec!["証明".into(), "300円".into()],
            ]),
        ])
        .unwrap();
        let lines: Vec<String> = [
            "X条例（平成17年X市条例第１号）の一部を次のように改正する。",
            "別表第１中「３００円」を「４００円」に改める。",
        ]
        .iter()
        .map(|l| canonical(l))
        .collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let units = crate::parse_units(&to_law_style(&lines)).unwrap();
        let applied = crate::apply_unit(&doc, &units[0], "applied").unwrap();
        assert!(applied.appendices[0].text().contains("４００円"));
    }

    #[test]
    fn positions_become_kanji_but_quoted_phrases_stay() {
        let s = to_law_style(&[
            "伊勢崎市市税条例（平成１７年伊勢崎市条例第７５号）の一部を次のように改正する。",
            "第34条の７第２項中「附則第５条の６第２項」を「附則第５条の６第３項」に改める。",
        ]);
        assert_eq!(
            s,
            "第一条　伊勢崎市市税条例（平成十七年伊勢崎市条例第七十五号）の一部を次のように改正する。\n\
             第三十四条の七第二項中「附則第５条の６第２項」を「附則第５条の６第３項」に改める。"
        );
    }

    #[test]
    fn an_added_article_ending_like_an_instruction_is_content() {
        let s = to_law_style(&[
            "X条例（平成１７年X市条例第１号）の一部を次のように改正する。",
            "第２０条の３を次のように改める。",
            "第２０条の３　育児休業法第１９条第２項の条例で定める期間は、１年とする。",
        ]);
        assert_eq!(
            s.lines().nth(2),
            Some("第二十条の三　育児休業法第１９条第２項の条例で定める期間は、１年とする。")
        );
    }

    #[test]
    fn added_provisions_keep_their_text_and_get_law_style_labels() {
        let s = to_law_style(&[
            "第２条　X条例（平成１７年X市条例第１号）の一部を次のように改正する。",
            "第５条の次に次の１条を加える。",
            "（特例）",
            "第５条の２　市長は、100分の３を減ずることができる。",
            "２　前項の規定は、次に掲げる場合に適用する。",
            "(1)　災害の場合",
            "⑵　その他市長が定める場合",
        ]);
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[1], "第五条の次に次の一条を加える。");
        assert_eq!(
            lines[3],
            "第五条の二　市長は、100分の３を減ずることができる。"
        );
        assert_eq!(lines[4], "２　前項の規定は、次に掲げる場合に適用する。");
        assert_eq!(lines[5], "一　災害の場合");
        assert_eq!(lines[6], "二　その他市長が定める場合");
    }
}
