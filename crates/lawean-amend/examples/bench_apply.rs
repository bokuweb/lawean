//! 衆議院の制定法律の改め文を、e-Gov の改正前の版に当てて改正後の版と突き合わせる（当てる側の精度の測定）。
//!
//! 1. 改正単位の一覧を出す（改正法と被改正法の法令番号つき）:
//!    cargo run --release -p lawean-amend --example bench_apply -- list <txt のディレクトリ> [--since 20170401] --out units.tsv
//! 2. `tools/fetch_egov_revisions.py units.tsv pairs.tsv` で、単位ごとに e-Gov の直前の版と直後の版を決めて取ってくる
//! 3. 当てて突き合わせる:
//!    cargo run --release -p lawean-amend --example bench_apply -- run <txt のディレクトリ> --pairs pairs.tsv [--out result.tsv]
//!
//! 突き合わせは本則と原始附則（`snapshot_main`: 条・項ごとの本文）。目次・別表はまだ比べない。
//! e-Gov の直前の版と直後の版で比べる部分が変わっていない単位は `match_trivial`（一致しても当てたことの確かめにならない）。
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

/// 本則と原始附則（キーに「附則」を冠する）の条・項ごとの本文
fn snapshot(doc: &lawean_source::LegalDocument) -> Snapshot {
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
    out
}

/// e-Gov の版（`fetch_egov_revisions.py` が保存した law_data_response の XML）
fn load_rev(dir: &Path, id: &str) -> Option<lawean_source::LegalDocument> {
    let xml = std::fs::read_to_string(dir.join(format!("{id}.xml"))).ok()?;
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
            "mismatch" => (2, self.diff),
            "unsupported" => (3, 0),
            _ => (4, 0),
        }
    }
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
    match std::panic::catch_unwind(|| lawean_amend::apply_unit(before, u, "applied")) {
        Ok(Ok(applied)) => {
            let exp = snapshot(expected);
            let d = lawean_amend::diff_snapshots(&snapshot(&applied), &exp);
            let changed = !lawean_amend::diff_snapshots(&snapshot(before), &exp).is_empty();
            if d.is_empty() && !changed {
                mk("match_trivial", String::new(), 0)
            } else if d.is_empty() {
                mk("match", String::new(), 0)
            } else {
                mk("mismatch", format!("{} lines: {}", d.len(), d[0]), d.len())
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
        Ok::<bool, String>(got.render() == ident::from_document(expected).render())
    });
    match r {
        Ok(Ok(true)) => "ident_match".into(),
        Ok(Ok(false)) => "ident_mismatch".into(),
        Ok(Err(e)) if e == "none" => "ident_none".into(),
        Ok(Err(_)) => "ident_bind_error".into(),
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
                    let o = try_pair(&before, &expected, &u, prev, after);
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
            // 版を読み込んだものは、同じページの中だけ持つ（大きい法令の版で溢れないように）
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
