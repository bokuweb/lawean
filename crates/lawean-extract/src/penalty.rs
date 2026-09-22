//! 罰則を規則で取る（層 1）。docs/07 §「罰則」。
//!
//! 罰則は最も語彙が閉じている:
//!
//! - 「第N条の規定に違反して〜した者は、一年以下の禁錮又は三十万円以下の罰金に処する。」
//! - 「次の各号のいずれかに該当する者は、…に処する。」+ 号ごとに（対象規定, 行為）
//!
//! 1 つの罰則 = (対象規定への参照, 行為, 刑)。対象規定は `lawean-resolve` で stable_id に解決する。
//!
//! 検査（`mismatches`）: 罰則が指す規定が義務・禁止でない、または罰則の行為（「表示しなかつた」）が
//! 対象規定に無い（「表示」の語が無い）= **罰則の空振り**。公職選挙法 平成30年法律第75号の実例
//! （第244条第1項第2号の2「第百四十二条の四第六項の規定に違反して同項に規定する事項を表示しなかつた者」の
//! 第六項が、繰り下げで「送信をしてはならない」の項になった）は、改め文を見なくても改正後の本文だけで出る。

use crate::effect::{classify, EffectKind};
use lawean_resolve::{resolve_sentence, Index, Resolution};
use lawean_source::{LegalDocument, StableId};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanctionKind {
    /// 懲役・禁錮・拘禁刑
    Imprisonment(String),
    Fine,
    /// 科料
    PettyFine,
    /// 過料（行政罰）
    AdministrativeFine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanction {
    pub kind: SanctionKind,
    /// 「一年以下」「三十万円以下」の数（月数 / 円）。無ければ None
    pub max: Option<i64>,
    pub text: String,
}

/// 罰則の対象 1 つ（号、または本文そのもの）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PenaltyTarget {
    pub sentence: StableId,
    pub text: String,
    /// 「第N条第M項の規定に違反して」の参照先（解決できたもの）
    pub refs: Vec<StableId>,
    pub unresolved: Vec<String>,
    /// 「〜の規定に違反して」の後の行為（「同項に規定する事項を表示しなかつた」）。無ければ「違反した」だけ
    pub act: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Penalty {
    pub sentence: StableId,
    pub text: String,
    pub sanctions: Vec<Sanction>,
    /// 併科（「に処し、又はこれを併科する」）
    pub cumulative: bool,
    pub targets: Vec<PenaltyTarget>,
}

const N: &str = "[一二三四五六七八九十百千万]+";

struct Rules {
    tail: Regex,
    sanction: Regex,
    violate: Regex,
}

fn rules() -> &'static Rules {
    static R: OnceLock<Rules> = OnceLock::new();
    R.get_or_init(|| Rules {
        tail: Regex::new(r"(に処する|に処し、又はこれを併科する|に処し、又はこれらを併科する|を併科する|を科する)。?$").unwrap(),
        sanction: Regex::new(&format!(
            r"(?:(?P<n>{N})(?P<u>年|月|日)以下の(?P<imp>懲役|禁錮|拘禁刑|拘留))|(?:(?P<yen>{N})円以下の(?P<fine>罰金|科料|過料))|(?P<bare>懲役|禁錮|拘禁刑|拘留|罰金|科料|過料)"
        ))
        .unwrap(),
        violate: Regex::new(r"の規定に違反して、?(?P<act>[^。]*?)(?:者|とき|場合)").unwrap(),
    })
}

fn yen(s: &str) -> Option<i64> {
    // 「三十万」「百万」「五千」
    let (man, rest) = match s.split_once('万') {
        Some((a, b)) => (a, b),
        None => ("", s),
    };
    let k = |t: &str| -> Option<i64> {
        if t.is_empty() {
            Some(0)
        } else {
            lawean_resolve::numeral::kanji_to_u32(t).map(|x| x as i64)
        }
    };
    let m = if man.is_empty() && s.contains('万') {
        1
    } else {
        k(man)?
    };
    Some(m * 10_000 + k(rest)?)
}

