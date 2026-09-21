//! identity patch（ADR-0013）。番号で書かれた改め文の `Op` を、発射台リビジョンに対して stable_id に束縛する。
//!
//! 束縛した結果は Lean の `Lawean.Ident`（`lean/Lawean/Ident.lean`）と同じ型で、ここにある `apply_unit` は
//! Lean の `applyUnit` を一行ずつ写したもの。Lean が正で、Rust 版は Lean に出す前の前検査（ADR-0011）。
//!
//! - 項は stable_id で指す。番号は状態に持たず、描画時に数える（`para_num`）
//! - 「A」を「B」に改める は、当たった項ごとに `Replace { expected: 改正前の本文, new: 改正後の本文 }` に展開する。
//!   字句の置換は束縛時に済ませ、Lean は本文全体を期待値と突き合わせる（Pijul の context）
//! - 繰り下げ・「第P項を第Q項とし」は番号だけを動かすので、id の世界では操作にならない（束縛で消える）
//! - 「次のように改める」（全部改正）は `Delete` + `InsertAfter`。旧 id は消える
//! - 新しい id は改正法 ID と、その改正単位の中での位置から決定的に振る

use crate::apply::*;
use crate::op::*;
use lawean_source::*;
use std::collections::BTreeMap;

/// 項。`art` は所属する条（描画用で、代数には効かない）。`conflicts` は期待した本文と違ったため当てられなかった新本文
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub art: String,
    pub text: String,
    pub conflicts: Vec<String>,
}

/// 法令の 1 リビジョン = 文書順の項の列（目次があれば先頭に `toc`）。番号は持たない
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentRevision {
    pub nodes: Vec<Node>,
}

/// 改正の操作。すべて id で対象を指す
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentOp {
    /// id の本文が expected なら new に改める。違えば衝突として記録する
    Replace {
        id: String,
        expected: String,
        new: String,
    },
    /// anchor の直後に new_id の項を加える
    InsertAfter {
        anchor: String,
        new_id: String,
        art: String,
        text: String,
    },
    /// id の項を削る
    Delete { id: String },
    /// 調整規定: 衝突を解消して本文を確定する
    Resolve { id: String, text: String },
    /// 条ずれ: id の項の条番号を art にする（本文は触らない）
    Renumber { id: String, art: String },
}

pub const TOC_ID: &str = "toc";

// ---------------------------------------------------------------- Lean `Ident` の写し

impl IdentOp {
    /// 操作が読む・書く id
    pub fn touches(&self) -> Vec<&str> {
        match self {
            IdentOp::Replace { id, .. }
            | IdentOp::Delete { id }
            | IdentOp::Resolve { id, .. }
            | IdentOp::Renumber { id, .. } => vec![id],
            IdentOp::InsertAfter { anchor, new_id, .. } => vec![anchor, new_id],
        }
    }

    /// 操作が本文を変える・作る・消す id（`insertAfter` の anchor は位置を指すだけなので含まない）
    pub fn modifies(&self) -> Vec<&str> {
        match self {
            IdentOp::Replace { id, .. }
            | IdentOp::Delete { id }
            | IdentOp::Resolve { id, .. }
            | IdentOp::Renumber { id, .. } => vec![id],
            IdentOp::InsertAfter { new_id, .. } => vec![new_id],
        }
    }

    /// 操作が新しく作る id
    pub fn creates(&self) -> Vec<&str> {
        match self {
            IdentOp::InsertAfter { new_id, .. } => vec![new_id],
            _ => vec![],
        }
    }

    /// 操作が探す id
    pub fn key(&self) -> &str {
        match self {
            IdentOp::Replace { id, .. }
            | IdentOp::Delete { id }
            | IdentOp::Resolve { id, .. }
            | IdentOp::Renumber { id, .. } => id,
            IdentOp::InsertAfter { anchor, .. } => anchor,
        }
    }

