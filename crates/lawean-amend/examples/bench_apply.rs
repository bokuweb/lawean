//! 衆議院の制定法律の改め文を、e-Gov の改正前の版に当てて改正後の版と突き合わせる（当てる側の精度の測定）。
//!
//! 1. 改正単位の一覧を出す（改正法と被改正法の法令番号つき）:
//!    cargo run --release -p lawean-amend --example bench_apply -- list <txt のディレクトリ> [--since 20170401] --out units.tsv
//! 2. `tools/fetch_egov_revisions.py units.tsv pairs.tsv` で、単位ごとに e-Gov の直前の版と直後の版を決めて取ってくる
//! 3. 当てて突き合わせる:
//!    cargo run --release -p lawean-amend --example bench_apply -- run <txt のディレクトリ> --pairs pairs.tsv [--out result.tsv]
//!
//! 突き合わせは本則と原始附則（`snapshot_main`: 条・項ごとの本文。目次を含む）、項の中の表、別表の本文。
//! e-Gov の直前の版と直後の版で比べる部分が変わっていない単位は `match_trivial`（一致しても当てたことの確かめにならない）。
//! 違いがこの単位の触らない条にだけあり、そこが e-Gov の版で変わっているものは `mismatch_other`（同じ版に入った別の改正の分）。
//! 段階施行（附則で文ごとに施行日が違う）の単位は、候補の版ごとにその版に近づく文だけを当て、一致すれば `match_staged`。
//!
//! 同じ単位を identity patch（`ident::bind` → Lean の `Ident.applyUnit` の写しの `ident::apply_unit`）でも当て、
//! 本則の描画が e-Gov の版と一致するかを `ident` の列に出す（証明した意味論が実データで文書への適用と揃うか）
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[path = "corpus/blocks.rs"]
mod blocks;
use blocks::blocks;

/// ページの 14 桁（国会 3 桁 + 公布日 8 桁 + 番号 3 桁）→ 法令番号（「令和三年法律第三十七号」）。
/// e-Gov の改正履歴とは法令番号で照らす（法令 ID は閣法・衆法・参法で形が違う）
fn amending_law_num(page: &str) -> Option<String> {
    use lawean_resolve::numeral::to_kanji;
    let date: u32 = page.get(3..11)?.parse().ok()?;
    let num: u32 = page.get(11..14)?.parse().ok()?;
    let year = date / 10000;
    let (era, y) = if date >= 20190501 {
        ("令和", year - 2018)
    } else if date >= 19890108 {
        ("平成", year - 1988)
    } else if date >= 19261225 {
        ("昭和", year - 1925)
    } else {
        return None;
    };
    let y = if y == 1 {
        "元".to_string()
    } else {
        to_kanji(y)
    };
    Some(format!("{era}{y}年法律第{}号", to_kanji(num)))
}

/// 被改正法の法令番号（「平成三年法律第九十号」）: 題名の直後の括弧書き。塊の中に無ければページ全体から
fn target_law_num(block: &str, page: &str, title: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(
        r"{}（((?:明治|大正|昭和|平成|令和)[^（）]*?(?:法律|政令|勅令)第[^（）]*?号)）",
        regex::escape(title)
    ))
    .ok()?;
    re.captures(block)
        .or_else(|| re.captures(page))
        .map(|c| c[1].to_string())
}

fn txt_files(dir: &str, since: Option<&str>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("txt dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .filter(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            since.is_none_or(|d| name.get(3..11).is_some_and(|x| x >= d))
        })
        .collect();
    v.sort();
    v
}

/// ページの改正単位を（塊の番号, 塊の中の番号, 塊, 単位）で並べる
fn units_of(text: &str) -> Vec<(usize, usize, String, lawean_amend::AmendUnit)> {
    let mut out = Vec::new();
    for (bi, b) in blocks(text).into_iter().enumerate() {
        let Ok(Ok(us)) = std::panic::catch_unwind(|| lawean_amend::parse_units(&b)) else {
            continue;
        };
        for (ui, u) in us.into_iter().enumerate() {
            out.push((bi, ui, b.clone(), u));
        }
    }
    out
}

