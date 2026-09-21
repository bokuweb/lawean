//! 期間・時点・施行の表現を規則で取る（層 1、ADR-0008。docs/11 L1 の「期間・金額」の行）。
//!
//! 法制執務で用語が統一された**閉じた語彙**なので規則で足りる:
//! 「〜の日から十年」「〜の日から起算して六月を経過した日」「〜の日から三月を経過することによって」「〜後二月以内に」
//! 「期間の満了の一年前から六月前までの間」「一年前までに」「三十年以上五十年未満」「三十年とする」
//! 「公布の日から起算して一年を超えない範囲内において政令で定める日から施行する」。
//!
//! 出力は `TimeExpr`（原文の位置つき）。数詞は月数に落とせるものは月数、日・週は日数で持つ。
//! Z3 の暦（`lawean-verify::temporal`）に渡すのは `Period` / `Window` / `Elapsed` / `Enforcement`。
//! 取れない時間表現（「年一割」は利率、「令和三年法律第三十七号」は法令番号）は対象外として区別する。

use lawean_resolve::numeral::kanji_to_u32;
use regex::Regex;
use std::sync::OnceLock;

const N: &str = "[一二三四五六七八九十百千]+";
/// 施行期日の句: 「公布の日」「令和三年九月一日」「公布の日から起算して一年を超えない範囲内において政令で定める日」
const ENF: &str = r"(?P<base>公布の日|(?P<era>明治|大正|昭和|平成|令和)(?P<y>{N}|元)年(?P<m>{N})月(?P<d>{N})日)(?P<until>までの間において政令で定める日)?(?:から起算して(?:(?P<yy>{N})年(?P<mm>{N})月|(?P<n>{N})(?P<u>年|月|日))(?P<how>を経過した日|を経過する日|(?:を超えない|をこえない)範囲内(?:において|で)(?:、各規定につき、)?政令で定める日))?";
/// 他法令の施行日に依る施行期日: 「民法改正法の施行の日から施行する」「刑法等一部改正法施行日から施行する」
const ENF_OTHER: &str =
    r"(?P<law>[^、。「」（）]{2,40}?)(?:（[^）]*）)?(?:の施行の日|施行日|の施行日)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Year,
    Month,
    Week,
    Day,
    Hour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dur {
    pub n: u32,
    pub unit: Unit,
}

