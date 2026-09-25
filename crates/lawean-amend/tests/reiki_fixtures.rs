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

/// `materialize.py` で組み立てた改正前・改正後の条文がそろっている。組み立てていなければ飛ばす。
/// lawean の読み（`parse_units`）がどれだけ例規の改め文を読めるかも出す（算用数字の横書きは法律の書式と違う。いまは測るだけ）
#[test]
fn materialized_reiki_cases_have_old_and_new() {
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
    let mut parsed = 0;
    for (at, c) in &all {
        let (old, new) = (&c["old"], &c["new"]);
        assert!(
            old.as_array().is_some_and(|a| !a.is_empty()),
            "{at}: no old"
        );
        assert!(
            new.as_array().is_some_and(|a| !a.is_empty()),
            "{at}: no new"
        );
        assert_ne!(old, new, "{at}: old and new are identical");
        let text: Vec<&str> = c["aratamebun"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        if lawean_amend::parse_units(&text.join("\n")).is_ok_and(|u| !u.is_empty()) {
            parsed += 1;
        }
    }
    eprintln!("reiki: {} cases, parse_units read {parsed}", all.len());
}
