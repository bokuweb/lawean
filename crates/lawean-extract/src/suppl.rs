//! 附則の施行期日を取る（層 1、規則）。docs/07 §「期間・効力・対象の齟齬」の効力の行。
//!
//! 改正法の附則第一条は決まった形をしている:
//!
//! - 本文「この法律は、公布の日から起算して四年を超えない範囲内において政令で定める日から施行する。」
//! - 各号「第三条の規定並びに附則第六十条中…の改正規定」（範囲欄）「公布の日」（日付欄）
//!
//! 範囲欄は改正法の**条**の列挙（「第五十九条から第六十三条まで」の範囲、「並びに」で区切った群、
//! 「附則」が付けば附則の条）。ここから「改正法の第 N 条（本則／附則）はいつ施行されるか」を引く。
//! 公布日は `AmendLawNum`（「令和四年五月二五日法律第四八号」）にある。
//!
//! 出力の `Enforcement` は `temporal` のもの。許容区間（`admissible`）は暦（`calendar`）で出す。

use crate::calendar::{self, Date};
use crate::temporal::{enforcement_list, parse_enforcement, Enforcement, Unit};
use lawean_source::ir::{ItemBody, LegalDocument, Provision, SupplChild, SupplProvision};
use regex::Regex;
use std::sync::OnceLock;

/// 「令和四年五月二五日法律第四八号」を読んだもの。年は元号年
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LawNumDate {
    pub era: String,
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub num: u32,
}

/// 漢数字。位取り（「二五」= 25、「一九」= 19）と命数（「二十五」）の両方
pub fn kanji_num(s: &str) -> Option<u32> {
    if s == "元" {
        return Some(1);
    }
    if s.chars().any(|c| matches!(c, '十' | '百' | '千')) {
        return lawean_resolve::numeral::kanji_to_u32(s);
    }
    let mut n = 0u32;
    for c in s.chars() {
        let d = "〇一二三四五六七八九".chars().position(|x| x == c)? as u32;
        n = n * 10 + d;
    }
    Some(n)
}

pub fn parse_law_num_date(s: &str) -> Option<LawNumDate> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| {
        Regex::new(r"^(明治|大正|昭和|平成|令和)([〇一二三四五六七八九十百]+|元)年([〇一二三四五六七八九十]+)月([〇一二三四五六七八九十]+)日(?:法律|政令|勅令|省令|規則)第([〇一二三四五六七八九十百千]+)号$").unwrap()
    });
    let c = r.captures(s.trim())?;
    Some(LawNumDate {
        era: c[1].to_string(),
        year: kanji_num(&c[2])?,
        month: kanji_num(&c[3])?,
        day: kanji_num(&c[4])?,
        num: kanji_num(&c[5])?,
    })
}

pub fn era_year(era: &str, y: u32) -> Option<i32> {
    let base = match era {
        "明治" | "Meiji" => 1868,
        "大正" | "Taisho" => 1912,
        "昭和" | "Showa" => 1926,
        "平成" | "Heisei" => 1989,
        "令和" | "Reiwa" => 2019,
        _ => return None,
    };
    Some(base + y as i32 - 1)
}