impl Dur {
    /// 月数（年・月）。日・週は None
    pub fn months(&self) -> Option<i64> {
        match self.unit {
            Unit::Year => Some(self.n as i64 * 12),
            Unit::Month => Some(self.n as i64),
            _ => None,
        }
    }
    /// 日数（日・週）。年・月は None（暦で数えるので日数に固定できない）
    pub fn days(&self) -> Option<i64> {
        match self.unit {
            Unit::Day => Some(self.n as i64),
            Unit::Week => Some(self.n as i64 * 7),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmpOp {
    Ge,
    Le,
    Lt,
    Gt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeKind {
    /// 「三十年とする」「最後の二年分」「（…二十年）」— 長さそのものが値
    DurationValue(Dur),
    /// 構造が取れなかった裸の「N年」。被覆率の分母に入れ、分子には入れない
    Duration(Dur),
    /// 「施行後四年を目途として」— おおよその時期
    Approx { event: String, dur: Dur },
    /// 「毎年三月、六月、九月及び十二月」「登録月の一日」「四月一日」— 暦の月・日（期間ではない）
    Calendar {
        month: Option<u32>,
        day: Option<u32>,
    },
    /// 「昭和五十八年六月三日」「平成二十七年（国勢調査）」— 元号つきの年・日付
    EraDate {
        era: String,
        y: u32,
        month: Option<u32>,
        day: Option<u32>,
    },
    /// 「十年ごとに」
    Every(Dur),
    /// 「選挙の期日後五日に当たる日」— 事象から N 日目の日（初日を含めて数える）
    NthDay { event: String, dur: Dur },
    /// 「任期が終る日の前三十日以内」— 事象の前 N 日の間
    WithinBefore { event: String, dur: Dur },
    /// 「更新の日から十年」「〜の日から起算して一年」— 事象からの期間。`counted_from_first` は「起算して」の有無
    Period {
        event: String,
        dur: Dur,
        counted_from_first: bool,
    },
    /// 「〜の日から六月を経過することによって」「〜を経過した後」「〜を経過した日」「〜を経過する日」
    Elapsed {
        event: String,
        dur: Dur,
        boundary: Boundary,
    },
    /// 「〜の後二月以内に」「〜から一年を超えない範囲内において」
    Within { event: String, dur: Dur },
    /// 「期間の満了の一年前から六月前までの間」
    Window {
        event: String,
        from_before: Dur,
        to_before: Dur,
    },
    /// 「一年前までに」「その一年前までに」
    Before { event: String, dur: Dur },
    /// 「三十年以上」「五十年未満」「一年を超えない」
    Compare { dur: Dur, op: CmpOp },
    /// 附則の施行期日: 「公布の日から施行する」「公布の日から起算して一年を超えない範囲内において政令で定める日から施行する」「令和三年九月一日から施行する」
    Enforcement(Enforcement),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Boundary {
    /// 「六月を経過する日」= 満了日
    On,
    /// 「六月を経過した日」「経過した後」「経過することによって」= 満了日の翌日以後
    After,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enforcement {
    /// 「公布の日から施行する」
    Promulgation,
    /// 「民法改正法の施行の日から施行する」— 他法令（略称のことが多い）の施行日。`LawSpace` で解く
    OtherLaw(String),
    /// 「令和三年九月一日から施行する」（年は元号のまま。西暦は呼ぶ側で）
    Date { era: String, y: u32, m: u32, d: u32 },
    /// 「公布の日から起算して一年を経過した日から施行する」
    ElapsedFromPromulgation(Dur),
    /// 「公布の日から起算して一年を超えない範囲内において政令で定める日から施行する」— 上限だけ決まる
    ByCabinetOrderWithin(Dur),
    /// 「令和五年二月一日までの間において政令で定める日から施行する」— 上限が暦日
    ByCabinetOrderUntil { era: String, y: u32, m: u32, d: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeExpr {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub kind: TimeKind,
}

fn re(s: &str) -> Regex {
    Regex::new(&s.replace("{N}", N)).unwrap()
}

fn dur(n: &str, u: &str) -> Dur {
    Dur {
        n: kanji_to_u32(n).unwrap_or(0),
        unit: match u {
            "年" => Unit::Year,
            "月" => Unit::Month,
            "週" | "週間" => Unit::Week,
            "時間" => Unit::Hour,
            _ => Unit::Day,
        },
    }
}

struct Rules {
    enforcement: Regex,
    enforcement_other: Regex,
    enforcement_list: Regex,
    within_event: Regex,
    sanction_term: Regex,
    enforcement_clause: Regex,
    window: Regex,
    elapsed: Regex,
    within: Regex,
    period: Regex,
    before: Regex,
    compare: Regex,
    approx: Regex,
    calendar: Regex,
    era_date: Regex,
    quoted: Regex,
    every: Regex,
    nth_day: Regex,
    within_before: Regex,
    duration_value: Regex,
    duration: Regex,
    rate: Regex,
    law_num: Regex,
}

fn rules() -> &'static Rules {
    static R: OnceLock<Rules> = OnceLock::new();
    R.get_or_init(|| Rules {
        enforcement: re(&format!("{ENF}(?:（[^）]*）)?から施行する")),
        sanction_term: re(r"{N}(?:年|月)以下の(?:懲役|禁錮|拘禁刑)"),
        enforcement_clause: re(&format!("^(?:この法律は、)?{ENF}(?:（[^）]*）)?(?:から施行する)?。?$")),
        enforcement_other: re(&format!("^(?:この法律は、)?{ENF_OTHER}から施行する。?$")),
        // ただし書きの中の「…は、公布の日から、…は、公布の日から起算して二年を経過する日から施行する」
        enforcement_list: re(&format!("(?P<scope>[^。]*?)(?:は、|は){ENF}(?:（[^）]*）)?から(?:施行する|、)")),
        window: re(r"(?P<ev>[^、。（）]{1,30}?)の(?P<n1>{N})(?P<u1>年|月|日)前から(?P<n2>{N})(?P<u2>年|月|日)前までの間"),
        elapsed: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の日から|の時から|から|の後、|後、|の後|後)(?P<ct>起算して)?(?P<n>{N})(?P<u>年|月|日|週間)を経過(?P<b>した日|した後|する日|する時|した時|した場合|する場合|することによって|したとき|すること|し)"),
        within: re(r"(?P<n>{N})(?P<u>年|月|日|週間|時間)(?:以内|を超えない範囲内)"),
        within_event: re(r"(?P<ev>(?:[^、。（）]|（[^（）]*）){1,60}?)(?:の後|後|から|の日から)(?:起算して)?$"),
        period: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の日|)から(?P<ct>起算して)?(?P<n>{N})(?P<u>年|月|日|週間)(?:間(?:（[^）]*）)?(?:存続する|とする|の間)?|(?:（[^）]*）)?(?:存続する|とする|の間))"),
        approx: re(r"(?P<ev>[^、。（）]{1,30}?)(?:の後|後)(?P<n>{N})(?P<u>年|月|日)(?:以内)?を目途"),
        calendar: re(r"(?:(?P<pre>毎年|、|及び|又は|翌年の|の年の)(?P<m>{N})月(?:(?P<d>{N})日|末日)?)|(?:(?P<pre2>から)(?P<m3>{N})月まで)|(?:(?P<m2>{N})月(?:(?P<d2>{N})日|末日|の第{N}(?:日曜日|月曜日|火曜日|水曜日|木曜日|金曜日|土曜日)))|(?:月の(?P<d3>{N})日)"),
        era_date: re(r"(?P<era>明治|大正|昭和|平成|令和)(?P<y>{N}|元)年(?:(?P<m>{N})月(?:(?P<d>{N})日)?)?"),
        quoted: re(r"「{N}(?:年|月|日|週間)(?:間)?」"),
        every: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?:ごとに|ごと|に{N}回(?:以上)?)"),
        nth_day: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の後|後|の日から|から)(?P<n>{N})(?P<u>日|月|年|週間)に当たる日"),
        within_before: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の前|以前|前)(?P<n>{N})(?P<u>日|月|年|週間)以内"),
        duration_value: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?:間)?(?P<tail>とする|分|）|間)"),
        before: re(r"(?:(?P<ev>(?:[^、。（）]|（[^（）]*）){1,60}?)(?:より|の|)(?:少なくとも)?(?P<n>{N})(?P<u>年|月|日|週間)前までに)|(?:(?:(?P<ev2>[^、。（）]{1,30}?)(?:より|の))?(?:少なくとも)?(?P<n2>{N})(?P<u2>年|月|日|週間)前に)"),
        compare: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?P<op>以上|以下|未満|を超えることができない|を超えない|を超える|を超え|より長い|より短い|に満たない|を下ることができない|を下つてはならない|を下ってはならない|を下回つてはならない|を下回ってはならない|を下回らない|を下らない)"),
        duration: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?:間)?"),
        rate: re(r"年{N}割|年{N}分|年{N}パーセント"),
        law_num: re(r"(?:明治|大正|昭和|平成|令和)(?:{N}|元)年(?:法律|政令|勅令|省令|規則)第{N}号"),
    })
}

