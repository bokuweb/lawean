//! 本則の条の連番: cargo run -p lawean-amend --example numbering -- <xml>...
use lawean_amend::numbering::check_document;
use lawean_source::parse_response;
fn main() {
    for p in std::env::args().skip(1) {
        let doc = parse_response(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let issues = check_document(&doc);
        println!("{p}: {} 件", issues.len());
        for i in &issues {
            println!("  {}", i.message);
        }
    }
}
