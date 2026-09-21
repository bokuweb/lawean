//! fixtures/revisions_index（e-Gov `law_revisions`、10 法令 119 リビジョン）の施行日が、本文に載る改正法の附則で説明できる。
//! 読めない附則・どの区間にも入らない施行日は 0

use lawean_extract::calendar::parse;
use lawean_extract::suppl::{explain, Explanation};
use lawean_source::parse_response;

#[test]
fn every_egov_revision_date_is_explained_by_the_suppl_provisions() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let (mut explained, mut other, mut none, mut bad) = (0, 0, 0, Vec::new());
    for p in std::fs::read_dir(format!("{root}/revisions_index"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
    {
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
        // 公職選挙法は fixtures/revisions/ にしか無い（本文の版が多い）
        let Some(xml) = xml else { continue };
        let doc = parse_response(&std::fs::read_to_string(&xml).unwrap()).unwrap();
        for r in v["revisions"].as_array().unwrap() {
            let Some(amend) = r["amendment_law_id"].as_str() else {
                continue;
            };
            let Some(day) = r["amendment_enforcement_date"].as_str().and_then(parse) else {
                continue;
            };
            match explain(&doc, amend, day) {
                Explanation::Explained => explained += 1,
                Explanation::DependsOnOtherLaw(_) => other += 1,
                Explanation::NoSupplProvision => none += 1,
                e => bad.push(format!("{law_id} {amend} {day:?}: {e:?}")),
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    // 2026-09-22: explained 100, depends on another law 12, no suppl 7（本文が改正より古い）
    assert!(
        explained >= 100 && other <= 12 && none <= 7,
        "{explained} {other} {none}"
    );
}