/// 事象の句の掃除: 先頭の「又は」「若しくは」「及び」「並びに」「が」「は」「を」「に」を落とし、
/// 「A又はB…の日」は最後の選択肢だけを事象にする。戻り値は (事象, 落とした先頭のバイト数)。
/// 「そ」だけになったら「その日」に戻す（「その日から三十日以内」）
fn clean_event(ev: &str) -> (String, usize) {
    let mut s = ev;
    let mut dropped = 0;
    loop {
        let mut again = false;
        for pre in [
            "又は",
            "若しくは",
            "及び",
            "並びに",
            "が",
            "は",
            "を",
            "に",
            "も",
            "の",
            "で",
            "翌日以後",
        ] {
            if let Some(rest) = s.strip_prefix(pre) {
                if !rest.is_empty() {
                    dropped += pre.len();
                    s = rest;
                    again = true;
                }
            }
        }
        if !again {
            break;
        }
    }
    // 「A にあっては B の日」は B が事象。「A 又は B の日」は両方が事象（「放棄又は申入れがあった日」）なので切らない
    let mut cut = 0;
    for sep in [
        "又は同号に規定する",
        "又は同項に規定する",
        "又は同条に規定する",
        "にあっては",
        "においては",
        "については",
        "以内に",
        "以内",
        "者で",
        "もので",
        "法人で",
        "）で",
        "者若しくは",
        "者又は",
        "とき若しくは",
        "とき又は",
    ] {
        if let Some(i) = s.rfind(sep) {
            let keep = if sep.starts_with("又は同") {
                "又は".len()
            } else {
                sep.len()
            };
            cut = cut.max(i + keep);
        }
    }
    if cut > 0 && cut < s.len() {
        dropped += cut;
        s = &s[cut..];
    }
    let out = match s {
        "そ" | "こ" | "あ" => format!("{s}の日"),
        _ => s.to_string(),
    };
    (out, dropped)
}

