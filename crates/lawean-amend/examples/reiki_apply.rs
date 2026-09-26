//! 例規の改め文を改正前の条文に当て、改正後の条文と突き合わせる（例規の当てる側の精度。判定は `reiki::check`）。
//!
//!     python3 tools/reiki/materialize.py      # fixtures/reiki/full（改正前・改正後の条文）を組み立てる
//!     cargo run --release -p lawean-amend --example reiki_apply [-- --out result.tsv] [--show 理由の一部]
use lawean_amend::reiki::{self, Block};
use std::collections::BTreeMap;
use std::io::Write;

pub fn blocks(v: &serde_json::Value) -> Vec<Block> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|b| match b["type"].as_str() {
            Some("table") => Block::Table(
                b["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| {
                        r.as_array()
                            .unwrap()
                            .iter()
                            .map(|c| c.as_str().unwrap_or_default().to_string())
                            .collect()
                    })
                    .collect(),
            ),
            _ => Block::Paragraph(b["text"].as_str().unwrap_or_default().to_string()),
        })
        .collect()
}

fn coarse(s: &str) -> String {
    let s = regex::Regex::new(r"「[^「」]*」")
        .unwrap()
        .replace_all(s, "「…」");
    let s = regex::Regex::new(r"[0-9０-９一二三四五六七八九十百千]+")
        .unwrap()
        .replace_all(&s, "N");
    s.chars().take(70).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .map(|i| args[i + 1].clone())
    };
    let (out, show) = (flag("--out"), flag("--show"));
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/reiki/full");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("python3 tools/reiki/materialize.py で fixtures/reiki/full を組み立てる")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    files.sort();
    let mut w = out.map(|p| std::fs::File::create(p).unwrap());
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for f in files {
        for line in std::fs::read_to_string(&f).unwrap().lines() {
            let c: serde_json::Value = serde_json::from_str(line).unwrap();
            let id = c["case_id"].as_str().unwrap_or_default().to_string();
            let lines: Vec<&str> = c["aratamebun"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            let r = std::panic::catch_unwind(|| {
                reiki::check(&blocks(&c["old"]), &blocks(&c["new"]), &lines)
            });
            let (kind, detail) = match r {
                Ok(Ok(())) => ("match", String::new()),
                Ok(Err(e)) => (e.kind(), e.detail().to_string()),
                Err(_) => ("panic", String::new()),
            };
            *counts.entry(kind).or_default() += 1;
            if kind != "match" {
                let e = reasons
                    .entry(format!("{kind}: {}", coarse(&detail)))
                    .or_insert((0, id.clone()));
                e.0 += 1;
            }
            if show
                .as_deref()
                .is_some_and(|s| format!("{kind}: {detail}").contains(s))
            {
                println!("{id}\n  {kind}: {detail}");
            }
            if let Some(w) = w.as_mut() {
                writeln!(w, "{id}\t{kind}\t{}", detail.replace(['\t', '\n'], " ")).unwrap();
            }
        }
    }
    let total: usize = counts.values().sum();
    println!("reiki apply: {counts:?} total={total}");
    let mut v: Vec<_> = reasons.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1 .0));
    for (k, (n, ex)) in v.iter().take(25) {
        println!("{n:5}  {k}  ({ex})");
    }
}
