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
const ENF: &str = r"(?P<base>公布の日|(?P<era>明治|大正|昭和|平成|令和)(?P<y>{N}|元)年(?P<m>{N})月(?P<d>{N})日)(?:から起算して(?P<n>{N})(?P<u>年|月|日)(?P<how>を経過した日|(?:を超えない|をこえない)範囲内(?:において|で)(?:、各規定につき、)?政令で定める日))?";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Year,
    Month,
    Week,
    Day,
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
    /// 「令和三年九月一日から施行する」（年は元号のまま。西暦は呼ぶ側で）
    Date { era: String, y: u32, m: u32, d: u32 },
    /// 「公布の日から起算して一年を経過した日から施行する」
    ElapsedFromPromulgation(Dur),
    /// 「公布の日から起算して一年を超えない範囲内において政令で定める日から施行する」— 上限だけ決まる
    ByCabinetOrderWithin(Dur),
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
            _ => Unit::Day,
        },
    }
}

struct Rules {
    enforcement: Regex,
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
        enforcement: re(&format!("{ENF}から施行する")),
        enforcement_clause: re(&format!("^(?:この法律は、)?{ENF}(?:から施行する)?。?$")),
        window: re(r"(?P<ev>[^、。（）]{1,30}?)の(?P<n1>{N})(?P<u1>年|月|日)前から(?P<n2>{N})(?P<u2>年|月|日)前までの間"),
        elapsed: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の日|の時|)から(?P<ct>起算して)?(?P<n>{N})(?P<u>年|月|日|週間)を経過(?P<b>した日|した後|する日|する時|した時|することによって|したとき|し)"),
        within: re(r"(?:(?P<ev>[^、。（）]{1,40}?)(?:の後|後|から|の日から))?(?P<n>{N})(?P<u>年|月|日|週間)(?:以内|を超えない範囲内)"),
        period: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の日|)から(?P<ct>起算して)?(?P<n>{N})(?P<u>年|月|日|週間)(?:間)?(?:（[^）]*）)?(?:存続する|とする|の間)"),
        approx: re(r"(?P<ev>[^、。（）]{1,30}?)(?:の後|後)(?P<n>{N})(?P<u>年|月|日)を目途"),
        calendar: re(r"(?:(?P<pre>毎年|、|及び|又は|翌年の|の年の)(?P<m>{N})月(?:(?P<d>{N})日|末日)?)|(?:(?P<m2>{N})月(?:(?P<d2>{N})日|末日|の第{N}(?:日曜日|月曜日|火曜日|水曜日|木曜日|金曜日|土曜日)))|(?:月の(?P<d3>{N})日)"),
        era_date: re(r"(?P<era>明治|大正|昭和|平成|令和)(?P<y>{N}|元)年(?:(?P<m>{N})月(?:(?P<d>{N})日)?)?"),
        quoted: re(r"「{N}(?:年|月|日|週間)(?:間)?」"),
        every: re(r"(?P<n>{N})(?P<u>年|月|日|週間)ごとに"),
        nth_day: re(r"(?P<ev>[^、。（）]{1,40}?)(?:の後|後|の日から|から)(?P<n>{N})(?P<u>日|月|年)に当たる日"),
        within_before: re(r"(?P<ev>[^、。（）]{1,40}?)の前(?P<n>{N})(?P<u>日|月|年)以内"),
        duration_value: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?:間)?(?P<tail>とする|分|）|間)"),
        before: re(r"(?:(?P<ev>[^、。（）]{1,30}?)(?:の|)(?P<n>{N})(?P<u>年|月|日)前までに)|(?:(?:少なくとも)?(?P<n2>{N})(?P<u2>年|月|日)前に)"),
        compare: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?P<op>以上|以下|未満|を超えない|を超える|を超え|より長い|より短い|に満たない)"),
        duration: re(r"(?P<n>{N})(?P<u>年|月|日|週間)(?:間)?"),
        rate: re(r"年{N}割|年{N}分|年{N}パーセント"),
        law_num: re(r"(?:明治|大正|昭和|平成|令和){N}年(?:法律|政令|勅令|省令|規則)第{N}号"),
    })
}

fn enforcement_of(c: &regex::Captures) -> Enforcement {
    match (c.name("n"), c.name("how")) {
        (Some(n), Some(how)) if how.as_str().starts_with("を経過") => {
            Enforcement::ElapsedFromPromulgation(dur(n.as_str(), &c["u"]))
        }
        (Some(n), Some(_)) => Enforcement::ByCabinetOrderWithin(dur(n.as_str(), &c["u"])),
        _ => match c.name("era") {
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
    rules()
        .enforcement_clause
        .captures(&text)
        .map(|c| enforcement_of(&c))
}

/// 1 文の中の時間表現をすべて取る（重ならないように、長い規則から順に）
pub fn time_exprs(text: &str) -> Vec<TimeExpr> {
    let r = rules();
    let mut out: Vec<TimeExpr> = Vec::new();
    let mut taken: Vec<(usize, usize)> = Vec::new();
    // 除外: 利率・法令番号の中の「年」、読替え規定の「「三日」とあるのは」の中の字句
    for m in r
        .rate
        .find_iter(text)
        .chain(r.law_num.find_iter(text))
        .chain(r.quoted.find_iter(text))
    {
        taken.push((m.start(), m.end()));
    }
    let overlaps =
        |taken: &[(usize, usize)], s: usize, e: usize| taken.iter().any(|(a, b)| s < *b && *a < e);
    let push = |out: &mut Vec<TimeExpr>,
                taken: &mut Vec<(usize, usize)>,
                s: usize,
                e: usize,
                kind: TimeKind| {
        if !overlaps(taken, s, e) {
            taken.push((s, e));
            out.push(TimeExpr {
                text: text[s..e].to_string(),
                start: s,
                end: e,
                kind,
            });
        }
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
    for c in r.window.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Window {
                event: c["ev"].into(),
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
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Elapsed {
                event: c["ev"].into(),
                dur: dur(&c["n"], &c["u"]),
                boundary,
            },
        );
    }
    for c in r.within.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Within {
                event: c
                    .name("ev")
                    .map(|x| x.as_str().to_string())
                    .unwrap_or_default(),
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    for c in r.period.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Period {
                event: c["ev"].into(),
                dur: dur(&c["n"], &c["u"]),
                counted_from_first: c.name("ct").is_some(),
            },
        );
    }
    for c in r.before.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (n, u, ev) = match c.name("n") {
            Some(n) => (n.as_str(), &c["u"], c["ev"].to_string()),
            None => (&c["n2"], &c["u2"], String::new()),
        };
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Before {
                event: ev,
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
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::NthDay {
                event: c["ev"].into(),
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    for c in r.within_before.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::WithinBefore {
                event: c["ev"].into(),
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
    for c in r.calendar.captures_iter(text) {
        let m = c.get(0).unwrap();
        let month = c
            .name("m")
            .or(c.name("m2"))
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
        let s0 = c.name("pre").map(|p| p.end()).unwrap_or(m.start());
        push(
            &mut out,
            &mut taken,
            s0,
            m.end(),
            TimeKind::Calendar { month, day },
        );
    }
    for c in r.approx.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(
            &mut out,
            &mut taken,
            m.start(),
            m.end(),
            TimeKind::Approx {
                event: c["ev"].into(),
                dur: dur(&c["n"], &c["u"]),
            },
        );
    }
    for c in r.compare.captures_iter(text) {
        let m = c.get(0).unwrap();
        let op = match &c["op"] {
            "以上" => CmpOp::Ge,
            "以下" | "を超えない" => CmpOp::Le,
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