/// e-Gov 法令 ID「504AC0000000048」→ (元号, 元号年, 号数)。先頭 1 桁が元号（1 明治 … 5 令和）
pub fn law_id_parts(id: &str) -> Option<(String, u32, u32)> {
    let b = id.as_bytes();
    if b.len() != 15 {
        return None;
    }
    let era = match b[0] {
        b'1' => "明治",
        b'2' => "大正",
        b'3' => "昭和",
        b'4' => "平成",
        b'5' => "令和",
        _ => return None,
    };
    let year: u32 = id[1..3].parse().ok()?;
    let num: u32 = id[5..].parse().ok()?;
    Some((era.to_string(), year, num))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementClause {
    pub text: String,
    /// 読めなければ None（「新非訟事件手続法の施行の日から施行する」など他法令に依るもの）
    pub enforcement: Option<Enforcement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementItem {
    pub scope: String,
    pub clause: EnforcementClause,
}

/// 改正法の条の範囲。`suppl` は附則の条
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtRange {
    pub suppl: bool,
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnforcementSpec {
    pub amend_law_num: Option<String>,
    /// 公布日（西暦）
    pub promulgated: Option<Date>,
    pub main: Option<EnforcementClause>,
    pub items: Vec<EnforcementItem>,
}

/// 括弧の中を落とす（入れ子込み）
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

/// 範囲欄から改正法の条の列挙を取る
pub fn scope_articles(scope: &str) -> Vec<ArtRange> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| {
        Regex::new(r"(附則)?第([一二三四五六七八九十百千]+)条(?:から第([一二三四五六七八九十百千]+)条まで)?").unwrap()
    });
    let s = strip_parens(scope);
    let mut out = Vec::new();
    for group in s.split("並びに") {
        // 群の中に「附則」があれば群全体が附則の条（「附則第八条第一項、第五十九条から第六十三条まで」）
        let suppl = group.contains("附則");
        for c in r.captures_iter(group) {
            // 「第五条中人事訴訟法第三十五条の改正規定」「同法第百五十七条第四項」の条は被改正法の条。
            // 改正法の条は列挙の頭（文頭・「、」「及び」「附則」の直後）にだけ現れる
            let before = group[..c.get(0).unwrap().start()].chars().next_back();
            if !matches!(
                before,
                None | Some('、') | Some('び') | Some('則') | Some('は')
            ) {
                continue;
            }
            let from = kanji_num(&c[2]).unwrap_or(0);
            let to = c.get(3).and_then(|x| kanji_num(x.as_str())).unwrap_or(from);
            out.push(ArtRange { suppl, from, to });
        }
    }
    out
}

/// 範囲欄が被改正法の条で書かれているか（「第三十四条の二第一項の改正規定」）。
/// 単独法の改正法（「宅地建物取引業法の一部を改正する法律」）の附則はこの形、整備法の号は改正法の条で書く
pub fn scope_is_target_side(scope: &str) -> bool {
    scope.contains("改正規定")
}

/// 被改正法の条で書かれた範囲欄から、条番号（枝番は「N_M」）を取る
pub fn scope_target_articles(scope: &str) -> Vec<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| {
        Regex::new(r"第([一二三四五六七八九十百千]+)条((?:の[一二三四五六七八九十百千]+)*)")
            .unwrap()
    });
    let s = strip_parens(scope);
    let mut out = Vec::new();
    for c in r.captures_iter(&s) {
        // 「同法第百二十九条の改正規定」「宅地建物取引業法第六十四条の三第三項」— 法令名の直後も被改正法の条。
        // 「附則第三条の規定」は改正法の附則の条なので除く
        if s[..c.get(0).unwrap().start()].ends_with("附則") {
            continue;
        }
        let Some(base) = kanji_num(&c[1]) else {
            continue;
        };
        let mut id = base.to_string();
        for b in c[2].split('の').filter(|x| !x.is_empty()) {
            if let Some(n) = kanji_num(b) {
                id.push('_');
                id.push_str(&n.to_string());
            }
        }
        out.push(id);
    }
    out
}

impl EnforcementSpec {
    /// 改め文が触る被改正法の条（`ArticleNum::to_num_string` の形「34_2」）から施行期日を引く。
    /// 範囲欄が「〜の改正規定」で被改正法の条を挙げている号（ただし書き）に当たれば、それ。無ければ本文
    pub fn for_target_articles(
        &self,
        arts: &[String],
    ) -> Option<(&EnforcementClause, Option<&str>)> {
        for it in &self.items {
            if !scope_is_target_side(&it.scope) {
                continue;
            }
            let listed = scope_target_articles(&it.scope);
            if arts.iter().any(|a| listed.contains(a)) {
                return Some((&it.clause, Some(it.scope.as_str())));
            }
        }
        self.main.as_ref().map(|m| (m, None))
    }

    /// 改正法の第 `art` 条がいつ施行されるか。`suppl` が None なら本則を先に、次に附則を探す。号に無ければ本文
    pub fn for_article(
        &self,
        art: u32,
        suppl: Option<bool>,
    ) -> Option<(&EnforcementClause, Option<&str>)> {
        let order: &[bool] = match suppl {
            Some(true) => &[true],
            Some(false) => &[false],
            None => &[false, true],
        };
        for &sp in order {
            for it in &self.items {
                if scope_articles(&it.scope)
                    .iter()
                    .any(|r| r.suppl == sp && r.from <= art && art <= r.to)
                {
                    return Some((&it.clause, Some(it.scope.as_str())));
                }
            }
        }
        self.main.as_ref().map(|m| (m, None))
    }
}

