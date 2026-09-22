//! 監査: 成功ケース全部で、既存の本文の参照を id で持ち、改正を当てた後の描画が変わる参照を、
//! 当てた後の実際の本文と比べる。実際の本文に「後」が無ければ、実際の改正が手当てしていない = 我々の誤検出（か実際の漏れ）
use lawean_amend::body::{body_of, render_pieces_styled, Body, Piece};
use lawean_amend::ident::{apply_unit, bind, from_document};
fn main() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| std::fs::read_to_string(format!("{root}/{p}")).unwrap();
    let cases: serde_json::Value = serde_json::from_str(&read("cases/cases.json")).unwrap();
    let (mut n_changed, mut n_ok, mut n_bad) = (0, 0, 0);
    for c in cases.as_array().unwrap() {
        if c["kind"] != "success" {
            continue;
        }
        let base = lawean_source::parse_response(&read(c["base"].as_str().unwrap())).unwrap();
        let title = base
            .title
            .as_ref()
            .map(|t| lawean_source::inline_text(&t.text))
            .unwrap_or_default();
        let units = lawean_amend::parse_units(&read(c["amendment"].as_str().unwrap())).unwrap();
        let rd = from_document(&base);
        for u in units.iter().filter(|u| u.target_title == title) {
            let Ok(b) = bind(&base, u, "x") else { continue };
            let rx = apply_unit(&rd, &b.ops).unwrap();
            for n in &rd.nodes {
                let body: Body = body_of(&rd, &n.id, &n.text);
                let before = render_pieces_styled(&rd, &n.id, &body);
                let after = render_pieces_styled(&rx, &n.id, &body);
                let Some(nx) = rx.nodes.iter().find(|m| m.id == n.id) else {
                    continue;
                };
                for (p, (a, bb)) in body.iter().zip(before.iter().zip(after.iter())) {
                    if !matches!(p, Piece::Ref { .. }) || a == bb {
                        continue;
                    }
                    n_changed += 1;
                    // 当てた後の本文に「後」の字句があれば手当て済み
                    if nx.text.contains(bb.as_str()) {
                        n_ok += 1;
                    } else {
                        n_bad += 1;
                        println!(
                            "{} {} 「{a}」→「{bb}」 実際の本文: …{}…",
                            c["id"],
                            n.id.rsplit("/main/").next().unwrap(),
                            nx.text.chars().take(60).collect::<String>()
                        );
                    }
                }
            }
        }
    }
    println!("changed={n_changed} handled={n_ok} unhandled={n_bad}");
}
