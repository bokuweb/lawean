//! `bench_apply export`: ベンチで e-Gov の版と一致した改正単位を、（改正前の条文, 改正後の条文, 改め文）の組として JSONL に書き出す。
//! 改め文を生成する側（差分 → 改め文）のデータセットにする。
//!
//! 書き出す前に、いまのコードで当て直して確かめる:
//! - `match`: 当てた結果が e-Gov の改正後の版と全部一致する
//! - `mismatch_other`: この単位が変えた条は e-Gov の改正後の版と一致し、違いはこの単位の触らない条（同じ版に入った別の改正の分）
//!
//! 条文は、この単位が変えた条（本則の条、原始附則の条、目次、別表）だけを、改正前の版と改正後の版から出す。
//! 形は構造付き plain text（`{"type": "paragraph", "text": …}` と `{"type": "table", "rows": [[…]]}` の列）
use super::{load_rev, snapshot, take_flag, units_of, Snapshot};
use lawean_source::xml::{Element, Node};
use lawean_source::{
    Article, ArticleChild, Item, ItemBody, ItemChild, LegalDocument, Paragraph, ParagraphChild,
    Provision, SupplChild,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

/// XML の字下げ（改行と空白だけのテキスト）
fn indent_only(t: &str) -> bool {
    t.contains('\n') && t.trim().is_empty()
}

/// Ruby の読み（Rt）を落とした本文
fn el_text(e: &Element) -> String {
    let mut s = String::new();
    for c in &e.children {
        match c {
            Node::Text(t) if indent_only(t) => {}
            Node::Text(t) => s.push_str(t),
            Node::Element(x) if x.name == "Rt" => {}
            Node::Element(x) => s.push_str(&el_text(x)),
        }
    }
    s
}

fn elems(e: &Element) -> impl Iterator<Item = &Element> {
    e.children.iter().filter_map(|c| match c {
        Node::Element(x) => Some(x),
        Node::Text(_) => None,
    })
}

fn para(out: &mut Vec<Value>, text: String) {
    para_line(out, Line::text(text));
}

/// 段落の本文。Ruby は読みと一緒に持つ（「禁錮（こ）」を「禁錮」にする改正は、読みが無いと旧新が同じになる）
#[derive(Default)]
struct Line(Vec<Seg>);

enum Seg {
    Text(String),
    Ruby(String, String),
}

impl Line {
    fn text(t: impl Into<String>) -> Line {
        Line(vec![Seg::Text(t.into())])
    }

    fn is_empty(&self) -> bool {
        self.0.iter().all(|s| match s {
            Seg::Text(t) => t.trim().is_empty(),
            Seg::Ruby(..) => false,
        })
    }

    /// 空でない部分を全角の空白でつなぐ（「第二条」+「　」+ 本文）
    fn join(parts: Vec<Line>) -> Line {
        let mut out = Line::default();
        for p in parts.into_iter().filter(|p| !p.is_empty()) {
            if !out.0.is_empty() {
                out.0.push(Seg::Text("\u{3000}".into()));
            }
            out.0.extend(p.0);
        }
        out
    }
}

fn inline_line(inl: &[lawean_source::Inline]) -> Line {
    let mut line = Line::default();
    for i in inl {
        match i {
            lawean_source::Inline::Text(t) => line.0.push(Seg::Text(t.clone())),
            lawean_source::Inline::Raw(e) if e.name == "Ruby" => {
                let reading: String = elems(e).filter(|x| x.name == "Rt").map(el_text).collect();
                line.0.push(Seg::Ruby(el_text(e), reading));
            }
            lawean_source::Inline::Raw(e) => line.0.push(Seg::Text(el_text(e))),
        }
    }
    line
}

fn para_line(out: &mut Vec<Value>, line: Line) {
    if line.is_empty() {
        return;
    }
    if !line.0.iter().any(|s| matches!(s, Seg::Ruby(..))) {
        let t: String = line
            .0
            .iter()
            .map(|s| match s {
                Seg::Text(t) => t.as_str(),
                Seg::Ruby(..) => "",
            })
            .collect();
        out.push(json!({"type": "paragraph", "text": t.trim()}));
        return;
    }
    let n = line.0.len();
    let content: Vec<Value> = line
        .0
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            Seg::Text(t) => {
                let t = if i == 0 {
                    t.trim_start().to_string()
                } else {
                    t
                };
                let t = if i + 1 == n {
                    t.trim_end().to_string()
                } else {
                    t
                };
                (!t.is_empty()).then(|| json!({"type": "text", "text": t}))
            }
            Seg::Ruby(base, reading) => {
                Some(json!({"type": "ruby", "baseText": base, "reading": reading}))
            }
        })
        .collect();
    out.push(json!({"type": "paragraph", "content": content}));
}

