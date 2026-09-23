//! 改め文を 1 行ずつ読んで操作を表示する（語彙の確認用）
//!
//!     echo '第三条中「A」を「B」に改める。' | cargo run -q --example parse_line
use std::io::BufRead;

fn main() {
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        match lawean_amend::parse_instruction(line.trim()) {
            Ok(ops) => println!("OK  {line}\n    {ops:?}"),
            Err(e) => println!("ERR {line}\n    {e}"),
        }
    }
}
