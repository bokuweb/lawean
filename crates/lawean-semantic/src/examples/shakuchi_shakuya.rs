//! 借地借家法（403AC0000000090_20260521_504AC0000000048）の docs/03-examples を IR にしたもの。
//! stable_id は実装の完全形（章・節込み）。docs 側は省略表記。

use crate::build::*;
use crate::ir::*;

pub const VERSION_ID: &str = "403AC0000000090_20260521_504AC0000000048";
const L: &str = "403AC0000000090";
const BY: &str = "bokuweb";

fn id(path: &str) -> String {
    format!("{L}/{path}")
}

pub fn model() -> SemanticModel {
    SemanticModel {
        document: VERSION_ID.into(),
        definitions: definitions(),
        rules: [
            art03(),
            art04(),
            art05(),
            art06(),
            art09(),
            art22(),
            art26(),
        ]
        .concat(),
        unknowns: Vec::new(),
    }
}

/// 第2条（定義）。body は v0.1 では Unknown でよい（docs/03-examples/art-02.md）
fn definitions() -> Vec<Definition> {
    let art2 = |n: u32| id(&format!("main/chap:1/art:2/para:1/item:{n}"));
    let law = id("");
    let law = law.trim_end_matches('/');
    let d = |did: &str, term: &str, n: u32| {
        definition(
            did,
            term,
            &[law],
            DefinitionBody::Unknown(unknown(UnknownKind::Unparsed, "")),
        )
        .provenance(&art2(n), Confidence::High, BY)
    };
    vec![
        d("D:借地権", "借地権", 1),
        d("D:借地権者", "借地権者", 2),
        d("D:借地権設定者", "借地権設定者", 3),
        d("D:転借地権", "転借地権", 4),
        d("D:転借地権者", "転借地権者", 5),
        // 第6条「借地権者（転借地権者を含む。以下この条において同じ。）」— 条限りで第2条を上書き
        definition(
            "D:借地権者@art6",
            "借地権者",
            &[&id("main/chap:2/sec:1/art:6")],
            DefinitionBody::Expr(or([
                Expr::Ref(RefTarget::Definition(DefinitionId("D:借地権者".into()))),
                Expr::Ref(RefTarget::Definition(DefinitionId("D:転借地権者".into()))),
            ])),
        )
        .provenance(&id("main/chap:2/sec:1/art:6/para:1/sent:1"), Confidence::High, BY),
        // 第22条第2項「電磁的記録（〜をいう。第三十八条第二項及び第三十九条第三項において同じ。）」— 飛び飛びの scope
        definition(
            "D:電磁的記録",
            "電磁的記録",
            &[
                &id("main/chap:2/sec:4/art:22/para:2"),
                &id("main/chap:3/sec:3/art:38/para:2"),
                &id("main/chap:3/sec:3/art:39/para:3"),
            ],
            DefinitionBody::Unknown(unknown(UnknownKind::Unparsed, "電子的方式、磁気的方式その他人の知覚によっては認識することができない方式で作られる記録であって、電子計算機による情報処理の用に供されるもの")),
        )
        .provenance(&id("main/chap:2/sec:4/art:22/para:2/sent:1"), Confidence::High, BY),
    ]
}

/// 第3条: 本文 Rule + ただし書き Rule。Set と overrides の最小例
fn art03() -> Vec<Rule> {
    let p = |s: u32| id(&format!("main/chap:2/sec:1/art:3/para:1/sent:{s}"));
    vec![
        rule("R3-1")
            .subject(def("D:借地権"))
            .effect(set("存続期間", Value::Duration(years(30))))
            .provenance(&p(1), Confidence::High, BY),
        rule("R3-2")
            .subject(def("D:借地権"))
            .condition(cmp(
                var("契約で定めた期間"),
                CmpOp::Gt,
                Value::Duration(years(30)),
            ))
            .effect(set("存続期間", var("契約で定めた期間")))
            .overrides(&["R3-1"])
            .provenance(&p(2), Confidence::High, BY),
    ]
}

