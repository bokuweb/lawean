//! 参照を id で持つ本文（`Refs.lean` の写し）。
//!
//! 改め文が加える本文の「第二十八条第五項」は字面の番号で、先行改正が第28条に項を挿すと意味がずれる（docs/13 §4）。
//! ここでは本文を「平文の断片」と「id で指す参照」の列（`Body`）にし、番号は描画時に数える（`render_body`）。
//! id は項の挿入・削除で動かないので、参照は壊れない。改正前後で描画が違う項が、そのまま手当て（`hane_fixes` = Lean `haneFixes`）。
//!
//! 参照の解決は本文を書いた時点のリビジョン（起草時の発射台に改正法を当てたもの）で行い、描画は施行時のリビジョンで行う。

use crate::ident::{IdentRevision, Node};
use lawean_resolve::numeral::to_kanji;
use lawean_resolve::{find_references, RefKind};
use lawean_source::ArticleNum;

/// 参照の書き方（`Refs.lean` の `RefForm`）。相対形は元の距離を持つ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefForm {
    /// 「第三項」（同じ条の項）
    Absolute,
    /// 「第百四十二条の四第六項」（他の条の項）
    AbsoluteArt,
    /// 「前項」「前二項」
    Prev(u32),
    /// 「次項」
    Next,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Ref { target: String, form: RefForm },
}

pub type Body = Vec<Piece>;

fn node<'a>(r: &'a IdentRevision, id: &str) -> Option<&'a Node> {
    r.nodes.iter().find(|n| n.id == id)
}

/// 条 `art` の `p` 番目（1 始まり）の項の id
fn nth_in_art(r: &IdentRevision, art: &str, p: u32) -> Option<String> {
    r.nodes
        .iter()
        .filter(|n| n.art == art)
        .nth(p.checked_sub(1)? as usize)
        .map(|n| n.id.clone())
}

/// 本文 `text`（id `src` の項の描画済みの本文）を、リビジョン `r` で参照を id に解決した `Body` にする。
/// 解決できない参照（他法令、条だけの参照、削除された項）は平文のまま
pub fn body_of(r: &IdentRevision, src: &str, text: &str) -> Body {
    let Some(sn) = node(r, src) else {
        return vec![Piece::Text(text.to_string())];
    };
    let src_art = sn.art.clone();
    let src_idx = r
        .nodes
        .iter()
        .filter(|n| n.art == src_art)
        .position(|n| n.id == src)
        .map(|i| i as u32 + 1)
        .unwrap_or(0);
    let mut out: Body = Vec::new();
    let mut pos = 0usize;
    // 「第二十八条第一項から第四項まで及び第六項」の「第四項」は直前の条に続く
    let mut ante_art: Option<String> = None;
    let mut prev_end = 0usize;
    let push_text = |out: &mut Body, s: &str| {
        if s.is_empty() {
            return;
        }
        if let Some(Piece::Text(t)) = out.last_mut() {
            t.push_str(s);
        } else {
            out.push(Piece::Text(s.to_string()));
        }
    };
    for sp in find_references(text) {
        let gap = &text[prev_end.min(sp.start)..sp.start];
        let connective = !gap.is_empty()
            && gap
                .replace("から", "")
                .replace("まで", "")
                .replace("及び", "")
                .replace("並びに", "")
                .replace("若しくは", "")
                .replace("又は", "")
                .replace(['、', '（', '）'], "")
                .is_empty();
        prev_end = sp.end;
        let resolved: Option<(String, RefForm)> = match (&sp.parsed.kind, sp.parsed.paragraph) {
            (RefKind::Article { suppl: false, num }, Some(p)) => {
                let art = num.to_num_string();
                ante_art = Some(art.clone());
                // 条を書いている参照は同じ条の項でも条つきで描く（読替え規定の「第百八十一条第一項」）
                nth_in_art(r, &art, p).map(|id| (id, RefForm::AbsoluteArt))
            }
            (RefKind::Article { suppl: false, num }, None) => {
                ante_art = Some(num.to_num_string());
                None
            }
            (RefKind::Paragraph(p), _) => {
                let art = if connective {
                    ante_art.clone().unwrap_or_else(|| src_art.clone())
                } else {
                    src_art.clone()
                };
                nth_in_art(r, &art, *p).map(|id| (id, RefForm::Absolute))
            }
            (RefKind::PrevParagraph(k), _) if src_idx > *k => {
                nth_in_art(r, &src_art, src_idx - k).map(|id| (id, RefForm::Prev(*k)))
            }
            (RefKind::NextParagraph, _) => {
                nth_in_art(r, &src_art, src_idx + 1).map(|id| (id, RefForm::Next))
            }
            _ => {
                ante_art = None;
                None
            }
        };
        if let Some((target, form)) = resolved {
            // 参照の字句のうち番号を描く部分は「…項」まで。続く「第三号」「前段」「ただし書」は平文として残す
            let core_end = sp
                .text
                .rfind('項')
                .map(|i| i + '項'.len_utf8())
                .unwrap_or(sp.text.len());
            push_text(&mut out, &text[pos..sp.start]);
            out.push(Piece::Ref { target, form });
            push_text(&mut out, &sp.text[core_end..]);
            pos = sp.end;
        }
    }
    push_text(&mut out, &text[pos..]);
    out
}

