//! 効力の区間（施行日）を Z3 で（docs/07 §「期間・効力・対象の齟齬」の 3 行目）。
//!
//! 規定は効力の区間 [施行, 廃止) を持つ。参照は「参照元が効力を持つ全ての時点で、参照先が意図した規定として存在する」べき。
//! そうでない時点があれば、その日を反例として出す。`lawean-space` の TimingGap（Rust）と同じ問いを、
//! 複数の区間の組合せ（段階施行、他法令の施行日）について一般に解く。時点は暦の日付（`temporal`）。

use crate::temporal::{script, DateExpr};
use crate::{run_z3, CheckError, Verdict};

/// 効力の区間 [from, to)。`to` が None なら無期限
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interval {
    pub from: (i32, u32, u32),
    pub to: Option<(i32, u32, u32)>,
}

impl Interval {
    pub fn from(y: i32, m: u32, d: u32) -> Self {
        Interval {
            from: (y, m, d),
            to: None,
        }
    }
    pub fn until(mut self, y: i32, m: u32, d: u32) -> Self {
        self.to = Some((y, m, d));
        self
    }
    /// 時点 t がこの区間にある
    pub fn contains(&self, t: &DateExpr) -> String {
        let (y, m, d) = self.from;
        let lo = DateExpr::lit(y, m, d).le(t);
        match self.to {
            Some((y2, m2, d2)) => format!("(and {lo} {})", t.lt(&DateExpr::lit(y2, m2, d2))),
            None => lo,
        }
    }
}

/// 参照の整合: 参照元 `referrer` が効力を持つ間、参照先が意図した規定であるべき区間 `target_ok` のどれかに入っているか
#[derive(Debug, Clone)]
pub struct Reference {
    pub name: String,
    pub referrer: Interval,
    pub target_ok: Vec<Interval>,
}

/// 参照元が効力を持つのに参照先が意図した規定でない時点があれば、その日を返す
pub fn gap(r: &Reference) -> Result<Option<(i64, i64, i64)>, CheckError> {
    let t = DateExpr::var("t");
    let ok: Vec<String> = r.target_ok.iter().map(|i| i.contains(&t)).collect();
    let ok = if ok.is_empty() {
        "false".into()
    } else {
        format!("(or {})", ok.join(" "))
    };
    let s = script(
        &[DateExpr::declare("t")],
        &[r.referrer.contains(&t), format!("(not {ok})")],
    );
    match run_z3(&s)? {
        Verdict::Counterexample(model) => {
            let v = |k: &str| {
                crate::model_value(&model, k)
                    .and_then(|x| {
                        x.trim_matches(|c| c == '(' || c == ')' || c == ' ')
                            .replace("- ", "-")
                            .parse::<i64>()
                            .ok()
                    })
                    .unwrap_or(0)
            };
            Ok(Some((v("t.y"), v("t.m"), v("t.d"))))
        }
        Verdict::Proved => Ok(None),
        Verdict::Unknown(u) => Err(CheckError::Solver(u)),
    }
}
