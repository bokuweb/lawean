//! 暦の算術（民法第140条〜第143条）。`lawean-verify::temporal` の SMT の暦と同じ規則の Rust 版。
//! 用途は施行期日の許容区間（`suppl::admissible`）のように、WASM でも Z3 無しで出したい値。
//! 2 つの暦が一致することは `lawean-verify` のテスト（`enforcement.rs`）で Z3 に確かめさせる。

pub type Date = (i32, u32, u32);

pub fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

pub fn next_day((y, m, d): Date) -> Date {
    if d < days_in_month(y, m) {
        (y, m, d + 1)
    } else if m < 12 {
        (y, m + 1, 1)
    } else {
        (y + 1, 1, 1)
    }
}

pub fn prev_day((y, m, d): Date) -> Date {
    if d > 1 {
        (y, m, d - 1)
    } else if m > 1 {
        (y, m - 1, days_in_month(y, m - 1))
    } else {
        (y - 1, 12, 31)
    }
}

pub fn add_days(mut t: Date, n: i64) -> Date {
    for _ in 0..n.abs() {
        t = if n > 0 { next_day(t) } else { prev_day(t) };
    }
    t
}

/// n 月後の応当日（無ければ月末）。n は負でもよい
pub fn corresponding((y, m, d): Date, months: i64) -> Date {
    let total = (y as i64) * 12 + (m as i64 - 1) + months;
    let (y2, m2) = (
        total.div_euclid(12) as i32,
        (total.rem_euclid(12) + 1) as u32,
    );
    (y2, m2, d.min(days_in_month(y2, m2)))
}

/// 月・年で定めた期間の満了日（第143条第2項）: 起算日の n 月後の応当日の前日。応当日が無い月はその月の末日
pub fn expiry(start: Date, months: i64) -> Date {
    let corr = corresponding(start, months);
    if start.2 > days_in_month(corr.0, corr.1) {
        corr
    } else {
        prev_day(corr)
    }
}

pub fn fmt(t: Date) -> String {
    format!("{:04}-{:02}-{:02}", t.0, t.1, t.2)
}

pub fn parse(s: &str) -> Option<Date> {
    let mut it = s.trim().split('-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)).then_some((y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minpo_143() {
        // 5/19 起算で一年: 応当日 翌年 5/19 の前日
        assert_eq!(expiry((2021, 5, 19), 12), (2022, 5, 18));
        // 1/31 起算で一月: 2 月に応当日が無いので 2 月末日
        assert_eq!(expiry((2023, 1, 31), 1), (2023, 2, 28));
        assert_eq!(expiry((2024, 1, 31), 1), (2024, 2, 29));
        // 2/29 起算で一年: 翌年に応当日が無いので 2/28
        assert_eq!(expiry((2024, 2, 29), 12), (2025, 2, 28));
        assert_eq!(corresponding((2022, 5, 25), 9), (2023, 2, 25));
        assert_eq!(add_days((2023, 12, 30), 3), (2024, 1, 2));
        assert_eq!(parse("2022-05-18"), Some((2022, 5, 18)));
        assert_eq!(parse("2022-02-30"), None);
    }
}