/// 項の番号（条の中で何番目か、1 始まり）。Lean `paraNum`
pub fn para_num(r: &IdentRevision, id: &str) -> Option<u32> {
    let n = node(r, id)?;
    r.nodes
        .iter()
        .filter(|x| x.art == n.art)
        .position(|x| x.id == id)
        .map(|i| i as u32 + 1)
}

/// Lean `renderRef`: 相対形は距離が元と同じならそのまま、1 なら「前項」「次項」、それ以外は絶対形
fn render_ref(form: &RefForm, t: Option<u32>, s: Option<u32>) -> String {
    let Some(t) = t else {
        return "（削除された項）".into();
    };
    match (form, s) {
        (RefForm::Prev(k), Some(s)) => {
            if s == t + k {
                if *k == 1 {
                    "前項".into()
                } else {
                    format!("前{}項", to_kanji(*k))
                }
            } else if s == t + 1 {
                "前項".into()
            } else {
                format!("第{}項", to_kanji(t))
            }
        }
        (RefForm::Next, Some(s)) if t == s + 1 => "次項".into(),
        _ => format!("第{}項", to_kanji(t)),
    }
}

/// Lean `renderPiece`
pub fn render_piece(r: &IdentRevision, src: &str, p: &Piece) -> String {
    match p {
        Piece::Text(s) => s.clone(),
        Piece::Ref {
            target,
            form: RefForm::AbsoluteArt,
        } => match (node(r, target), para_num(r, target)) {
            (Some(n), Some(p)) => format!(
                "{}第{}項",
                crate::apply::article_label(&ArticleNum::parse(&n.art)),
                to_kanji(p)
            ),
            _ => "（削除された規定）".into(),
        },
        Piece::Ref { target, form } => {
            let same_art = match (node(r, src), node(r, target)) {
                (Some(a), Some(b)) => a.art == b.art,
                _ => false,
            };
            let s = if same_art { para_num(r, src) } else { None };
            render_ref(form, para_num(r, target), s)
        }
    }
}

/// Lean `renderBody`
pub fn render_body(r: &IdentRevision, src: &str, body: &Body) -> String {
    body.iter().map(|p| render_piece(r, src, p)).collect()
}

/// 断片ごとの描画に法制執務の慣行を 1 つ足したもの（Lean の写しの外。手当ての字句を実際の改め文に近づける）:
/// 相対形（「前項」）が絶対形に変わった直後に、同じ項を指す相対形がまた絶対形に変わるなら「同項」と書く
/// （令3-37 第24条: 第61条第11項の「前項に」→「第十一項に」、「前項の」→「同項の」）
pub fn render_pieces_styled(r: &IdentRevision, src: &str, body: &Body) -> Vec<String> {
    let mut out = Vec::with_capacity(body.len());
    let mut prev_converted: Option<&str> = None; // 直前に相対形→絶対形になった参照の id
    for p in body {
        let rendered = render_piece(r, src, p);
        match p {
            Piece::Ref {
                target,
                form: RefForm::Prev(_) | RefForm::Next,
            } => {
                let converted = rendered.starts_with('第');
                if converted && prev_converted == Some(target.as_str()) {
                    out.push("同項".into());
                } else {
                    out.push(rendered);
                }
                prev_converted = converted.then_some(target.as_str());
            }
            Piece::Ref { .. } => {
                prev_converted = None;
                out.push(rendered);
            }
            Piece::Text(_) => out.push(rendered),
        }
    }
    out
}

/// Lean `haneFixes`: 改正の前後（`r` → `r2`）で描画が変わる項の (id, 改正前の描画, 改正後の描画)
pub fn hane_fixes(
    r: &IdentRevision,
    r2: &IdentRevision,
    bodies: &[(String, Body)],
) -> Vec<(String, String, String)> {
    bodies
        .iter()
        .filter_map(|(id, body)| {
            let before = render_body(r, id, body);
            let after = render_body(r2, id, body);
            (before != after).then(|| (id.clone(), before, after))
        })
        .collect()
}

/// 描画が変わった参照だけ: (id, 改正前の字句, 改正後の字句)
pub fn changed_refs(
    r: &IdentRevision,
    r2: &IdentRevision,
    bodies: &[(String, Body)],
) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for (id, body) in bodies {
        let before = render_pieces_styled(r, id, body);
        let after = render_pieces_styled(r2, id, body);
        for (p, (a, b)) in body.iter().zip(before.iter().zip(after.iter())) {
            if matches!(p, Piece::Ref { .. }) && a != b {
                out.push((id.clone(), a.clone(), b.clone()));
            }
        }
    }
    out
}
