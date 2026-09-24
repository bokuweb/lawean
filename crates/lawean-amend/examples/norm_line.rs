//! 改め文の正規化を 1 行ずつ表示する（語彙の確認用）
use std::io::BufRead;

fn main() {
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        let t = line.trim_start_matches(['\u{3000}', ' ']);
        let n = lawean_amend::parse::normalize_instruction(t);
        if n != t {
            println!("IN  {t}\nOUT {n}");
        }
    }
}
