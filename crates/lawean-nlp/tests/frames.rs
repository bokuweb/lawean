//! 係り受けからの主体・客体・行為。`JEWEL_GINZA_BUNDLE` が無ければスキップ（CI にはモデルが無い）

use lawean_extract::candidate::{Confidence, Field};
use lawean_nlp::{Parser, Stripped};
use lawean_source::StableId;

fn parser() -> Option<Parser> {
    match Parser::from_env() {
        Ok(p) => Some(p),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn sid() -> StableId {
    StableId("403AC0000000090/main/art:13/para:1/sent:1".into())
}

#[test]
fn stripped_offsets_round_trip() {
    let t = "借地権者は、借地権設定者（前条第一項の者を含む。）に対し、建物（附属物を含む。）の買取りを請求することができる。";
    let s = Stripped::new(t);
    assert_eq!(
        s.text,
        "借地権者は、借地権設定者に対し、建物の買取りを請求することができる。"
    );
    let i = s.text.find("建物").unwrap();
    let (a, b) = s.to_orig(i, i + "建物".len());
    assert_eq!(&t[a..b], "建物");
    let i = s.text.find("請求").unwrap();
    let (a, b) = s.to_orig(i, i + "請求".len());
    assert_eq!(&t[a..b], "請求");
    // 末尾
    let (a, b) = s.to_orig(s.text.len(), s.text.len());
    assert_eq!((a, b), (t.len(), t.len()));
    // 括弧だけ・空
    assert_eq!(Stripped::new("（略）").text, "");
    assert_eq!(Stripped::new("").text, "");
}

#[test]
fn subject_object_and_counterparty() {
    let Some(p) = parser() else { return };
    let t = "借地権者は、借地権設定者に対し、建物の買取りを請求することができる。";
    let cs = p.candidates(&sid(), t).unwrap();
    let subj = cs
        .iter()
        .find(|c| c.field == Field::Subject && c.reason == "ginza:nsubj")
        .unwrap();
    assert_eq!(subj.raw, "借地権者");
    assert_eq!(subj.role.as_deref(), Some("は"));
    assert_eq!(subj.source_label.as_deref(), Some("請求"));
    let obj = cs
        .iter()
        .find(|c| c.field == Field::Object && c.reason == "ginza:obj")
        .unwrap();
    assert_eq!(obj.raw, "建物の買取り");
    let obl = cs
        .iter()
        .find(|c| c.field == Field::Object && c.reason == "ginza:obl")
        .unwrap();
    assert_eq!(obl.raw, "借地権設定者");
    assert_eq!(obl.role.as_deref(), Some("に"));
    let act = cs
        .iter()
        .find(|c| c.field == Field::Act && c.reason == "ginza:root")
        .unwrap();
    assert_eq!(act.raw, "請求");
    // 位置は元の本文から再現できる
    for c in &cs {
        assert_eq!(&t[c.evidence.start..c.evidence.end], c.evidence.snippet);
    }
}

#[test]
fn topic_is_inherited_by_the_main_predicate_and_parentheses_do_not_break_offsets() {
    let Some(p) = parser() else { return };
    let t = "建物の賃貸人（転貸人を含む。）は、前項の規定による書面の交付に代えて、政令で定めるところにより、建物の賃借人の承諾を得て、当該書面に記載すべき事項を電磁的方法により提供することができる。";
    let cs = p.candidates(&sid(), t).unwrap();
    let subj = cs
        .iter()
        .find(|c| c.field == Field::Subject && c.confidence == Confidence::Medium)
        .unwrap();
    assert_eq!(subj.raw, "建物の賃貸人");
    assert_eq!(subj.reason, "ginza:topic");
    assert_eq!(subj.source_label.as_deref(), Some("提供"));
    for c in &cs {
        assert_eq!(
            &t[c.evidence.start..c.evidence.end],
            c.evidence.snippet,
            "{c:?}"
        );
    }
}

#[test]
fn item_fragment_uses_the_relative_clause_as_the_act() {
    let Some(p) = parser() else { return };
    let t = "第百三十八条の規定に違反して戸別訪問をした者";
    let cs = p.candidates(&sid(), t).unwrap();
    let act = cs
        .iter()
        .find(|c| c.field == Field::Act && c.confidence == Confidence::Medium)
        .unwrap();
    assert_eq!(act.raw, "戸別訪問");
}

#[test]
fn degenerate_inputs_do_not_panic() {
    let Some(p) = parser() else { return };
    for t in [
        "",
        "   ",
        "（略）",
        "。",
        "第一条",
        "ア",
        "x".repeat(5000).as_str(),
    ] {
        let cs = p.candidates(&sid(), t).unwrap();
        for c in &cs {
            assert_eq!(&t[c.evidence.start..c.evidence.end], c.evidence.snippet);
        }
    }
    // 決定的
    let t = "借地権者は、借地権設定者に対し、建物の買取りを請求することができる。";
    assert_eq!(
        p.candidates(&sid(), t).unwrap(),
        p.candidates(&sid(), t).unwrap()
    );
}

#[test]
fn refine_events_shrinks_only_at_clause_boundaries() {
    let Some(p) = parser() else { return };
    use lawean_extract::temporal::time_candidates;
    // 「講習で」は別の節: 事象は「交付の申請」まで縮める
    let t = "宅地建物取引士証の交付を受けようとする者は、登録をしている都道府県知事が国土交通省令の定めるところにより指定する講習で交付の申請前六月以内に行われるものを受講しなければならない。";
    let cs = p.refine_events(t, time_candidates(&sid(), t));
    let c = cs.iter().find(|c| c.field == Field::WithinBefore).unwrap();
    assert_eq!(c.raw, "交付の申請前六月以内");
    assert_eq!(c.source_label.as_deref(), Some("交付の申請"));
    assert!(c.reason.ends_with("+ginza:event"));
    assert_eq!(&t[c.evidence.start..c.evidence.end], c.evidence.snippet);
    // 「被相続人の」は連体の句: 縮めない
    let t = "その相続人は、被相続人の死亡後六十日以内に都道府県知事に申請して、その承認を受けなければならない。";
    let cs = p.refine_events(t, time_candidates(&sid(), t));
    let c = cs.iter().find(|c| c.field == Field::Within).unwrap();
    assert_eq!(c.raw, "被相続人の死亡後六十日以内");
    assert!(!c.reason.contains("ginza"));
}