fn enforcement_of(c: &regex::Captures) -> Enforcement {
    // 「一年六月」は月数に畳む
    let d = match (c.name("yy"), c.name("n")) {
        (Some(yy), _) => Some(Dur {
            n: kanji_to_u32(yy.as_str()).unwrap_or(0) * 12 + kanji_to_u32(&c["mm"]).unwrap_or(0),
            unit: Unit::Month,
        }),
        (None, Some(n)) => Some(dur(n.as_str(), &c["u"])),
        _ => None,
    };
    match (d, c.name("how")) {
        (Some(d), Some(how)) if how.as_str().starts_with("を経過") => {
            Enforcement::ElapsedFromPromulgation(d)
        }
        (Some(d), Some(_)) => Enforcement::ByCabinetOrderWithin(d),
        _ => match c.name("era") {
            Some(era) if c.name("until").is_some() => Enforcement::ByCabinetOrderUntil {
                era: era.as_str().into(),
                y: if &c["y"] == "元" {
                    1
                } else {
                    kanji_to_u32(&c["y"]).unwrap_or(0)
                },
                m: kanji_to_u32(&c["m"]).unwrap_or(0),
                d: kanji_to_u32(&c["d"]).unwrap_or(0),
            },
            Some(era) => Enforcement::Date {
                era: era.as_str().into(),
                y: if &c["y"] == "元" {
                    1
                } else {
                    kanji_to_u32(&c["y"]).unwrap_or(0)
                },
                m: kanji_to_u32(&c["m"]).unwrap_or(0),
                d: kanji_to_u32(&c["d"]).unwrap_or(0),
            },
            None => Enforcement::Promulgation,
        },
    }
}

/// 施行期日の句だけの文字列を読む。附則第一条の本文「この法律は、…から施行する。」と、
/// 各号の日付欄「公布の日から起算して九月を超えない範囲内において政令で定める日」の両方
pub fn parse_enforcement(text: &str) -> Option<Enforcement> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let r = rules();
    if let Some(c) = r.enforcement_clause.captures(&text) {
        return Some(enforcement_of(&c));
    }
    r.enforcement_other
        .captures(&text)
        .map(|c| Enforcement::OtherLaw(c["law"].trim_start_matches("この法律は、").to_string()))
}

