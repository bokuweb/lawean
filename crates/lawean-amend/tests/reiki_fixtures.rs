//! `fixtures/reiki`: 自治体が公布した例規（横書き）の改め文と、その出典（`tools/reiki/`）。
//!
//! 公開のリポジトリには、改め文（自治体の公報・公布した条例の PDF から）と出典（PDF の URL、条例Webアーカイブの版の ID）だけを置く。
//! 改正前・改正後の条文（`old` / `new`）は条例Webの版から `tools/reiki/materialize.py` で手元に組み立て、
//! `fixtures/reiki/full/`（git に入れない）に置く。無ければ、それを使うテストは飛ばす。
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/reiki")
}

fn cases(dir: &Path) -> Vec<(String, Value)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    files.retain(|p| p.extension().is_some_and(|x| x == "jsonl"));
    files.sort();
    let mut out = Vec::new();
    for f in files {
        for (i, line) in std::fs::read_to_string(&f).unwrap().lines().enumerate() {
            let at = format!("{}:{}", f.display(), i + 1);
            out.push((at, serde_json::from_str(line).unwrap()));
        }
    }
    out
}

/// 置いてあるのは改め文と出典だけで、条例Webの版から取った条文（`old` / `new`）が紛れ込んでいない
#[test]
fn reiki_fixtures_hold_aratamebun_and_sources_only() {
    let all = cases(&root().join("cases"));
    assert!(!all.is_empty(), "fixtures/reiki/cases is empty");
    let mut ids = BTreeSet::new();
    let mut bad = Vec::new();
    for (at, c) in &all {
        let keys: BTreeSet<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
        let want: BTreeSet<&str> = [
            "case_id",
            "verification",
            "source",
            "checks",
            "changed",
            "aratamebun",
        ]
        .into_iter()
        .collect();
        if keys != want {
            bad.push(format!("{at}: keys {keys:?}"));
        }
        let id = c["case_id"].as_str().unwrap_or_default();
        if !ids.insert(id.to_string()) {
            bad.push(format!("{at}: duplicate {id}"));
        }
        let s = &c["source"];
        for k in [
            "city",
            "municipality_id",
            "amending_num",
            "amending_pdf",
            "target_title",
            "jorei_web_before",
            "jorei_web_after",
        ] {
            if s[k].as_str().is_none_or(str::is_empty) {
                bad.push(format!("{at}: source.{k} is missing"));
            }
        }
        let head = c["aratamebun"][0].as_str().unwrap_or_default();
        if !head.ends_with("の一部を次のように改正する。") {
            bad.push(format!(
                "{at}: aratamebun does not start with an amending clause"
            ));
        }
        if c["verification"] != "consistent" {
            bad.push(format!("{at}: not consistent"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad[..bad.len().min(20)].join("\n")
    );
}

fn blocks(v: &Value) -> Vec<lawean_amend::reiki::Block> {
    use lawean_amend::reiki::Block;
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

/// `materialize.py` で組み立てた改正前・改正後の条文で、改め文を当てて確かめる。組み立てていなければ飛ばす。
///
/// **回帰の gate**: `fixtures/reiki/apply_baseline.txt` の case（いま当てて改正後と一致するもの）が 1 件でも一致しなくなったら失敗する。
/// 新しく一致した case は表示する（`LAWEAN_REIKI_UPDATE_BASELINE=1` で書き換える）
#[test]
fn materialized_reiki_cases_keep_applying_to_the_amended_text() {
    let full = root().join("full");
    let all = cases(&full);
    if all.is_empty() {
        eprintln!(
            "skip: {} is empty (python3 tools/reiki/materialize.py)",
            full.display()
        );
        return;
    }
    let committed = cases(&root().join("cases")).len();
    assert_eq!(
        all.len(),
        committed,
        "materialize again: fixtures/reiki/full is out of date"
    );
    let mut matched = BTreeSet::new();
    let mut failures = std::collections::BTreeMap::new();
    for (at, c) in &all {
        let (old, new) = (&c["old"], &c["new"]);
        assert_ne!(old, new, "{at}: old and new are identical");
        let lines: Vec<&str> = c["aratamebun"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let id = c["case_id"].as_str().unwrap().to_string();
        match std::panic::catch_unwind(|| {
            lawean_amend::reiki::check(&blocks(old), &blocks(new), &lines)
        }) {
            Ok(Ok(())) => {
                matched.insert(id);
            }
            Ok(Err(e)) => {
                failures.insert(id, format!("{}: {}", e.kind(), e.detail()));
            }
            Err(_) => {
                failures.insert(id, "panic".into());
            }
        }
    }
    let path = root().join("apply_baseline.txt");
    if std::env::var_os("LAWEAN_REIKI_UPDATE_BASELINE").is_some() {
        let header: String = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .lines()
            .take_while(|l| l.starts_with('#'))
            .map(|l| format!("{l}\n"))
            .collect();
        let body: String = matched.iter().map(|id| format!("{id}\n")).collect();
        std::fs::write(&path, header + &body).unwrap();
        eprintln!("apply baseline updated: {} cases", matched.len());
        return;
    }
    let text = std::fs::read_to_string(&path).unwrap();
    let baseline: BTreeSet<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let regressions: Vec<String> = baseline
        .iter()
        .filter(|id| !matched.contains(**id))
        .map(|id| {
            format!(
                "{id}: {}",
                failures.get(*id).map_or("missing", String::as_str)
            )
        })
        .collect();
    let newly: Vec<&String> = matched
        .iter()
        .filter(|id| !baseline.contains(id.as_str()))
        .collect();
    eprintln!(
        "reiki apply: matched {} / {}, baseline {}, regressions {}, newly matched {}",
        matched.len(),
        all.len(),
        baseline.len(),
        regressions.len(),
        newly.len()
    );
    assert!(
        regressions.is_empty(),
        "{} baseline cases no longer apply to the amended text:\n{}",
        regressions.len(),
        regressions.join("\n")
    );
}