    /// 見つかった項を 0 個以上の項の列に置き換える
    fn edit(&self, x: &Node) -> Vec<Node> {
        match self {
            IdentOp::Replace { expected, new, .. } => {
                if &x.text == expected {
                    vec![Node {
                        text: new.clone(),
                        ..x.clone()
                    }]
                } else {
                    let mut y = x.clone();
                    y.conflicts.push(new.clone());
                    vec![y]
                }
            }
            IdentOp::InsertAfter {
                new_id, art, text, ..
            } => vec![
                x.clone(),
                Node {
                    id: new_id.clone(),
                    art: art.clone(),
                    text: text.clone(),
                    conflicts: vec![],
                },
            ],
            IdentOp::Delete { .. } => vec![],
            IdentOp::Resolve { text, .. } => vec![Node {
                text: text.clone(),
                conflicts: vec![],
                ..x.clone()
            }],
            IdentOp::Renumber { art, .. } => vec![Node {
                art: art.clone(),
                ..x.clone()
            }],
        }
    }
}

/// 最初に id が key の要素を見つけ、置き換える。無ければ None（発射台に無いものを触っている）
pub fn apply_op(r: &IdentRevision, op: &IdentOp) -> Option<IdentRevision> {
    let i = r.nodes.iter().position(|n| n.id == op.key())?;
    let mut nodes = r.nodes[..i].to_vec();
    nodes.extend(op.edit(&r.nodes[i]));
    nodes.extend_from_slice(&r.nodes[i + 1..]);
    Some(IdentRevision { nodes })
}

/// 改正単位の適用。途中で失敗したら全体が失敗
pub fn apply_unit(r: &IdentRevision, u: &[IdentOp]) -> Option<IdentRevision> {
    u.iter().try_fold(r.clone(), |r, op| apply_op(&r, op))
}

impl IdentRevision {
    /// id に重複が無いか
    pub fn wf(&self) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        self.nodes.iter().all(|n| seen.insert(&n.id))
    }

    pub fn has_conflict(&self) -> bool {
        self.nodes.iter().any(|n| !n.conflicts.is_empty())
    }

    pub fn conflicts(&self) -> Vec<(&str, &str, &[String])> {
        self.nodes
            .iter()
            .filter(|n| !n.conflicts.is_empty())
            .map(|n| (n.id.as_str(), n.text.as_str(), n.conflicts.as_slice()))
            .collect()
    }

    /// 番号は描画時に計算する: 同じ条の中での位置 + 1
    pub fn para_num(&self, id: &str) -> Option<usize> {
        let n = self.nodes.iter().find(|n| n.id == id)?;
        self.nodes
            .iter()
            .filter(|m| m.art == n.art)
            .position(|m| m.id == id)
            .map(|i| i + 1)
    }

    /// id を捨てた描画: (条, 本文) の列。e-Gov のリビジョンとの一致はこれで見る
    pub fn render(&self) -> Vec<(String, String)> {
        self.nodes
            .iter()
            .map(|n| (n.art.clone(), n.text.clone()))
            .collect()
    }
}

pub fn touches(u: &[IdentOp]) -> Vec<&str> {
    u.iter().flat_map(|op| op.touches()).collect()
}

pub fn creates(u: &[IdentOp]) -> Vec<&str> {
    u.iter().flat_map(|op| op.creates()).collect()
}

/// 独立: 触る id が交わらない
pub fn independent_units(u: &[IdentOp], v: &[IdentOp]) -> bool {
    let tv = touches(v);
    touches(u).iter().all(|i| !tv.contains(i))
}

/// b は a に依存する: a が作った id を b が触る
pub fn depends_on(b: &[IdentOp], a: &[IdentOp]) -> bool {
    let ca = creates(a);
    touches(b).iter().any(|i| ca.contains(i))
}

/// 施行順序が依存の半順序の線形拡張か
pub fn schedule_ok(units: &[&[IdentOp]]) -> bool {
    units
        .iter()
        .enumerate()
        .all(|(i, u)| units[i + 1..].iter().all(|v| !depends_on(u, v)))
}

// ---------------------------------------------------------------- Source IR → IdentRevision

const ID_ATTR: &str = "lawean-id";

fn id_of(p: &Paragraph) -> String {
    p.attrs
        .iter()
        .find(|(k, _)| k == ID_ATTR)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| p.stable_id.0.clone())
}

fn nodes_of(ps: &[Provision], out: &mut Vec<Node>) {
    for p in ps {
        match p {
            Provision::Container(c) => nodes_of(&c.children, out),
            Provision::Article(a) => {
                for p in paragraphs(a) {
                    out.push(Node {
                        id: id_of(p),
                        art: a.num.to_num_string(),
                        text: para_text(p),
                        conflicts: vec![],
                    });
                }
            }
            Provision::Paragraph(p) => out.push(Node {
                id: id_of(p),
                art: "main".into(),
                text: para_text(p),
                conflicts: vec![],
            }),
            Provision::Raw(_) => {}
        }
    }
}

