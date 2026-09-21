//! 金額・割合・数量を規則で取る（層 1）。docs/11 L1 の「金額・数量」の行。
//!
//! 閉じた語彙: 「三十万円」「五千万円以上」「十分の三を超える」「五分の四以上の多数」「年一割」「百分の五」
//! 「二百平方メートル」「三人以上」「二回以上」「二以上の都道府県」。正規化は円・千分率（‰ で整数）・個数。
//! 刑の金額（「三十万円以下の罰金」）は `penalty` の側が刑として取るので、ここでは取らない。
//! 法令番号（「第百七十六号」）と条項番号は数量ではない（「第」の後の数詞は見ない）。

use crate::candidate::{Candidate, Confidence, Field, ValueKind};
use lawean_resolve::numeral::kanji_to_u32;
use lawean_source::StableId;
use regex::Regex;
use std::sync::OnceLock;

const N: &str = "[一二三四五六七八九十百千万億兆]+";
const OP: &str = "(?P<op>以上|以下|未満|を超える|を超え|をこえる|をこえ|に満たない|を下らない)?";

struct Rules {
    money: Regex,
    fraction: Regex,
    percent: Regex,
    rate: Regex,
    area: Regex,
    count: Regex,
    sanction: Regex,
}

fn rules() -> &'static Rules {
    static R: OnceLock<Rules> = OnceLock::new();
    R.get_or_init(|| Rules {
        money: Regex::new(&format!(r"(?P<n>{N})円{OP}")).unwrap(),
        fraction: Regex::new(&format!(r"(?P<d>{N})分の(?P<n>{N}){OP}")).unwrap(),
        percent: Regex::new(&format!(r"(?P<n>{N}(?:・{N})?)パーセント{OP}")).unwrap(),
        rate: Regex::new(&format!(r"年(?P<n>{N})(?P<u>割|分|パーセント)")).unwrap(),
        area: Regex::new(&format!(r"(?P<n>{N})(?P<u>平方メートル|メートル|ヘクタール|アール){OP}")).unwrap(),
        count: Regex::new(&format!(r"(?P<n>{N})(?P<u>人|回|戸|棟|個|件|通|部|箇所|か所|名)?(?P<op2>以上|以下|未満|を超える)")).unwrap(),
        sanction: Regex::new(&format!(r"{N}円以下の(?:罰金|科料|過料)|{N}円以上{N}円以下の(?:罰金|科料|過料)")).unwrap(),
    })
}

/// 万・億・兆を含む漢数字
pub fn kanji_big(s: &str) -> Option<i64> {
    let mut total: i64 = 0;
    let mut rest = s;
    for (unit, mul) in [
        ("兆", 1_000_000_000_000i64),
        ("億", 100_000_000),
        ("万", 10_000),
    ] {
        if let Some((head, tail)) = rest.split_once(unit) {
            let h = if head.is_empty() {
                1
            } else {
                kanji_to_u32(head)? as i64
            };
            total += h * mul;
            rest = tail;
        }
    }
    if !rest.is_empty() {
        total += kanji_to_u32(rest)? as i64;
    }
    Some(total)
}

fn op_role(op: Option<regex::Match>) -> Option<String> {
    op.map(|m| match m.as_str() {
        "以上" | "を下らない" => "Ge",
        "以下" => "Le",
        "未満" | "に満たない" => "Lt",
        _ => "Gt",
    })
    .map(str::to_string)
}

