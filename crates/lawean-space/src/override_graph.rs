//! 法令をまたぐ Semantic IR を 1 つのモデルに束ね、上書き関係の循環（互いに「かかわらず」）を検出する。

use lawean_semantic::*;
use std::collections::{BTreeMap, BTreeSet};

/// Rule の ID に法令の接頭辞を付けて、複数法令のモデルを結合できるようにする。
/// `known` にある接頭辞で始まる ID（他法令への参照）はそのまま
pub fn prefix_model(prefix: &str, m: &SemanticModel, known: &[&str]) -> SemanticModel {
    let mut m = m.clone();
    let p = |id: &str| {
        if known.iter().any(|k| id.starts_with(&format!("{k}:"))) {
            id.to_string()
        } else {
            format!("{prefix}:{id}")
        }
    };
    for r in &mut m.rules {
        r.id = RuleId(p(&r.id.0));
        for o in &mut r.overrides {
            if let Override::Rule(id) = o {
                *id = RuleId(p(&id.0));
            }
        }
    }
    m
}

/// `overrides` グラフの循環。A の Rule が B の Rule に優先し、B の Rule が A の Rule に優先する、など
pub fn override_cycles(m: &SemanticModel) -> Vec<Vec<RuleId>> {
    let mut edges: BTreeMap<&RuleId, Vec<&RuleId>> = BTreeMap::new();
    for r in &m.rules {
        for o in &r.overrides {
            if let Override::Rule(t) = o {
                edges.entry(&r.id).or_default().push(t);
            }
        }
    }
    let mut cycles = Vec::new();
    let mut seen_cycles: BTreeSet<Vec<RuleId>> = BTreeSet::new();
    for start in edges.keys() {
        let mut stack: Vec<(&RuleId, Vec<RuleId>)> = vec![(start, vec![(*start).clone()])];
        while let Some((node, path)) = stack.pop() {
            for next in edges.get(node).into_iter().flatten() {
                if *next == *start {
                    let mut c = path.clone();
                    let min = c.iter().min().cloned().unwrap();
                    while c[0] != min {
                        c.rotate_left(1);
                    }
                    if seen_cycles.insert(c.clone()) {
                        cycles.push(c);
                    }
                } else if !path.contains(next) {
                    let mut p = path.clone();
                    p.push((*next).clone());
                    stack.push((next, p));
                }
            }
        }
    }
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;
    use lawean_semantic::build::*;

    #[test]
    fn detects_mutual_override() {
        let a = rule("A:R30b")
            .overrides(&["B:R52"])
            .provenance("x", Confidence::Low, "t");
        let b = rule("B:R52")
            .overrides(&["A:R30", "A:R30b"])
            .provenance("y", Confidence::Low, "t");
        let m = SemanticModel {
            document: "".into(),
            definitions: vec![],
            rules: vec![a, b],
            unknowns: vec![],
        };
        let c = override_cycles(&m);
        assert_eq!(c.len(), 1, "{c:?}");
    }
}