fn take_flag(args: &mut Vec<String>, flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    let v = args[i + 1].clone();
    args.drain(i..=i + 1);
    Some(v)
}

fn list(mut args: Vec<String>) {
    let out = take_flag(&mut args, "--out").expect("--out");
    let since = take_flag(&mut args, "--since");
    let mut w = std::fs::File::create(out).unwrap();
    writeln!(
        w,
        "page\tblock\tunit\tamending_law_num\tarticle\ttitle\ttarget_law_num"
    )
    .unwrap();
    for f in txt_files(&args[0], since.as_deref()) {
        let page = f.file_stem().unwrap().to_string_lossy().to_string();
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Some(law_id) = amending_law_num(&page) else {
            continue;
        };
        for (bi, ui, b, u) in units_of(&text) {
            let num = target_law_num(&b, &text, &u.target_title).unwrap_or_default();
            writeln!(
                w,
                "{page}\t{bi}\t{ui}\t{law_id}\t{}\t{}\t{num}",
                u.article_of_amending_law, u.target_title
            )
            .unwrap();
        }
    }
}

type Snapshot = BTreeMap<String, Vec<(u32, String)>>;

/// 字形の揃え: e-Gov は JIS X 0213:2004 の字形（常用漢字表 2010 の「塡」「剝」「頰」…）で持つ。
/// 衆議院のページは古い字形のことがあるので、比べる前に揃える
fn fold_glyphs(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '填' => '塡',
            '剥' => '剝',
            '頬' => '頰',
            '呑' => '吞',
            '屏' => '屛',
            '并' => '幷',
            '醤' => '醬',
            '蝉' => '蟬',
            '繋' => '繫',
            '鴎' => '鷗',
            '掴' => '摑',
            '噛' => '嚙',
            // 衆議院の「(2)」は e-Gov の「（２）」
            '(' => '（',
            ')' => '）',
            '0'..='9' => char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap_or(c),
            c => c,
        })
        .collect()
}

/// 本則と原始附則（キーに「附則」を冠する）の条・項ごとの本文
fn snapshot(doc: &lawean_source::LegalDocument) -> Snapshot {
    raw_snapshot(doc)
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                v.into_iter().map(|(n, t)| (n, fold_glyphs(&t))).collect(),
            )
        })
        .collect()
}

fn raw_snapshot(doc: &lawean_source::LegalDocument) -> Snapshot {
    let mut out = lawean_amend::snapshot_main(doc);
    if let Some(sp) = doc
        .suppl_provisions
        .iter()
        .find(|s| s.amend_law_num.is_none())
    {
        let mut d = doc.clone();
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
        for (k, v) in lawean_amend::snapshot_main(&d) {
            out.insert(format!("附則{k}"), v);
        }
    }
    // 項の中の表（読替え表・税率表など）と別表: `snapshot_main` の本文には入らないので、ここで足す
    let ws = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    fn tables(ps: &[lawean_source::Provision], ws: &dyn Fn(&str) -> String, out: &mut Snapshot) {
        for p in ps {
            match p {
                lawean_source::Provision::Container(c) => tables(&c.children, ws, out),
                lawean_source::Provision::Article(a) => {
                    let mut v = Vec::new();
                    let mut k = 0;
                    for c in &a.children {
                        if let lawean_source::ArticleChild::Paragraph(p) = c {
                            k += 1;
                            for pc in &p.children {
                                if let lawean_source::ParagraphChild::Raw(e) = pc {
                                    v.push((k, ws(&e.text())));
                                }
                            }
                        }
                    }
                    if !v.is_empty() {
                        out.insert(format!("{}の表", a.num.to_num_string()), v);
                    }
                }
                _ => {}
            }
        }
    }
    tables(&doc.main_provision, &ws, &mut out);
    for ap in &doc.appendices {
        let text = ws(&ap.text());
        let title: String = text.chars().take(12).collect();
        out.insert(format!("別表:{title}"), vec![(0, text)]);
    }
    out
}