/// 本則を id ベースのリビジョンにする。id は項の stable_id（束縛の途中では付けた印）、本文は `para_text`
pub fn from_document(doc: &LegalDocument) -> IdentRevision {
    let mut nodes = Vec::new();
    if let Some(t) = toc_text(doc) {
        nodes.push(Node {
            id: TOC_ID.into(),
            art: TOC_ID.into(),
            text: t,
            conflicts: vec![],
        });
    }
    nodes_of(&doc.main_provision, &mut nodes);
    IdentRevision { nodes }
}

/// 2 つのリビジョン（起草時の版と施行時の版）の差を、起草時の版の id で書いた操作列にする。
/// 改め文が手元に無い先行改正 B を、X と同じ id の世界に持ち込むためのもの（docs/13）。
/// 対応は条ごとに本文が同じ項どうし、残りは本文の近い項どうし（順序を保つ）。
/// 本文の違う項は `replace`、対応の無い新しい項は直前の項の後ろに `insertAfter`、消えた項は `delete`、条番号の違いは `renumber`。
/// 当てた結果の `render` は施行時の版と一致する（テスト `derived_unit_reproduces_revision`）
pub fn derive_unit(draft: &IdentRevision, enf: &IdentRevision, amend_id: &str) -> Vec<IdentOp> {
    fn sim(a: &str, b: &str) -> f64 {
        let ga: Vec<(char, char)> = a
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[0], w[1]))
            .collect();
        let gb: Vec<(char, char)> = b
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[0], w[1]))
            .collect();
        if ga.is_empty() || gb.is_empty() {
            return if a == b { 1.0 } else { 0.0 };
        }
        let mut counts: BTreeMap<(char, char), i64> = BTreeMap::new();
        for g in &ga {
            *counts.entry(*g).or_default() += 1;
        }
        let mut common = 0i64;
        for g in &gb {
            if let Some(c) = counts.get_mut(g) {
                if *c > 0 {
                    *c -= 1;
                    common += 1;
                }
            }
        }
        2.0 * common as f64 / (ga.len() + gb.len()) as f64
    }
    // 条（art）ごとに、施行時の項 → 起草時の項（index）
    let n_e = enf.nodes.len();
    let mut e_to_d: Vec<Option<usize>> = vec![None; n_e];
    let mut used_d = vec![false; draft.nodes.len()];
    // 同じ本文（同じ条の中で）
    for (ei, en) in enf.nodes.iter().enumerate() {
        if let Some(di) = draft
            .nodes
            .iter()
            .enumerate()
            .position(|(di, dn)| !used_d[di] && dn.art == en.art && dn.text == en.text)
        {
            used_d[di] = true;
            e_to_d[ei] = Some(di);
        }
    }
    // 残りは近い本文（同じ条、順序を保つ）
    for (ei, en) in enf.nodes.iter().enumerate() {
        if e_to_d[ei].is_some() {
            continue;
        }
        let lo = (0..ei)
            .rev()
            .find_map(|k| e_to_d[k])
            .map(|d| d + 1)
            .unwrap_or(0);
        let hi = (ei + 1..n_e)
            .find_map(|k| e_to_d[k])
            .unwrap_or(draft.nodes.len());
        let best = (lo..hi)
            .filter(|di| !used_d[*di] && draft.nodes[*di].art == en.art)
            .map(|di| (di, sim(&draft.nodes[di].text, &en.text)))
            .filter(|(_, s)| *s >= 0.6)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        if let Some((di, _)) = best {
            used_d[di] = true;
            e_to_d[ei] = Some(di);
        }
    }
    // 条番号の違い（条ずれ）も拾う: 本文が同じで条だけ違う項
    for (ei, en) in enf.nodes.iter().enumerate() {
        if e_to_d[ei].is_some() {
            continue;
        }
        if let Some(di) = draft
            .nodes
            .iter()
            .enumerate()
            .position(|(di, dn)| !used_d[di] && dn.text == en.text)
        {
            used_d[di] = true;
            e_to_d[ei] = Some(di);
        }
    }
    let mut ops = Vec::new();
    let mut prev: Option<String> = None;
    let mut k = 0u32;
    for (ei, en) in enf.nodes.iter().enumerate() {
        match e_to_d[ei] {
            Some(di) => {
                let dn = &draft.nodes[di];
                if dn.text != en.text {
                    ops.push(IdentOp::Replace {
                        id: dn.id.clone(),
                        expected: dn.text.clone(),
                        new: en.text.clone(),
                    });
                }
                if dn.art != en.art {
                    ops.push(IdentOp::Renumber {
                        id: dn.id.clone(),
                        art: en.art.clone(),
                    });
                }
                prev = Some(dn.id.clone());
            }
            None => {
                k += 1;
                let new_id = format!("{amend_id}/art:{}/new:{k}", en.art);
                let anchor = match &prev {
                    Some(p) => p.clone(),
                    None => continue, // 先頭に加える形は無い（目次があるので実際は起きない）
                };
                ops.push(IdentOp::InsertAfter {
                    anchor,
                    new_id: new_id.clone(),
                    art: en.art.clone(),
                    text: en.text.clone(),
                });
                prev = Some(new_id);
            }
        }
    }
    for (di, dn) in draft.nodes.iter().enumerate() {
        if !used_d[di] {
            ops.push(IdentOp::Delete { id: dn.id.clone() });
        }
    }
    ops
}

