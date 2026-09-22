//! # lawean-consolidate — 改め文の溶け込みと検査
//!
//! e-Gov 法令 API の XML（`law_data` の応答か、裸の `<Law>` 要素）と、改正法の本文（改め文）から、
//! 改正後の法令 XML（e-Gov 法令標準 XML の `<Law>`）と検査の報告を返す。他のプロジェクトから使う入口。
//!
//! ```no_run
//! use lawean_consolidate::{consolidate, verify, Input, Kind};
//!
//! let base = std::fs::read_to_string("base.xml").unwrap();       // 発射台（施行日の直前に効力のある版）
//! let amend = std::fs::read_to_string("amend.txt").unwrap();     // 「第N条　X法（…）の一部を次のように改正する。」から始まる本文
//!
//! // 溶け込みだけ
//! let out = consolidate(&base, &amend).unwrap();
//! std::fs::write("after.xml", &out.xml).unwrap();
//!
//! // 検査（発射台・順序・衝突・ハネ・溶け込み・改正後との一致・他法令・施行期日・罰則・先行改正）
//! let report = verify(&Input { base_xml: &base, amendment: &amend, enforced: Some("2026-04-01"), ..Default::default() });
//! if report.failed(Kind::Hane) { eprintln!("{:#?}", report.suggested_fixes); }
//! ```
//!
//! 何が要るか: Rust だけ。Lean・Z3・形態素解析は要らない（`lean` feature を立てると、`lean` があれば証明した
//! `applyUnit` で溶け込みを計算し、報告の `engine` が `"lean"` になる。無ければ Rust の写し。結果は同じ）。
//!
//! 読める改め文の形は [docs/08](https://github.com/bokuweb/lawean/blob/main/docs/08-amendment.md)、
//! 検査の一覧と実例は [docs/12](https://github.com/bokuweb/lawean/blob/main/docs/12-cases.md)。

pub use lawean_check::{Check, Kind, Report, Status, TextInput as Input, UnitSummary};
use lawean_source::emit_law;

/// 溶け込みの結果
#[derive(Debug, Clone)]
pub struct Consolidated {
    /// 改正後の法令（e-Gov 法令標準 XML の `<Law>` 要素）
    pub xml: String,
    /// 改正単位ごとの要約（読んだ文と操作の数）
    pub units: Vec<UnitSummary>,
    /// 発射台からの差分（人が読む形: 「変更 第N条第M項: …」）
    pub diff: Vec<String>,
    /// 溶け込みに使った計算機（`"lean"` か `"rust"`）
    pub engine: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 改め文が読めない・発射台に当たらない・溶け込みが止まった。どの検査でどう落ちたか
    #[error("{0}")]
    Failed(String),
}

/// 発射台の XML と改め文から、改正後の XML を作る。改め文が発射台に当たらなければ Err
pub fn consolidate(base_xml: &str, amendment: &str) -> Result<Consolidated, Error> {
    let (out, report) = consolidate_and_verify(&Input {
        base_xml,
        amendment,
        ..Default::default()
    });
    out.ok_or_else(|| Error::Failed(failure_message(&report)))
}

/// 検査だけ（`lawean-check` の報告をそのまま）
pub fn verify(input: &Input<'_>) -> Report {
    lawean_check::run_text_input(input)
}

/// 溶け込みと検査を一度に。溶け込みが最後まで通ったときだけ XML がある
pub fn consolidate_and_verify(input: &Input<'_>) -> (Option<Consolidated>, Report) {
    let report = lawean_check::run_text_input(input);
    let stopped =
        report.failed(Kind::Parse) || report.failed(Kind::Base) || report.failed(Kind::Order);
    if stopped {
        return (None, report);
    }
    let Ok(doc) = lawean_check::consolidated_document(input.base_xml, input.amendment) else {
        return (None, report);
    };
    let out = Consolidated {
        xml: emit_law(&doc).to_xml(),
        units: report.units.clone(),
        diff: report.diff.clone(),
        engine: report.engine.clone(),
    };
    (Some(out), report)
}

/// 落ちた検査を 1 つの文に
pub fn failure_message(report: &Report) -> String {
    report
        .checks
        .iter()
        .filter(|c| c.status == Status::Fail)
        .map(|c| {
            let mut s = format!("{:?}: {}", c.kind, c.message);
            for d in &c.details {
                s.push_str("\n  ");
                s.push_str(d);
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}
