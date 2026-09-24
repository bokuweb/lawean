//! 制定法律の平文（`tools/shugiin_text.py` で HTML から作ったもの）をまとめて `parse_units` にかけ、
//! 読めない改め文を数える。
//!
//!   cargo run --release -p lawean-amend --example survey_corpus -- <txt のディレクトリかファイル>... [--out 結果.tsv]
//!
//! 法令ごとに本則（附則より前）を改正単位に切る: 「第N条　X法（…）の一部を次のように改正する。」（整備法）、
//! 「X法（…）の一部を次のように改正する。」（単独の一部改正法）、「第N条　次に掲げる法律の規定中…」（列挙形）。
//! 新規制定・全部改正など改正単位の無い法令は数えない。
use std::collections::BTreeMap;
use std::io::Write;

#[path = "corpus/blocks.rs"]
mod blocks;
use blocks::blocks;

/// 「第十条第三項中「A」を「B」に改め」→「第N条第N項中「…」を「…」に改め」
fn shape(e: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    let mut last_n = false;
    for c in e.chars() {
        match c {
            '「' => {
                if depth == 0 {
                    out.push_str("「…");
                }
                depth += 1;
            }
            '」' => {
                depth -= 1;
                if depth <= 0 {
                    depth = 0;
                    out.push('」');
                }
            }
            _ if depth > 0 => {}
            _ if "一二三四五六七八九十百千〇".contains(c) => {
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
    out.chars().take(60).collect()
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let out_path = args.iter().position(|a| a == "--out").map(|i| {
        let p = args[i + 1].clone();
        args.drain(i..=i + 1);
        p
    });
    // 「--since 19890108」: 公布日（ファイル名の 4〜11 桁目）がこの日以後の法律だけ
    let since = args.iter().position(|a| a == "--since").map(|i| {
        let p = args[i + 1].clone();
        args.drain(i..=i + 1);
        p
    });
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for a in &args {
        let p = std::path::Path::new(a);
        if p.is_dir() {
            let mut v: Vec<_> = std::fs::read_dir(p)
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "txt"))
                .collect();
            v.sort();
            files.extend(v);
        } else {
            files.push(p.to_path_buf());
        }
    }
    let (mut laws, mut units_ok, mut units_ng) = (0usize, 0usize, 0usize);
    let mut kinds: BTreeMap<String, (usize, String)> = BTreeMap::new();
    let mut by_session: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    let mut tsv = out_path.as_ref().map(|p| std::fs::File::create(p).unwrap());
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        if since
            .as_deref()
            .is_some_and(|d| name.get(3..11).is_none_or(|x| x < d))
        {
            continue;
        }
        let session: u32 = name.get(0..3).and_then(|s| s.parse().ok()).unwrap_or(0);
        let bs = blocks(&text);
        if bs.is_empty() {
            continue;
        }
        laws += 1;
        for b in bs {
            let head: String = b.lines().next().unwrap_or("").chars().take(40).collect();
            let r = std::panic::catch_unwind(|| lawean_amend::parse_units(&b));
            // （理由, 読めなかった文）
            let e = match r {
                Ok(Ok(_)) => None,
                Ok(Err(e)) => Some((
                    e.cause().to_string(),
                    match &e {
                        lawean_amend::ParseError::InSentence { line, .. } => line.clone(),
                        _ => String::new(),
                    },
                )),
                Err(_) => Some(("panic".to_string(), String::new())),
            };
            let s = by_session.entry(session).or_default();
            match e {
                None => {
                    units_ok += 1;
                    s.0 += 1;
                }
                Some((e, line)) => {
                    units_ng += 1;
                    s.1 += 1;
                    // 種類: エラーの断片の形（数をならし、「」の中を落とす）
                    let key = shape(&e);
                    let k = kinds.entry(key).or_insert((0, format!("{name} {head}")));
                    k.0 += 1;
                    if let Some(t) = tsv.as_mut() {
                        writeln!(
                            t,
                            "{name}\t{head}\t{}\t{}",
                            e.replace(['\t', '\n'], " "),
                            line.replace(['\t', '\n'], " ")
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    let total = units_ok + units_ng;
    println!(
        "laws with amendments {laws}, units {total}, ok {units_ok}, ng {units_ng} ({:.1}%)",
        100.0 * units_ok as f64 / total.max(1) as f64
    );
    let mut ks: Vec<_> = kinds.into_iter().collect();
    ks.sort_by_key(|k| std::cmp::Reverse(k.1 .0));
    for (k, (n, ex)) in ks.iter().take(60) {
        println!("{n:5}  {k}  [{ex}]");
    }
    // 国会ごと（10 国会ずつ）
    let mut agg: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    for (s, (o, n)) in by_session {
        let e = agg.entry(s / 10 * 10).or_default();
        e.0 += o;
        e.1 += n;
    }
    for (s, (o, n)) in agg {
        println!("sessions {s:3}-{:3}: ok {o} ng {n}", s + 9);
    }
}
