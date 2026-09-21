//! **証明した定義そのものを呼ぶ**（ADR-0014 の経路 1、ADR-0015）。
//!
//! `lean/Lawean/Ffi.lean` の `@[export]` 関数は `Ident.applyUnit` / `checkUnit` を包んだだけで、
//! Lean のコンパイラが C にしたものを `lake build Lawean:static` で静的ライブラリにし、ここからリンクする。
//! Rust 側の写し（`lawean_amend::ident::apply_unit`）と同じ入力で同じ答えになることをテストで確かめる
//! （`tests/agree.rs`）。将来はこの経路を本線にして写しを消す。
//!
//! `lake` / `lean` が無い環境（CI の rust ジョブ）では `no_lean` で組まれ、全部 `Err(Unavailable)` を返す。

use lawean_amend::ident::{IdentOp, IdentRevision, Node};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Lean ランタイム無しで組まれている
    Unavailable,
    /// Lean 側が入力を読めなかった
    BadInput(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unavailable => write!(f, "Lean runtime not linked (built with no_lean)"),
            Error::BadInput(s) => write!(f, "Lean rejected the input: {s}"),
        }
    }
}

/// 行形式（`Ffi.lean` と同じ）
pub fn encode_revision(r: &IdentRevision) -> String {
    r.nodes
        .iter()
        .map(|n| format!("{}\t{}\t{}", n.id, n.art, n.text))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn encode_unit(ops: &[IdentOp]) -> String {
    ops.iter()
        .map(|op| match op {
            IdentOp::Replace { id, expected, new } => format!("replace\t{id}\t{expected}\t{new}"),
            IdentOp::InsertAfter {
                anchor,
                new_id,
                art,
                text,
            } => format!("insertAfter\t{anchor}\t{new_id}\t{art}\t{text}"),
            IdentOp::Delete { id } => format!("delete\t{id}"),
            IdentOp::Resolve { id, text } => format!("resolve\t{id}\t{text}"),
            IdentOp::Renumber { id, art } => format!("renumber\t{id}\t{art}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn decode_revision(s: &str) -> IdentRevision {
    IdentRevision {
        nodes: s
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                Node {
                    id: f[0].into(),
                    art: f.get(1).copied().unwrap_or("").into(),
                    text: f.get(2).copied().unwrap_or("").into(),
                    conflicts: f
                        .get(3)
                        .filter(|c| !c.is_empty())
                        .map(|c| c.split('\x1f').map(String::from).collect())
                        .unwrap_or_default(),
                }
            })
            .collect(),
    }
}

/// Lean の `applyUnit` が呼べるか（ネイティブ: リンク済み。wasm32: ページが Lean の WASM を用意しているか）
pub fn available() -> bool {
    #[cfg(lean_js)]
    {
        ffi::js_available()
    }
    #[cfg(not(lean_js))]
    {
        !cfg!(no_lean)
    }
}

/// wasm32: ページ側が `globalThis` に置く関数を呼ぶ（`docs/playground/index.html`）。
/// Lean の C 出力を Emscripten で組んだモジュール（`build-lean-wasm.sh`）がその実体
#[cfg(lean_js)]
mod ffi {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = lawean_lean_available)]
        fn lawean_lean_available() -> bool;
        #[wasm_bindgen(js_name = lawean_lean_apply_unit)]
        fn lawean_lean_apply_unit(rev: &str, unit: &str) -> String;
        #[wasm_bindgen(js_name = lawean_lean_check_unit)]
        fn lawean_lean_check_unit(rev: &str, unit: &str) -> String;
        #[wasm_bindgen(js_name = lawean_lean_relation)]
        fn lawean_lean_relation(a: &str, b: &str) -> String;
    }

    pub fn js_available() -> bool {
        lawean_lean_available()
    }
    pub fn apply_unit(rev: &str, unit: &str) -> String {
        lawean_lean_apply_unit(rev, unit)
    }
    pub fn check_unit(rev: &str, unit: &str) -> String {
        lawean_lean_check_unit(rev, unit)
    }
    pub fn relation(a: &str, b: &str) -> String {
        lawean_lean_relation(a, b)
    }
}

#[cfg(all(not(no_lean), not(lean_js)))]
mod ffi {
    use std::ffi::{c_char, CStr, CString};
    use std::sync::Once;

