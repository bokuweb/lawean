//! Lean の C 出力（`lake build Lawean:static`）と C の皮をリンクする。
//! `lake` が無ければ（CI の rust ジョブ等）`no_lean` を立てて、呼び出しは Err を返す。

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=LAWEAN_NO_LEAN");
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-changed=csrc/shim.c");
    println!("cargo:rerun-if-changed=../../lean/Lawean/Ffi.lean");
    println!("cargo:rerun-if-changed=../../lean/Lawean/Ident.lean");
    println!("cargo:rerun-if-changed=../../lean/Lawean/Check.lean");
    println!("cargo::rustc-check-cfg=cfg(no_lean)");
    let target = std::env::var("TARGET").unwrap_or_default();
    if std::env::var("LAWEAN_NO_LEAN").is_ok() || target.starts_with("wasm32") {
        // wasm32 は Lean → C → Emscripten の経路がまだ無いので Rust の写しで動く（docs/12 §4）
        println!("cargo:rustc-cfg=no_lean");
        return;
    }
    let prefix = match Command::new("lean").arg("--print-prefix").output() {
        Ok(o) if o.status.success() => String::from_utf8(o.stdout).unwrap().trim().to_string(),
        _ => {
            println!(
                "cargo:warning=lean not found; lawean-leanrt is built without the Lean runtime"
            );
            println!("cargo:rustc-cfg=no_lean");
            return;
        }
    };
    let lean_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../lean");
    let status = Command::new("lake")
        .args(["build", "Lawean:static"])
        .current_dir(&lean_dir)
        .status()
        .expect("run lake");
    assert!(status.success(), "lake build Lawean:static failed");
    let lib = lean_dir.join(".lake/build/lib");
    let toolchain_lib = PathBuf::from(&prefix).join("lib/lean");
    cc::Build::new()
        .file("csrc/shim.c")
        .include(PathBuf::from(&prefix).join("include"))
        .compile("lawean_shim");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=Lawean");
    println!("cargo:rustc-link-search=native={}", toolchain_lib.display());
    println!(
        "cargo:rustc-link-search=native={}",
        PathBuf::from(&prefix).join("lib").display()
    );
    // 静的にリンクする（Lean が生成する実行ファイルと同じ: Init + ランタイム + gmp + uv）。
    // dylib（leanshared）だと rpath が依存側の crate に伝わらない
    println!("cargo:rustc-link-lib=static=Init");
    println!("cargo:rustc-link-lib=static=leanrt");
    println!("cargo:rustc-link-lib=static=gmp");
    println!("cargo:rustc-link-lib=static=uv");
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
    }
}
