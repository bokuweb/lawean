//! 改正単位の適用で、被改正法令の条・項がどこへ行くか（旧 stable_id → 新 stable_id）と、本文が変わったか。
//! 条・項に印（属性）を付けて適用し、印を追って読む。

use lawean_amend::{apply_unit, AmendUnit, ApplyError};
use lawean_source::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub struct ProvisionMapping {
    /// 旧 → 新。削られたものは None
    pub moved: BTreeMap<StableId, Option<StableId>>,
    /// 本文が変わった旧 stable_id（項単位）
    pub text_changed: BTreeSet<StableId>,
    /// 改正で新設された stable_id（項単位）
    pub created: BTreeSet<StableId>,
}

const TAG: &str = "lawean-orig";

fn tag_all(doc: &mut LegalDocument) {
    fn para(p: &mut Paragraph) {
        p.attrs.push((TAG.into(), p.stable_id.0.clone()));
    }
    fn prov(ps: &mut [Provision]) {
        for p in ps {
            match p {
                Provision::Container(c) => prov(&mut c.children),
                Provision::Article(a) => {
                    a.attrs.push((TAG.into(), a.stable_id.0.clone()));
                    for c in &mut a.children {
                        if let ArticleChild::Paragraph(p) = c {
                            para(p);
                        }
                    }
                }
                Provision::Paragraph(p) => para(p),
                Provision::Raw(_) => {}
            }
        }
    }
    prov(&mut doc.main_provision);
}

fn para_text(p: &Paragraph) -> String {
    p.sentences
        .iter()
        .map(|s| s.plain_text())
        .collect::<String>()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn collect_paras<'a>(ps: &'a [Provision], out: &mut Vec<(&'a Paragraph, Option<&'a Article>)>) {
    for p in ps {
        match p {
            Provision::Container(c) => collect_paras(&c.children, out),
            Provision::Article(a) => {
                for c in &a.children {
                    if let ArticleChild::Paragraph(p) = c {
                        out.push((p, Some(a)));
                    }
                }
            }
            Provision::Paragraph(p) => out.push((p, None)),
            Provision::Raw(_) => {}
        }
    }
}

pub fn provision_mapping(
    doc: &LegalDocument,
    unit: &AmendUnit,
) -> Result<ProvisionMapping, ApplyError> {
    let mut tagged = doc.clone();
    tag_all(&mut tagged);
    let after = apply_unit(&tagged, unit, "mapping")?;

    let mut before_paras = Vec::new();
    collect_paras(&doc.main_provision, &mut before_paras);
    let before_text: BTreeMap<&StableId, String> = before_paras
        .iter()
        .map(|(p, _)| (&p.stable_id, para_text(p)))
        .collect();
    let mut before_articles: BTreeSet<StableId> = BTreeSet::new();
    for (_, a) in &before_paras {
        if let Some(a) = a {
            before_articles.insert(a.stable_id.clone());
        }
    }

    let mut m = ProvisionMapping::default();
    let mut after_paras = Vec::new();
    collect_paras(&after.main_provision, &mut after_paras);
    let mut seen_articles = BTreeSet::new();
    for (p, a) in &after_paras {
        match p.attrs.iter().find(|(k, _)| k == TAG) {
            Some((_, orig)) => {
                let orig = StableId(orig.clone());
                if before_text.get(&orig).is_some_and(|t| *t != para_text(p)) {
                    m.text_changed.insert(orig.clone());
                }
                m.moved.insert(orig, Some(p.stable_id.clone()));
            }
            None => {
                m.created.insert(p.stable_id.clone());
            }
        }
        if let Some(a) = a {
            if let Some((_, orig)) = a.attrs.iter().find(|(k, _)| k == TAG) {
                if seen_articles.insert(orig.clone()) {
                    m.moved
                        .insert(StableId(orig.clone()), Some(a.stable_id.clone()));
                }
            }
        }
    }
    for (p, _) in &before_paras {
        m.moved.entry(p.stable_id.clone()).or_insert(None);
    }
    for a in before_articles {
        m.moved.entry(a).or_insert(None);
    }
    Ok(m)
}
