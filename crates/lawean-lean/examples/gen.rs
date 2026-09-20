//! 実データを Lean のデータとして `lean/Lawean/Data/` に書き出す。
//! cargo run -p lawean-lean --example gen

fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("lean/Lawean/Data");
    std::fs::create_dir_all(&out).unwrap();
    for (file, content) in lawean_lean::generate(&root.join("fixtures")) {
        let path = out.join(&file);
        std::fs::write(&path, &content).unwrap();
        println!("wrote {} ({} bytes)", path.display(), content.len());
    }
}
