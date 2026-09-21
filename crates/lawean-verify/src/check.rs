//! 性質の検査。z3 を子プロセスで起動する。

use crate::smt::{Compiler, Smt};
use lawean_resolve::ResolvedModel;
use lawean_semantic::Expr;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// 否定が unsat = 性質は常に成り立つ
    Proved,
    /// 否定が sat = 反例（z3 のモデル）
    Counterexample(String),
    Unknown(String),
}

#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("z3 not found on PATH")]
    NoSolver,
    #[error("z3 failed: {0}")]
    Solver(String),
}

pub struct Property {
    pub name: String,
    pub claim: Expr,
    /// 人が与える補題。IR ではなく検証入力（Provenance を持たない）
    pub lemmas: Vec<Expr>,
}

impl Property {
    pub fn new(name: &str, claim: Expr) -> Self {
        Property {
            name: name.into(),
            claim,
            lemmas: Vec::new(),
        }
    }
    pub fn lemma(mut self, e: Expr) -> Self {
        self.lemmas.push(e);
        self
    }
}

/// モデル + 性質 → 検査用の SMT-LIB 全文
pub fn script(rm: &ResolvedModel<'_>, prop: &Property) -> String {
    let mut c = Compiler::new(rm);
    c.compile_model();
    let claim = c.expr(&prop.claim);
    let lemmas: Vec<String> = prop.lemmas.iter().map(|l| c.expr(l)).collect();
    let mut smt: Smt = c.smt;
    for (i, l) in lemmas.into_iter().enumerate() {
        smt.assert(format!("lemma {i}"), l);
    }
    smt.assert(
        format!("negation of: {}", prop.name),
        format!("(not {claim})"),
    );
    let mut s = String::from("(set-option :produce-models true)\n(set-logic ALL)\n");
    s.push_str(&smt.render());
    s.push_str("(check-sat)\n(get-model)\n");
    s
}

pub fn z3_available() -> bool {
    Command::new("z3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn check(rm: &ResolvedModel<'_>, prop: &Property) -> Result<Verdict, CheckError> {
    run_z3(&script(rm, prop))
}

/// SMT-LIB を z3 に渡し、unsat → `Proved`、sat → `Counterexample(モデル)`
pub fn run_z3(src: &str) -> Result<Verdict, CheckError> {
    let mut child = Command::new("z3")
        .args(["-in", "-smt2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| CheckError::NoSolver)?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(src.as_bytes())
        .map_err(|e| CheckError::Solver(e.to_string()))?;
    let out = child
        .wait_with_output()
        .map_err(|e| CheckError::Solver(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let first = stdout.lines().next().unwrap_or("").trim();
    match first {
        "unsat" => Ok(Verdict::Proved),
        "sat" => Ok(Verdict::Counterexample(
            stdout.lines().skip(1).collect::<Vec<_>>().join("\n"),
        )),
        _ => Ok(Verdict::Unknown(format!(
            "{stdout}{}",
            String::from_utf8_lossy(&out.stderr)
        ))),
    }
}

/// 反例モデルから、指定した変数の値を取り出す（表示用。雑なテキスト走査）
pub fn model_value(model: &str, name: &str) -> Option<String> {
    // z3 は `|` が要らない名前（t.y 等）は素のまま出す
    let key = format!("(define-fun |{name}|");
    let key2 = format!("(define-fun {name} ");
    let (i, klen) = match model.find(&key) {
        Some(i) => (i, key.len()),
        None => (model.find(&key2)?, key2.len()),
    };
    let rest = &model[i + klen..];
    // "() Int\n    480)" / "() Int 480)" / "() Int\n    (- 3))" のような形。型の後ろから、対応する閉じ括弧までを取る
    let rest = rest.trim_start().strip_prefix("()")?.trim_start();
    let rest = rest.split_once(|c: char| c.is_whitespace())?.1.trim_start();
    let mut depth = 0i32;
    for (j, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(rest[..j].trim().to_string());
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}