fn table(e: &Element, out: &mut Vec<Value>) {
    let rows: Vec<Vec<String>> = elems(e)
        .filter(|r| r.name == "TableRow" || r.name == "TableHeaderRow")
        .map(|r| elems(r).map(|c| el_text(c).trim().to_string()).collect())
        .collect();
    if !rows.is_empty() {
        out.push(json!({"type": "table", "rows": rows}));
    }
}

fn has_table(e: &Element) -> bool {
    elems(e).any(|c| c.name == "Table" || c.name == "TableStruct" || has_table(c))
}

/// 表を含む要素は表を行と升目のまま、ほかは本文を 1 段落に
fn raw(e: &Element, out: &mut Vec<Value>) {
    match e.name.as_str() {
        "Table" => table(e, out),
        _ if has_table(e) => {
            // 題名の類（「別表第一（第三条関係）」）は 1 行にまとめる
            let mut head = String::new();
            for c in elems(e) {
                if c.name.ends_with("Title")
                    || c.name.ends_with("Label")
                    || c.name == "RelatedArticleNum"
                {
                    head.push_str(&el_text(c));
                } else {
                    para(out, std::mem::take(&mut head));
                    raw(c, out);
                }
            }
            para(out, head);
        }
        _ => para(out, el_text(e)),
    }
}

fn sentences(ss: &[lawean_source::Sentence]) -> Line {
    let mut line = Line::default();
    for s in ss {
        line.0.extend(inline_line(&s.text).0);
    }
    line
}

fn opt_line(inl: &Option<Vec<lawean_source::Inline>>) -> Line {
    inl.as_deref().map(inline_line).unwrap_or_default()
}

fn item(it: &Item, out: &mut Vec<Value>) {
    let body = match &it.body {
        ItemBody::Sentences(ss) => sentences(ss),
        ItemBody::Columns(cs) => Line::join(cs.iter().map(|c| sentences(&c.sentences)).collect()),
        ItemBody::Mixed(e) => Line::text(el_text(e)),
        ItemBody::None => Line::default(),
    };
    para_line(out, Line::join(vec![opt_line(&it.title), body]));
    for c in &it.children {
        match c {
            ItemChild::Subitem(s) => item(s, out),
            ItemChild::Raw(e) => raw(e, out),
        }
    }
}

fn paragraph(p: &Paragraph, head: Option<Line>, out: &mut Vec<Value>) {
    para_line(out, opt_line(&p.caption));
    let num = head.unwrap_or_else(|| opt_line(&p.num_text));
    para_line(out, Line::join(vec![num, sentences(&p.sentences)]));
    for c in &p.children {
        match c {
            ParagraphChild::Item(it) => item(it, out),
            ParagraphChild::Raw(e) => raw(e, out),
        }
    }
}

fn article(a: &Article, out: &mut Vec<Value>) {
    para_line(out, opt_line(&a.caption));
    let mut first = true;
    for c in &a.children {
        match c {
            ArticleChild::Paragraph(p) => {
                paragraph(p, first.then(|| opt_line(&a.title)), out);
                first = false;
            }
            ArticleChild::Raw(e) => raw(e, out),
        }
    }
    if first {
        para_line(out, opt_line(&a.title));
    }
}

/// 読み（Ruby）を「本体《読み》」で含めた本文（見出しの読みだけの改正も比べる）
fn with_reading(inl: &[lawean_source::Inline]) -> String {
    inline_line(inl)
        .0
        .into_iter()
        .map(|s| match s {
            Seg::Text(t) => t,
            Seg::Ruby(base, reading) => format!("{base}《{reading}》"),
        })
        .collect()
}