fn spec_of(sp: &SupplProvision, own_promulgated: Option<Date>) -> EnforcementSpec {
    let mut spec = EnforcementSpec {
        amend_law_num: sp.amend_law_num.clone(),
        promulgated: None,
        main: None,
        items: vec![],
    };
    spec.promulgated = match &sp.amend_law_num {
        Some(n) => {
            parse_law_num_date(n).and_then(|d| Some((era_year(&d.era, d.year)?, d.month, d.day)))
        }
        None => own_promulgated,
    };
    // 施行期日の項: 「施行する」を含む最初の項（附則第一条か、条の無い附則の第 1 項）
    let paras: Vec<&lawean_source::ir::Paragraph> = sp
        .children
        .iter()
        .flat_map(|c| match c {
            SupplChild::Provision(Provision::Article(a)) => a
                .children
                .iter()
                .filter_map(|ch| match ch {
                    lawean_source::ir::ArticleChild::Paragraph(p) => Some(p),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            SupplChild::Paragraph(p) => vec![p],
            _ => vec![],
        })
        .collect();
    let Some(p) = paras.iter().find(|p| {
        p.sentences
            .iter()
            .any(|s| s.plain_text().contains("施行する"))
    }) else {
        return spec;
    };
    let texts: Vec<String> = p.sentences.iter().map(|s| s.plain_text()).collect();
    let text = texts
        .iter()
        .find(|t| t.contains("施行する"))
        .cloned()
        .unwrap_or_default();
    spec.main = Some(EnforcementClause {
        enforcement: parse_enforcement(&text),
        text: text.clone(),
    });
    // ただし書き「ただし、A の改正規定は公布の日から、B の改正規定は…から施行する」: 範囲ごとに号と同じ扱い
    for t in texts
        .iter()
        .filter(|t| **t != text && t.contains("施行する"))
    {
        for (scope, e) in enforcement_list(t) {
            spec.items.push(EnforcementItem {
                scope,
                clause: EnforcementClause {
                    enforcement: Some(e),
                    text: t.clone(),
                },
            });
        }
    }
    for ch in &p.children {
        if let lawean_source::ir::ParagraphChild::Item(it) = ch {
            if let ItemBody::Columns(cols) = &it.body {
                if cols.len() >= 2 {
                    let scope: String = cols[0].sentences.iter().map(|s| s.plain_text()).collect();
                    let text: String = cols[1].sentences.iter().map(|s| s.plain_text()).collect();
                    spec.items.push(EnforcementItem {
                        scope,
                        clause: EnforcementClause {
                            enforcement: parse_enforcement(&text),
                            text,
                        },
                    });
                }
            }
        }
    }
    spec
}

/// 法令自身の公布日（`Law` 要素の Era / Year / PromulgateMonth / PromulgateDay）
pub fn own_promulgation(doc: &LegalDocument) -> Option<Date> {
    let attr = |k: &str| {
        doc.law_attrs
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, v)| v.as_str())
    };
    let y = era_year(attr("Era")?, attr("Year")?.parse().ok()?)?;
    Some((
        y,
        attr("PromulgateMonth")?.parse().ok()?,
        attr("PromulgateDay")?.parse().ok()?,
    ))
}

/// 全ての附則（原始附則と改正法ごとの附則）の施行期日
pub fn enforcement_specs(doc: &LegalDocument) -> Vec<EnforcementSpec> {
    let own = own_promulgation(doc);
    doc.suppl_provisions
        .iter()
        .map(|sp| spec_of(sp, own))
        .collect()
}

/// 改正法（e-Gov 法令 ID）の附則の施行期日
pub fn spec_for_law_id(doc: &LegalDocument, amend_law_id: &str) -> Option<EnforcementSpec> {
    let (era, year, num) = law_id_parts(amend_law_id)?;
    doc.suppl_provisions
        .iter()
        .find(|sp| {
            sp.amend_law_num
                .as_deref()
                .and_then(parse_law_num_date)
                .is_some_and(|d| d.era == era && d.year == year && d.num == num)
        })
        .map(|sp| spec_of(sp, None))
}

