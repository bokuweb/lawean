//! 改め文を 1 行ずつ読んで操作を表示する（語彙の確認用）。ファイルを与えれば改正単位として読む
//!
//!     echo '第三条中「A」を「B」に改める。' | cargo run -q --example parse_line
//!     cargo run -q --example parse_line -- law.txt
use std::io::BufRead;

fn main() {
    if let Some(path) = std::env::args().nth(1) {
        let text = std::fs::read_to_string(path).unwrap();
        match lawean_amend::parse_units(&text) {
            Ok(units) => {
                for u in units {
                    println!("UNIT {} {}", u.article_of_amending_law, u.target_title);
                    for ins in u.instructions {
                        println!("  {}\n    {:?}", ins.text, ins.ops);
                    }
                }
            }
            Err(e) => println!("ERR {e}"),
        }
        return;
    }
    for line in std::io::stdin().lock().lines() {
        let line = line.unwrap();
        match lawean_amend::parse_instruction(line.trim()) {
            Ok(ops) => println!("OK  {line}\n    {ops:?}"),
            Err(e) => println!("ERR {line}\n    {e}"),
        }
    }
}
