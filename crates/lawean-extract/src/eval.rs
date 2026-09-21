//! 候補の適合率・再現率（`fixtures/gold`）。gold に載った文だけを対象に、(field, raw) の完全一致で数える

use crate::candidate::{Candidate, Field};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct GoldLine {
    pub sentence: String,
    pub expected: Vec<GoldCandidate>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct GoldCandidate {
    pub field: Field,
    pub raw: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    pub tp: usize,
    pub fp: usize,
    pub fn_: usize,
}

impl Counts {
    pub fn precision(&self) -> f64 {
        if self.tp + self.fp == 0 {
            1.0
        } else {
            self.tp as f64 / (self.tp + self.fp) as f64
        }
    }
    pub fn recall(&self) -> f64 {
        if self.tp + self.fn_ == 0 {
            1.0
        } else {
            self.tp as f64 / (self.tp + self.fn_) as f64
        }
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub per_field: BTreeMap<Field, Counts>,
    pub total: Counts,
    /// (文, field, raw) の誤検出と見逃し
    pub false_positives: Vec<(String, Field, String)>,
    pub false_negatives: Vec<(String, Field, String)>,
}

pub fn parse_gold(jsonl: &str) -> Result<Vec<GoldLine>, serde_json::Error> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect()
}

/// `candidates` は法令全体の候補。gold に無い文は見ない
pub fn evaluate(gold: &[GoldLine], candidates: &[Candidate]) -> Report {
    let mut r = Report::default();
    for g in gold {
        let mut got: Vec<(Field, String)> = candidates
            .iter()
            .filter(|c| c.evidence.sentence.0 == g.sentence)
            .map(|c| (c.field, c.raw.clone()))
            .collect();
        for e in &g.expected {
            if let Some(i) = got
                .iter()
                .position(|(f, raw)| *f == e.field && *raw == e.raw)
            {
                got.remove(i);
                r.per_field.entry(e.field).or_default().tp += 1;
                r.total.tp += 1;
            } else {
                r.per_field.entry(e.field).or_default().fn_ += 1;
                r.total.fn_ += 1;
                r.false_negatives
                    .push((g.sentence.clone(), e.field, e.raw.clone()));
            }
        }
        for (f, raw) in got {
            r.per_field.entry(f).or_default().fp += 1;
            r.total.fp += 1;
            r.false_positives.push((g.sentence.clone(), f, raw));
        }
    }
    r
}