/// 施行日の許容区間 [lo, hi]（両端を含む）。公布日 `p` から暦（民法第143条）で
///
/// - 「公布の日から」: p
/// - 「令和三年九月一日から」: その日
/// - 「公布の日から起算して n 月を経過した日から」: 起算日 p、満了日の翌日
/// - 「公布の日から起算して n 月を超えない範囲内において政令で定める日から」: p 〜 満了日
pub fn admissible(p: Date, e: &Enforcement) -> Option<(Date, Date)> {
    Some(match e {
        Enforcement::Promulgation => (p, p),
        Enforcement::OtherLaw(_) => return None,
        Enforcement::ByCabinetOrderUntil { era, y, m, d } => (p, (era_year(era, *y)?, *m, *d)),
        Enforcement::Date { era, y, m, d } => {
            let t = (era_year(era, *y)?, *m, *d);
            (t, t)
        }
        Enforcement::ElapsedFromPromulgation(dur) => {
            let t = match dur.unit {
                Unit::Day | Unit::Week => calendar::add_days(p, dur.days()?),
                _ => calendar::next_day(calendar::expiry(p, dur.months()?)),
            };
            (t, t)
        }
        Enforcement::ByCabinetOrderWithin(dur) => {
            let hi = match dur.unit {
                Unit::Day | Unit::Week => calendar::add_days(p, dur.days()? - 1),
                _ => calendar::expiry(p, dur.months()?),
            };
            (p, hi)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::Dur;

    #[test]
    fn law_num_date() {
        let d = parse_law_num_date("令和四年五月二五日法律第四八号").unwrap();
        assert_eq!((d.year, d.month, d.day, d.num), (4, 5, 25, 48));
        assert_eq!(era_year(&d.era, d.year), Some(2022));
        let d = parse_law_num_date("平成二三年五月二五日法律第五三号").unwrap();
        assert_eq!((d.year, d.num), (23, 53));
        assert_eq!(
            law_id_parts("504AC0000000048"),
            Some(("令和".into(), 4, 48))
        );
        assert_eq!(kanji_num("元"), Some(1));
        assert_eq!(kanji_num("十九"), Some(19));
        assert_eq!(kanji_num("一九"), Some(19));
    }

    #[test]
    fn scope_groups() {
        let s = "第二十七条（住民基本台帳法別表第一から別表第五までの改正規定に限る。）、第四十五条、第四十七条及び第五十五条（…に限る。）並びに附則第八条第一項、第五十九条から第六十三条まで、第六十七条及び第七十一条から第七十三条までの規定";
        let a = scope_articles(s);
        let main: Vec<u32> = a.iter().filter(|r| !r.suppl).map(|r| r.from).collect();
        assert_eq!(main, vec![27, 45, 47, 55]);
        let suppl: Vec<(u32, u32)> = a
            .iter()
            .filter(|r| r.suppl)
            .map(|r| (r.from, r.to))
            .collect();
        assert_eq!(suppl, vec![(8, 8), (59, 63), (67, 67), (71, 73)]);
        // 「第五条中人事訴訟法第三十五条の改正規定」の第三十五条は被改正法の条
        let s = "第一条の規定、第四条中民事訴訟費用等に関する法律第二十八条の二第一項の改正規定、第五条中人事訴訟法第三十五条の改正規定、第六条の規定並びに第九条中民事執行法第百五十六条の改正規定、同法第百五十七条第四項の改正規定並びに附則第四十五条及び第四十八条の規定、附則第七十一条中民事保全法（平成元年法律第九十一号）第五十条第五項の改正規定、附則第七十三条の規定";
        let a = scope_articles(s);
        let main: Vec<u32> = a.iter().filter(|r| !r.suppl).map(|r| r.from).collect();
        assert_eq!(main, vec![1, 4, 5, 6, 9]);
        let suppl: Vec<u32> = a.iter().filter(|r| r.suppl).map(|r| r.from).collect();
        assert_eq!(suppl, vec![45, 48, 71, 73]);
    }

    #[test]
    fn admissible_ranges() {
        let p = (2021, 5, 19);
        assert_eq!(admissible(p, &Enforcement::Promulgation), Some((p, p)));
        let one_year = Dur {
            n: 1,
            unit: Unit::Year,
        };
        assert_eq!(
            admissible(p, &Enforcement::ByCabinetOrderWithin(one_year)),
            Some((p, (2022, 5, 18)))
        );
        assert_eq!(
            admissible(p, &Enforcement::ElapsedFromPromulgation(one_year)),
            Some(((2022, 5, 19), (2022, 5, 19)))
        );
        let twenty_days = Dur {
            n: 20,
            unit: Unit::Day,
        };
        assert_eq!(
            admissible(p, &Enforcement::ElapsedFromPromulgation(twenty_days)),
            Some(((2021, 6, 8), (2021, 6, 8)))
        );
        assert_eq!(
            parse_enforcement("この法律は、令和三年九月一日から施行する。"),
            Some(Enforcement::Date {
                era: "令和".into(),
                y: 3,
                m: 9,
                d: 1
            })
        );
        assert_eq!(
            parse_enforcement("公布の日から起算して九月を超えない範囲内において政令で定める日"),
            Some(Enforcement::ByCabinetOrderWithin(Dur {
                n: 9,
                unit: Unit::Month
            }))
        );
        assert_eq!(
            parse_enforcement("新非訟事件手続法の施行の日から施行する。"),
            Some(Enforcement::OtherLaw("新非訟事件手続法".into()))
        );
        assert_eq!(
            parse_enforcement("この法律は、刑法等一部改正法施行日から施行する。"),
            Some(Enforcement::OtherLaw("刑法等一部改正法".into()))
        );
        assert_eq!(
            parse_enforcement(
                "公布の日から起算して一年六月を超えない範囲内において政令で定める日から施行する。"
            ),
            Some(Enforcement::ByCabinetOrderWithin(Dur {
                n: 18,
                unit: Unit::Month
            }))
        );
        assert_eq!(
            parse_enforcement(
                "この法律は、令和五年二月一日までの間において政令で定める日から施行する。"
            ),
            Some(Enforcement::ByCabinetOrderUntil {
                era: "令和".into(),
                y: 5,
                m: 2,
                d: 1
            })
        );
        let list = crate::temporal::enforcement_list("ただし、第一条中宅地建物取引業法第六十四条の三第三項を同条第四項とし、同条第二項の次に一項を加える改正規定及び同法第六十四条の十二第七項の改正規定並びに附則第六項の規定は公布の日から、同法第三十四条の次に二条を加える改正規定は公布の日から起算して二年を経過する日から施行する。");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].1, Enforcement::Promulgation);
        assert_eq!(
            list[1].1,
            Enforcement::ElapsedFromPromulgation(Dur {
                n: 2,
                unit: Unit::Year
            })
        );
        assert!(list[1].0.contains("第三十四条の次に二条を加える改正規定"));
    }
}

/// e-Gov のリビジョン（改正法 ID + 施行日）が附則で説明できるか
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Explanation {
    /// 本文か号のどれかの区間に入る
    Explained,
    /// 本文が他法令の施行日に依り、号にも無い
    DependsOnOtherLaw(String),
    /// この本文に改正法の附則が載っていない（本文が古い、または附則を残さない改正）
    NoSupplProvision,
    /// 附則はあるが施行期日が読めない
    Unreadable(Vec<String>),
    /// どの区間にも入らない
    Outside(Vec<(Date, Date)>),
}

/// 改正法 `amend_law_id` による施行日 `day` を、この本文に載る附則で説明する。
/// 改正法のどの条がこの法令を改めたかは分からないので「本文か号のどれかの区間」を見る
pub fn explain(doc: &LegalDocument, amend_law_id: &str, day: Date) -> Explanation {
    let Some(spec) = spec_for_law_id(doc, amend_law_id) else {
        return Explanation::NoSupplProvision;
    };
    let clauses: Vec<&EnforcementClause> = spec
        .main
        .iter()
        .chain(spec.items.iter().map(|i| &i.clause))
        .collect();
    let Some(p) = spec.promulgated else {
        return Explanation::Unreadable(clauses.iter().map(|c| c.text.clone()).collect());
    };
    let ranges: Vec<(Date, Date)> = clauses
        .iter()
        .filter_map(|c| c.enforcement.as_ref())
        .filter_map(|e| admissible(p, e))
        .collect();
    if ranges.iter().any(|(lo, hi)| *lo <= day && day <= *hi) {
        return Explanation::Explained;
    }
    if let Some(Enforcement::OtherLaw(law)) =
        spec.main.as_ref().and_then(|m| m.enforcement.as_ref())
    {
        return Explanation::DependsOnOtherLaw(law.clone());
    }
    if clauses.iter().all(|c| c.enforcement.is_none()) {
        return Explanation::Unreadable(clauses.iter().map(|c| c.text.clone()).collect());
    }
    Explanation::Outside(ranges)
}