/// 第4条: 1 文から 2 Rule。括弧書きの場合分けをただし書きと同じ overrides 構造で表す
fn art04() -> Vec<Rule> {
    let p = |s: u32| id(&format!("main/chap:2/sec:1/art:4/para:1/sent:{s}"));
    vec![
        rule("R4-1")
            .subject(def("D:借地権"))
            .condition(pred("更新する").expr())
            .effect(set(
                "更新後の期間",
                Value::Period(after("更新の日", years(10))),
            ))
            .provenance(&p(1), Confidence::High, BY),
        rule("R4-1'")
            .subject(def("D:借地権"))
            .condition(and([pred("更新する").expr(), pred("最初の更新").expr()]))
            .effect(set(
                "更新後の期間",
                Value::Period(after("更新の日", years(20))),
            ))
            .overrides(&["R4-1"])
            .provenance(&p(1), Confidence::High, BY)
            .note("括弧書き「（借地権の設定後の最初の更新にあっては、二十年）」"),
        // ただし書き「これより長い期間」の「これ」は、上書きされる側の Rule が定める値。
        // 1 つの Rule で RuleValue(R4-1) と比べると、最初の更新で 15 年と定めた場合に 20 年を下回る
        // （z3 が反例を出した。docs/07-verification.md）。上書き先ごとに分ける
        rule("R4-2")
            .subject(def("D:借地権"))
            .condition(and([
                pred("更新する").expr(),
                cmp(var("当事者が定めた期間"), CmpOp::Gt, rule_value("R4-1")),
            ]))
            .effect(set("更新後の期間", var("当事者が定めた期間")))
            .overrides(&["R4-1"])
            .provenance(&p(2), Confidence::High, BY),
        rule("R4-2'")
            .subject(def("D:借地権"))
            .condition(and([
                pred("更新する").expr(),
                pred("最初の更新").expr(),
                cmp(var("当事者が定めた期間"), CmpOp::Gt, rule_value("R4-1'")),
            ]))
            .effect(set("更新後の期間", var("当事者が定めた期間")))
            .overrides(&["R4-1'"])
            .provenance(&p(2), Confidence::High, BY),
    ]
}

/// 第5条: みなす、ただし書きの Exception、「前項と同様」、「みなして〜適用する」
fn art05() -> Vec<Rule> {
    let p = |para: u32, s: u32| id(&format!("main/chap:2/sec:1/art:5/para:{para}/sent:{s}"));
    vec![
        rule("R5-1")
            .subject(def("D:借地権者"))
            .condition(and([
                pred("存続期間が満了する").expr(),
                pred("更新を請求した")
                    .arg("by", entity(def("D:借地権者")))
                    .expr(),
                pred("建物がある").expr(),
            ]))
            .effect(Effect::Deem(
                pred("契約を更新した")
                    .arg("conditions", text("従前と同一"))
                    .fact(),
            ))
            .provenance(&p(1, 1), Confidence::High, BY)
            .note("「前条の規定によるもののほか」— 更新後の期間は R4 が独立に決める"),
        rule("R5-1-proviso")
            .condition(
                pred("異議を述べた")
                    .arg("by", entity(def("D:借地権設定者")))
                    .arg("timing", expr_arg(intentional("遅滞なく")))
                    .expr(),
            )
            .effect(exception("R5-1"))
            .overrides(&["R5-1"])
            .provenance(&p(1, 2), Confidence::High, BY),
        rule("R5-2")
            .subject(def("D:借地権者"))
            .condition(and([
                pred("存続期間が満了した").expr(),
                pred("土地の使用を継続する")
                    .arg("by", entity(def("D:借地権者")))
                    .expr(),
                pred("建物がある").expr(),
            ]))
            .effect(same_as("R5-1"))
            .provenance(&p(2, 1), Confidence::Medium, BY)
            .note("ただし書き（R5-1-proviso）を引き継ぐかは解釈問題"),
        rule("R5-3")
            .condition(pred("転借地権が設定されている").expr())
            .effect(deem_and_apply(
                pred("使用継続")
                    .arg("by", entity(def("D:転借地権者")))
                    .fact(),
                pred("使用継続").arg("by", entity(def("D:借地権者"))).fact(),
                "R5-2",
            ))
            .provenance(&p(3, 1), Confidence::Medium, BY),
    ]
}

/// 第6条: 正当事由 = Unknown(Intentional) + 考慮要素。「でなければ〜できない」= Prohibition
fn art06() -> Vec<Rule> {
    vec![rule("R6")
        .subject(def("D:借地権設定者"))
        .condition(not(pred("正当の事由がある")
            .arg(
                "factors",
                Arg::List(vec![
                    text("使用を必要とする事情"),
                    text("従前の経過"),
                    text("土地の利用状況"),
                    text("立退料の申出"),
                ]),
            )
            .arg(
                "judgement",
                expr_arg(intentional("正当の事由があると認められる")),
            )
            .expr()))
        .effect(Effect::Prohibition(action("異議を述べる").arg(
            "ref",
            Arg::Ref(RefTarget::Rule(RuleId("R5-1-proviso".into()))),
        )))
        .provenance(
            &id("main/chap:2/sec:1/art:6/para:1/sent:1"),
            Confidence::High,
            BY,
        )]
}