/// e-Gov の版（`fetch_egov_revisions.py` が保存した law_data_response の XML）
fn load_rev(dir: &Path, id: &str) -> Option<lawean_source::LegalDocument> {
    use std::io::Read;
    let xml = match std::fs::File::open(dir.join(format!("{id}.xml.gz"))) {
        Ok(f) => {
            let mut s = String::new();
            flate2::read::GzDecoder::new(f)
                .read_to_string(&mut s)
                .ok()?;
            s
        }
        Err(_) => std::fs::read_to_string(dir.join(format!("{id}.xml"))).ok()?,
    };
    lawean_source::parse_response(&xml).ok()
}

/// 1 つの候補（直前の版, 直後の版）に当てた結果
struct Outcome {
    result: String,
    detail: String,
    /// 違う行の数（mismatch のとき）
    diff: usize,
    prev: String,
    after: String,
    /// identity patch の経路の結果（ident_match / ident_mismatch / ident_bind_error / ident_none）
    ident: String,
}

impl Outcome {
    /// 候補の良さ（小さいほど良い）: 一致 < 比べる部分が変わらない一致 < 違いの少ない不一致 < 当てられない
    fn rank(&self) -> (u8, usize) {
        match self.result.as_str() {
            "match" => (0, 0),
            "match_trivial" => (1, 0),
            "match_staged" => (1, 0),
            "mismatch_other" => (2, self.diff),
            "mismatch" => (3, self.diff),
            "unsupported" => (4, 0),
            "skip:empty_rev" => (6, 0),
            _ => (5, 0),
        }
    }
}

/// 当てた結果と e-Gov の版の違いが、どれも「この単位が変えていない条」で「e-Gov の版では直前の版から変わった条」か
fn other_amendments_only(before: &Snapshot, applied: &Snapshot, want: &Snapshot) -> bool {
    let keys: std::collections::BTreeSet<&String> = applied
        .keys()
        .chain(want.keys())
        .chain(before.keys())
        .collect();
    let mut any = false;
    for k in keys {
        if applied.get(k) == want.get(k) {
            continue;
        }
        any = true;
        let touched = applied.get(k) != before.get(k);
        let changed_upstream = want.get(k) != before.get(k);
        if touched || !changed_upstream {
            return false;
        }
    }
    any
}

/// 最初に違う条・項と、違う字の前後（当てた結果 | e-Gov）
fn first_difference(got: &Snapshot, want: &Snapshot) -> String {
    let keys: std::collections::BTreeSet<&String> = got.keys().chain(want.keys()).collect();
    for k in keys {
        match (got.get(k), want.get(k)) {
            (Some(g), Some(w)) if g == w => continue,
            (Some(_), None) => return format!("{k}: 当てた結果にだけある"),
            (None, Some(_)) => return format!("{k}: e-Gov にだけある"),
            (Some(g), Some(w)) => {
                for i in 0..g.len().max(w.len()) {
                    match (g.get(i), w.get(i)) {
                        (Some(a), Some(b)) if a == b => continue,
                        (Some(a), Some(b)) => {
                            let (ac, bc): (Vec<char>, Vec<char>) =
                                (a.1.chars().collect(), b.1.chars().collect());
                            let at = ac.iter().zip(&bc).take_while(|(x, y)| x == y).count();
                            let from = at.saturating_sub(15);
                            let cut = |v: &[char]| -> String {
                                v[from.min(v.len())..(at + 25).min(v.len())]
                                    .iter()
                                    .collect()
                            };
                            return format!("{k} 第{}項: …{}… | …{}…", a.0, cut(&ac), cut(&bc));
                        }
                        (Some(a), None) => return format!("{k} 第{}項: 当てた結果にだけある", a.0),
                        (None, Some(b)) => return format!("{k} 第{}項: e-Gov にだけある", b.0),
                        (None, None) => {}
                    }
                }
            }
            (None, None) => {}
        }
    }
    String::new()
}

