//! 法令空間: ある時点に施行されている法令のリビジョンの集合と、法令名 → law_id の解決。

use lawean_source::{inline_text, LegalDocument};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct LawSpace {
    pub laws: BTreeMap<String, LegalDocument>,
    /// 題名（および略称）→ law_id
    pub names: BTreeMap<String, String>,
}

impl LawSpace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, doc: LegalDocument) -> &mut Self {
        let id = doc.law_id.clone().expect("law_id");
        if let Some(t) = &doc.title {
            self.names.insert(inline_text(&t.text), id.clone());
        }
        self.laws.insert(id, doc);
        self
    }

    /// 「新借地借家法」「旧法」のような略称を登録する
    pub fn alias(&mut self, name: &str, law_id: &str) -> &mut Self {
        self.names.insert(name.into(), law_id.into());
        self
    }

    pub fn resolve_name(&self, name: &str) -> Option<&str> {
        self.names.get(name).map(String::as_str)
    }

    pub fn get(&self, law_id: &str) -> Option<&LegalDocument> {
        self.laws.get(law_id)
    }
}

/// `law_revision_id`（`{law_id}_{YYYYMMDD}_{amend}`）から施行日を取り出す
pub fn enforced_on(doc: &LegalDocument) -> Option<String> {
    let v = doc.version_id.as_ref()?;
    let d = v.split('_').nth(1)?;
    (d.len() == 8).then(|| format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..]))
}
