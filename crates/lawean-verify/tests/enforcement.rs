//! 附則の施行期日 → Z3 の暦。実際の附則（借地借家法の 令和3年法律第37号・令和4年法律第48号）と、
//! Rust の暦（`lawean-extract::calendar`）が Z3 の暦と一致すること（閏日の縁を含む）。

use lawean_extract::suppl::{self, spec_for_law_id};
use lawean_extract::temporal::{Dur, Enforcement, Unit};
use lawean_source::parse_response;
use lawean_verify::enforcement::{bounds, check};
use lawean_verify::z3_available;

macro_rules! need_z3 {
    () => {
        if !z3_available() {
            eprintln!("z3 not on PATH; skipping");
            return;
        }
    };
}

fn shakuchi() -> lawean_source::LegalDocument {
    let p = format!(
        "{}/../../fixtures/403AC0000000090.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    parse_response(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// 令和3年法律第37号 第35条: 公布 2021-05-19、「一年を超えない範囲内」→ e-Gov の施行日 2022-05-18 は上限ちょうど。翌日は不可
#[test]
fn r3_37_enforced_on_the_last_admissible_day() {
    need_z3!();
    let spec = spec_for_law_id(&shakuchi(), "503AC0000000037").unwrap();
    let p = spec.promulgated.unwrap();
    let e = spec
        .for_article(35, Some(false))
        .unwrap()
        .0
        .enforcement
        .clone()
        .unwrap();
    assert_eq!(bounds(p, &e).unwrap(), Some(((2021, 5, 19), (2022, 5, 18))));
    assert_eq!(check(p, &e, (2022, 5, 18)).unwrap(), Some(true));
    assert_eq!(check(p, &e, (2022, 5, 19)).unwrap(), Some(false));
    assert_eq!(check(p, &e, (2021, 5, 18)).unwrap(), Some(false));
}

/// 令和4年法律第48号 附則第73条: 公布 2022-05-25、「九月を超えない範囲内」→ 2023-02-24 まで。e-Gov は 2023-02-20
#[test]
fn r4_48_suppl_art73_within_nine_months() {
    need_z3!();
    let spec = spec_for_law_id(&shakuchi(), "504AC0000000048").unwrap();
    let p = spec.promulgated.unwrap();
    let e = spec
        .for_article(73, None)
        .unwrap()
        .0
        .enforcement
        .clone()
        .unwrap();
    assert_eq!(bounds(p, &e).unwrap(), Some(((2022, 5, 25), (2023, 2, 24))));
    assert_eq!(check(p, &e, (2023, 2, 20)).unwrap(), Some(true));
    // 第74条は本文の「四年を超えない範囲内」: 2026-05-24 まで。現行リビジョンの 2026-05-21 は可
    let e = spec
        .for_article(74, None)
        .unwrap()
        .0
        .enforcement
        .clone()
        .unwrap();
    assert_eq!(bounds(p, &e).unwrap(), Some(((2022, 5, 25), (2026, 5, 24))));
    assert_eq!(check(p, &e, (2026, 5, 21)).unwrap(), Some(true));
    assert_eq!(check(p, &e, (2026, 5, 25)).unwrap(), Some(false));
}

/// Rust の暦と Z3 の暦が同じ区間を出す（閏日・月末の縁を含む）
#[test]
fn rust_calendar_agrees_with_z3() {
    need_z3!();
    let dur = |n, unit| Dur { n, unit };
    let promulgations = [
        (2021, 5, 19),
        (2024, 2, 29),
        (2023, 1, 31),
        (2023, 12, 31),
        (2024, 8, 31),
    ];
    let enforcements = [
        Enforcement::Promulgation,
        Enforcement::ByCabinetOrderWithin(dur(1, Unit::Year)),
        Enforcement::ByCabinetOrderWithin(dur(6, Unit::Month)),
        Enforcement::ByCabinetOrderWithin(dur(20, Unit::Day)),
        Enforcement::ElapsedFromPromulgation(dur(1, Unit::Year)),
        Enforcement::ElapsedFromPromulgation(dur(1, Unit::Month)),
        Enforcement::ElapsedFromPromulgation(dur(3, Unit::Month)),
        Enforcement::ElapsedFromPromulgation(dur(10, Unit::Day)),
        Enforcement::Date {
            era: "令和".into(),
            y: 3,
            m: 9,
            d: 1,
        },
    ];
    for p in promulgations {
        for e in &enforcements {
            let rust = suppl::admissible(p, e).unwrap();
            let z3 = bounds(p, e).unwrap().unwrap();
            assert_eq!(rust, z3, "p={p:?} e={e:?}");
        }
    }
}