    extern "C" {
        fn lawean_leanrt_init() -> i32;
        fn lawean_leanrt_apply_unit(rev: *const c_char, unit: *const c_char) -> *mut c_char;
        fn lawean_leanrt_check_unit(rev: *const c_char, unit: *const c_char) -> *mut c_char;
        fn lawean_leanrt_relation(a: *const c_char, b: *const c_char) -> *mut c_char;
        fn lawean_leanrt_free(p: *mut c_char);
        fn lawean_leanrt_thread_init();
        fn lawean_leanrt_thread_fini();
    }

    static INIT: Once = Once::new();

    /// スレッドごとの Lean ヒープ。ランタイムを初期化したスレッドは初期化済みなので、それ以外のスレッドで一度だけ
    struct ThreadHeap(bool);
    impl Drop for ThreadHeap {
        fn drop(&mut self) {
            if self.0 {
                unsafe { lawean_leanrt_thread_fini() }
            }
        }
    }
    thread_local! {
        static HEAP: std::cell::RefCell<Option<ThreadHeap>> = const { std::cell::RefCell::new(None) };
    }

    fn init() {
        INIT.call_once(|| {
            let rc = unsafe { lawean_leanrt_init() };
            assert_eq!(rc, 0, "Lean runtime initialization failed");
            HEAP.with(|h| *h.borrow_mut() = Some(ThreadHeap(false)));
        });
        HEAP.with(|h| {
            if h.borrow().is_none() {
                unsafe { lawean_leanrt_thread_init() }
                *h.borrow_mut() = Some(ThreadHeap(true));
            }
        });
    }

    fn call(
        f: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char,
        a: &str,
        b: &str,
    ) -> String {
        init();
        let a = CString::new(a).expect("no NUL");
        let b = CString::new(b).expect("no NUL");
        unsafe {
            let p = f(a.as_ptr(), b.as_ptr());
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            lawean_leanrt_free(p);
            s
        }
    }

    pub fn apply_unit(rev: &str, unit: &str) -> String {
        call(lawean_leanrt_apply_unit, rev, unit)
    }
    pub fn check_unit(rev: &str, unit: &str) -> String {
        call(lawean_leanrt_check_unit, rev, unit)
    }
    pub fn relation(a: &str, b: &str) -> String {
        call(lawean_leanrt_relation, a, b)
    }
}

/// Lean の `applyUnit`。`Ok(None)` は対象の id が無い（発射台の不一致）
pub fn apply_unit(rev: &IdentRevision, ops: &[IdentOp]) -> Result<Option<IdentRevision>, Error> {
    #[cfg(no_lean)]
    {
        let _ = (rev, ops);
        Err(Error::Unavailable)
    }
    #[cfg(not(no_lean))]
    {
        if !available() {
            return Err(Error::Unavailable);
        }
        let out = ffi::apply_unit(&encode_revision(rev), &encode_unit(ops));
        match out.split_once('\n') {
            Some(("ok", rest)) => Ok(Some(decode_revision(rest))),
            None if out == "ok" => Ok(Some(IdentRevision { nodes: vec![] })),
            None if out == "none" => Ok(None),
            _ => Err(Error::BadInput(out)),
        }
    }
}

/// Lean の `checkUnit`（溶け込めて、id 重複なく、衝突なし）
pub fn check_unit(rev: &IdentRevision, ops: &[IdentOp]) -> Result<bool, Error> {
    #[cfg(no_lean)]
    {
        let _ = (rev, ops);
        Err(Error::Unavailable)
    }
    #[cfg(not(no_lean))]
    {
        if !available() {
            return Err(Error::Unavailable);
        }
        match ffi::check_unit(&encode_revision(rev), &encode_unit(ops)).as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(Error::BadInput(other.into())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    Independent,
    BDependsOnA,
    ADependsOnB,
    Overlap,
}

/// Lean の `independentUnits` / `dependsOn`
pub fn relation(a: &[IdentOp], b: &[IdentOp]) -> Result<Relation, Error> {
    #[cfg(no_lean)]
    {
        let _ = (a, b);
        Err(Error::Unavailable)
    }
    #[cfg(not(no_lean))]
    {
        if !available() {
            return Err(Error::Unavailable);
        }
        match ffi::relation(&encode_unit(a), &encode_unit(b)).as_str() {
            "independent" => Ok(Relation::Independent),
            "b_depends_on_a" => Ok(Relation::BDependsOnA),
            "a_depends_on_b" => Ok(Relation::ADependsOnB),
            "overlap" => Ok(Relation::Overlap),
            other => Err(Error::BadInput(other.into())),
        }
    }
}
