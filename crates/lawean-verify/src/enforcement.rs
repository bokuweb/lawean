//! 施行期日の許容区間を Z3 の暦で（docs/07 §「期間・効力・対象の齟齬」の効力の行）。
//!
//! 附則から取った `Enforcement`（`lawean-extract::suppl`）を、公布日を起点に日付の制約に落とす。
//! 「公布の日から起算して一年を超えない範囲内において政令で定める日」なら
//! 公布日 ≤ t ≤ 起算日の一年後の応当日の前日（第143条第2項）。
//!
//! `lawean-check` は WASM でも動く必要があるので Rust の暦（`lawean-extract::calendar`）で同じ区間を出す。
//! ここは Rust の暦が Z3 の暦と一致することを確かめる側（`tests/enforcement.rs`）と、
//! 施行日の候補を Z3 に**解かせる**側（「この附則の下で許される最も遅い日は」）。

use crate::temporal::{expiry, DateExpr};
use crate::{run_z3, CheckError, Verdict};
use lawean_extract::calendar::Date;
use lawean_extract::suppl::era_year;
use lawean_extract::temporal::{Enforcement, Unit};

fn lit((y, m, d): Date) -> DateExpr {
    DateExpr::lit(y, m, d)
}

/// n 日後。`next` を入れ子にすると式が 3^n に膨れるので `let` で鎖にする（各成分が O(n)）
fn add_days(t: DateExpr, n: i64) -> DateExpr {
    let mut prefix = format!("(let ((y0 {}) (m0 {}) (d0 {})) ", t.y, t.m, t.d);
    for i in 0..n {
        let (j, k) = (i, i + 1);
        prefix.push_str(&format!(
            "(let ((y{k} (next_y y{j} m{j} d{j})) (m{k} (next_m y{j} m{j} d{j})) (d{k} (next_d y{j} m{j} d{j}))) "
        ));
    }
    let close = ")".repeat(n as usize + 1);
    DateExpr {
        y: format!("{prefix}y{n}{close}"),
        m: format!("{prefix}m{n}{close}"),
        d: format!("{prefix}d{n}{close}"),
    }
}

/// 「t はこの附則の下で施行日として許される」の SMT 式。公布日 `p`
pub fn admissible(p: Date, e: &Enforcement, t: &DateExpr) -> Option<String> {
    let p = lit(p);
    Some(match e {
        Enforcement::Promulgation => t.eq(&p),
        Enforcement::OtherLaw(_) => return None,
        Enforcement::ByCabinetOrderUntil { era, y, m, d } => {
            let hi = DateExpr::lit(era_year(era, *y)?, *m, *d);
            format!("(and {} {})", p.le(t), t.le(&hi))
        }
        Enforcement::Date { era, y, m, d } => t.eq(&DateExpr::lit(era_year(era, *y)?, *m, *d)),
        Enforcement::ElapsedFromPromulgation(dur) => {
            // 起算日は公布日（「起算して」）。満了日の翌日
            let day = match dur.unit {
                Unit::Day | Unit::Week => add_days(p, dur.days()?),
                _ => expiry(&p, dur.months()?).next(),
            };
            t.eq(&day)
        }
        Enforcement::ByCabinetOrderWithin(dur) => {
            let hi = match dur.unit {
                Unit::Day | Unit::Week => add_days(p.clone(), dur.days()? - 1),
                _ => expiry(&p, dur.months()?),
            };
            format!("(and {} {})", p.le(t), t.le(&hi))
        }
    })
}

fn date_of(model: &str, name: &str) -> Date {
    let v = |k: &str| {
        crate::model_value(model, &format!("{name}.{k}"))
            .and_then(|x| {
                x.trim_matches(|c| c == '(' || c == ')' || c == ' ')
                    .replace("- ", "-")
                    .parse::<i64>()
                    .ok()
            })
            .unwrap_or(0)
    };
    (v("y") as i32, v("m") as u32, v("d") as u32)
}

/// 許容区間の両端を Z3 に求めさせる（最も早い日、最も遅い日）。Rust の暦との突き合わせ用
pub fn bounds(p: Date, e: &Enforcement) -> Result<Option<(Date, Date)>, CheckError> {
    let t = DateExpr::var("t");
    let Some(adm) = admissible(p, e, &t) else {
        return Ok(None);
    };
    let mut out = [p, p];
    for (i, obj) in ["minimize", "maximize"].iter().enumerate() {
        // 辞書順の目的関数: 年・月・日をひとつの整数に
        let key = format!("(+ (* 10000 {}) (* 100 {}) {})", t.y, t.m, t.d);
        let s = crate::temporal::script(&[DateExpr::declare("t")], std::slice::from_ref(&adm))
            .replace("(check-sat)", &format!("({obj} {key})\n(check-sat)"));
        match run_z3(&s)? {
            Verdict::Counterexample(model) => out[i] = date_of(&model, "t"),
            Verdict::Proved => return Ok(None),
            Verdict::Unknown(u) => return Err(CheckError::Solver(u)),
        }
    }
    Ok(Some((out[0], out[1])))
}

/// 施行日 `actual` が附則の下で許されるか
pub fn check(p: Date, e: &Enforcement, actual: Date) -> Result<Option<bool>, CheckError> {
    let t = lit(actual);
    let Some(adm) = admissible(p, e, &t) else {
        return Ok(None);
    };
    match run_z3(&crate::temporal::script(&[], &[adm]))? {
        Verdict::Counterexample(_) => Ok(Some(true)),
        Verdict::Proved => Ok(Some(false)),
        Verdict::Unknown(u) => Err(CheckError::Solver(u)),
    }
}
