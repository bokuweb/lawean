//! 期間の計算（民法第140〜143条）を Z3 で。docs/03-examples/minpo-140-143.md の例と、期間のミスの検出。

use lawean_verify::temporal::*;
use lawean_verify::{run_z3, z3_available, Verdict};

macro_rules! need_z3 {
    () => {
        if !z3_available() {
            eprintln!("z3 not on PATH; skipping");
            return;
        }
    };
}

/// 主張が成り立つ世界があるか（sat）
fn sat(decls: &[String], asserts: &[String]) -> bool {
    matches!(
        run_z3(&script(decls, asserts)).unwrap(),
        Verdict::Counterexample(_)
    )
}
/// 主張が全ての世界で成り立つか（否定が unsat）
fn valid(decls: &[String], claim: &str) -> bool {
    matches!(
        run_z3(&script(decls, &[format!("(not {claim})")])).unwrap(),
        Verdict::Proved
    )
}

/// docs の例: 起算日 2020-03-15 + 1 年 → 2021-03-14 に満了（第143条第2項本文: 応当日の前日）
#[test]
fn one_year_from_march_15_ends_march_14() {
    need_z3!();
    let e = expiry(&DateExpr::lit(2020, 3, 15), 12);
    assert!(sat(&[], &[e.eq(&DateExpr::lit(2021, 3, 14))]));
    assert!(!sat(&[], &[e.eq(&DateExpr::lit(2021, 3, 15))]));
}

/// docs の例: 起算日 2020-01-31 + 1 月 → 応当日（2 月 31 日）が無いので 2020-02-29 に満了（同ただし書き、閏年）
#[test]
fn one_month_from_january_31_ends_february_29_in_a_leap_year() {
    need_z3!();
    let e = expiry(&DateExpr::lit(2020, 1, 31), 1);
    assert!(sat(&[], &[e.eq(&DateExpr::lit(2020, 2, 29))]));
    let e = expiry(&DateExpr::lit(2021, 1, 31), 1);
    assert!(sat(&[], &[e.eq(&DateExpr::lit(2021, 2, 28))]));
}

/// docs の例: 2000-04-01 契約の借地権（三十年、初日不算入）は 2030-04-01 に満了
#[test]
fn thirty_years_from_april_1_2000_ends_april_1_2030() {
    need_z3!();
    let e = expiry_from_event(&DateExpr::lit(2000, 4, 1), 360);
    assert!(sat(&[], &[e.eq(&DateExpr::lit(2030, 4, 1))]));
}

/// 全ての日付について: n 月（n ≥ 1）の期間の満了日は起算日より後（期間が空にならない）
#[test]
fn expiry_is_always_after_the_start() {
    need_z3!();
    let s = DateExpr::var("start");
    let e = expiry(&s, 12);
    assert!(valid(&[DateExpr::declare("start")], &s.lt(&e)));
    let e1 = expiry(&s, 1);
    assert!(valid(&[DateExpr::declare("start")], &s.lt(&e1)));
}

/// **期間のミス 1**: 「六月を経過する日」と「六月を経過した日」は 1 日違う。
/// 施行日 2022-05-18 から六月: 経過する日 = 2022-11-18（満了日）、経過した日 = 2022-11-19
#[test]
fn elapsed_on_versus_elapsed_after_differ_by_one_day() {
    need_z3!();
    let event = DateExpr::lit(2022, 5, 18);
    let on = expiry_from_event(&event, 6);
    let after = on.next();
    assert!(sat(&[], &[on.eq(&DateExpr::lit(2022, 11, 18))]));
    assert!(sat(&[], &[after.eq(&DateExpr::lit(2022, 11, 19))]));
    // 一般に: 全ての施行日について両者は一致しない
    let s = DateExpr::var("s");
    let on = expiry_from_event(&s, 6);
    assert!(valid(
        &[DateExpr::declare("s")],
        &format!("(not {})", on.eq(&on.next()))
    ));
}

/// **期間のミス 2**: 第26条の「期間の満了の一年前から六月前までの間」は、全ての満了日について空でない区間。
/// 前後を取り違えた「六月前から一年前まで」は、全ての満了日について空（始点が終点より後）= 起案のミスとして検出できる
#[test]
fn notice_window_is_nonempty_and_the_reversed_one_is_always_empty() {
    need_z3!();
    let exp = DateExpr::var("expiry");
    let decl = [DateExpr::declare("expiry")];
    let one_year_before = before(&exp, 12);
    let six_months_before = before(&exp, 6);
    // 正しい向き: 始点 < 終点
    assert!(valid(&decl, &one_year_before.lt(&six_months_before)));
    // 逆向き: 始点 ≤ 終点 になる満了日は 1 つも無い
    assert!(!sat(&decl, &[six_months_before.le(&one_year_before)]));
}