/// 改正法が振った id と e-Gov の id の対応。束縛した操作を発射台に当てた結果と、e-Gov の改正後リビジョンを
/// 文書順で突き合わせる（`render` が一致していることが前提）。返すのは (当てた結果の id, e-Gov の id)
pub fn id_map(
    base: &IdentRevision,
    ops: &[IdentOp],
    egov_after: &IdentRevision,
) -> Option<Vec<(String, String)>> {
    let got = apply_unit(base, ops)?;
    if got.render() != egov_after.render() {
        return None;
    }
    Some(
        got.nodes
            .iter()
            .zip(&egov_after.nodes)
            .map(|(a, b)| (a.id.clone(), b.id.clone()))
            .collect(),
    )
}

fn to_egov_ids(map: &[(String, String)], ids: Vec<&str>) -> Vec<String> {
    let mut out = Vec::new();
    for t in ids {
        match map.iter().find(|(a, _)| a == t) {
            Some((_, e)) => out.push(e.clone()),
            None => out.push(t.to_string()),
        }
    }
    out.dedup();
    out
}

/// 改正単位が触った項（anchor を含む）を e-Gov の改正後リビジョンの id で返す（削った項は発射台の id のまま）。
/// Semantic IR の Rule（`provenance.source` は e-Gov の id）と突き合わせるためのもの
pub fn touched_egov_ids(
    base: &IdentRevision,
    ops: &[IdentOp],
    egov_after: &IdentRevision,
) -> Option<Vec<String>> {
    let map = id_map(base, ops, egov_after)?;
    Some(to_egov_ids(&map, touches(ops)))
}

/// 改正単位が本文を変えた・作った・消した項（anchor は含まない）を e-Gov の id で返す。
/// 触っただけの anchor は本文が変わらないので、その項の Rule はそのまま（frame 定理の `Sub` が成り立つ）
pub fn modified_egov_ids(
    base: &IdentRevision,
    ops: &[IdentOp],
    egov_after: &IdentRevision,
) -> Option<Vec<String>> {
    let map = id_map(base, ops, egov_after)?;
    Some(to_egov_ids(
        &map,
        ops.iter().flat_map(|op| op.modifies()).collect(),
    ))
}

// ---------------------------------------------------------------- 束縛

/// 項に id の印を付ける。すでに印があれば（前の改正単位が束縛した文書）そのまま
fn tag(p: &mut Paragraph) {
    if !p.attrs.iter().any(|(k, _)| k == ID_ATTR) {
        let id = p.stable_id.0.clone();
        p.attrs.push((ID_ATTR.into(), id));
    }
}

fn tag_all(ps: &mut [Provision]) {
    for p in ps {
        match p {
            Provision::Container(c) => tag_all(&mut c.children),
            Provision::Article(a) => {
                for c in &mut a.children {
                    if let ArticleChild::Paragraph(p) = c {
                        tag(p);
                    }
                }
            }
            Provision::Paragraph(p) => tag(p),
            Provision::Raw(_) => {}
        }
    }
}

fn last_para_id_in(ps: &[Provision]) -> Option<String> {
    ps.iter().rev().find_map(|p| match p {
        Provision::Container(c) => last_para_id_in(&c.children),
        Provision::Article(a) => paragraphs(a).last().map(|p| id_of(p)),
        Provision::Paragraph(p) => Some(id_of(p)),
        Provision::Raw(_) => None,
    })
}

