//! CLI: `lawean-consolidate <発射台.xml> <改め文.txt> [--out 改正後.xml] [--expected 改正後.xml] [--enforced YYYY-MM-DD]
//!                          [--law 他法令.xml]... [--base-draft 起草時の発射台.xml] [--suppl 附則.txt --promulgated YYYY-MM-DD]
//!                          [--other-law-date 法令名=YYYY-MM-DD]... [--json]`
//! 発射台の XML は e-Gov 法令 API（`law_data`）の応答でも、裸の `<Law>` でもよい

use lawean_consolidate::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let read = |p: &str| {
        std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("{p}: {e}");
            std::process::exit(2)
        })
    };
    let (mut out, mut expected, mut enforced, mut base_draft, mut json) =
        (None, None, None, None, false);
    let (mut suppl, mut promulgated) = (None, None);
    let mut other_law_dates: Vec<(String, String)> = Vec::new();
    let mut laws = Vec::new();
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = it.next().cloned(),
            "--expected" => expected = it.next().map(|p| read(p)),
            "--enforced" => enforced = it.next().cloned(),
            "--base-draft" => base_draft = it.next().map(|p| read(p)),
            "--suppl" => suppl = it.next().map(|p| read(p)),
            "--promulgated" => promulgated = it.next().cloned(),
            "--other-law-date" => {
                let v = it.next().expect("--other-law-date 法令名=YYYY-MM-DD");
                let (n, d) = v.split_once('=').expect("法令名=YYYY-MM-DD");
                other_law_dates.push((n.to_string(), d.to_string()));
            }
            "--law" => laws.push(read(it.next().expect("--law <xml>"))),
            "--json" => json = true,
            p => pos.push(p.to_string()),
        }
    }
    if pos.len() < 2 {
        eprintln!("usage: lawean-consolidate <発射台.xml> <改め文.txt> [--out 改正後.xml] [--expected 改正後.xml] [--enforced YYYY-MM-DD] [--law 他法令.xml]... [--base-draft 起草時の発射台.xml] [--suppl 附則.txt --promulgated YYYY-MM-DD] [--json]");
        std::process::exit(2);
    }
    let base = read(&pos[0]);
    let amend = read(&pos[1]);
    let (result, report) = consolidate_and_verify(&Input {
        base_xml: &base,
        amendment: &amend,
        expected_xml: expected.as_deref(),
        enforced: enforced.as_deref(),
        other_laws: &laws,
        base_draft_xml: base_draft.as_deref(),
        suppl: suppl.as_deref(),
        promulgated: promulgated.as_deref(),
        other_law_dates: &other_law_dates,
        ..Default::default()
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!(
            "{} (engine: {})",
            if report.ok { "PASS" } else { "FAIL" },
            report.engine
        );
        for c in &report.checks {
            let mark = match c.status {
                Status::Pass => "✓",
                Status::Fail => "✗",
                Status::Warn => "!",
                Status::Skip => "-",
            };
            println!("{mark} {:?}: {}", c.kind, c.message);
            for d in &c.details {
                println!("    {d}");
            }
        }
        for f in &report.suggested_fixes {
            println!("suggested fix: {f}");
        }
    }
    match (result, out) {
        (Some(r), Some(path)) => {
            std::fs::write(&path, &r.xml).unwrap_or_else(|e| {
                eprintln!("{path}: {e}");
                std::process::exit(2)
            });
            eprintln!(
                "wrote {path} ({} bytes, {} changes)",
                r.xml.len(),
                r.diff.len()
            );
        }
        (None, Some(_)) => eprintln!("溶け込みが止まったので XML は書かない"),
        _ => {}
    }
    std::process::exit(if report.ok { 0 } else { 1 });
}
