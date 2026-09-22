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
use lawean_extract::suppl::{
    kanji_num, scope_articles, scope_is_target_side, EnforcementClause, EnforcementSpec,
};

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

/// 「{label}中」に続く改正規定の列挙: 「改正規定」で終わる語を「、」「及び」でつないだ範囲まで
fn after_naka(rest: &str) -> String {
    let group = rest.split("並びに").next().unwrap_or("");
    let mut taken: Vec<&str> = Vec::new();
    for tok in group.split(['、']).flat_map(|t| t.split("及び")) {
        let t = tok.trim_end_matches("の規定");
        if t.ends_with("改正規定") {
            taken.push(t);
        } else {
            break;
        }
    }
    taken.join("及び")
}

fn mention(scope: &str, label: &str, art: u32) -> Option<Mention> {
    // 「第六条中医師法第十六条の十一第一項の改正規定」
    if let Some((_, rest)) = scope.split_once(&format!("{label}中")) {
        let x = after_naka(rest);
        if !x.is_empty() {
            return Some(Mention::Only(locs_of(&x)));
        }
    }
    // 「第六条の規定（…に限る。）」「第六条の規定（…を除く。）」「第六条（…に限る。）」
    for head in [format!("{label}の規定（"), format!("{label}（")] {
        if let Some((_, rest)) = scope.split_once(&head) {
            let inner = rest.split('）').next().unwrap_or("");
            if let Some(x) = inner.strip_suffix("に限る。") {
                return Some(Mention::Only(locs_of(x)));
            }
            if let Some(x) = inner.strip_suffix("を除く。") {
                return Some(Mention::Except(locs_of(x)));
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

/// 単位を施行期日ごとの部分に分ける。号に触れられていなければ本文の 1 つだけ。附則が読めなければ空
pub fn parts_of(spec: &EnforcementSpec, label: &str, unit: &AmendUnit) -> Vec<Part> {
    let Some(art) = label_article(label) else {
        return spec
            .main
            .iter()
            .map(|m| Part {
                unit: unit.clone(),
                clause: m.clone(),
                scope: None,
            })
            .collect();
    };
    let mut remaining = unit.clone();
    let mut parts = Vec::new();
    for it in &spec.items {
        if remaining.instructions.is_empty() {
            break;
        }
        let plain = strip_parens(&it.scope);
        let m = match mention(&it.scope, label, art) {
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
        if let Some(m) = &spec.main {
            parts.push(Part {
                unit: remaining,
                clause: m.clone(),
                scope: None,
            });
        }
    }
    parts
}

fn empty_like(u: &AmendUnit) -> AmendUnit {
    AmendUnit {
        article_of_amending_law: u.article_of_amending_law.clone(),
        target_title: u.target_title.clone(),
        instructions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lawean_amend::parse_units;
    use lawean_extract::suppl::spec_from_text;

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