/// 条の見出しと、その条から始まる編・章・節の題名（`snapshot` は本文だけを比べるので、見出しの改正は別に見る）
fn headings(doc: &LegalDocument) -> BTreeMap<String, String> {
    fn go(
        ps: &[Provision],
        prefix: &str,
        pending: &mut String,
        out: &mut BTreeMap<String, String>,
    ) {
        for p in ps {
            match p {
                Provision::Container(c) => {
                    pending.push_str(&c.title.as_deref().map(with_reading).unwrap_or_default());
                    pending.push('\n');
                    go(&c.children, prefix, pending, out);
                }
                Provision::Article(a) => {
                    let mut h = std::mem::take(pending);
                    h.push_str(&a.caption.as_deref().map(with_reading).unwrap_or_default());
                    out.insert(format!("{prefix}{}", a.num.to_num_string()), h);
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeMap::new();
    go(&doc.main_provision, "", &mut String::new(), &mut out);
    if let Some(sp) = doc
        .suppl_provisions
        .iter()
        .find(|s| s.amend_law_num.is_none())
    {
        let ps: Vec<Provision> = sp
            .children
            .iter()
            .filter_map(|c| match c {
                SupplChild::Provision(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        go(&ps, "附則", &mut String::new(), &mut out);
    }
    out
}

/// 本文の Ruby（読み）: `snapshot` は読みを落とすので、読みだけの改正（「禁錮（こ）」→「禁錮」）は別に見る
fn rubies(doc: &LegalDocument) -> BTreeMap<String, Vec<String>> {
    fn collect(e: &Element, out: &mut Vec<String>) {
        if e.name == "Ruby" {
            out.push(e.text());
        }
        for c in elems(e) {
            collect(c, out);
        }
    }
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let emitted = lawean_source::emit_law(doc);
    fn walk(e: &Element, out: &mut BTreeMap<String, Vec<String>>, in_suppl: bool, original: bool) {
        for c in elems(e) {
            match c.name.as_str() {
                "SupplProvision" => {
                    let orig = !c.attrs.iter().any(|(k, _)| k == "AmendLawNum");
                    walk(c, out, true, orig);
                }
                "Article" if !in_suppl || original => {
                    let num = c
                        .attrs
                        .iter()
                        .find(|(k, _)| k == "Num")
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default();
                    let k = format!("{}{}", if in_suppl { "附則" } else { "" }, num);
                    let mut v = Vec::new();
                    collect(c, &mut v);
                    if !v.is_empty() {
                        out.insert(k, v);
                    }
                }
                "Article" => {}
                _ => walk(c, out, in_suppl, original),
            }
        }
    }
    walk(&emitted, &mut out, false, false);
    out
}

fn toc(e: &Element, out: &mut Vec<Value>) {
    for c in elems(e).filter(|c| c.name.starts_with("TOC")) {
        let line: String = c
            .children
            .iter()
            .map(|x| match x {
                Node::Text(t) if indent_only(t) => String::new(),
                Node::Text(t) => t.clone(),
                Node::Element(x) if !x.name.starts_with("TOC") => el_text(x),
                Node::Element(_) => String::new(),
            })
            .collect();
        para(out, line);
        toc(c, out);
    }
}

fn wanted(p: &Provision, want: &BTreeSet<String>, main_key: &str) -> bool {
    match p {
        Provision::Container(c) => c.children.iter().any(|x| wanted(x, want, main_key)),
        Provision::Article(a) => want.contains(&a.num.to_num_string()),
        Provision::Paragraph(_) => want.contains(main_key),
        Provision::Raw(_) => false,
    }
}

/// 変わった条を、その条を含む編・章・節の題名（文脈）と一緒に出す
fn provisions(ps: &[Provision], want: &BTreeSet<String>, main_key: &str, out: &mut Vec<Value>) {
    for p in ps {
        if !wanted(p, want, main_key) {
            continue;
        }
        match p {
            Provision::Container(c) => {
                para_line(out, opt_line(&c.title));
                provisions(&c.children, want, main_key, out);
            }
            Provision::Article(a) => article(a, out),
            Provision::Paragraph(pa) => paragraph(pa, None, out),
            Provision::Raw(_) => {}
        }
    }
}

/// 別表の題名（「別表第一（第三条関係）」）
fn appendix_title(e: &Element) -> String {
    elems(e)
        .filter(|c| c.name.ends_with("Title") || c.name == "RelatedArticleNum")
        .map(el_text)
        .collect::<String>()
        .trim()
        .to_string()
}

fn appendix_key(e: &Element) -> String {
    let t: String = e.text().chars().filter(|c| !c.is_whitespace()).collect();
    format!("別表:{}", t.chars().take(12).collect::<String>())
}

/// スナップショットのキー（「3」「3_2の表」「附則2」「TOC」「別表:…」「main」）に当たる部分を、文書の順に出す。
/// 先頭は法令の題名（totoro の plain text 入力と同じ）
fn render(doc: &LegalDocument, keys: &BTreeSet<String>) -> Vec<Value> {
    let mut out = Vec::new();
    para_line(
        &mut out,
        doc.title
            .as_ref()
            .map(|t| inline_line(&t.text))
            .unwrap_or_default(),
    );
    if keys.contains("TOC") {
        if let Some(t) = &doc.toc {
            toc(t, &mut out);
        }
    }
    let main: BTreeSet<String> = keys
        .iter()
        .filter(|k| !k.starts_with("附則"))
        .map(|k| k.trim_end_matches("の表").to_string())
        .collect();
    provisions(&doc.main_provision, &main, "main", &mut out);
    let suppl: BTreeSet<String> = keys
        .iter()
        .filter_map(|k| k.strip_prefix("附則"))
        .map(|k| k.to_string())
        .collect();
    if !suppl.is_empty() {
        if let Some(sp) = doc
            .suppl_provisions
            .iter()
            .find(|s| s.amend_law_num.is_none())
        {
            let mut body = Vec::new();
            for c in &sp.children {
                match c {
                    SupplChild::Provision(p) => {
                        provisions(std::slice::from_ref(p), &suppl, "main", &mut body)
                    }
                    SupplChild::Paragraph(p) if suppl.contains("main") => {
                        paragraph(p, None, &mut body)
                    }
                    _ => {}
                }
            }
            if !body.is_empty() {
                para(&mut out, "附　則".into());
                out.extend(body);
            }
        }
    }
    for ap in &doc.appendices {
        if keys.contains(&appendix_key(ap)) {
            raw(ap, &mut out);
        }
    }
    out
}

/// 人が読む変わった所の名前（「第三条」「附則第二条」「目次」「別表第一…」）
fn label(k: &str, appendix: &BTreeMap<String, String>) -> String {
    use lawean_resolve::numeral::to_kanji;
    fn art(s: &str) -> String {
        if let Some((a, b)) = s.split_once(':') {
            return format!("{}から{}まで", art(a), art(b));
        }
        let mut it = s.split('_');
        let base = it.next().and_then(|b| b.parse::<u32>().ok());
        match base {
            Some(b) => {
                let mut l = format!("第{}条", to_kanji(b));
                for br in it {
                    if let Ok(n) = br.parse::<u32>() {
                        l.push_str(&format!("の{}", to_kanji(n)));
                    }
                }
                l
            }
            None => s.to_string(),
        }
    }
    if k == "TOC" {
        "目次".into()
    } else if k == "main" {
        "本則".into()
    } else if let Some(t) = k.strip_prefix("別表:") {
        appendix
            .get(k)
            .filter(|x| !x.is_empty())
            .cloned()
            .unwrap_or_else(|| t.to_string())
    } else if let Some(s) = k.strip_prefix("附則") {
        format!("附則{}", art(s.trim_end_matches("の表")))
    } else {
        art(k.trim_end_matches("の表"))
    }
}

/// 改め文の原文: 塊（「第N条　X法（…）の一部を次のように改正する。」から次の単位まで）。
/// 列挙形（「次に掲げる法律の規定中「A」を「B」に改める。」+ 号）は、柱書きとその法律の号だけ
fn aratamebun(block: &str, title: &str, n_units: usize) -> Option<Vec<String>> {
    let mut lines: Vec<String> = block.lines().map(|l| l.trim_end().to_string()).collect();
    // 塊の終わりの、次の条の見出し（「（X法の一部改正に伴う経過措置）」）と整備法の章名（「第三章　厚生労働省関係」）は
    // 改め文ではない（加える章の章名なら、後に条が続く）
    let heading =
        regex::Regex::new(r"^第[一二三四五六七八九十百]+(?:編|章|節|款|目)\u{3000}[^。]+$")
            .unwrap();
    loop {
        let n = lines.len();
        if n < 2 {
            break;
        }
        let t = lines[n - 1].trim_start_matches(['\u{3000}', ' ']);
        // 「…の見出しを次のように改める。」+「（新しい見出し）」は改め文
        let prev = &lines[n - 2];
        let new_caption = prev.contains("見出し")
            && (prev.ends_with("次のように改める。") || prev.ends_with("付する。"));
        let caption = t.starts_with('（') && t.ends_with('）') && !new_caption;
        if t.is_empty() || caption || heading.is_match(t) {
            lines.pop();
        } else {
            break;
        }
    }
    let head = lines.first()?;
    if !head.contains("次に掲げる法律の規定中") {
        return (n_units == 1).then_some(lines);
    }
    let item = regex::Regex::new(&format!(
        r"^[\u{{3000}} ]*[一二三四五六七八九十百]+\u{{3000}}{}",
        regex::escape(title)
    ))
    .ok()?;
    let hits: Vec<&String> = lines[1..].iter().filter(|l| item.is_match(l)).collect();
    (hits.len() == 1).then(|| vec![head.clone(), hits[0].clone()])
}

/// 変わった所のキー。原始附則の中の「附則TOC」は本則の目次と同じなので外す
fn changed_keys(a: &Snapshot, b: &Snapshot) -> BTreeSet<String> {
    a.keys()
        .chain(b.keys())
        .filter(|k| *k != "附則TOC" && a.get(*k) != b.get(*k))
        .cloned()
        .collect()
}

fn changed_in<V: PartialEq>(a: &BTreeMap<String, V>, b: &BTreeMap<String, V>) -> BTreeSet<String> {
    a.keys()
        .chain(b.keys())
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect()
}

/// 文書の順（目次、本則の条、附則の条、別表）に並べるための鍵
fn doc_order(k: &str) -> (u8, Vec<u32>, String) {
    let nums = |s: &str| -> Vec<u32> {
        s.trim_end_matches("の表")
            .split([':', '_'])
            .filter_map(|x| x.parse().ok())
            .collect()
    };
    if k == "TOC" {
        (0, vec![], String::new())
    } else if let Some(t) = k.strip_prefix("別表:") {
        (3, vec![], t.to_string())
    } else if let Some(s) = k.strip_prefix("附則") {
        (2, nums(s), String::new())
    } else {
        (1, nums(k), String::new())
    }
}

/// 結果の行: (塊, 単位, 結果, 直前の版, 直後の版)
type Row = (usize, usize, String, String, String);

pub fn export(mut args: Vec<String>) {
    let result = take_flag(&mut args, "--result").expect("--result result.tsv");
    let out = take_flag(&mut args, "--out").expect("--out cases.jsonl");
    let index = take_flag(&mut args, "--index");
    let egov = take_flag(&mut args, "--egov").unwrap_or_else(|| {
        format!(
            "{}/.cache/lawean/egov/rev",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let egov = Path::new(&egov);
    // ページ → (houritsu | housei, 改正法の題名)
    let mut titles: BTreeMap<String, (String, String)> = BTreeMap::new();
    if let Some(ix) = index {
        for l in std::fs::read_to_string(ix).unwrap().lines() {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() >= 4 {
                titles.insert(c[2].into(), (c[1].into(), c[3].into()));
            }
        }
    }
    // (page, block, unit) → (結果, 直前の版, 直後の版)
    let mut rows: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for l in std::fs::read_to_string(&result).unwrap().lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() >= 8 && (c[4] == "match" || c[4] == "mismatch_other") {
            rows.entry(c[0].into()).or_default().push((
                c[1].parse().unwrap(),
                c[2].parse().unwrap(),
                c[4].into(),
                c[6].into(),
                c[7].into(),
            ));
        }
    }
    let mut w = std::io::BufWriter::new(std::fs::File::create(out).unwrap());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (page, want) in rows {
        let Ok(text) = std::fs::read_to_string(Path::new(&args[0]).join(format!("{page}.txt")))
        else {
            continue;
        };
        let units = units_of(&text);
        let per_block = |b: usize| units.iter().filter(|u| u.0 == b).count();
        for (bi, ui, status, prev, after) in want {
            let Some((_, _, block, u)) = units.iter().find(|x| x.0 == bi && x.1 == ui) else {
                *counts.entry("skip:no_unit".into()).or_default() += 1;
                continue;
            };
            let (Some(before), Some(expected)) = (load_rev(egov, &prev), load_rev(egov, &after))
            else {
                *counts.entry("skip:no_rev".into()).or_default() += 1;
                continue;
            };
            let Ok(Ok(applied)) =
                std::panic::catch_unwind(|| lawean_amend::apply_unit(&before, u, "applied"))
            else {
                *counts.entry("skip:apply".into()).or_default() += 1;
                continue;
            };
            let (sb, sa, se) = (snapshot(&before), snapshot(&applied), snapshot(&expected));
            let (hb, ha, he) = (headings(&before), headings(&applied), headings(&expected));
            let (rb, re) = (rubies(&before), rubies(&expected));
            // この単位が変えた所（本文、見出し、読み）。どれも e-Gov の改正後の版と一致していなければ出さない
            let mut keys = changed_keys(&sb, &sa);
            keys.extend(changed_in(&hb, &ha));
            let heading_ok = changed_in(&hb, &ha).iter().all(|k| ha.get(k) == he.get(k));
            // 読みは lawean が当てない（読みを落として比べる）ので、e-Gov の版で読みが変わった条を足す
            keys.extend(
                changed_in(&rb, &re)
                    .into_iter()
                    .filter(|k| sa.contains_key(k) || se.contains_key(k)),
            );
            let ok = !keys.is_empty()
                && keys.iter().all(|k| sa.get(k) == se.get(k))
                && heading_ok
                && (status != "match" || (sa == se && ha == he));
            if !ok {
                if std::env::var_os("BENCH_DEBUG").is_some() {
                    for k in changed_in(&ha, &he) {
                        // 見出し: 直前の版, 当てた結果, 直後の版
                        eprintln!(
                            "{page}_{bi}_{ui}\t{k}\t{:?}\t{:?}\t{:?}",
                            hb.get(&k),
                            ha.get(&k),
                            he.get(&k)
                        );
                    }
                }
                *counts.entry(format!("skip:reverify_{status}")).or_default() += 1;
                continue;
            }
            let Some(lines) = aratamebun(block, &u.target_title, per_block(bi)) else {
                *counts.entry("skip:aratamebun".into()).or_default() += 1;
                continue;
            };
            let mut ordered: Vec<&String> = keys.iter().collect();
            ordered.sort_by_key(|k| doc_order(k));
            let appendix: BTreeMap<String, String> = before
                .appendices
                .iter()
                .chain(&expected.appendices)
                .map(|a| (appendix_key(a), appendix_title(a)))
                .collect();
            let mut changed: Vec<String> = Vec::new();
            for k in ordered {
                let l = label(k, &appendix);
                if !changed.contains(&l) {
                    changed.push(l);
                }
            }
            let (kind, amending_title) = titles.get(&page).cloned().unwrap_or_default();
            let law_id = prev.split('_').next().unwrap_or_default().to_string();
            let case = json!({
                "case_id": format!("shugiin_{page}_{bi}_{ui}"),
                "verification": status,
                "source": {
                    "shugiin_page": page,
                    "shugiin_url": if kind.is_empty() { Value::Null } else {
                        json!(format!("https://www.shugiin.go.jp/internet/itdb_housei.nsf/html/{kind}/{page}.htm"))
                    },
                    "amending_law_num": super::amending_law_num(&page),
                    "amending_law_title": amending_title,
                    "amending_article": u.article_of_amending_law,
                    "target_law_title": u.target_title,
                    "target_law_num": expected.law_num,
                    "egov_law_id": law_id,
                    "egov_revision_before": prev,
                    "egov_revision_after": after,
                },
                "changed": changed,
                "aratamebun": lines,
                "old": render(&before, &keys),
                "new": render(&expected, &keys),
            });
            serde_json::to_writer(&mut w, &case).unwrap();
            writeln!(w).unwrap();
            *counts.entry(format!("ok:{status}")).or_default() += 1;
        }
    }
    for (k, n) in counts {
        println!("{k}\t{n}");
    }
}
