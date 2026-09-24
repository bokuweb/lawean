//! 衆議院の制定法律の改め文を、e-Gov の改正前の版に当てて改正後の版と突き合わせる（当てる側の精度の測定）。
//!
//! 1. 改正単位の一覧を出す（改正法の法令 ID と被改正法の法令番号つき）:
//!    cargo run --release -p lawean-amend --example bench_apply -- list <txt のディレクトリ> [--since 20170401] --out units.tsv
//! 2. `tools/fetch_egov_revisions.py units.tsv pairs.tsv` で、単位ごとに e-Gov の直前の版と直後の版を決めて取ってくる
//! 3. 当てて突き合わせる:
//!    cargo run --release -p lawean-amend --example bench_apply -- run <txt のディレクトリ> --pairs pairs.tsv [--out result.tsv]
//!
//! 突き合わせは本則（`snapshot_main`: 条・項ごとの本文）。附則だけを改める単位は本則が変わらないことを確かめるにとどまる。
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[path = "corpus/blocks.rs"]
mod blocks;
use blocks::blocks;

/// ページの 14 桁（国会 3 桁 + 公布日 8 桁 + 番号 3 桁）→ e-Gov の法令 ID（「503AC0000000037」）
fn amending_law_id(page: &str) -> Option<String> {
    let date: u32 = page.get(3..11)?.parse().ok()?;
    let num: u32 = page.get(11..14)?.parse().ok()?;
    let year = date / 10000;
    let (era, y) = if date >= 20190501 {
        (5, year - 2018)
    } else if date >= 19890108 {
        (4, year - 1988)
    } else if date >= 19261225 {
        (3, year - 1925)
    } else {
        return None;
    };
    Some(format!("{era}{y:02}AC{num:010}"))
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
        "page\tblock\tunit\tamending_law_id\tarticle\ttitle\ttarget_law_num"
    )
    .unwrap();
    for f in txt_files(&args[0], since.as_deref()) {
        let page = f.file_stem().unwrap().to_string_lossy().to_string();
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let Some(law_id) = amending_law_id(&page) else {
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

/// e-Gov の版（`fetch_egov_revisions.py` が保存した law_data_response の XML）
fn load_rev(dir: &Path, id: &str) -> Option<lawean_source::LegalDocument> {
    let xml = std::fs::read_to_string(dir.join(format!("{id}.xml"))).ok()?;
    lawean_source::parse_response(&xml).ok()
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
    // (page, block, unit) → (状態, 直前の版, 直後の版)
    let mut want: BTreeMap<(String, usize, usize), (String, String, String)> = BTreeMap::new();
    for l in std::fs::read_to_string(&pairs).unwrap().lines().skip(1) {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 6 {
            continue;
        }
        want.insert(
            (c[0].into(), c[1].parse().unwrap(), c[2].parse().unwrap()),
            (c[3].into(), c[4].into(), c[5].into()),
        );
    }
    let pages: std::collections::BTreeSet<&str> = want.keys().map(|k| k.0.as_str()).collect();
    let mut w = out.map(|p| std::fs::File::create(p).unwrap());
    if let Some(w) = w.as_mut() {
        writeln!(w, "page\tblock\tunit\ttitle\tresult\tdetail").unwrap();
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unsupported: BTreeMap<String, usize> = BTreeMap::new();
    for page in pages {
        let f = Path::new(&args[0]).join(format!("{page}.txt"));
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        for (bi, ui, _, u) in units_of(&text) {
            let Some((status, prev, after)) = want.get(&(page.to_string(), bi, ui)) else {
                continue;
            };
            let (result, detail) = if status != "ok" {
                (format!("skip:{status}"), String::new())
            } else {
                match (load_rev(egov, prev), load_rev(egov, after)) {
                    (Some(before), Some(expected)) => {
                        let r = std::panic::catch_unwind(|| {
                            lawean_amend::apply_unit(&before, &u, "applied")
                        });
                        match r {
                            Ok(Ok(applied)) => {
                                let d = lawean_amend::diff_snapshots(
                                    &lawean_amend::snapshot_main(&applied),
                                    &lawean_amend::snapshot_main(&expected),
                                );
                                if d.is_empty() {
                                    ("match".to_string(), String::new())
                                } else {
                                    (
                                        "mismatch".to_string(),
                                        format!("{} lines: {}", d.len(), d[0]),
                                    )
                                }
                            }
                            Ok(Err(e)) => {
                                let s = e.to_string();
                                if let lawean_amend::ApplyError::Unsupported(what) = &e {
                                    *unsupported.entry(what.clone()).or_default() += 1;
                                    ("unsupported".to_string(), s)
                                } else {
                                    ("apply_error".to_string(), s)
                                }
                            }
                            Err(_) => ("panic".to_string(), String::new()),
                        }
                    }
                    _ => ("skip:no_xml".to_string(), String::new()),
                }
            };
            *counts.entry(result.clone()).or_default() += 1;
            if let Some(w) = w.as_mut() {
                writeln!(
                    w,
                    "{page}\t{bi}\t{ui}\t{}\t{result}\t{}",
                    u.target_title,
                    detail.replace(['\t', '\n'], " ")
                )
                .unwrap();
            }
        }
    }
    let tried: usize = counts
        .iter()
        .filter(|(k, _)| !k.starts_with("skip"))
        .map(|(_, n)| n)
        .sum();
    let ok = counts.get("match").copied().unwrap_or(0);
    println!(
        "units compared {tried}, match {ok} ({:.1}%)",
        100.0 * ok as f64 / tried.max(1) as f64
    );
    for (k, n) in &counts {
        println!("  {k:24} {n}");
    }
    let mut us: Vec<_> = unsupported.into_iter().collect();
    us.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (k, n) in us {
        println!("  unsupported {n:5}  {k}");
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
