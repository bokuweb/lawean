//! 制定法律の平文（衆議院ページから抽出したもの）を条ごとに切って parse_units にかけ、読めない改め文を数える。
//! cargo run -p lawean-amend --example survey -- <text>...
fn main() {
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut blocks: Vec<Vec<String>> = Vec::new();
        let mut in_suppl = false;
        for line in text.lines() {
            let t = line.trim_start_matches(['\u{3000}', ' ']);
            if line.contains("附　則") && !line.starts_with('\u{3000}') || t.starts_with("附　則")
            {
                in_suppl = true;
            }
            if in_suppl {
                continue;
            }
            let is_header =
                !line.starts_with('\u{3000}') && t.starts_with('第') && t.contains("条\u{3000}");
            if is_header {
                blocks.push(vec![line.to_string()]);
            } else if let Some(b) = blocks.last_mut() {
                b.push(line.to_string());
            }
        }
        let (mut ok, mut ng) = (0, 0);
        let mut errs: std::collections::BTreeMap<String, usize> = Default::default();
        for b in &blocks {
            if !b[0].contains("の一部を次のように改正する") {
                continue;
            }
            match lawean_amend::parse_units(&b.join("\n")) {
                Ok(_) => ok += 1,
                Err(e) => {
                    ng += 1;
                    let msg = e.to_string();
                    println!(
                        "NG {}: {}",
                        b[0].chars().take(40).collect::<String>(),
                        msg.chars().take(160).collect::<String>()
                    );
                    *errs.entry(msg.chars().take(12).collect()).or_default() += 1;
                }
            }
        }
        println!("{path}: ok {ok} / ng {ng}");
    }
}
