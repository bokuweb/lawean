//! e-Gov の全リビジョンの施行日が、本文に載る改正法の附則から出る許容区間で説明できるか。
//! cargo run -p lawean-extract --example enforcement_audit
use lawean_extract::calendar::{fmt, parse};
use lawean_extract::suppl::{explain, Explanation};
use lawean_source::parse_response;

fn main() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut idx: Vec<_> = std::fs::read_dir(format!("{root}/revisions_index"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    idx.sort();
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for p in idx {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let law_id = v["law_id"].as_str().unwrap();
        let xml = std::fs::read_dir(format!("{root}/laws"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().unwrap().to_string_lossy().starts_with(law_id))
            .or_else(|| Some(format!("{root}/{law_id}.xml").into()))
            .filter(|p| p.exists());
        let Some(xml) = xml else { continue };
        let doc = parse_response(&std::fs::read_to_string(&xml).unwrap()).unwrap();
        for r in v["revisions"].as_array().unwrap() {
            let Some(amend) = r["amendment_law_id"].as_str() else {
                continue;
            };
            let Some(day) = r["amendment_enforcement_date"].as_str().and_then(parse) else {
                continue;
            };
            let e = explain(&doc, amend, day);
            let k = match &e {
                Explanation::Explained => "explained",
                Explanation::DependsOnOtherLaw(_) => "depends on another law",
                Explanation::NoSupplProvision => "no suppl (fixture older than the revision)",
                Explanation::Unreadable(_) => "unreadable",
                Explanation::Outside(_) => "outside",
            };
            *counts.entry(k).or_default() += 1;
            if !matches!(e, Explanation::Explained) {
                println!("{law_id} {amend} {}: {e:?}", fmt(day));
            }
        }
    }
    println!("{counts:?}");
}
