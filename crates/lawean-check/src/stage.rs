//! 施行日ごとの分割: 附則第一条の号が 1 つの改正単位（改正法の条）の一部の改正規定だけを別の日に施行するとき、
//! 単位を「その日に施行する文」ごとに分ける（令3-49 第6条「第六条の規定（医師法第十六条の十一第一項の改正規定を除く。）」）。
//!
//! 号の範囲欄の形:
//! - 「第六条の規定」「第五条及び第六条の規定」: 単位全体
//! - 「第三条中医療法第三十五条第一項第二号の改正規定」「第六条の規定（…の改正規定に限る。）」: 挙げた改正規定だけ
//! - 「第六条の規定（…の改正規定を除く。）」: 挙げた改正規定以外
//! - 被改正法の条だけで書く「第三十四条の二第一項の改正規定」（単独法の改正法）: 挙げた改正規定だけ
//!
//! 号は上から順に取り、残った文は本文の日。

use lawean_amend::{parse_scope_locs, AmendUnit, Loc};
use lawean_extract::calendar::Date;
use lawean_extract::suppl::{
    admissible, kanji_num, scope_articles, scope_is_target_side, EnforcementClause,
    EnforcementItem, EnforcementSpec,
};
use regex::Regex;
use std::sync::OnceLock;

/// 単位のうち同じ施行期日で施行される部分
#[derive(Debug, Clone)]
pub struct Part {
    pub unit: AmendUnit,
    pub clause: EnforcementClause,
    /// 号の範囲欄（本文なら None）
    pub scope: Option<String>,
}

/// 範囲欄の中の、改正法の条 `label`（「第六条」）に係る部分の読み。None ならこの号はこの条に触れていない
enum Mention {
    All,
    Only(Vec<Loc>),
    Except(Vec<Loc>),
}

