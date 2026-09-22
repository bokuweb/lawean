use lawean_amend::*;
fn main() {
    let root = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let read = |p: &str| std::fs::read_to_string(format!("{root}/{p}")).unwrap();
    let rev =
        |id: &str| lawean_source::parse_response(&read(&format!("revisions/{id}.xml"))).unwrap();
    let u4 = parse_units(&read("amendments/507AC0000000047_art4.txt")).unwrap();
    let u5 = parse_units(&read("amendments/507AC0000000047_art5.txt")).unwrap();
    let mid = apply_unit(
        &rev("412AC1000000149_20250601_504AC0000000068"),
        &u4[0],
        "s2",
    )
    .unwrap();
    let d1 = diff_snapshots(
        &snapshot_main(&mid),
        &snapshot_main(&rev("412AC1000000149_20251128_507AC0000000047")),
    );
    println!("stage2 diffs: {}", d1.len());
    let got = apply_unit(&mid, &u5[0], "main").unwrap();
    let want = rev("412AC1000000149_20260401_507AC0000000047");
    for (k, v) in snapshot_main(&got) {
        if k == "73" || k == "TOC" {
            let w = snapshot_main(&want);
            let w = w.get(&k).unwrap();
            for (a, b) in v.iter().zip(w.iter()) {
                if a != b {
                    let (x, y) = (&a.1, &b.1);
                    let i = x
                        .chars()
                        .zip(y.chars())
                        .position(|(p, q)| p != q)
                        .unwrap_or(0);
                    println!(
                        "{k}: ours …{}…\n      egov …{}…",
                        x.chars()
                            .skip(i.saturating_sub(30))
                            .take(80)
                            .collect::<String>(),
                        y.chars()
                            .skip(i.saturating_sub(30))
                            .take(80)
                            .collect::<String>()
                    );
                }
            }
        }
    }
}