/// **期間のミス 3**: 初日算入と不算入を取り違えると満了日がずれる。
/// 「常に 1 日ずれる」と書いたら Z3 に反例を出された:
/// - 閏日 2020-02-29 から一年: 算入だと応当日（2021-02-29）が無く月末 02-28 に満了（第143条第2項ただし書き）、
///   不算入（起算 03-01 → 応当 2021-03-01 の前日 = 02-28）と**一致**する（差 0）
/// - 2019-02-28 から一年: 算入は 2020-02-27、不算入は起算 03-01 → 2020-02-29 で**差 2 日**（閏日をまたぐ）
///
/// 正しい主張は「算入 ≤ 不算入、差は 0〜2 日」。暦の規則は直感で書くと間違える、という実例
#[test]
fn counting_the_first_day_shifts_the_expiry_by_at_most_one_day() {
    need_z3!();
    let ev = DateExpr::var("ev");
    let decl = [DateExpr::declare("ev")];
    let excl = expiry(&start_day(&ev, false), 12);
    let incl = expiry(&start_day(&ev, true), 12);
    assert!(valid(&decl, &incl.le(&excl)));
    assert!(valid(
        &decl,
        &format!(
            "(or {} {} {})",
            incl.eq(&excl),
            incl.next().eq(&excl),
            incl.next().next().eq(&excl)
        )
    ));
    // 差 0 の日（閏日）と差 2 の日（閏日の前日の前年）がある。「常に 1 日」は偽
    assert!(sat(
        &decl,
        &[incl.eq(&excl), ev.eq(&DateExpr::lit(2020, 2, 29))]
    ));
    assert!(sat(
        &decl,
        &[
            incl.next().next().eq(&excl),
            ev.eq(&DateExpr::lit(2019, 2, 28))
        ]
    ));
    assert!(!valid(&decl, &incl.next().eq(&excl)));
}

// ---------------------------------------------------------------- 効力の区間

use lawean_verify::validity::*;

/// 公職選挙法 第244条（罰則）の「第百四十二条の四第六項」は、平成30年法律第75号の施行（2018-10-25）から
/// 令和3年法律第51号の施行（2021-06-02）までの間、表示義務の項を指していなかった（docs/12 §5）。
/// 罰則の効力の区間と、参照先が正しい区間を与えると、Z3 がその間の日を反例として出す
#[test]
fn koshoku_senkyo_penalty_reference_was_wrong_for_three_years() {
    need_z3!();
    let r = Reference {
        name: "第244条第1項第2号の2 → 第142条の4の表示義務".into(),
        referrer: Interval::from(2013, 5, 26), // 電子メール解禁（平成25年改正）以降ずっと効力
        target_ok: vec![
            Interval::from(2013, 5, 26).until(2018, 10, 25), // 第六項 = 表示義務
            Interval::from(2021, 6, 2),                      // 訂正後: 第七項 = 表示義務
        ],
    };
    let t = gap(&r).unwrap().expect("空白の期間がある");
    assert!(
        (2018, 10, 25) <= (t.0, t.1, t.2) && (t.0, t.1, t.2) < (2021, 6, 2),
        "{t:?}"
    );
    // 訂正法の施行日を 2018-10-25 にしていれば空白は無い
    let fixed = Reference {
        target_ok: vec![Interval::from(2013, 5, 26)],
        ..r
    };
    assert_eq!(gap(&fixed).unwrap(), None);
}

/// 段階施行: 令和4年法律第48号の第61条は 2023-02-20 に新設される。第61条を参照する規定を 2022-05-25 に施行すると、
/// 2022-05-25 から 2023-02-19 までの間は参照先が無い（docs/09 の TimingGap を区間の問いとして）
#[test]
fn a_reference_enforced_before_its_target_has_a_gap() {
    need_z3!();
    let r = Reference {
        name: "施行令 → 借地借家法第61条".into(),
        referrer: Interval::from(2022, 5, 25),
        target_ok: vec![Interval::from(2023, 2, 20)],
    };
    let t = gap(&r).unwrap().unwrap();
    assert!(
        (2022, 5, 25) <= (t.0, t.1, t.2) && (t.0, t.1, t.2) < (2023, 2, 20),
        "{t:?}"
    );
    let ok = Reference {
        referrer: Interval::from(2023, 2, 20),
        ..r
    };
    assert_eq!(gap(&ok).unwrap(), None);
}