/// 1 文の中の「X は、<施行期日>から」の列挙（ただし書き）。(範囲の句, 施行期日)
pub fn enforcement_list(text: &str) -> Vec<(String, Enforcement)> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    rules()
        .enforcement_list
        .captures_iter(&text)
        .map(|c| {
            (
                c["scope"].trim_start_matches("ただし、").to_string(),
                enforcement_of(&c),
            )
        })
        .collect()
}

/// 1 文の中の時間表現をすべて取る（重ならないように、長い規則から順に）
pub fn time_exprs(text: &str) -> Vec<TimeExpr> {
    let r = rules();
    let mut out: Vec<TimeExpr> = Vec::new();
    let mut taken: Vec<(usize, usize)> = Vec::new();
    // 除外: 利率・法令番号の中の「年」、読替え規定の「「三日」とあるのは」の中の字句、刑の「一年以下の懲役」
    for m in r
        .rate
        .find_iter(text)
        .chain(r.law_num.find_iter(text))
        .chain(r.quoted.find_iter(text))
        .chain(r.sanction_term.find_iter(text))
    {
        taken.push((m.start(), m.end()));
    }
    // 読替え規定の「」の中は読替え先の字句。ここでは取らない
    if text.contains("とあるのは") || text.contains("読み替える") {
        let mut depth = 0usize;
        let mut open = 0usize;
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
    let push = |out: &mut Vec<TimeExpr>,
                taken: &mut Vec<(usize, usize)>,
                s: usize,
                e: usize,
                kind: TimeKind| {
        if s < e && !overlaps(taken, s, e) {
            taken.push((s, e));
            out.push(TimeExpr {
                text: text[s..e].to_string(),
                start: s,
                end: e,
                kind,
            });
        }
    };
    // 事象の句: 掃除して、落とした分だけ範囲の先頭を進める
    let ev = |c: &regex::Captures| -> (String, usize) {
        c.name("ev")
            .map(|x| clean_event(x.as_str()))
            .unwrap_or_default()
    };
    for c in r.enforcement.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Enforcement(enforcement_of(&c)),
        );
    }
    // 附則の号の日付欄（「公布の日から起算して一年を超えない範囲内において政令で定める日」）は文全体が施行期日
    if let Some(c) = r.enforcement_clause.captures(text.trim_end_matches('。')) {
        if c.name("n").is_some() || c.name("era").is_some() || text.starts_with("公布の日") {
            let m = c.get(0).unwrap();
            push(
                &mut out,
                &mut taken,
                m.start(),
                m.end(),
                TimeKind::Enforcement(enforcement_of(&c)),
            );
        }
    }
    for c in r.window.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (event, drop) = ev(&c);
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::Window {
                event,
                from_before: dur(&c["n1"], &c["u1"]),
                to_before: dur(&c["n2"], &c["u2"]),
            },
        );
    }
    for c in r.elapsed.captures_iter(text) {
        let m = c.get(0).unwrap();
        let boundary = if &c["b"] == "する日" {
            Boundary::On
        } else {
            Boundary::After
        };
        let (event, drop) = ev(&c);
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::Elapsed {
                event,
                dur: dur(&c["n"], &c["u"]),
                boundary,
            },
        );
    }
    // 「公布後一年以内を目途」は within より先に
    for c in r.approx.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (event, drop) = ev(&c);
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::Approx {
                event,
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    for c in r.within_before.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (event, drop) = ev(&c);
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::WithinBefore {
                event,
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    // 「二週間以内にその請求の日から四週間以内」: 数詞の側から見つけ、事象はその直前の句（末尾が「から」「後」）
    for c in r.within.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (event, start) = match r.within_event.captures(&text[..m.start()]) {
            Some(e) => {
                let em = e.get(0).unwrap();
                // 事象は左に長く取れるので、除外した範囲（法令番号など）を含んだら、その後ろから
                let floor = taken
                    .iter()
                    .filter(|(a, b)| *a < m.start() && *b > em.start())
                    .map(|(_, b)| *b)
                    .max();
                match floor {
                    Some(f) if f > em.start() && f < m.start() => {
                        match r.within_event.captures(&text[f..m.start()]) {
                            Some(e2) => {
                                let (event, drop) = clean_event(&e2["ev"]);
                                (event, f + e2.get(0).unwrap().start() + drop)
                            }
                            None => (String::new(), m.start()),
                        }
                    }
                    _ => {
                        let (event, drop) = clean_event(&e["ev"]);
                        (event, em.start() + drop)
                    }
                }
            }
            None => (String::new(), m.start()),
        };
        push(
            &mut out,
            &mut taken,
            start,
            m.end(),
            TimeKind::Within {
                event,
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    for c in r.period.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (event, drop) = ev(&c);
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::Period {
                event,
                dur: dur(&c["n"], &c["u"]),
                counted_from_first: c.name("ct").is_some(),
            },
        );
    }
    for c in r.before.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (n, u, (event, drop)) = match c.name("n") {
            Some(n) => {
                // 事象は左に長く取れるので、除外した範囲（法令番号など）を含んだら、その後ろから
                let e = c.name("ev").unwrap();
                let floor = taken
                    .iter()
                    .filter(|(a, b)| *a < e.end() && *b > e.start())
                    .map(|(_, b)| *b)
                    .max()
                    .filter(|f| *f > e.start() && *f < e.end());
                match floor {
                    Some(f) => {
                        let (event, drop) = clean_event(&text[f..e.end()]);
                        (n.as_str(), &c["u"], (event, f - m.start() + drop))
                    }
                    None => (n.as_str(), &c["u"], ev(&c)),
                }
            }
            None => (
                &c["n2"],
                &c["u2"],
                c.name("ev2")
                    .map(|x| clean_event(x.as_str()))
                    .unwrap_or_default(),
            ),
        };
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::Before {
                event,
                dur: dur(n, u),
            },
        );
    }
    for c in r.era_date.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::EraDate {
                era: c["era"].into(),
                y: if &c["y"] == "元" {
                    1
                } else {
                    kanji_to_u32(&c["y"]).unwrap_or(0)
                },
                month: c.name("m").and_then(|x| kanji_to_u32(x.as_str())),
                day: c.name("d").and_then(|x| kanji_to_u32(x.as_str())),
            },
        );
    }
    for c in r.nth_day.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (event, drop) = ev(&c);
        push(
            &mut out,
            &mut taken,
            m.start() + drop,
            m.end(),
            TimeKind::NthDay {
                event,
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    for c in r.every.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Every(dur(&c["n"], &c["u"])),
        );
    }
    for c in r.compare.captures_iter(text) {
        let m = c.get(0).unwrap();
        let op = match &c["op"] {
            "以上"
            | "を下ることができない"
            | "を下つてはならない"
            | "を下ってはならない"
            | "を下らない"
            | "を下回つてはならない"
            | "を下回ってはならない"
            | "を下回らない" => CmpOp::Ge,
            "以下" | "を超えない" | "を超えることができない" => CmpOp::Le,
            "未満" | "より短い" | "に満たない" => CmpOp::Lt,
            _ => CmpOp::Gt,
        };
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Compare {
                dur: dur(&c["n"], &c["u"]),
                op,
            },
        );
    }
    for c in r.duration_value.captures_iter(text) {
        let (s0, e0) = (c.get(0).unwrap().start(), c.name("tail").unwrap().start());
        push(
            &mut out,
            &mut taken,
            s0,
            e0,
            TimeKind::DurationValue(dur(&c["n"], &c["u"])),
        );
    }
    for c in r.calendar.captures_iter(text) {
        let m = c.get(0).unwrap();
        let month = c
            .name("m")
            .or(c.name("m2"))
            .or(c.name("m3"))
            .and_then(|x| kanji_to_u32(x.as_str()));
        let day = c
            .name("d")
            .or(c.name("d2"))
            .or(c.name("d3"))
            .and_then(|x| kanji_to_u32(x.as_str()));
        // 「、六月を経過」のように期間の途中に当たるものは除く: 直後が「を」「以」「間」なら期間
        let after: String = text[m.end()..].chars().take(1).collect();
        if matches!(after.as_str(), "を" | "以" | "間" | "前" | "後") {
            continue;
        }
        let s0 = c
            .name("pre")
            .or(c.name("pre2"))
            .map(|p| p.end())
            .unwrap_or(m.start());
        let e0 = m.end()
            - if text[s0..m.end()].ends_with("まで") {
                "まで".len()
            } else {
                0
            };
        push(
            &mut out,
            &mut taken,
            s0,
            e0,
            TimeKind::Calendar { month, day },
        );
    }
    for c in r.duration.captures_iter(text) {
        let m = c.get(0).unwrap();
        // 欄の中身が「十年」だけ（表の値）なら値
        let kind = if m.start() == 0 && m.end() == text.len() {
            TimeKind::DurationValue(dur(&c["n"], &c["u"]))
        } else {
            TimeKind::Duration(dur(&c["n"], &c["u"]))
        };
        push(&mut out, &mut taken, m.start(), m.end(), kind);
    }
    out.sort_by_key(|e| e.start);
    out
}

