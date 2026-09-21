//! 法令全体の候補（時間表現・罰則）を共通の形で。層 1.5（主体・行為、`lawean-nlp`）は別 crate から足す

use crate::amount::amount_candidates;
use crate::candidate::{dedup_sorted, Candidate};
use crate::penalty::penalties;
use crate::temporal::time_candidates;
use lawean_source::LegalDocument;

pub fn candidates(doc: &LegalDocument) -> Vec<Candidate> {
    let mut out = Vec::new();
    for g in doc.sentence_groups() {
        for s in &g.sentences {
            let text = s.sentence.plain_text();
            let times = time_candidates(&s.sentence.stable_id, &text);
            let spans: Vec<(usize, usize)> = times
                .iter()
                .map(|c| (c.evidence.start, c.evidence.end))
                .collect();
            out.extend(amount_candidates(&s.sentence.stable_id, &text, &spans));
            out.extend(times);
        }
    }
    for p in penalties(doc) {
        out.extend(p.to_candidates());
    }
    dedup_sorted(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_reproduces_snippet() {
        let p = format!(
            "{}/../../fixtures/403AC0000000090.xml",
            env!("CARGO_MANIFEST_DIR")
        );
        let doc = lawean_source::parse_response(&std::fs::read_to_string(p).unwrap()).unwrap();
        let texts: std::collections::BTreeMap<String, String> = doc
            .sentence_groups()
            .iter()
            .flat_map(|g| {
                g.sentences
                    .iter()
                    .map(|s| (s.sentence.stable_id.0.clone(), s.sentence.plain_text()))
            })
            .collect();
        let cs = candidates(&doc);
        assert!(cs.len() >= 40, "{}", cs.len());
        for c in &cs {
            let t = &texts[&c.evidence.sentence.0];
            assert_eq!(
                &t[c.evidence.start..c.evidence.end],
                c.evidence.snippet,
                "{c:?}"
            );
            assert_eq!(c.category, c.field.category());
        }
        // 決定的（2 回走らせて同じ）
        assert_eq!(cs, candidates(&doc));
        // JSON に出せる
        let json = serde_json::to_string(&cs[0]).unwrap();
        let back: Candidate = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cs[0]);
    }
}