/// 違いの大きさ: （違う条・項の数, 違う条・項の本文の前後の一致の長さの和の符号を返したもの）
fn distance(got: &Snapshot, want: &Snapshot) -> (usize, i64) {
    let keys: std::collections::BTreeSet<&String> = got.keys().chain(want.keys()).collect();
    let (mut n, mut close) = (0usize, 0i64);
    for k in keys {
        let (g, w) = (got.get(k), want.get(k));
        if g == w {
            continue;
        }
        let (g, w) = (
            g.cloned().unwrap_or_default(),
            w.cloned().unwrap_or_default(),
        );
        for i in 0..g.len().max(w.len()) {
            let (a, b) = (g.get(i).map(|x| &x.1), w.get(i).map(|x| &x.1));
            if a == b {
                continue;
            }
            n += 1;
            if let (Some(a), Some(b)) = (a, b) {
                let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
                let pre = ac.iter().zip(&bc).take_while(|(x, y)| x == y).count();
                let suf = ac
                    .iter()
                    .rev()
                    .zip(bc.iter().rev())
                    .take_while(|(x, y)| x == y)
                    .count();
                close += (pre + suf).min(ac.len().min(bc.len())) as i64;
            }
        }
    }
    (n, -close)
}

/// 段階施行: 文を順に試し、その版に近づく文だけを当てる。最後に一致すれば、当てた文の添字
fn stage_match(
    before: &lawean_source::LegalDocument,
    expected: &lawean_source::LegalDocument,
    ins: &[&lawean_amend::Instruction],
    prev: &str,
    after: &str,
) -> Option<Vec<usize>> {
    if raw_snapshot(before).is_empty() || raw_snapshot(expected).is_empty() {
        return None;
    }
    let exp = snapshot(expected);
    let mut cur = before.clone();
    let mut d = distance(&snapshot(&cur), &exp);
    if d.0 == 0 {
        return None;
    }
    let mut used = Vec::new();
    for (j, one) in ins.iter().enumerate() {
        let unit = lawean_amend::AmendUnit {
            article_of_amending_law: String::new(),
            target_title: String::new(),
            instructions: vec![(*one).clone()],
        };
        let Ok(Ok(next)) =
            std::panic::catch_unwind(|| lawean_amend::apply_unit_unrefreshed(&cur, &unit))
        else {
            continue;
        };
        let nd = distance(&snapshot(&next), &exp);
        if nd < d {
            cur = next;
            d = nd;
            used.push(j);
        }
    }
    if std::env::var("BENCH_DEBUG").is_ok() {
        eprintln!(
            "  staged {prev} > {after}: used {}, left {} {}",
            used.len(),
            d.0,
            if d.0 > 0 {
                first_difference(&snapshot(&cur), &exp)
            } else {
                String::new()
            }
        );
    }
    (d.0 == 0 && !used.is_empty()).then_some(used)
}

