//! 節境界・上書き・譲歩の認識。
//!
//! - 「〜の規定にかかわらず」→ Rule の上書き（参照は lawean-resolve で解決）
//! - 「契約の条件 / 特約 / 定め にかかわらず」→ 契約への優先
//! - 「〜かどうか / 〜の有無 にかかわらず」「〜ても」「〜がなくても」→ 譲歩（条件に入れない）
//! - 「〜場合において」「〜ときは」「〜に限り」→ 条件節

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverrideSpan {
    /// 「X の規定にかかわらず」。`start..end` は X の範囲（参照表現が入る）
    Rule {
        start: usize,
        end: usize,
        text: String,
    },
    /// 「契約の条件にかかわらず」
    Contract { text: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concession {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionClause {
    pub text: String,
    /// 「場合において」「ときは」「に限り」など
    pub marker: String,
    pub start: usize,
    pub end: usize,
}

fn re_override() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"([^、。]*?)(の規定|規定)にかかわらず").unwrap())
}
fn re_contract() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(契約の条件|契約の定め|特約|定め|約定)にかかわらず").unwrap())
}
fn re_concession() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"([^、。]*?)(かどうかにかかわらず|の有無にかかわらず|がなくても|であっても|があっても|をした場合であっても|た場合であっても)")
            .unwrap()
    })
}
fn re_condition() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // 括弧書き（「。」を含みうる）は 1 段まで飛ばす
    R.get_or_init(|| {
        Regex::new(r"((?:[^。（）]|（[^（）]*）)*?)(場合においては|場合において|場合には|場合は|場合に|ときは|ときに限り|に限り|でなければ)、?").unwrap()
    })
}

pub fn overrides(text: &str) -> Vec<OverrideSpan> {
    let mut out = Vec::new();
    for c in re_override().captures_iter(text) {
        let x = c.get(1).unwrap();
        // 「契約の条件にかかわらず」は別扱い（「規定」を含まない）
        out.push(OverrideSpan::Rule {
            start: x.start(),
            end: x.end(),
            text: x.as_str().to_string(),
        });
    }
    for c in re_contract().captures_iter(text) {
        out.push(OverrideSpan::Contract {
            text: c.get(0).unwrap().as_str().to_string(),
        });
    }
    out
}

pub fn concessions(text: &str) -> Vec<Concession> {
    re_concession()
        .captures_iter(text)
        .map(|c| Concession {
            text: c.get(0).unwrap().as_str().to_string(),
        })
        .collect()
}

/// 条件節。「ただし、」「この場合において、」の導入部は除く
pub fn conditions(text: &str) -> Vec<ConditionClause> {
    re_condition()
        .captures_iter(text)
        .map(|c| {
            let body = c.get(1).unwrap();
            let marker = c.get(2).unwrap();
            let t = body
                .as_str()
                .trim_start_matches(['、', ' '])
                .trim_start_matches("ただし")
                .trim_start_matches('、');
            // 「この場合において」は前段の効果を条件にする定型。本文は空
            if t.is_empty() || t == "この" || t == "前項の" || t == "前二項の" {
                return ConditionClause {
                    text: format!("{}{}", t, marker.as_str()),
                    marker: marker.as_str().to_string(),
                    start: body.start(),
                    end: marker.end(),
                };
            }
            ConditionClause {
                text: t.to_string(),
                marker: marker.as_str().to_string(),
                start: body.start(),
                end: marker.end(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_kinds() {
        let o = overrides("存続期間を五十年以上として借地権を設定する場合においては、第九条及び第十六条の規定にかかわらず、契約の更新がないこととする旨を定めることができる。");
        assert!(
            matches!(&o[0], OverrideSpan::Rule { text, .. } if text == "第九条及び第十六条"),
            "{o:?}"
        );
        let o = overrides(
            "不相当となったときは、契約の条件にかかわらず、当事者は、増減を請求することができる。",
        );
        assert!(matches!(&o[0], OverrideSpan::Contract { .. }), "{o:?}");
        assert!(overrides("保証人があるかどうかにかかわらず").is_empty());
    }

    #[test]
    fn concession_kinds() {
        assert_eq!(
            concessions("借地権は、その登記がなくても、対抗することができる。").len(),
            1
        );
        assert_eq!(concessions("保証人があるかどうかにかかわらず、").len(), 1);
        assert_eq!(concessions("前項の通知をした場合であっても、").len(), 1);
        assert!(concessions("第九条の規定にかかわらず").is_empty());
    }

    #[test]
    fn condition_clauses() {
        let c = conditions("借地権の存続期間が満了する場合において、借地権者が契約の更新を請求したときは、建物がある場合に限り、従前の契約と同一の条件で契約を更新したものとみなす。");
        let texts: Vec<&str> = c.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "借地権の存続期間が満了する",
                "借地権者が契約の更新を請求した",
                "建物がある"
            ]
        );
        let c = conditions("ただし、契約でこれより長い期間を定めたときは、その期間とする。");
        assert_eq!(c[0].text, "契約でこれより長い期間を定めた");
        // 括弧書きの中の「。」で切れない
        let c = conditions("前項前段の特約が電磁的記録（電子的方式をいう。第三十八条において同じ。）によってされたときは、適用する。");
        assert_eq!(c[0].text, "前項前段の特約が電磁的記録（電子的方式をいう。第三十八条において同じ。）によってされた");
    }
}