fn strip_parens(s: &str) -> String {
    let mut depth = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '（' => depth += 1,
            '）' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// 「第六条中<X>」「第六条の規定（<X>に限る。）」の X（改正規定の列挙）を位置に読む
fn locs_of(x: &str) -> Vec<Loc> {
    parse_scope_locs(x).unwrap_or_default()
}

/// 「{label}中」に続く改正規定の列挙: 「改正規定」で終わる語を「、」「及び」でつないだ範囲まで（「並びに」で群が切れる）。
/// 法令名の中の「及び」（「地域における医療及び介護の…」）で切らないよう、「改正規定」の出現で区切る
fn after_naka(rest: &str) -> String {
    let mut g = rest.split("並びに").next().unwrap_or("");
    let mut taken: Vec<&str> = Vec::new();
    while let Some(k) = g.find("改正規定") {
        let end = k + "改正規定".len();
        let piece = g[..end].trim_start_matches("及び").trim_start_matches('、');
        if piece.is_empty() {
            break;
        }
        taken.push(piece);
        g = &g[end..];
        // 続きが「及び」「、」で始まらなければ列挙はここまで
        if !(g.starts_with("及び") || g.starts_with('、')) {
            break;
        }
    }
    taken.join("、")
}

fn mention(scope: &str, label: &str, art: u32, items: &[EnforcementItem]) -> Option<Mention> {
    // 「第六条中医師法第十六条の十一第一項の改正規定」
    if let Some((_, rest)) = scope.split_once(&format!("{label}中")) {
        let x = after_naka(rest);
        if !x.is_empty() {
            return Some(Mention::Only(locs_of(&x)));
        }
    }
    // 「第六条の規定（…に限る。）」「第六条の規定（…を除く。）」「第六条（…に限る。）」。
    // 「第十三条の規定（第四号に掲げる改正規定を除く。）」の「第N号に掲げる改正規定」は、その号がこの条について挙げる改正規定
    let resolve = |x: &str| -> Vec<Loc> {
        static ITEM_REF: OnceLock<Regex> = OnceLock::new();
        let r = ITEM_REF.get_or_init(|| {
            Regex::new(r"^第([一二三四五六七八九十]+)号に掲げる改正規定$").unwrap()
        });
        if let Some(c) = r.captures(x) {
            if let Some(it) = kanji_num(&c[1]).and_then(|n| items.get(n as usize - 1)) {
                return match mention(&it.scope, label, art, &[]) {
                    Some(Mention::Only(l)) => l,
                    _ => vec![],
                };
            }
        }
        locs_of(x)
    };
    for head in [format!("{label}の規定（"), format!("{label}（")] {
        if let Some((_, rest)) = scope.split_once(&head) {
            let inner = rest.split('）').next().unwrap_or("");
            if let Some(x) = inner.strip_suffix("に限る。") {
                return Some(Mention::Only(resolve(x)));
            }
            if let Some(x) = inner.strip_suffix("を除く。") {
                return Some(Mention::Except(resolve(x)));
            }
        }
    }
    // 列挙の中の改正法の条（本則）として挙がっていれば全体
    if scope_articles(scope)
        .iter()
        .any(|r| !r.suppl && r.from <= art && art <= r.to)
    {
        return Some(Mention::All);
    }
    None
}

fn label_article(label: &str) -> Option<u32> {
    if label.starts_with("附則") {
        return None;
    }
    label
        .trim_start_matches('第')
        .split('条')
        .next()
        .and_then(kanji_num)
}

/// 単位を施行期日ごとの部分に分ける。号に触れられていなければ本文の 1 つだけ。
/// 本文の施行期日が読めなくても命令は落とさず、`enforcement: None` の部分に残す（部分をつなぐと元の単位の部分列への分割になる）
pub fn parts_of(spec: &EnforcementSpec, label: &str, unit: &AmendUnit) -> Vec<Part> {
    let Some(art) = label_article(label) else {
        return vec![Part {
            unit: unit.clone(),
            clause: spec.main.clone().unwrap_or_else(unreadable),
            scope: None,
        }];
    };
    let mut remaining = unit.clone();
    let mut parts = Vec::new();
    for it in &spec.items {
        if remaining.instructions.is_empty() {
            break;
        }
        let plain = strip_parens(&it.scope);
        let m = match mention(&it.scope, label, art, &spec.items) {
            Some(m) => m,
            // 被改正法の条だけで書く範囲欄（単独法の改正法）: 挙げた改正規定だけ
            None if scope_is_target_side(&plain) && !plain.contains(label) => {
                let locs = locs_of(&plain);
                if locs.is_empty() {
                    continue;
                }
                Mention::Only(locs)
            }
            None => continue,
        };
        let (sel, rest) = match m {
            Mention::All => (remaining.clone(), empty_like(&remaining)),
            Mention::Only(locs) => remaining.split_by_locs(&locs),
            Mention::Except(locs) => {
                let (a, b) = remaining.split_by_locs(&locs);
                (b, a)
            }
        };
        if sel.instructions.is_empty() {
            continue;
        }
        parts.push(Part {
            unit: sel,
            clause: it.clause.clone(),
            scope: Some(it.scope.clone()),
        });
        remaining = rest;
    }
    if !remaining.instructions.is_empty() {
        parts.push(Part {
            unit: remaining,
            clause: spec.main.clone().unwrap_or_else(unreadable),
            scope: None,
        });
    }
    parts
}

/// 分けた部分のうち、施行日 `day` に当てる部分。区間が `day` を含む部分のうち、日が確定している部分（暦日・公布の日、
/// 区間の幅が 0）があればそれだけ（政令で定める日の区間が `day` を含むだけの部分は、別の日に政令で決まると見る）。
/// 返すのは `parts` と同じ長さの列（当てるか、当てない理由）
pub fn select_for_day(parts: &[Part], p: Date, day: Date) -> Vec<Selection> {
    let range = |pt: &Part| {
        pt.clause
            .enforcement
            .as_ref()
            .and_then(|e| admissible(p, e))
    };
    let on: Vec<Option<(Date, Date)>> = parts
        .iter()
        .map(|pt| range(pt).filter(|(lo, hi)| *lo <= day && day <= *hi))
        .collect();
    let exact = on.iter().any(|r| r.is_some_and(|(lo, hi)| lo == hi));
    parts
        .iter()
        .enumerate()
        .map(|(i, pt)| match (on[i], range(pt)) {
            (Some((lo, hi)), _) if !exact || lo == hi => Selection::Apply,
            (Some(_), _) => Selection::CabinetOrderElsewhere,
            (None, Some((_, hi))) if hi < day => Selection::AlreadyEnforced,
            (None, Some(_)) => Selection::Later,
            (None, None) => Selection::Unreadable,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// この施行日に当てる
    Apply,
    /// 政令で定める日の区間がこの日を含むが、この日に確定した別の部分があるので当てない
    CabinetOrderElsewhere,
    /// この日より前に施行済み
    AlreadyEnforced,
    /// この日より後に施行
    Later,
    /// 施行期日が読めない
    Unreadable,
}

/// 施行期日が読めない部分に付ける節（本文が読めないまま残った命令を落とさない）
fn unreadable() -> EnforcementClause {
    EnforcementClause {
        text: "施行期日が読めない".into(),
        enforcement: None,
    }
}

/// 同じ条（項が分かれば項）を触る命令が別の部分に入った組 `(i, j, 箇所)`（i < j）。
/// 部分どうしが独立なら施行日の順序によらず単位全体を当てたのと同じ結果になる（Lean `Stage.lean` の `staged_eq_whole`）。
/// ここに挙がる組はその前提が崩れていて、施行の順で結果が変わりうる
pub fn overlaps(parts: &[Part]) -> Vec<(usize, usize, String)> {
    use lawean_amend::numbering::label;
    let keys = |p: &Part| {
        p.unit
            .instructions
            .iter()
            .flat_map(|i| &i.ops)
            .filter_map(|op| op.article().map(|a| (a.clone(), op.paragraph())))
            .collect::<Vec<_>>()
    };
    let ks: Vec<_> = parts.iter().map(keys).collect();
    let mut out = Vec::new();
    for i in 0..ks.len() {
        for j in i + 1..ks.len() {
            let hit = ks[i].iter().find(|(a, p)| {
                ks[j].iter().any(|(b, q)| {
                    a == b
                        && match (p, q) {
                            (Some(p), Some(q)) => p == q,
                            _ => true,
                        }
                })
            });
            if let Some((a, p)) = hit {
                let at = match p {
                    Some(p) => format!("{}第{}項", label(a), lawean_resolve::numeral::to_kanji(*p)),
                    None => label(a),
                };
                out.push((i, j, at));
            }
        }
    }
    out
}

fn empty_like(u: &AmendUnit) -> AmendUnit {
    AmendUnit {
        article_of_amending_law: u.article_of_amending_law.clone(),
        target_title: u.target_title.clone(),
        instructions: Vec::new(),
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use lawean_amend::parse_units;
    use lawean_extract::suppl::spec_from_text;

    pub(super) const SUPPL_FOR_CONSERVATION: &str = SUPPL;

    const SUPPL: &str = "第一条　この法律は、令和六年四月一日から施行する。ただし、次の各号に掲げる規定は、当該各号に定める日から施行する。
　六　第五条の規定並びに附則第十九条の規定　令和五年四月一日
　七　第三条中医療法第三十五条第一項第二号の改正規定（「第十一条第二号若しくは」を「第十一条第一項第二号若しくは」に改める部分に限る。）及び第六条の規定（医師法第十六条の十一第一項の改正規定を除く。）並びに附則第十一条の規定　令和七年四月一日";

    fn art6() -> AmendUnit {
        let t = "第六条　医師法の一部を次のように改正する。
　　第十一条第一号中「者」の下に「（大学）」を加える。
　　第十二条中「前条第三号」を「前条第一項第三号」に改める。
　　第十六条の十一第一項中「医師が」の下に「、長時間」を加える。
　　第十七条の三中「前条第一項」を「前条」に改める。";
        parse_units(t).unwrap().remove(0)
    }

    #[test]
    fn except_scope_splits_the_unit_into_two_dates() {
        let spec = spec_from_text(SUPPL, Some((2021, 5, 28)));
        let parts = parts_of(&spec, "第六条", &art6());
        assert_eq!(parts.len(), 2, "{parts:?}");
        // 第七号（除く）: 第16条の11 以外の 3 文
        assert_eq!(parts[0].unit.instructions.len(), 3);
        assert_eq!(parts[0].clause.text, "令和七年四月一日");
        // 本文: 第16条の11 の 1 文
        assert_eq!(parts[1].unit.instructions.len(), 1);
        assert!(parts[1].unit.instructions[0]
            .text
            .starts_with("第十六条の十一"));
        assert!(parts[1].scope.is_none());
    }

    #[test]
    fn whole_unit_in_an_item_is_one_part() {
        let spec = spec_from_text(SUPPL, Some((2021, 5, 28)));
        let u = AmendUnit {
            article_of_amending_law: "第五条".into(),
            ..art6()
        };
        let parts = parts_of(&spec, "第五条", &u);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].clause.text, "令和五年四月一日");
        assert_eq!(parts[0].unit.instructions.len(), 4);
    }

    #[test]
    fn naka_scope_takes_only_the_listed_provisions() {
        let spec = spec_from_text(
            "第一条　この法律は、令和六年四月一日から施行する。ただし、次の各号に掲げる規定は、当該各号に定める日から施行する。
　一　第六条中医師法第十二条の改正規定及び同法第十七条の三の改正規定、第八条の規定　公布の日",
            Some((2021, 5, 28)),
        );
        let parts = parts_of(&spec, "第六条", &art6());
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert_eq!(parts[0].unit.instructions.len(), 2);
        assert_eq!(parts[0].clause.text, "公布の日");
        assert_eq!(parts[1].unit.instructions.len(), 2);
    }
}

#[cfg(test)]
mod tests_art13 {
    use super::*;
    use lawean_amend::parse_units;
    use lawean_extract::suppl::spec_from_text;

    /// 令3-49 第13条: 第二号「第十三条の規定（第四号に掲げる改正規定を除く。）」の「第四号に掲げる改正規定」は
    /// 第四号が第十三条について挙げる「附則第一条の二第二項の改正規定…」に解く
    #[test]
    fn item_reference_in_except_resolves_to_that_items_scope() {
        let suppl = "第一条　この法律は、令和六年四月一日から施行する。ただし、次の各号に掲げる規定は、当該各号に定める日から施行する。
　一　第十四条の規定　公布の日
　二　第十三条の規定（第四号に掲げる改正規定を除く。）及び附則第二十五条（同号に掲げる改正規定を除く。）の規定　令和三年四月一日又はこの法律の公布の日のいずれか遅い日
　三　第九条から第十二条までの規定　令和三年十月一日
　四　第一条の規定（第一号に掲げる改正規定を除く。）並びに第十三条中地域における医療及び介護の総合的な確保の促進に関する法律附則第一条の二第二項の改正規定及び同条を同法附則第一条の三とし、同法附則第一条の次に一条を加える改正規定並びに附則第四条及び第九条の規定　令和四年三月三十一日までの間において政令で定める日";
        let spec = spec_from_text(suppl, Some((2021, 5, 28)));
        assert_eq!(spec.items.len(), 4);
        let t = "第十三条　地域における医療及び介護の総合的な確保の促進に関する法律（平成元年法律第六十四号）の一部を次のように改正する。
　　第六条中「三分の二」の下に「（全額）」を加える。
　　第三十五条第一項中「第十八条」を「第十一条の七又は第十八条」に改める。
　　附則第一条の二第二項中「附則第一条の二第一項各号」を「附則第一条の三第一項各号」に改め、同条を附則第一条の三とし、附則第一条の次に次の一条を加える。
　第一条の二　甲。";
        let u = parse_units(t).unwrap().remove(0);
        let l = parse_scope_locs("地域における医療及び介護の総合的な確保の促進に関する法律附則第一条の二第二項の改正規定");
        assert!(matches!(&l, Ok(v) if v.len() == 1 && v[0].suppl), "{l:?}");
        let x = after_naka(spec.items[3].scope.split_once("第十三条中").unwrap().1);
        assert!(x.starts_with("地域における医療及び介護の総合的な確保の促進に関する法律附則第一条の二第二項の改正規定"), "{x}");
        let parts = parts_of(&spec, "第十三条", &u);
        assert_eq!(parts.len(), 2, "{parts:?}");
        assert_eq!(parts[0].unit.instructions.len(), 2);
        assert!(parts[0]
            .scope
            .as_deref()
            .unwrap()
            .starts_with("第十三条の規定（第四号"));
        // 「令和三年四月一日又はこの法律の公布の日のいずれか遅い日」= 2021-05-28（公布の方が遅い）
        assert_eq!(
            admissible((2021, 5, 28), parts[0].clause.enforcement.as_ref().unwrap()),
            Some(((2021, 5, 28), (2021, 5, 28)))
        );
        // 政令で定める日の区間がこの日を含むだけの部分は、確定した部分がある日には当てない
        let sel = select_for_day(&parts, (2021, 5, 28), (2021, 5, 28));
        assert_eq!(
            sel,
            vec![Selection::Apply, Selection::CabinetOrderElsewhere]
        );
        let sel = select_for_day(&parts, (2021, 5, 28), (2022, 2, 1));
        assert_eq!(sel, vec![Selection::AlreadyEnforced, Selection::Apply]);
        assert_eq!(parts[1].unit.instructions.len(), 1);
        assert!(parts[1].scope.as_deref().unwrap().contains("第十三条中"));
    }
}

/// 分割の保存則: 部分をつなぐと元の単位の命令がちょうど 1 回ずつ現れ、各部分の中では元の順序のまま（部分列への分割）。
/// Lean `Stage.lean` の `Interleave` の前提を Rust 側で満たしていることの確認
#[cfg(test)]
mod tests_conservation {
    use super::*;
    use lawean_amend::parse_units;
    use lawean_extract::suppl::spec_from_text;

    const ITEMS_ONLY: &str = "第一条　この法律は、新非訟事件手続法の施行の日から施行する。ただし、次の各号に掲げる規定は、当該各号に定める日から施行する。
　一　第六条中医師法第十二条の改正規定　公布の日";

    fn unit(label: &str) -> AmendUnit {
        let t = format!(
            "{label}　医師法の一部を次のように改正する。
　　第十一条第一号中「者」の下に「（大学）」を加える。
　　第十二条中「前条第三号」を「前条第一項第三号」に改める。
　　第十二条中「医師」を「医師等」に改める。
　　第十七条の三中「前条第一項」を「前条」に改める。"
        );
        parse_units(&t).unwrap().remove(0)
    }

    fn texts(u: &AmendUnit) -> Vec<String> {
        u.instructions.iter().map(|i| i.text.clone()).collect()
    }

    /// 各部分が元の単位の部分列で、全部合わせると元の命令の多重集合に一致する
    fn assert_conserved(unit: &AmendUnit, parts: &[Part]) {
        let all = texts(unit);
        let mut used = vec![false; all.len()];
        for p in parts {
            let mut from = 0;
            for t in texts(&p.unit) {
                let k = (from..all.len())
                    .find(|&k| !used[k] && all[k] == t)
                    .unwrap_or_else(|| panic!("順序が崩れたか重複: {t}"));
                used[k] = true;
                from = k + 1;
            }
        }
        let lost: Vec<_> = all.iter().zip(&used).filter(|(_, u)| !**u).collect();
        assert!(lost.is_empty(), "落ちた命令: {lost:?}");
    }

    #[test]
    fn remainder_is_kept_when_main_clause_is_unreadable() {
        let spec = spec_from_text(ITEMS_ONLY, Some((2021, 5, 28)));
        let spec = EnforcementSpec { main: None, ..spec };
        let u = unit("第六条");
        let parts = parts_of(&spec, "第六条", &u);
        assert_conserved(&u, &parts);
        // 残りは施行期日が読めない部分として残る
        let rest = parts.iter().find(|p| p.scope.is_none()).unwrap();
        assert!(rest.clause.enforcement.is_none());
        let sel = select_for_day(&parts, (2021, 5, 28), (2021, 5, 28));
        assert!(sel.contains(&Selection::Unreadable), "{sel:?}");
    }

    #[test]
    fn suppl_label_keeps_unit_when_main_clause_is_unreadable() {
        let spec = spec_from_text(ITEMS_ONLY, Some((2021, 5, 28)));
        let spec = EnforcementSpec { main: None, ..spec };
        let u = unit("附則第三条");
        let parts = parts_of(&spec, "附則第三条", &u);
        assert_conserved(&u, &parts);
    }

    #[test]
    fn existing_splits_conserve_instructions() {
        let spec = spec_from_text(ITEMS_ONLY, Some((2021, 5, 28)));
        let u = unit("第六条");
        assert_conserved(&u, &parts_of(&spec, "第六条", &u));
        let spec = spec_from_text(super::tests::SUPPL_FOR_CONSERVATION, Some((2021, 5, 28)));
        for label in ["第五条", "第六条", "第七条"] {
            let u = unit(label);
            assert_conserved(&u, &parts_of(&spec, label, &u));
        }
    }

    /// 「第十二条中…の改正規定」だけを先に施行: 第十二条を触る 2 文のうち、
    /// 範囲欄の位置（条・項）は 2 文とも拾うので同じ部分に入る → 部分どうしは独立
    #[test]
    fn same_paragraph_stays_in_one_part() {
        let spec = spec_from_text(ITEMS_ONLY, Some((2021, 5, 28)));
        let u = unit("第六条");
        let parts = parts_of(&spec, "第六条", &u);
        assert!(overlaps(&parts).is_empty(), "{:?}", overlaps(&parts));
    }

    /// 同じ項を触る命令が別の日の部分に分かれたら報告する（Lean の `staged_eq_whole` の前提が崩れる）
    #[test]
    fn parts_touching_the_same_paragraph_are_reported() {
        let spec = spec_from_text(ITEMS_ONLY, Some((2021, 5, 28)));
        let u = unit("第六条");
        let mut parts = parts_of(&spec, "第六条", &u);
        // 第十二条の 2 文目を本文の日の部分へ移す（範囲欄が「部分に限る」で文の一部だけを挙げた場合と同じ形）
        let moved = parts[0].unit.instructions.pop().unwrap();
        parts[1].unit.instructions.push(moved);
        let o = overlaps(&parts);
        assert_eq!(o.len(), 1, "{o:?}");
        assert_eq!((o[0].0, o[0].1), (0, 1));
        assert!(o[0].2.contains("第十二条"), "{o:?}");
    }
}

#[cfg(test)]
mod tests_overlap_report {
    use lawean_amend::parse_units;
    use lawean_extract::suppl::spec_from_text;

    /// 第十二条第一項だけ公布の日、条全体を触る文は本文の日: 別の日の部分が同じ条を触るので施行期日の検査が注意を出す
    #[test]
    fn enforcement_check_warns_when_parts_touch_the_same_provision() {
        let spec = spec_from_text(
            "第一条　この法律は、令和六年四月一日から施行する。ただし、次の各号に掲げる規定は、当該各号に定める日から施行する。
　一　第六条中医師法第十二条第一項の改正規定　公布の日",
            Some((2021, 5, 28)),
        );
        let u = parse_units(
            "第六条　医師法の一部を次のように改正する。
　　第十二条第一項中「前条第三号」を「前条第一項第三号」に改める。
　　第十二条の見出し中「試験」を「国家試験」に改める。",
        )
        .unwrap()
        .remove(0);
        let parts = super::parts_of(&spec, "第六条", &u);
        assert_eq!(parts.len(), 1, "見出しも第十二条第一項の範囲に拾われる: {parts:?}");

        let u2 = parse_units(
            "第六条　医師法の一部を次のように改正する。
　　第十二条第一項中「前条第三号」を「前条第一項第三号」に改める。
　　第十二条第二項中「医師」を「医師等」に改める。",
        )
        .unwrap()
        .remove(0);
        let c = crate::check_enforcement_with(
            &spec,
            "起草中の附則",
            "2021-05-28",
            &[("第六条".into(), u2)],
            false,
        );
        // 別の項なら独立: 注意は施行日の分割だけで、同じ箇所の注意は出ない
        assert!(
            !c.details.iter().any(|d| d.contains("同じ箇所")),
            "{:?}",
            c.details
        );

        let mut parts = super::parts_of(&spec, "第六条", &{
            parse_units(
                "第六条　医師法の一部を次のように改正する。
　　第十二条第一項中「前条第三号」を「前条第一項第三号」に改める。
　　第十二条第二項中「医師」を「医師等」に改める。",
            )
            .unwrap()
            .remove(0)
        });
        assert_eq!(parts.len(), 2);
        // 2 つ目の部分に第十二条第一項を触る文を足す（「部分に限る」で文の一部だけ先に施行した形）
        let extra = parts[0].unit.instructions[0].clone();
        parts[1].unit.instructions.push(extra);
        let lines = crate::overlap_lines("第六条", &parts);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("同じ箇所"), "{lines:?}");
        assert!(lines[0].contains("第十二条第一項"), "{lines:?}");
    }
}
