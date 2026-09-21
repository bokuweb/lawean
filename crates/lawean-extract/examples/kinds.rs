//! 効果種別の分布: cargo run -p lawean-extract --example kinds -- <xml>
use lawean_extract::extract;
use lawean_source::parse_response;
use std::collections::BTreeMap;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let doc = parse_response(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut n = BTreeMap::new();
    let (mut total, mut with_cond, mut with_over) = (0, 0, 0);
    for s in extract(&doc) {
        *n.entry(format!("{:?}", s.effect)).or_insert(0) += 1;
        total += 1;
        if !s.conditions.is_empty() {
            with_cond += 1;
        }
        if !s.overrides.is_empty() {
            with_over += 1;
        }
    }
    let mut v: Vec<_> = n.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("sentences {total}, with conditions {with_cond}, with overrides {with_over}");
    for (k, c) in v {
        println!("{c:5} {k}");
    }
}