struct Binder<'a> {
    doc: LegalDocument,
    amend_id: &'a str,
    /// 条ごとに、この改正単位の中で作った項の数（新しい id の連番）
    created: BTreeMap<String, u32>,
    ops: Vec<IdentOp>,
}

impl Binder<'_> {
    fn new_id(&mut self, art: &ArticleNum) -> String {
        let k = self.created.entry(art.to_num_string()).or_insert(0);
        *k += 1;
        format!("{}/art:{}/new:{}", self.amend_id, art.to_num_string(), k)
    }

    fn new_para(&mut self, art: &ArticleNum, mut p: Paragraph) -> (String, Paragraph) {
        let id = self.new_id(art);
        p.attrs.push((ID_ATTR.into(), id.clone()));
        (id, p)
    }

    /// 条（idx=None）または項の中で from を含む項ごとに、本文全体の Replace を出す
    fn replace(
        &mut self,
        at: &Loc,
        from: &str,
        to: &str,
        snapshots: &mut BTreeMap<String, Vec<Option<u32>>>,
    ) -> Result<(), ApplyError> {
        let art = article_mut(&mut self.doc, &at.article)?;
        let idx = para_index(art, &at.paragraph, snapshots)?;
        let mut hit = 0;
        for i in 0..paragraphs(art).len() {
            if idx.is_some_and(|j| j != i) {
                continue;
            }
            let expected = para_text(paragraph_mut(art, i));
            if replace_in_article_item(art, Some(i), at.item.as_deref(), from, to) == 0 {
                continue;
            }
            hit += 1;
            let p = paragraph_mut(art, i);
            self.ops.push(IdentOp::Replace {
                id: id_of(p),
                expected,
                new: para_text(p),
            });
        }
        if hit == 0 {
            return Err(ApplyError::PhraseNotFound {
                at: loc_name(at),
                phrase: from.into(),
            });
        }
        Ok(())
    }

    fn instruction(&mut self, ins: &Instruction) -> Result<(), ApplyError> {
        let mut snapshots: BTreeMap<String, Vec<Option<u32>>> = BTreeMap::new();
        for op in &ins.ops {
            match op {
                Op::ReplaceToc { from, to } => {
                    let expected = toc_text(&self.doc).unwrap_or_default();
                    replace_toc(&mut self.doc, from, to)?;
                    self.ops.push(IdentOp::Replace {
                        id: TOC_ID.into(),
                        expected,
                        new: toc_text(&self.doc).unwrap_or_default(),
                    });
                }
                Op::Replace { at, from, to } => {
                    for at in crate::apply::expand_range(&self.doc, at) {
                        self.replace(&at, from, to, &mut snapshots)?
                    }
                }
                Op::InsertAfterPhrase { at, anchor, text } => {
                    self.replace(at, anchor, &format!("{anchor}{text}"), &mut snapshots)?
                }
                Op::AppendParagraph { article, text } => {
                    for p in parse_paragraphs(text)? {
                        let (id, p) = self.new_para(article, p);
                        let art = article_mut(&mut self.doc, article)?;
                        snapshot(art, &mut snapshots);
                        let anchor = paragraphs(art)
                            .last()
                            .map(|p| id_of(p))
                            .ok_or_else(|| ApplyError::BadContent("項の無い条に加える".into()))?;
                        self.ops.push(IdentOp::InsertAfter {
                            anchor,
                            new_id: id,
                            art: article.to_num_string(),
                            text: para_text(&p),
                        });
                        art.children.push(ArticleChild::Paragraph(p));
                        snapshots
                            .get_mut(&article.to_num_string())
                            .unwrap()
                            .push(None);
                    }
                }
                Op::InsertParagraphAfter {
                    article,
                    after,
                    text,
                } => {
                    for (k, p) in parse_paragraphs(text)?.into_iter().enumerate() {
                        let (id, p) = self.new_para(article, p);
                        let art = article_mut(&mut self.doc, article)?;
                        let idx =
                            para_index(art, &Some(after.clone()), &mut snapshots)?.unwrap() + k;
                        let anchor = id_of(paragraph_mut(art, idx));
                        self.ops.push(IdentOp::InsertAfter {
                            anchor,
                            new_id: id,
                            art: article.to_num_string(),
                            text: para_text(&p),
                        });
                        let pos = nth_paragraph_child(art, idx) + 1;
                        art.children.insert(pos, ArticleChild::Paragraph(p));
                        snapshots
                            .get_mut(&article.to_num_string())
                            .unwrap()
                            .insert(idx + 1, None);
                    }
                }
                // 番号だけを動かす操作。id の世界では何もしない（番号は描画時に数える）
                Op::RenumberParagraph { article, from, to } => {
                    let art = article_mut(&mut self.doc, article)?;
                    let idx = para_index(art, &Some(from.clone()), &mut snapshots)?.unwrap();
                    set_label(paragraph_mut(art, idx), *to);
                }
                Op::ShiftParagraphs {
                    article,
                    from,
                    to,
                    by,
                } => {
                    let art = article_mut(&mut self.doc, article)?;
                    snapshot(art, &mut snapshots);
                    let snap = snapshots[&article.to_num_string()].clone();
                    for (i, orig) in snap.iter().enumerate() {
                        if let Some(n) = orig {
                            if *n >= *from && *n <= *to {
                                set_label(paragraph_mut(art, i), (*n as i32 + by) as u32);
                            }
                        }
                    }
                }
                Op::AppendArticle { chapter, text } => {
                    let a = parse_article(text)?;
                    let ch = chapter_mut(&mut self.doc, *chapter)?;
                    let mut anchor = last_para_id_in(&ch.children)
                        .ok_or(ApplyError::ChapterNotFound(*chapter))?;
                    let mut a = a;
                    let mut children = Vec::new();
                    for c in std::mem::take(&mut a.children) {
                        let ArticleChild::Paragraph(p) = c else {
                            children.push(c);
                            continue;
                        };
                        let (id, p) = self.new_para(&a.num, p);
                        self.ops.push(IdentOp::InsertAfter {
                            anchor: anchor.clone(),
                            new_id: id.clone(),
                            art: a.num.to_num_string(),
                            text: para_text(&p),
                        });
                        anchor = id;
                        children.push(ArticleChild::Paragraph(p));
                    }
                    a.children = children;
                    chapter_mut(&mut self.doc, *chapter)?
                        .children
                        .push(Provision::Article(a));
                }
                Op::InsertChapterAfter { after, text } => {
                    // 章の挿入 = 直前の章の最後の項の後ろに、新しい章の全部の項を順に並べる
                    let ch = crate::apply::parse_chapter(text)?;
                    let prev = chapter_mut(&mut self.doc, *after)?;
                    let mut anchor = last_para_id_in(&prev.children)
                        .ok_or(ApplyError::ChapterNotFound(*after))?;
                    fn walk(b: &mut Binder<'_>, ps: &mut [Provision], anchor: &mut String) {
                        for p in ps {
                            match p {
                                Provision::Container(c) => walk(b, &mut c.children, anchor),
                                Provision::Article(a) => {
                                    let mut children = Vec::new();
                                    for c in std::mem::take(&mut a.children) {
                                        let ArticleChild::Paragraph(p) = c else {
                                            children.push(c);
                                            continue;
                                        };
                                        let (id, p) = b.new_para(&a.num, p);
                                        b.ops.push(IdentOp::InsertAfter {
                                            anchor: anchor.clone(),
                                            new_id: id.clone(),
                                            art: a.num.to_num_string(),
                                            text: para_text(&p),
                                        });
                                        *anchor = id;
                                        children.push(ArticleChild::Paragraph(p));
                                    }
                                    a.children = children;
                                }
                                _ => {}
                            }
                        }
                    }
                    let mut ch = ch;
                    let mut inner = std::mem::take(&mut ch.children);
                    walk(self, &mut inner, &mut anchor);
                    ch.children = inner;
                    let pos = self
                        .doc
                        .main_provision
                        .iter()
                        .position(|p| matches!(p, Provision::Container(c) if c.kind == ContainerKind::Chapter && c.num.as_deref() == Some(&after.to_string())))
                        .ok_or(ApplyError::ChapterNotFound(*after))?;
                    self.doc
                        .main_provision
                        .insert(pos + 1, Provision::Container(ch));
                }
                // 章の番号だけ。id の世界では何もしない
                Op::RenumberChapter { from, to } => {
                    let ch = chapter_mut(&mut self.doc, *from)?;
                    ch.num = Some(to.to_string());
                }
                Op::InsertArticleAfter { after, text } => {
                    // 条の挿入 = 直前の条の最後の項の後ろに新しい項を並べる（「次の二条」なら順に）
                    let mut anchor = paragraphs(article_mut(&mut self.doc, after)?)
                        .last()
                        .map(|p| id_of(p))
                        .ok_or_else(|| ApplyError::BadContent("項の無い条の次に加える".into()))?;
                    let mut prev_art = after.clone();
                    for mut a in crate::apply::parse_articles(text)? {
                        let mut children = Vec::new();
                        for c in std::mem::take(&mut a.children) {
                            let ArticleChild::Paragraph(p) = c else {
                                children.push(c);
                                continue;
                            };
                            let (id, p) = self.new_para(&a.num, p);
                            self.ops.push(IdentOp::InsertAfter {
                                anchor: anchor.clone(),
                                new_id: id.clone(),
                                art: a.num.to_num_string(),
                                text: para_text(&p),
                            });
                            anchor = id;
                            children.push(ArticleChild::Paragraph(p));
                        }
                        a.children = children;
                        let num = a.num.clone();
                        if !insert_article_after(&mut self.doc.main_provision, &prev_art, a) {
                            return Err(ApplyError::ArticleNotFound(prev_art.to_num_string()));
                        }
                        prev_art = num;
                    }
                }
                Op::RenumberArticle { from, to } => {
                    // 条ずれ = その条の全項の art を付け替える。id は変わらない
                    let ids: Vec<String> = paragraphs(article_mut(&mut self.doc, from)?)
                        .iter()
                        .map(|p| id_of(p))
                        .collect();
                    for id in ids {
                        self.ops.push(IdentOp::Renumber {
                            id,
                            art: to.to_num_string(),
                        });
                    }
                    crate::apply::renumber_article(&mut self.doc, from, to)?;
                    if let Some(v) = snapshots.remove(&from.to_num_string()) {
                        snapshots.insert(to.to_num_string(), v);
                    }
                }
                Op::ShiftArticles { from, to, by } => {
                    let mut nums: Vec<ArticleNum> = crate::numbering::article_nums(&self.doc)
                        .into_iter()
                        .filter(|n| matches!(n, ArticleNum::Single { base, .. } if *base >= *from && *base <= *to))
                        .collect();
                    if *by > 0 {
                        nums.reverse();
                    }
                    for n in nums {
                        let ArticleNum::Single { base, branch } = &n else {
                            continue;
                        };
                        let target = ArticleNum::Single {
                            base: (*base as i32 + by) as u32,
                            branch: branch.clone(),
                        };
                        let ids: Vec<String> = paragraphs(article_mut(&mut self.doc, &n)?)
                            .iter()
                            .map(|p| id_of(p))
                            .collect();
                        for id in ids {
                            self.ops.push(IdentOp::Renumber {
                                id,
                                art: target.to_num_string(),
                            });
                        }
                        crate::apply::renumber_article(&mut self.doc, &n, &target)?;
                        if let Some(v) = snapshots.remove(&n.to_num_string()) {
                            snapshots.insert(target.to_num_string(), v);
                        }
                    }
                }
                // 見出しは項の本文ではないので id 操作は無い（文書の側だけ）
                Op::ReplaceCaption { article, from, to } => {
                    let art = article_mut(&mut self.doc, article)?;
                    let cur = art
                        .caption
                        .as_ref()
                        .map(|c| inline_text(c))
                        .unwrap_or_default();
                    if !cur.contains(from.as_str()) {
                        return Err(ApplyError::PhraseNotFound {
                            at: format!("{}の見出し", crate::apply::article_label(article)),
                            phrase: from.clone(),
                        });
                    }
                    art.caption = Some(vec![Inline::Text(cur.replace(from.as_str(), to))]);
                }
                Op::SetCaption { article, text } => {
                    let art = article_mut(&mut self.doc, article)?;
                    art.caption = Some(vec![Inline::Text(text.clone())]);
                }
                Op::ReplaceSentencePart { at, part, text } => {
                    let art = article_mut(&mut self.doc, &at.article)?;
                    let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                    let p = paragraph_mut(art, idx);
                    let expected = para_text(p);
                    let first = text.first().cloned().unwrap_or_default();
                    crate::apply::replace_sentence_part(p, *part, &first)?;
                    self.ops.push(IdentOp::Replace {
                        id: id_of(p),
                        expected,
                        new: para_text(p),
                    });
                }
                Op::AppendSentence { at, text } => {
                    // 後段の追加 = 項の本文の書き換え
                    let art = article_mut(&mut self.doc, &at.article)?;
                    let idx = para_index(art, &at.paragraph, &mut snapshots)?.unwrap_or(0);
                    let expected = para_text(paragraph_mut(art, idx));
                    let p = paragraph_mut(art, idx);
                    let n = p.sentences.len();
                    for (k, s) in make_sentences(&text.join("")).into_iter().enumerate() {
                        let mut s = s;
                        s.num = Some((n + k + 1).to_string());
                        p.sentences.push(s);
                    }
                    let p = paragraph_mut(art, idx);
                    self.ops.push(IdentOp::Replace {
                        id: id_of(p),
                        expected,
                        new: para_text(p),
                    });
                }
                Op::ReplaceArticle { article, text } => {
                    // 全部改正 = 旧第1項の後ろに新しい項を並べてから、旧 id を削る。
                    // 隣の条ではなく旧第1項に anchor するので、隣の条を触る改正単位とは独立のまま
                    let a = parse_article(text)?;
                    let old_ids: Vec<String> = paragraphs(article_mut(&mut self.doc, article)?)
                        .iter()
                        .map(|p| id_of(p))
                        .collect();
                    let mut anchor = old_ids
                        .first()
                        .cloned()
                        .ok_or_else(|| ApplyError::BadContent("項の無い条を改める".into()))?;
                    let mut a = a;
                    let mut children = Vec::new();
                    for c in std::mem::take(&mut a.children) {
                        let ArticleChild::Paragraph(p) = c else {
                            children.push(c);
                            continue;
                        };
                        let (id, p) = self.new_para(article, p);
                        self.ops.push(IdentOp::InsertAfter {
                            anchor: anchor.clone(),
                            new_id: id.clone(),
                            art: article.to_num_string(),
                            text: para_text(&p),
                        });
                        anchor = id;
                        children.push(ArticleChild::Paragraph(p));
                    }
                    for id in old_ids {
                        self.ops.push(IdentOp::Delete { id });
                    }
                    let art = article_mut(&mut self.doc, article)?;
                    art.caption = a.caption;
                    art.title = a.title;
                    art.children = children;
                }
                Op::Delete { at } => {
                    let art = article_mut(&mut self.doc, &at.article)?;
                    match para_index(art, &at.paragraph, &mut snapshots)? {
                        Some(idx) => {
                            let id = id_of(paragraph_mut(art, idx));
                            self.ops.push(IdentOp::Delete { id });
                            let pos = nth_paragraph_child(art, idx);
                            art.children.remove(pos);
                            snapshots
                                .get_mut(&at.article.to_num_string())
                                .unwrap()
                                .remove(idx);
                        }
                        None => {
                            for p in paragraphs(art) {
                                self.ops.push(IdentOp::Delete { id: id_of(p) });
                            }
                            remove_article(&mut self.doc.main_provision, &at.article);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// 束縛の結果
#[derive(Debug, Clone)]
pub struct Bound {
    /// id ベースの操作列
    pub ops: Vec<IdentOp>,
    /// 操作を当てた後の Source IR。項に id の印が付いたままなので、次の改正単位の発射台にそのまま使える
    /// （`apply_unit` のように再パースして stable_id を振り直すと、この単位が作った id が消える）
    pub doc: LegalDocument,
}

/// 改正単位を発射台リビジョンに対して束縛し、id ベースの操作列にする。
/// `amend_id` は改正法 ID と条（例 `503AC0000000037/art35`）。新しい項の id はこれと位置から振る。
/// 番号の解決に失敗すれば発射台の不一致がその場で出る（`apply_unit` と同じエラー）
pub fn bind(base: &LegalDocument, unit: &AmendUnit, amend_id: &str) -> Result<Bound, ApplyError> {
    let mut doc = base.clone();
    tag_all(&mut doc.main_provision);
    let mut b = Binder {
        doc,
        amend_id,
        created: BTreeMap::new(),
        ops: Vec::new(),
    };
    for ins in &unit.instructions {
        b.instruction(ins)?;
    }
    check_numbering(&b.doc)?;
    Ok(Bound {
        ops: b.ops,
        doc: b.doc,
    })
}
