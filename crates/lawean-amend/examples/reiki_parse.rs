//! 例規の改め文（`fixtures/reiki/cases`）を `parse_units` で読み、読めないものを理由ごとに数える。
//!
//!     cargo run --release -p lawean-amend --example reiki_parse [-- --show 理由の一部]
use std::collections::BTreeMap;

fn coarse(s: &str) -> String {
    let s = regex::Regex::new(r"「[^「」]*」")
        .unwrap()
        .replace_all(s, "「…」");
    let s = regex::Regex::new(r"[0-9０-９一二三四五六七八九十百千]+")
        .unwrap()
        .replace_all(&s, "N");
    s.chars().take(80).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let show = args
        .iter()
        .position(|a| a == "--show")
        .map(|i| args[i + 1].clone());
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/reiki/cases");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    files.sort();
    let (mut ok, mut total) = (0, 0);
    let mut reasons: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for f in files {
        for line in std::fs::read_to_string(&f).unwrap().lines() {
            let c: serde_json::Value = serde_json::from_str(line).unwrap();
            let text: Vec<&str> = c["aratamebun"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            let text = if std::env::var_os("RAW").is_some() {
                text.join("\n")
            } else {
                lawean_amend::reiki::to_law_style(&text)
            };
            total += 1;
            match std::panic::catch_unwind(|| lawean_amend::parse_units(&text)) {
                Ok(Ok(u)) if !u.is_empty() => ok += 1,
                Ok(Ok(_)) => {
                    let e = reasons
                        .entry("no units".into())
                        .or_insert((0, c["case_id"].to_string()));
                    e.0 += 1;
                }
                Ok(Err(e)) => {
                    let msg = e.to_string();
                    let k = coarse(&msg);
                    if show.as_deref().is_some_and(|s| k.contains(s)) {
                        println!(
                            "{}\n  {msg}\n  {}",
                            c["case_id"],
                            text.replace('\n', "\n  ")
                        );
                    }
                    let e = reasons.entry(k).or_insert((0, c["case_id"].to_string()));
                    e.0 += 1;
                }
                Err(_) => {
                    reasons
                        .entry("panic".into())
                        .or_insert((0, String::new()))
                        .0 += 1
                }
            }
        }
    }
    println!("parsed {ok} / {total}");
    let mut v: Vec<_> = reasons.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1 .0));
    for (k, (n, ex)) in v.iter().take(25) {
        println!("{n:5}  {k}  ({ex})");
    }
}
