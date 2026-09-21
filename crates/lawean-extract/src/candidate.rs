//! 原文根拠付きの候補 — 層 1・層 2 の抽出が共通で返す形（ADR-0008 の合流点）。
//!
//! 抽出は値を**確定しない**。原文の位置と根拠、正規化した値、信頼度と固定の理由を持つ候補を返し、
//! 採用は上位（起草者、Semantic IR への写し）が決める。項目は jlsi/elsa の `variable-extractor`
//! （RFC0050「原文根拠付き変数抽出」）の契約に揃えてある。フィールドは法令用。
//!
//! 位置は文の stable_id + 文内の UTF-8 バイト範囲。`snippet` は `text[start..end]` そのもので、
//! 位置から再現できることをテストで確かめる。

use lawean_source::StableId;
use serde::{Deserialize, Serialize};

/// 用途別のフィールド（安定した snake_case 名で JSON に出る）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    // 期間・時点（temporal）
    DurationValue,
    Duration,
    Period,
    Elapsed,
    Within,
    Window,
    Before,
    WithinBefore,
    NthDay,
    Every,
    Approx,
    Compare,
    CalendarDay,
    EraDate,
    Enforcement,
    // 罰則（penalty）
    Sanction,
    PenaltyTarget,
    PenaltyAct,
    // 主体・行為（層 1.5、係り受け）
    Subject,
    Object,
    Act,
}

/// 画面表示用の粗い分類。`Field` から一意に決まる
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Duration,
    Date,
    Enforcement,
    Sanction,
    Reference,
    Party,
    Action,
}

impl Field {
    pub fn category(self) -> Category {
        use Field::*;
        match self {
            DurationValue | Duration | Period | Elapsed | Within | Window | Before
            | WithinBefore | NthDay | Every | Approx | Compare => Category::Duration,
            CalendarDay | EraDate => Category::Date,
            Enforcement => Category::Enforcement,
            Sanction => Category::Sanction,
            PenaltyTarget => Category::Reference,
            PenaltyAct | Act => Category::Action,
            Subject | Object => Category::Party,
        }
    }
}

/// 正規化した値の型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Text,
    /// 月数
    Months,
    /// 日数
    Days,
    /// 円
    Yen,
    /// `YYYY-MM-DD`
    Date,
    /// 構造ノードの stable_id（参照先）
    NodeRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// 原文上の根拠
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// 文の stable_id
    pub sentence: StableId,
    /// 文の本文（`plain_text`）の中の UTF-8 バイト範囲
    pub start: usize,
    pub end: usize,
    /// `text[start..end]`
    pub snippet: String,
    /// 前後の文脈（前後それぞれ最大 20 文字）
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub field: Field,
    pub category: Category,
    pub value_kind: ValueKind,
    /// 原文の値（snippet と同じか、その一部）
    pub raw: String,
    /// 正規化した値（月数・日数・円・ISO 日付・stable_id）。無ければ None
    pub normalized: Option<String>,
    pub unit: Option<String>,
    /// 役割（罰則の「対象」「行為」、主体の格「は」「が」「に対し」など）
    pub role: Option<String>,
    /// 原文でこの値を説明しているラベル（条の見出し、定義語、事象の句「借地権の設定後」）
    pub source_label: Option<String>,
    pub evidence: Evidence,
    pub confidence: Confidence,
    /// 固定の非機密な理由（規則名）
    pub reason: String,
}

impl Candidate {
    /// 文 `text` の `[start, end)` を根拠にした候補
    pub fn new(
        field: Field,
        sentence: &StableId,
        text: &str,
        start: usize,
        end: usize,
        reason: &'static str,
    ) -> Self {
        let snippet = text.get(start..end).unwrap_or("").to_string();
        let before: String = text[..start]
            .chars()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let after: String = text[end.min(text.len())..].chars().take(20).collect();
        Candidate {
            field,
            category: field.category(),
            value_kind: ValueKind::Text,
            raw: snippet.clone(),
            normalized: None,
            unit: None,
            role: None,
            source_label: None,
            evidence: Evidence {
                sentence: sentence.clone(),
                start,
                end,
                snippet,
                context: format!("{before}【{}】{after}", text.get(start..end).unwrap_or("")),
            },
            confidence: Confidence::High,
            reason: reason.to_string(),
        }
    }
    pub fn value(
        mut self,
        kind: ValueKind,
        normalized: impl Into<String>,
        unit: Option<&str>,
    ) -> Self {
        self.value_kind = kind;
        self.normalized = Some(normalized.into());
        self.unit = unit.map(str::to_string);
        self
    }
    pub fn role(mut self, r: impl Into<String>) -> Self {
        self.role = Some(r.into());
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.source_label = Some(l.into());
        self
    }
    pub fn confidence(mut self, c: Confidence) -> Self {
        self.confidence = c;
        self
    }
}

/// 同じフィールド・同じ範囲・同じ正規化値は 1 つに。位置順に並べる
pub fn dedup_sorted(mut v: Vec<Candidate>) -> Vec<Candidate> {
    v.sort_by(|a, b| {
        (
            &a.evidence.sentence,
            a.evidence.start,
            a.evidence.end,
            a.field,
        )
            .cmp(&(
                &b.evidence.sentence,
                b.evidence.start,
                b.evidence.end,
                b.field,
            ))
    });
    v.dedup_by(|a, b| {
        a.field == b.field
            && a.evidence.sentence == b.evidence.sentence
            && a.evidence.start == b.evidence.start
            && a.evidence.end == b.evidence.end
            && a.normalized == b.normalized
    });
    v
}