/// 第9条: 強行規定。条件に「この節の Rule 群」への参照を含む通常の Rule
fn art09() -> Vec<Rule> {
    vec![rule("R9")
        .subject(named("特約"))
        .condition(and([
            pred("反する")
                .arg(
                    "target",
                    Arg::Ref(RefTarget::Scope(lawean_source::StableId(id(
                        "main/chap:2/sec:1",
                    )))),
                )
                .expr(),
            pred("不利")
                .arg("to", entity(def("D:借地権者")))
                .arg("judgement", expr_arg(intentional("不利")))
                .expr(),
        ]))
        .effect(Effect::Void(Target::Contract("特約".into())))
        .provenance(
            &id("main/chap:2/sec:1/art:9/para:1/sent:1"),
            Confidence::High,
            BY,
        )]
}

/// 第22条: 強行規定への特則、前段/後段、書面要件、電磁的記録の DeemAndApply
fn art22() -> Vec<Rule> {
    let p = |para: u32, s: u32| id(&format!("main/chap:2/sec:4/art:22/para:{para}/sent:{s}"));
    vec![
        rule("R22-1a")
            .subject(named("当事者"))
            .condition(cmp(var("存続期間"), CmpOp::Ge, Value::Duration(years(50))))
            .effect(power(action("特約を定める").arg(
                "content",
                Arg::List(vec![
                    text("契約の更新がない"),
                    text("建物の築造による存続期間の延長がない"),
                    text("第十三条の買取請求をしない"),
                ]),
            )))
            .overrides(&["R9"])
            .provenance(&p(1, 1), Confidence::High, BY)
            .note("「第九条及び第十六条の規定にかかわらず」。R16 は未作成"),
        rule("R22-1b")
            .subject(named("当事者"))
            .condition(rule_ref("R22-1a"))
            .effect(Effect::Obligation(
                action("書面による").arg("form", text("公正証書等")),
            ))
            .provenance(&p(1, 2), Confidence::High, BY)
            .note("「この場合においては」= 前段の特約をする場合"),
        rule("R22-2")
            .condition(
                pred("特約が電磁的記録によってされた")
                    .arg("of", Arg::Ref(RefTarget::Rule(RuleId("R22-1a".into()))))
                    .expr(),
            )
            .effect(deem_and_apply(
                pred("電磁的記録によってされた").fact(),
                pred("書面によってされた").fact(),
                "R22-1b",
            ))
            .provenance(&p(2, 1), Confidence::High, BY),
    ]
}

/// 第26条: 時間窓、ただし書きによる部分上書き、SameAs、DeemAndApply
fn art26() -> Vec<Rule> {
    let p = |para: u32, s: u32| id(&format!("main/chap:3/sec:1/art:26/para:{para}/sent:{s}"));
    let notice_window = window(
        before("期間の満了", years(1)),
        before("期間の満了", months(6)),
    );
    vec![
        rule("R26-1")
            .subject(named("当事者"))
            .condition(and([
                pred("期間の定めがある").expr(),
                not(pred("通知した")
                    .arg(
                        "kind",
                        Arg::List(vec![
                            text("更新しない"),
                            text("条件を変更しなければ更新しない"),
                        ]),
                    )
                    .arg(
                        "within",
                        expr_arg(Expr::Time(TimeCond::Within(notice_window))),
                    )
                    .expr()),
            ]))
            .effect(Effect::Deem(
                pred("契約を更新した")
                    .arg("conditions", text("従前と同一"))
                    .fact(),
            ))
            .provenance(&p(1, 1), Confidence::High, BY),
        rule("R26-1-proviso")
            .condition(rule_ref("R26-1"))
            .effect(set("期間", Value::PeriodValue(PeriodValue::Indefinite)))
            .overrides(&["R26-1"])
            .provenance(&p(1, 2), Confidence::High, BY)
            .note("Rule 全体ではなく「同一の条件」のうち期間だけを上書きする"),
        rule("R26-2")
            .condition(and([
                pred("通知した").expr(),
                pred("期間が満了した").expr(),
                pred("使用を継続する")
                    .arg("by", entity(named("賃借人")))
                    .expr(),
                not(pred("異議を述べた")
                    .arg("by", entity(named("賃貸人")))
                    .arg("timing", expr_arg(intentional("遅滞なく")))
                    .expr()),
            ]))
            .effect(same_as("R26-1"))
            .provenance(&p(2, 1), Confidence::High, BY),
        rule("R26-3")
            .condition(pred("転貸借がされている").expr())
            .effect(deem_and_apply(
                pred("使用継続").arg("by", entity(named("転借人"))).fact(),
                pred("使用継続").arg("by", entity(named("賃借人"))).fact(),
                "R26-2",
            ))
            .provenance(&p(3, 1), Confidence::High, BY),
    ]
}