/// 文末の刑を読む。罰則の文でなければ None
pub fn parse_sanctions(text: &str) -> Option<(Vec<Sanction>, bool)> {
    let r = rules();
    let t = r.tail.find(text)?;
    let cumulative = t.as_str().contains("併科");
    // 刑の句は文末から遡って「、」「は」まで
    let head = &text[..t.start()];
    // 刑の句は「者は、」「ときは、」の後。「又は」の「は」で切らないよう読点で区切る
    let start = head.rfind('、').map(|i| i + '、'.len_utf8()).unwrap_or(0);
    let clause = &head[start.min(head.len())..];
    let mut out = Vec::new();
    for c in r.sanction.captures_iter(clause) {
        let text = c.get(0).unwrap().as_str().to_string();
        if let Some(imp) = c.name("imp") {
            let n = lawean_resolve::numeral::kanji_to_u32(&c["n"]).map(|x| x as i64);
            // 年・月は月数、「三十日以下の拘留」は日数のまま
            let max = n.map(|x| match &c["u"] {
                "年" => x * 12,
                _ => x,
            });
            out.push(Sanction {
                kind: SanctionKind::Imprisonment(imp.as_str().into()),
                max,
                text,
            });
        } else if let Some(f) = c.name("fine") {
            let kind = match f.as_str() {
                "罰金" => SanctionKind::Fine,
                "科料" => SanctionKind::PettyFine,
                _ => SanctionKind::AdministrativeFine,
            };
            out.push(Sanction {
                kind,
                max: yen(&c["yen"]),
                text,
            });
        } else if let Some(b) = c.name("bare") {
            let kind = match b.as_str() {
                "罰金" => SanctionKind::Fine,
                "科料" => SanctionKind::PettyFine,
                "過料" => SanctionKind::AdministrativeFine,
                x => SanctionKind::Imprisonment(x.into()),
            };
            out.push(Sanction {
                kind,
                max: None,
                text,
            });
        }
    }
    if out.is_empty() {
        return None;
    }
    Some((out, cumulative))
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

/// 1 文の中の対象。「A の規定に違反して X した者又は B の規定に違反して Y した者は」のように
/// 複数あれば分ける: 「〜の規定に違反して」ごとに、その前の区切り（「者又は」「とき若しくは」）からの参照が対象規定
fn targets_of(index: &Index, id: &StableId, text: &str) -> Vec<PenaltyTarget> {
    let r = rules();
    // 括弧書き（「（第二百一条の十五第一項において準用する場合を含む。）」）は準用の文脈で、対象規定ではない
    let text = strip_parens(text);
    let spans = resolve_sentence(index, id, &text);
    let boundary = Regex::new(r"(?:者|とき|場合)(?:又は|若しくは|、)").unwrap();
    let bounds: Vec<usize> = boundary.find_iter(&text).map(|m| m.end()).collect();
    let mut out = Vec::new();
    let mut prev_end = 0;
    let matches: Vec<_> = r.violate.captures_iter(&text).collect();
    if matches.is_empty() {
        return out;
    }
    for c in matches {
        let m = c.get(0).unwrap();
        let start = bounds
            .iter()
            .filter(|&&b| b <= m.start())
            .max()
            .copied()
            .unwrap_or(0)
            .max(prev_end);
        let mut refs = Vec::new();
        let mut unresolved = Vec::new();
        for s in spans
            .iter()
            .filter(|s| s.span.start >= start && s.span.start < m.start())
        {
            match &s.resolution {
                Resolution::Internal(ids) => refs.extend(ids.iter().cloned()),
                Resolution::External { .. } => {}
                Resolution::Unresolved(_) => unresolved.push(s.span.text.clone()),
            }
        }
        let act = Some(c["act"].to_string()).filter(|a| !a.is_empty());
        out.push(PenaltyTarget {
            sentence: id.clone(),
            text: text.clone(),
            refs,
            unresolved,
            act,
        });
        prev_end = m.end();
    }
    out
}

/// 法令の全ての罰則
pub fn penalties(doc: &LegalDocument) -> Vec<Penalty> {
    let index = Index::build(doc);
    let mut out = Vec::new();
    for g in doc.sentence_groups() {
        let mut heads: Vec<(usize, Penalty)> = Vec::new();
        for (i, s) in g.sentences.iter().enumerate() {
            let text = s.sentence.plain_text();
            if s.in_item {
                continue;
            }
            if let Some((sanctions, cumulative)) = parse_sanctions(&text) {
                let mut p = Penalty {
                    sentence: s.sentence.stable_id.clone(),
                    text: text.clone(),
                    sanctions,
                    cumulative,
                    targets: vec![],
                };
                // 本文に対象規定が書いてあれば本文自身が対象（「第百四十条の規定に違反した者は」）
                p.targets
                    .extend(targets_of(&index, &s.sentence.stable_id, &text));
                heads.push((i, p));
            }
        }
        // 「次の各号のいずれかに該当する者は」の号
        for (_, p) in heads.iter_mut() {
            if p.text.contains("各号") {
                for s in g.sentences.iter().filter(|s| s.in_item) {
                    let text = s.sentence.plain_text();
                    p.targets
                        .extend(targets_of(&index, &s.sentence.stable_id, &text));
                }
            }
        }
        out.extend(heads.into_iter().map(|(_, p)| p));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MismatchKind {
    /// 対象規定に義務・禁止・不能・限定の効果が無い
    NotADuty { kinds: Vec<String> },
    /// 罰則の行為の語（「表示」）が対象規定に無い
    ActNotInTarget { word: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    pub penalty: StableId,
    pub target: StableId,
    pub target_text: String,
    pub referenced: StableId,
    pub referenced_text: String,
    pub kind: MismatchKind,
}

/// 行為の句の中心語: 「又は」「若しくは」で分けた各行為の最後の漢字の連なり
/// （「同項に規定する事項を表示しなかつた」→「表示」、「戸別訪問をした」→「戸別訪問」、
/// 「報告書の提出をせず又は虚偽の記入をした」→「提出」「記入」）
pub fn act_words(act: &str) -> Vec<String> {
    act.split("又は")
        .flat_map(|a| a.split("若しくは"))
        .filter_map(|a| {
            a.split(|c: char| !is_kanji(c))
                .rfind(|r| r.chars().count() >= 2)
                .map(str::to_string)
        })
        .collect()
}

/// 送り仮名の違い（「届出」と「届け出」）を無視して、語が本文にあるか
fn contains_word(text: &str, word: &str) -> bool {
    let k: String = text.chars().filter(|c| is_kanji(*c)).collect();
    k.contains(word)
}

fn is_kanji(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c) || c == '々'
}

/// 条の見出し（「（新聞広告）」）。行為の語は見出しにも現れる
fn captions(doc: &LegalDocument) -> BTreeMap<String, String> {
    use lawean_source::ir::{inline_text, Provision};
    fn walk(ps: &[Provision], out: &mut BTreeMap<String, String>) {
        for p in ps {
            match p {
                Provision::Container(c) => walk(&c.children, out),
                Provision::Article(a) => {
                    if let Some(c) = &a.caption {
                        out.insert(a.stable_id.0.clone(), inline_text(c));
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&doc.main_provision, &mut out);
    out
}

/// 罰則の空振り。対象規定の本文（+ 条の見出し）と効果種別を突き合わせる。
/// 対象規定が複数（「第百四十九条第一項又は第四項の規定に違反して」）なら、行為の語はどれかにあればよい
pub fn mismatches(doc: &LegalDocument) -> Vec<Mismatch> {
    let texts: BTreeMap<String, Vec<String>> = doc
        .sentence_groups()
        .iter()
        .map(|g| {
            (
                g.paragraph.0.clone(),
                g.sentences
                    .iter()
                    .map(|s| s.sentence.plain_text())
                    .collect(),
            )
        })
        .collect();
    let caps = captions(doc);
    let para_of = |id: &StableId| -> Option<(&String, &Vec<String>)> {
        texts
            .iter()
            .find(|(k, _)| id.0 == **k || id.0.starts_with(&format!("{k}/")))
    };
    // 「できる」も「通常の方法により…頒布することができる」のように限定つきの許容なら違反がある。
    // 義務・禁止でないと言い切れるのは、定義・みなし・適用・施行・効力の類だけ
    let duty = |k: &EffectKind| {
        !matches!(
            k,
            EffectKind::Definition
                | EffectKind::Deem
                | EffectKind::Presume
                | EffectKind::Apply
                | EffectKind::ApplyMutatis
                | EffectKind::DeemAndApply
                | EffectKind::Enforce
                | EffectKind::Repeal
                | EffectKind::FormerExample
                | EffectKind::Preserve
                | EffectKind::Lapse
                | EffectKind::Void
                | EffectKind::Exception
                | EffectKind::NotApply
                | EffectKind::Suffice
        )
    };
    let mut out = Vec::new();
    for p in penalties(doc) {
        for t in &p.targets {
            // 参照先ごとの本文（条全体への参照は全項）と見出し
            let mut resolved: Vec<(String, String, Vec<EffectKind>)> = Vec::new();
            for r in &t.refs {
                let art_id = r.0.split("/para:").next().unwrap_or(&r.0).to_string();
                let cap = caps.get(&art_id).cloned().unwrap_or_default();
                let all: Vec<&String> = if r.0.contains("/para:") {
                    match para_of(r) {
                        Some((_, sents)) => sents.iter().collect(),
                        None => continue,
                    }
                } else {
                    texts
                        .iter()
                        .filter(|(k, _)| k.starts_with(&format!("{}/", r.0)))
                        .flat_map(|(_, v)| v)
                        .collect()
                };
                let pid = para_of(r)
                    .map(|(k, _)| k.clone())
                    .unwrap_or_else(|| r.0.clone());
                let kinds: Vec<EffectKind> = all.iter().map(|s| classify(s).0).collect();
                let joined: String = std::iter::once(cap.as_str())
                    .chain(all.iter().map(|s| s.as_str()))
                    .collect();
                resolved.push((pid, joined, kinds));
            }
            if resolved.is_empty() {
                continue;
            }
            for (pid, joined, kinds) in &resolved {
                if !kinds.is_empty() && !kinds.iter().any(duty) {
                    out.push(Mismatch {
                        penalty: p.sentence.clone(),
                        target: t.sentence.clone(),
                        target_text: t.text.clone(),
                        referenced: StableId(pid.clone()),
                        referenced_text: joined.chars().take(80).collect(),
                        kind: MismatchKind::NotADuty {
                            kinds: kinds.iter().map(|k| format!("{k:?}")).collect(),
                        },
                    });
                }
            }
            let words = t.act.as_deref().map(act_words).unwrap_or_default();
            if words.is_empty() {
                continue;
            }
            let found = words.iter().any(|w| {
                resolved.iter().any(|(_, joined, _)| {
                    contains_word(joined, w)
                        || (w.chars().count() > 2
                            && contains_word(
                                joined,
                                &w.chars().skip(w.chars().count() - 2).collect::<String>(),
                            ))
                })
            });
            if !found {
                let (pid, joined, _) = &resolved[0];
                out.push(Mismatch {
                    penalty: p.sentence.clone(),
                    target: t.sentence.clone(),
                    target_text: t.text.clone(),
                    referenced: StableId(pid.clone()),
                    referenced_text: joined.chars().take(80).collect(),
                    kind: MismatchKind::ActNotInTarget {
                        word: words.join("／"),
                    },
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------- 候補（共通の契約）

use crate::candidate::{Candidate, Confidence, Field, ValueKind};

impl Penalty {
    /// 刑・対象規定・行為を共通の候補の形に
    pub fn to_candidates(&self) -> Vec<Candidate> {
        let mut out = Vec::new();
        for s in &self.sanctions {
            if let Some(start) = self.text.find(&s.text) {
                let c = Candidate::new(
                    Field::Sanction,
                    &self.sentence,
                    &self.text,
                    start,
                    start + s.text.len(),
                    "penalty:sanction",
                );
                let c = match (&s.kind, s.max) {
                    (SanctionKind::Imprisonment(k), Some(m)) => c
                        .value(ValueKind::Months, m.to_string(), Some("months"))
                        .role(k.clone()),
                    (SanctionKind::Imprisonment(k), None) => c.role(k.clone()),
                    (SanctionKind::Fine, Some(y)) => c
                        .value(ValueKind::Yen, y.to_string(), Some("JPY"))
                        .role("罰金"),
                    (SanctionKind::PettyFine, Some(y)) => c
                        .value(ValueKind::Yen, y.to_string(), Some("JPY"))
                        .role("科料"),
                    (SanctionKind::AdministrativeFine, Some(y)) => c
                        .value(ValueKind::Yen, y.to_string(), Some("JPY"))
                        .role("過料"),
                    (k, None) => c.role(format!("{k:?}")),
                };
                let c = if s.max.is_none() {
                    c.confidence(Confidence::Medium)
                } else {
                    c
                };
                out.push(if self.cumulative {
                    c.label("併科")
                } else {
                    c
                });
            }
        }
        for t in &self.targets {
            let r = rules();
            if let Some(c) = r.violate.captures(&t.text) {
                let m = c.get(0).unwrap();
                // 対象規定: 「〜の規定に違反して」の直前まで（参照の解決結果を normalized に）
                let refs: Vec<&str> = t.refs.iter().map(|r| r.0.as_str()).collect();
                let target = Candidate::new(
                    Field::PenaltyTarget,
                    &t.sentence,
                    &t.text,
                    0,
                    m.start(),
                    "penalty:target",
                )
                .value(ValueKind::NodeRef, refs.join(" "), None)
                .role("対象規定")
                .confidence(if t.refs.is_empty() {
                    Confidence::Low
                } else {
                    Confidence::High
                });
                out.push(target);
                if let Some(a) = c.name("act") {
                    let act_c = Candidate::new(
                        Field::PenaltyAct,
                        &t.sentence,
                        &t.text,
                        a.start(),
                        a.end(),
                        "penalty:act",
                    )
                    .value(ValueKind::Text, act_words(a.as_str()).join("／"), None)
                    .role("行為");
                    out.push(act_c);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanctions() {
        let (s, c) = parse_sanctions(
            "次の各号のいずれかに該当する者は、一年以下の禁錮又は三十万円以下の罰金に処する。",
        )
        .unwrap();
        assert!(!c);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].kind, SanctionKind::Imprisonment("禁錮".into()));
        assert_eq!(s[0].max, Some(12));
        assert_eq!(s[1].kind, SanctionKind::Fine);
        assert_eq!(s[1].max, Some(300_000));
        let (s, c) = parse_sanctions(
            "三年以下の懲役若しくは禁錮又は百万円以下の罰金に処し、又はこれを併科する。",
        )
        .unwrap();
        assert!(c);
        assert_eq!(s.len(), 3);
        assert_eq!(s[2].max, Some(1_000_000));
        assert!(parse_sanctions("この法律は、公布の日から施行する。").is_none());
        assert_eq!(yen("三十万"), Some(300_000));
        assert_eq!(yen("五千"), Some(5_000));
        assert_eq!(yen("万"), Some(10_000));
    }

    #[test]
    fn act_word_forms() {
        assert_eq!(
            act_words("同項に規定する事項を表示しなかつた"),
            vec!["表示"]
        );
        assert_eq!(act_words("戸別訪問をした"), vec!["戸別訪問"]);
        assert_eq!(
            act_words("報告書若しくはこれに添付すべき書面の提出をせず又はこれらに虚偽の記入をした"),
            vec!["報告書", "提出", "記入"]
        );
        assert!(contains_word(
            "直ちにその旨を届け出なければならない",
            "届出"
        ));
        assert!(!contains_word("送信をしてはならない", "表示"));
    }
}