/// 1 文の金額・割合・数量。位置が重ならないように、長い規則から。
/// `exclude` は先に取った範囲（時間表現「二週間に一回以上」の中の「一回以上」を取らない）
pub fn amount_candidates(
    sentence: &StableId,
    text: &str,
    exclude: &[(usize, usize)],
) -> Vec<Candidate> {
    let r = rules();
    let mut out: Vec<Candidate> = Vec::new();
    let mut taken: Vec<(usize, usize)> = r
        .sanction
        .find_iter(text)
        .map(|m| (m.start(), m.end()))
        .collect();
    taken.extend_from_slice(exclude);
    // 読替え規定の「」の中は読替え先の字句
    if text.contains("とあるのは") || text.contains("読み替える") {
        let (mut depth, mut open) = (0usize, 0usize);
        for (i, ch) in text.char_indices() {
            match ch {
                '「' => {
                    if depth == 0 {
                        open = i;
                    }
                    depth += 1;
                }
                '」' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        taken.push((open, i + ch.len_utf8()));
                    }
                }
                _ => {}
            }
        }
    }
    let overlaps =
        |taken: &[(usize, usize)], s: usize, e: usize| taken.iter().any(|(a, b)| s < *b && *a < e);
    // 「第N号」「第N条」の数詞は番号
    let numbered = |s: usize| text[..s].ends_with('第');
    let mut push = |out: &mut Vec<Candidate>, taken: &mut Vec<(usize, usize)>, c: Candidate| {
        let (s, e) = (c.evidence.start, c.evidence.end);
        if !overlaps(taken, s, e) && !numbered(s) {
            taken.push((s, e));
            out.push(c);
        }
    };
    for c in r.rate.captures_iter(text) {
        let m = c.get(0).unwrap();
        let n = kanji_to_u32(&c["n"]).unwrap_or(0) as i64;
        // 年一割 = 10%、年五分 = 5%（利率の慣用）
        let permille = match &c["u"] {
            "割" => n * 100,
            "分" => n * 10,
            _ => n * 10,
        };
        push(
            &mut out,
            &mut taken,
            Candidate::new(
                Field::Rate,
                sentence,
                text,
                m.start(),
                m.end(),
                "amount:annual_rate",
            )
            .value(ValueKind::Permille, permille.to_string(), Some("‰/year")),
        );
    }
    for c in r.money.captures_iter(text) {
        let m = c.get(0).unwrap();
        let mut cand = Candidate::new(
            Field::Money,
            sentence,
            text,
            m.start(),
            m.end(),
            "amount:yen",
        );
        match kanji_big(&c["n"]) {
            Some(y) => cand = cand.value(ValueKind::Yen, y.to_string(), Some("JPY")),
            None => cand = cand.confidence(Confidence::Low),
        }
        if let Some(r) = op_role(c.name("op")) {
            cand = cand.role(r);
        }
        push(&mut out, &mut taken, cand);
    }
    for c in r.fraction.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (d, n) = (
            kanji_to_u32(&c["d"]).unwrap_or(0) as i64,
            kanji_to_u32(&c["n"]).unwrap_or(0) as i64,
        );
        let mut cand = Candidate::new(
            Field::Ratio,
            sentence,
            text,
            m.start(),
            m.end(),
            "amount:fraction",
        );
        if d > 0 {
            cand = cand.value(ValueKind::Permille, (n * 1000 / d).to_string(), Some("‰"));
        }
        if let Some(r) = op_role(c.name("op")) {
            cand = cand.role(r);
        }
        push(&mut out, &mut taken, cand);
    }
    for c in r.percent.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (int, frac) = c["n"]
            .split_once('・')
            .map(|(a, b)| (a, Some(b)))
            .unwrap_or((&c["n"], None));
        let mut cand = Candidate::new(
            Field::Ratio,
            sentence,
            text,
            m.start(),
            m.end(),
            "amount:percent",
        );
        if let Some(i) = kanji_to_u32(int) {
            let f = frac.and_then(kanji_to_u32).unwrap_or(0) as i64;
            cand = cand.value(
                ValueKind::Permille,
                (i as i64 * 10 + f).to_string(),
                Some("‰"),
            );
        }
        if let Some(r) = op_role(c.name("op")) {
            cand = cand.role(r);
        }
        push(&mut out, &mut taken, cand);
    }
    for c in r.area.captures_iter(text) {
        let m = c.get(0).unwrap();
        let mut cand = Candidate::new(
            Field::Quantity,
            sentence,
            text,
            m.start(),
            m.end(),
            "amount:area",
        );
        if let Some(n) = kanji_big(&c["n"]) {
            cand = cand.value(ValueKind::Count, n.to_string(), Some(&c["u"]));
        }
        if let Some(r) = op_role(c.name("op")) {
            cand = cand.role(r);
        }
        push(&mut out, &mut taken, cand);
    }
    for c in r.count.captures_iter(text) {
        let m = c.get(0).unwrap();
        // 「一年以上」「六月以内」は期間（temporal）。単位が無い「二以上」は個数
        let after: String = text[m.end()..].chars().take(1).collect();
        if c.name("u").is_none() && matches!(after.as_str(), "年" | "月" | "日" | "週") {
            continue;
        }
        let unit = c.name("u").map(|u| u.as_str()).unwrap_or("");
        let mut cand = Candidate::new(
            Field::Quantity,
            sentence,
            text,
            m.start(),
            m.end(),
            "amount:count",
        );
        if let Some(n) = kanji_big(&c["n"]) {
            cand = cand.value(
                ValueKind::Count,
                n.to_string(),
                (!unit.is_empty()).then_some(unit),
            );
        }
        if let Some(r) = op_role(c.name("op2")) {
            cand = cand.role(r);
        }
        push(&mut out, &mut taken, cand);
    }
    out.sort_by_key(|c| c.evidence.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_ratio_count() {
        let s = StableId("x/main/art:1/para:1/sent:1".into());
        let t = "資本金五千万円以上の株式会社は、十分の三をこえる額の手付金を、三人以上の委員で、二以上の都道府県に、年一割の割合による利息とともに、百分の五以下で支払う。";
        let cs = amount_candidates(&s, t, &[]);
        let raws: Vec<(&str, Field, Option<&str>)> = cs
            .iter()
            .map(|c| (c.raw.as_str(), c.field, c.normalized.as_deref()))
            .collect();
        assert_eq!(
            raws,
            vec![
                ("五千万円以上", Field::Money, Some("50000000")),
                ("十分の三をこえる", Field::Ratio, Some("300")),
                ("三人以上", Field::Quantity, Some("3")),
                ("二以上", Field::Quantity, Some("2")),
                ("年一割", Field::Rate, Some("100")),
                ("百分の五以下", Field::Ratio, Some("50")),
            ]
        );
        assert_eq!(cs[0].role.as_deref(), Some("Ge"));
        // 刑の金額と法令番号・期間は取らない
        let cs = amount_candidates(
            &s,
            "三十万円以下の罰金に処する。平成三年法律第九十号。一年以上の期間。第二号",
            &[],
        );
        assert!(cs.is_empty(), "{cs:?}");
        assert_eq!(kanji_big("一億二千万"), Some(120_000_000));
    }
}
