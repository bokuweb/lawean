//! `lean/Lawean/Data/` が今の Rust の出力と一致しているか。ずれていたら `cargo run -p lawean-lean --example gen`

#[test]
fn generated_lean_data_matches_checked_in_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut stale = Vec::new();
    for (file, content) in lawean_lean::generate(&root.join("fixtures")) {
        let path = root.join("lean/Lawean/Data").join(&file);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()) {
            stale.push(file);
        }
    }
    assert!(
        stale.is_empty(),
        "stale: {stale:?}. run `cargo run -p lawean-lean --example gen`"
    );
}

#[test]
fn lean_string_escapes() {
    assert_eq!(lawean_lean::lean_string(r#"a"b\c"#), r#""a\"b\\c""#);
    assert_eq!(lawean_lean::lean_string("「前項」"), "\"「前項」\"");
}
