//! smt_scripts の出力を手元の z3 で走らせて答え合わせ: cargo run -p lawean-wasm --example smt_probe [--dump]
fn main() {
    let dump = std::env::args().any(|a| a == "--dump");
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");
    let base = std::fs::read_to_string(format!(
        "{root}/revisions/403AC0000000090_20210519_503AC0000000037.xml"
    ))
    .unwrap();
    let amend =
        std::fs::read_to_string(format!("{root}/amendments/503AC0000000037_art35.txt")).unwrap();
    let input = serde_json::json!({"base": base, "amendment": amend, "enforced": "2022-05-18", "promulgated": "2021-05-19",
        "suppl": "第一条　この法律は、令和三年九月一日から施行する。ただし、第三十五条の規定は、公布の日から起算して一年を超えない範囲内において政令で定める日から施行する。"});
    let out: serde_json::Value =
        serde_json::from_str(&lawean_wasm::smt_scripts(&input.to_string())).unwrap();
    for o in out.as_array().unwrap() {
        let script = o["script"].as_str().unwrap();
        if dump {
            for (i, l) in script.lines().enumerate() {
                println!("{:3} {l}", i + 1);
            }
        }
        let v = lawean_verify::run_z3(script).unwrap();
        let a = match v {
            lawean_verify::Verdict::Proved => "unsat".to_string(),
            lawean_verify::Verdict::Counterexample(_) => "sat".to_string(),
            lawean_verify::Verdict::Unknown(u) => u.chars().take(80).collect(),
        };
        println!("{} [{a}] {}", o["kind"], o["name"]);
    }
    println!("scripts: {}", out.as_array().unwrap().len());
}
