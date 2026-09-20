//! 改正単位が、被改正法令を参照する他法令に与える影響（docs/09 §3）。

use crate::mapping::{provision_mapping, ProvisionMapping};
use crate::space::{enforced_on, LawSpace};
use crate::xref::{cross_refs, locate, CrossRef};
use lawean_amend::{apply_unit, AmendUnit, ApplyError};
use lawean_source::{LegalDocument, StableId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpactKind {
    /// 参照先が改正で無くなる
    Dangling,
    /// 参照先の番号が動いたのに、参照は旧番号のまま。改正後は別の規定を指す
    Shifted {
        moved_to: StableId,
        now_points_to: Option<StableId>,
    },
    /// 参照先は同じ位置だが本文が変わった
    SemanticChange { before: String, after: String },
    /// 参照元の施行日が改正の施行日より前で、その間は参照の意味が改正後と違う
    TimingGap {
        from: String,
        to: String,
        before: Option<String>,
        after: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impact {
    pub reference: CrossRef,
    pub kind: ImpactKind,
}

fn para_text(doc: &LegalDocument, id: &StableId) -> Option<String> {
    doc.sentence_groups()
        .iter()
        .find(|g| *g.paragraph == *id || id.0.starts_with(&format!("{}/", g.paragraph.0)))
        .map(|g| {
            g.sentences
                .iter()
                .map(|s| s.sentence.plain_text())
                .collect()
        })
}

/// 被改正法令 `target` への改正単位 `unit`（施行日 `enforced`）が、空間内の他法令に与える影響
pub fn impact(
    space: &LawSpace,
    target: &str,
    unit: &AmendUnit,
    enforced: &str,
) -> Result<Vec<Impact>, ApplyError> {
    let before = space.get(target).expect("target law in space");
    let after = apply_unit(before, unit, "after")?;
    let mapping: ProvisionMapping = provision_mapping(before, unit)?;
    let mut out = Vec::new();

    for (law_id, doc) in &space.laws {
        if law_id == target {
            continue;
        }
        let refs = cross_refs(space, law_id, target);
        let written_before = enforced_on(doc).is_some_and(|d| d.as_str() < enforced);
        for r in refs {
            let after_target = locate(&after, &r.article, r.paragraph, r.item);
            if !written_before {
                // 改正後の状態を意図して書かれている。解決できなければ参照切れ
                if after_target.is_none() {
                    out.push(Impact {
                        reference: r,
                        kind: ImpactKind::Dangling,
                    });
                }
                continue;
            }
            // 改正前の状態を意図して書かれている
            let Some(t) = &r.target else {
                out.push(Impact {
                    reference: r,
                    kind: ImpactKind::Dangling,
                });
                continue;
            };
            let moved = mapping.moved.get(t).cloned().flatten();
            let kind = match moved {
                None => Some(ImpactKind::Dangling),
                Some(ref new_id) if Some(new_id) != after_target.as_ref() => {
                    Some(ImpactKind::Shifted {
                        moved_to: new_id.clone(),
                        now_points_to: after_target.clone(),
                    })
                }
                Some(_)
                    if mapping
                        .text_changed
                        .iter()
                        .any(|c| c == t || c.0.starts_with(&format!("{}/", t.0))) =>
                {
                    Some(ImpactKind::SemanticChange {
                        before: para_text(before, t).unwrap_or_default(),
                        after: after_target
                            .as_ref()
                            .and_then(|a| para_text(&after, a))
                            .unwrap_or_default(),
                    })
                }
                Some(_) => None,
            };
            if let Some(kind) = kind {
                let gap = ImpactKind::TimingGap {
                    from: enforced_on(doc).unwrap_or_default(),
                    to: enforced.into(),
                    before: para_text(before, t),
                    after: after_target.as_ref().and_then(|a| para_text(&after, a)),
                };
                out.push(Impact {
                    reference: r.clone(),
                    kind,
                });
                out.push(Impact {
                    reference: r,
                    kind: gap,
                });
            }
        }
    }
    Ok(out)
}