fn try_pair(
    before: &lawean_source::LegalDocument,
    expected: &lawean_source::LegalDocument,
    u: &lawean_amend::AmendUnit,
    prev: &str,
    after: &str,
) -> Outcome {
    let mk = |result: &str, detail: String, diff: usize| Outcome {
        result: result.into(),
        detail,
        diff,
        prev: prev.into(),
        after: after.into(),
        ident: String::new(),
    };
    // e-Gov の古い版には本文の無いもの（MainProvision が空）がある
    if raw_snapshot(before).is_empty() || raw_snapshot(expected).is_empty() {
        return mk("skip:empty_rev", String::new(), 0);
    }
    match std::panic::catch_unwind(|| lawean_amend::apply_unit(before, u, "applied")) {
        Ok(Ok(applied)) => {
            let exp = snapshot(expected);
            let d = lawean_amend::diff_snapshots(&snapshot(&applied), &exp);
            let changed = !lawean_amend::diff_snapshots(&snapshot(before), &exp).is_empty();
            if d.is_empty() && !changed {
                mk("match_trivial", String::new(), 0)
            } else if d.is_empty() {
                mk("match", String::new(), 0)
            } else if other_amendments_only(&snapshot(before), &snapshot(&applied), &exp) {
                // 違いがこの単位の触らない条にだけあり、そこは e-Gov の版で直前の版から変わっている:
                // 同じ版に入った別の改正（同じ日の施行など）の分
                mk(
                    "mismatch_other",
                    first_difference(&snapshot(&applied), &exp),
                    d.len(),
                )
            } else {
                mk(
                    "mismatch",
                    format!(
                        "{} lines: {}",
                        d.len(),
                        first_difference(&snapshot(&applied), &exp)
                    ),
                    d.len(),
                )
            }
        }
        Ok(Err(lawean_amend::ApplyError::Unsupported(what))) => mk("unsupported", what, 0),
        Ok(Err(e)) => mk("apply_error", e.to_string(), 0),
        Err(_) => mk("panic", String::new(), 0),
    }
}

/// 理由の文の形（番号を N に、「」の中を … に）: 集計用
fn coarse(s: &str) -> String {
    let mut out = String::new();
    let (mut depth, mut last_n) = (0, false);
    for c in s.chars() {
        match c {
            '「' => {
                if depth == 0 {
                    out.push_str("「…");
                }
                depth += 1;
                continue;
            }
            '」' => {
                depth = (depth - 1).max(0);
                if depth == 0 {
                    out.push('」');
                }
                continue;
            }
            _ if depth > 0 => continue,
            _ if "〇一二三四五六七八九十百千0123456789０１２３４５６７８９".contains(c) =>
            {
                if !last_n {
                    out.push('N');
                }
                last_n = true;
                continue;
            }
            _ => out.push(c),
        }
        last_n = false;
    }
    out.chars().take(40).collect()
}

/// identity patch の経路: 発射台に束縛して `ident::apply_unit` で当て、本則の描画を e-Gov の版と比べる
fn ident_path(
    before: &lawean_source::LegalDocument,
    expected: &lawean_source::LegalDocument,
    u: &lawean_amend::AmendUnit,
) -> String {
    use lawean_amend::ident;
    let r = std::panic::catch_unwind(|| {
        let b = ident::bind(before, u, "bench").map_err(|e| e.to_string())?;
        let got = ident::apply_unit(&ident::from_document(before), &b.ops).ok_or("none")?;
        let (g, w) = (got.render(), ident::from_document(expected).render());
        if g != w && std::env::var("BENCH_DEBUG").is_ok() {
            if let Some(i) = (0..g.len().max(w.len())).find(|&i| g.get(i) != w.get(i)) {
                let cut = |x: Option<&(String, String)>| {
                    x.map(|(a, t)| format!("{a}:{}", t.chars().take(40).collect::<String>()))
                };
                eprintln!("ident diff #{i}: {:?} | {:?}", cut(g.get(i)), cut(w.get(i)));
            }
        }
        Ok::<bool, String>(g == w)
    });
    match r {
        Ok(Ok(true)) => "ident_match".into(),
        Ok(Ok(false)) => "ident_mismatch".into(),
        Ok(Err(e)) if e == "none" => "ident_none".into(),
        Ok(Err(e)) => {
            if std::env::var("BENCH_DEBUG").is_ok() {
                eprintln!("ident: {e}");
            }
            "ident_bind_error".into()
        }
        Err(_) => "ident_panic".into(),
    }
}

