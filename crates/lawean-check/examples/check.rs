//! 改正案を検査して報告を表示する。
//! cargo run -p lawean-check --example check -- <発射台.xml> <改め文.txt> [--expected 改正後.xml] [--taisho 新旧対照表.txt] [--law 他法令.xml]... [--enforced YYYY-MM-DD] [--json]
//! cargo run -p lawean-check --example check -- --case <id>   # fixtures/cases/cases.json のケース

use lawean_check::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let read = |p: &str| {
        std::fs::read_to_string(root.join(p))
            .or_else(|_| std::fs::read_to_string(p))
            .expect(p)
    };
    let json = args.iter().any(|a| a == "--json");
    let report = if let Some(i) = args.iter().position(|a| a == "--case") {
        let id = &args[i + 1];
        let cases: serde_json::Value = serde_json::from_str(&read("cases/cases.json")).unwrap();
        let c = cases
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == *id)
            .unwrap_or_else(|| panic!("no case {id}"));
        let s = |k: &str| c[k].as_str().map(&read);
        let others: Vec<String> = c["other_laws"]
            .as_array()
            .map(|v| v.iter().map(|p| read(p.as_str().unwrap())).collect())
            .unwrap_or_default();
        println!(
            "# {}\n{}\n",
            c["title"].as_str().unwrap(),
            c["note"].as_str().unwrap_or("")
        );
        run_texts(
            &s("base").unwrap(),
            &s("amendment").unwrap(),
            s("expected").as_deref(),
            s("taisho").as_deref(),
            &others,
            c["enforced"].as_str(),
        )
    } else {
        let mut expected = None;
        let mut taisho = None;
        let mut laws = Vec::new();
        let mut enforced = None;
        let mut pos = Vec::new();
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--expected" => expected = Some(read(it.next().unwrap())),
                "--taisho" => taisho = Some(read(it.next().unwrap())),
                "--law" => laws.push(read(it.next().unwrap())),
                "--enforced" => enforced = Some(it.next().unwrap().clone()),
                "--json" => {}
                p => pos.push(p.to_string()),
            }
        }
        run_texts(
            &read(&pos[0]),
            &read(&pos[1]),
            expected.as_deref(),
            taisho.as_deref(),
            &laws,
            enforced.as_deref(),
        )
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return;
    }
    println!(
        "{} (engine: {})",
        if report.ok { "PASS" } else { "FAIL" },
        report.engine
    );
    for u in &report.units {
        println!(
            "- {}: {} 文, {} 操作 → id 操作 {} 個",
            u.label,
            u.instructions,
            u.ops,
            u.ident_ops.len()
        );
        for o in &u.ident_ops {
            println!("    {o}");
        }
    }
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
        println!("suggested fix (改め文、番号は改正前): {f}");
    }
    if !report.diff.is_empty() {
        println!("diff:");
        for d in &report.diff {
            println!("    {d}");
        }
    }
}
