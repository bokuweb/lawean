//! 手書き Semantic IR を原文と並べて表示する（レビュー用）。
//! cargo run -p lawean-render --example review -- fixtures/403AC0000000090.xml

use lawean_render::render_model;
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_source::parse_response;

fn main() {
    let path = std::env::args().nth(1);
    let doc = path.map(|p| parse_response(&std::fs::read_to_string(p).unwrap()).unwrap());
    print!("{}", render_model(&shakuchi_shakuya::model(), doc.as_ref()));
}