/// 文の中の「数詞 + 年/月/日/週」のうち、利率・法令番号を除いたものが、すべて `Duration` 以外の構造で取れているか。
/// 被覆率の計測用: (時間表現の数, そのうち裸の Duration で終わった数)
pub fn coverage(text: &str) -> (usize, usize) {
    let es = time_exprs(text);
    let bare = es
        .iter()
        .filter(|e| matches!(e.kind, TimeKind::Duration(_)))
        .count();
    (es.len(), bare)
}

// ---------------------------------------------------------------- 候補（共通の契約）

use crate::candidate::{Candidate, Confidence, Field, ValueKind};
use crate::suppl::era_year;
use lawean_source::StableId;

fn dur_value(c: Candidate, d: &Dur) -> Candidate {
    match (d.months(), d.days()) {
        (Some(m), _) => c.value(ValueKind::Months, m.to_string(), Some("months")),
        (_, Some(n)) => c.value(ValueKind::Days, n.to_string(), Some("days")),
        _ => c,
    }
}

impl TimeExpr {
    /// 共通の候補の形に。`text` は根拠の文の本文（`time_exprs` に渡したもの）
    pub fn to_candidate(&self, sentence: &StableId, text: &str) -> Candidate {
        use TimeKind::*;
        let mk = |f: Field, reason: &'static str| {
            Candidate::new(f, sentence, text, self.start, self.end, reason)
        };
        match &self.kind {
            DurationValue(d) => dur_value(mk(Field::DurationValue, "temporal:duration_value"), d),
            Duration(d) => dur_value(mk(Field::Duration, "temporal:bare_duration"), d)
                .confidence(Confidence::Low),
            Approx { event, dur } => dur_value(mk(Field::Approx, "temporal:approx"), dur)
                .label(event.clone())
                .confidence(Confidence::Medium),
            Calendar { month, day } => {
                let c = mk(Field::CalendarDay, "temporal:calendar");
                match (month, day) {
                    (Some(m), Some(d)) => {
                        c.value(ValueKind::Text, format!("--{m:02}-{d:02}"), None)
                    }
                    (Some(m), None) => c.value(ValueKind::Text, format!("--{m:02}"), None),
                    _ => c,
                }
            }
            EraDate { era, y, month, day } => {
                let c = mk(Field::EraDate, "temporal:era_date");
                match (era_year(era, *y), month, day) {
                    (Some(yy), Some(m), Some(d)) => {
                        c.value(ValueKind::Date, format!("{yy:04}-{m:02}-{d:02}"), None)
                    }
                    (Some(yy), Some(m), None) => {
                        c.value(ValueKind::Text, format!("{yy:04}-{m:02}"), None)
                    }
                    (Some(yy), None, None) => c.value(ValueKind::Text, format!("{yy:04}"), None),
                    _ => c.confidence(Confidence::Low),
                }
            }
            Every(d) => dur_value(mk(Field::Every, "temporal:every"), d),
            NthDay { event, dur } => {
                dur_value(mk(Field::NthDay, "temporal:nth_day"), dur).label(event.clone())
            }
            WithinBefore { event, dur } => {
                dur_value(mk(Field::WithinBefore, "temporal:within_before"), dur)
                    .label(event.clone())
            }
            Period {
                event,
                dur,
                counted_from_first,
            } => dur_value(
                mk(
                    Field::Period,
                    if *counted_from_first {
                        "temporal:period_from_first"
                    } else {
                        "temporal:period"
                    },
                ),
                dur,
            )
            .label(event.clone()),
            Elapsed {
                event,
                dur,
                boundary,
            } => dur_value(
                mk(
                    Field::Elapsed,
                    match boundary {
                        Boundary::On => "temporal:elapsed_on",
                        Boundary::After => "temporal:elapsed_after",
                    },
                ),
                dur,
            )
            .label(event.clone())
            .role(match boundary {
                Boundary::On => "満了日",
                Boundary::After => "満了日の翌日以後",
            }),
            Within { event, dur } => {
                let c = dur_value(mk(Field::Within, "temporal:within"), dur);
                if event.is_empty() {
                    c.confidence(Confidence::Medium)
                } else {
                    c.label(event.clone())
                }
            }
            Window {
                event,
                from_before,
                to_before,
            } => mk(Field::Window, "temporal:window")
                .value(
                    ValueKind::Text,
                    format!(
                        "{}〜{}",
                        from_before.months().unwrap_or(0),
                        to_before.months().unwrap_or(0)
                    ),
                    Some("months_before"),
                )
                .label(event.clone()),
            Before { event, dur } => {
                let c = dur_value(mk(Field::Before, "temporal:before"), dur);
                if event.is_empty() {
                    c.confidence(Confidence::Medium)
                } else {
                    c.label(event.clone())
                }
            }
            Compare { dur, op } => {
                dur_value(mk(Field::Compare, "temporal:compare"), dur).role(format!("{op:?}"))
            }
            Enforcement(e) => {
                let c = mk(Field::Enforcement, "temporal:enforcement");
                match e {
                    self::Enforcement::Promulgation => c.value(ValueKind::Text, "公布の日", None),
                    self::Enforcement::ByCabinetOrderUntil { era, y, m, d } => {
                        match era_year(era, *y) {
                            Some(yy) => c
                                .value(ValueKind::Date, format!("{yy:04}-{m:02}-{d:02}"), None)
                                .role("その日までの間において政令で定める日"),
                            None => c.confidence(Confidence::Low),
                        }
                    }
                    self::Enforcement::OtherLaw(law) => c
                        .value(ValueKind::Text, format!("{law}の施行の日"), None)
                        .confidence(Confidence::Medium),
                    self::Enforcement::Date { era, y, m, d } => match era_year(era, *y) {
                        Some(yy) => {
                            c.value(ValueKind::Date, format!("{yy:04}-{m:02}-{d:02}"), None)
                        }
                        None => c.confidence(Confidence::Low),
                    },
                    self::Enforcement::ElapsedFromPromulgation(d) => {
                        dur_value(c, d).role("公布の日から起算して経過した日")
                    }
                    self::Enforcement::ByCabinetOrderWithin(d) => {
                        dur_value(c, d).role("公布の日から起算して超えない範囲内で政令で定める日")
                    }
                }
            }
        }
    }
}

/// 1 文の時間表現を候補で
pub fn time_candidates(sentence: &StableId, text: &str) -> Vec<Candidate> {
    time_exprs(text)
        .iter()
        .map(|e| e.to_candidate(sentence, text))
        .collect()
}