fn run(mut args: Vec<String>) {
    let pairs = take_flag(&mut args, "--pairs").expect("--pairs");
    let out = take_flag(&mut args, "--out");
    let egov = take_flag(&mut args, "--egov").unwrap_or_else(|| {
        format!(
            "{}/.cache/lawean/egov/rev",
            std::env::var("HOME").unwrap_or_default()
        )
    });
    let egov = Path::new(&egov);
    // (page, block, unit) → (状態, 候補の（直前の版, 直後の版）)
    type Cands = Vec<(String, String)>;
    let mut want: BTreeMap<(String, usize, usize), (String, Cands)> = BTreeMap::new();
    for l in std::fs::read_to_string(&pairs).unwrap().lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 5 {
            continue;
        }
        let cands = c[4]
            .split(';')
            .filter_map(|x| x.split_once('>'))
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        want.insert(
            (c[0].into(), c[1].parse().unwrap(), c[2].parse().unwrap()),
            (c[3].into(), cands),
        );
    }
    let pages: std::collections::BTreeSet<&str> = want.keys().map(|k| k.0.as_str()).collect();
    let mut w = out.map(|p| std::fs::File::create(p).unwrap());
    if let Some(w) = w.as_mut() {
        writeln!(
            w,
            "page\tblock\tunit\ttitle\tresult\tident\tprev\tafter\tdetail"
        )
        .unwrap();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unsupported: BTreeMap<String, usize> = BTreeMap::new();
    let mut docs: BTreeMap<String, Option<std::rc::Rc<lawean_source::LegalDocument>>> =
        BTreeMap::new();
    for page in pages {
        let f = Path::new(&args[0]).join(format!("{page}.txt"));
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        // 版を読み込んだもの（docs）は、同じページの中だけ持つ（大きい法令の版で溢れないように）
        #[allow(clippy::type_complexity)]
        let mut results: Vec<(
            usize,
            usize,
            lawean_amend::AmendUnit,
            Outcome,
            Vec<(String, String)>,
        )> = Vec::new();
        for (bi, ui, _, u) in units_of(&text) {
            let Some((status, cands)) = want.get(&(page.to_string(), bi, ui)) else {
                continue;
            };
            let mut best: Option<Outcome> = None;
            if status == "ok" {
                for (prev, after) in cands {
                    let mut load = |id: &str| {
                        docs.entry(id.to_string())
                            .or_insert_with(|| load_rev(egov, id).map(std::rc::Rc::new))
                            .clone()
                    };
                    let (Some(before), Some(expected)) = (load(prev), load(after)) else {
                        continue;
                    };
                    // 候補の一つが落ちても（突き合わせの側の不具合でも）全体は止めない
                    let o = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        try_pair(&before, &expected, &u, prev, after)
                    }))
                    .unwrap_or_else(|_| Outcome {
                        result: "panic".into(),
                        detail: String::new(),
                        diff: 0,
                        prev: prev.clone(),
                        after: after.clone(),
                        ident: String::new(),
                    });
                    if std::env::var("BENCH_DEBUG").is_ok() {
                        eprintln!("  {prev} > {after}: {} {}", o.result, o.detail);
                    }
                    if best.as_ref().is_none_or(|b| o.rank() < b.rank()) {
                        best = Some(o);
                    }
                    if best.as_ref().is_some_and(|b| b.result == "match") {
                        break;
                    }
                }
                // 最良の候補について identity patch の経路も
                if let Some(b) = best.as_mut().filter(|b| b.result.starts_with("match")) {
                    let before = docs.get(&b.prev).cloned().flatten();
                    let expected = docs.get(&b.after).cloned().flatten();
                    if let (Some(before), Some(expected)) = (before, expected) {
                        b.ident = ident_path(&before, &expected, &u);
                    }
                }
            }
            let o = best.unwrap_or(Outcome {
                result: if status == "ok" {
                    "skip:no_xml".into()
                } else {
                    format!("skip:{status}")
                },
                detail: String::new(),
                diff: 0,
                prev: String::new(),
                after: String::new(),
                ident: String::new(),
            });
            results.push((bi, ui, u, o, cands.clone()));
        }
        // 段階施行（附則で文ごとに施行日が違う）: 同じ法律を改める単位の文をまとめ、改正法の版ごとに
        // その版に近づく文だけを当てる。一致した版で使った文は確かめられた。文が全部確かめられた単位は match_staged
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (k, r) in results.iter().enumerate() {
            if !r.3.result.starts_with("match") && !r.3.result.starts_with("skip") {
                groups.entry(r.2.target_title.clone()).or_default().push(k);
            }
        }
        for (_, ks) in groups {
            let pool: Vec<(usize, usize)> = ks
                .iter()
                .flat_map(|&k| (0..results[k].2.instructions.len()).map(move |i| (k, i)))
                .collect();
            // 文が多すぎる法律（税法の一括改正など）は時間が掛かるので試さない
            if pool.len() < 2 || pool.len() > 150 {
                continue;
            }
            let mut verified: std::collections::BTreeSet<(usize, usize)> = Default::default();
            let mut stages: BTreeMap<usize, (String, String)> = BTreeMap::new();
            for (prev, after) in results[ks[0]].4.clone() {
                let mut load = |id: &str| {
                    docs.entry(id.to_string())
                        .or_insert_with(|| load_rev(egov, id).map(std::rc::Rc::new))
                        .clone()
                };
                let (Some(before), Some(expected)) = (load(&prev), load(&after)) else {
                    continue;
                };
                let todo: Vec<(usize, usize)> = pool
                    .iter()
                    .copied()
                    .filter(|x| !verified.contains(x))
                    .collect();
                let ins: Vec<&lawean_amend::Instruction> = todo
                    .iter()
                    .map(|&(k, i)| &results[k].2.instructions[i])
                    .collect();
                let staged = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    stage_match(&before, &expected, &ins, &prev, &after)
                }))
                .ok()
                .flatten();
                if let Some(used) = staged {
                    for j in used {
                        verified.insert(todo[j]);
                        stages.insert(todo[j].0, (prev.clone(), after.clone()));
                    }
                }
            }
            for &k in &ks {
                let n = results[k].2.instructions.len();
                if (0..n).all(|i| verified.contains(&(k, i))) {
                    let (prev, after) = stages.get(&k).cloned().unwrap_or_default();
                    results[k].3 = Outcome {
                        result: "match_staged".into(),
                        detail: format!("{n} 文"),
                        diff: 0,
                        prev,
                        after,
                        ident: String::new(),
                    };
                }
            }
        }
        for (bi, ui, u, o, _) in results {
            if o.result == "unsupported" || o.result == "apply_error" {
                *unsupported
                    .entry(format!("{} {}", o.result, coarse(&o.detail)))
                    .or_default() += 1;
            }
            *counts.entry(o.result.clone()).or_default() += 1;
            if !o.ident.is_empty() {
                *counts.entry(format!("  {}", o.ident)).or_default() += 1;
            }
            if let Some(w) = w.as_mut() {
                writeln!(
                    w,
                    "{page}\t{bi}\t{ui}\t{}\t{}\t{}\t{}\t{}\t{}",
                    u.target_title,
                    o.result,
                    o.ident,
                    o.prev,
                    o.after,
                    o.detail.replace(['\t', '\n'], " ")
                )
                .unwrap();
            }
        }
        docs.clear();
    }
    let tried: usize = counts
        .iter()
        .filter(|(k, _)| !k.starts_with("skip") && !k.starts_with(' ') && *k != "match_trivial")
        .map(|(_, n)| n)
        .sum();
    let ok = counts.get("match").copied().unwrap_or(0);
    println!(
        "units compared {tried} (match_trivial を除く), match {ok} ({:.1}%)",
        100.0 * ok as f64 / tried.max(1) as f64
    );
    for (k, n) in &counts {
        println!("  {k:24} {n}");
    }
    let mut us: Vec<_> = unsupported.into_iter().collect();
    us.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (k, n) in us {
        println!("  {n:5}  {k}");
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.remove(0);
    match mode.as_str() {
        "list" => list(args),
        "run" => run(args),
        _ => panic!("list | run"),
    }
}
