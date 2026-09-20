//! 手書き IR（借地借家法）を SMT-LIB にして表示する。引数で Rule ID の接頭辞を絞れる。
//! cargo run -p lawean-verify --example smt -- R3 R4

use lawean_resolve::ResolvedModel;
use lawean_semantic::examples::shakuchi_shakuya;
use lawean_verify::*;

fn main() {
    let prefixes: Vec<String> = std::env::args().skip(1).collect();
    let mut model = shakuchi_shakuya::model();
    if !prefixes.is_empty() {
        model
            .rules
            .retain(|r| prefixes.iter().any(|p| r.id.0.starts_with(p.as_str())));
    }
    let rm = ResolvedModel::new(&model);
    let mut c = Compiler::new(&rm);
    c.compile_model();
    print!("{}", c.smt.render());
}
