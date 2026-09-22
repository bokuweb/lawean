# lawean-consolidate

改め文（改正法の本文）を e-Gov の法令 XML に溶け込ませ、検査する。lawean の他のクレートに依らずに使える入口。

```toml
[dependencies]
lawean-consolidate = { git = "https://github.com/bokuweb/lawean", package = "lawean-consolidate" }
```

Rust だけで動く。Lean・Z3・形態素解析は要らない（`features = ["lean"]` を立てて `lean` があれば、証明した
`applyUnit` で溶け込みを計算し、報告の `engine` が `"lean"` になる。無ければ Rust の写し。結果は同じ）。

## API

```rust
use lawean_consolidate::{consolidate, verify, consolidate_and_verify, Input, Kind};

// 発射台: e-Gov 法令 API の law_data の応答か、裸の <Law>。施行日の直前に効力のある版を選ぶ
let base = std::fs::read_to_string("403AC0000000090_20260521_504AC0000000048.xml")?;
// 改め文: 「第百二十五条　借地借家法（平成三年法律第九十号）の一部を次のように改正する。」から始まる本文。
// 指示の行は全角空白 2 つ、加える条文は 1 つのインデント（衆議院「制定法律」の体裁）
let amend = std::fs::read_to_string("505AC0000000053_art125.txt")?;

// 1. 溶け込み → 改正後の <Law> XML
let out = consolidate(&base, &amend)?;          // Err なら、どの単位のどこが発射台に無いか
std::fs::write("after.xml", &out.xml)?;
println!("{} changes, engine={}", out.diff.len(), out.engine);

// 2. 検査（改正後リビジョン・施行日・他法令・起草時の発射台があれば渡す）
let report = verify(&Input {
    base_xml: &base,
    amendment: &amend,
    expected_xml: Some(&std::fs::read_to_string("403AC0000000090_20280613_505AC0000000053.xml")?),
    enforced: Some("2028-06-13"),
    other_laws: &[std::fs::read_to_string("425AC0000000061.xml")?],   // 参照の波及を見る
    base_draft_xml: None,                                             // 起草時の発射台（先行改正との競合）
    ..Default::default()
});
assert!(report.ok);
for c in &report.checks { println!("{:?} {:?}: {}", c.status, c.kind, c.message); }
for f in &report.suggested_fixes { println!("生成したハネの手当て: {f}"); }

// 3. 両方
let (xml, report) = consolidate_and_verify(&Input { base_xml: &base, amendment: &amend, ..Default::default() });
```

検査の種類（`Kind`）: `Parse` 改め文 / `Base` 発射台 / `Order` 施行順序 / `Conflict` 衝突 / `Hane` ハネ /
`Consolidate` 溶け込みと条番号の連番 / `Expected` 改正後との一致 / `Taisho` 新旧対照表 / `CrossLaw` 他法令 /
`Enforcement` 施行期日 / `Penalty` 罰則 / `Stale` 先行改正との競合。詳しくは
[docs/12](../../docs/12-cases.md)、読める改め文の形は [docs/08](../../docs/08-amendment.md) §3。

## CLI

```bash
cargo run -p lawean-consolidate -- 発射台.xml 改め文.txt --out 改正後.xml \
    [--expected 改正後.xml] [--enforced 2026-04-01] [--law 他法令.xml]... [--base-draft 起草時.xml] [--json]
```

終了コードは検査が全部通れば 0、落ちれば 1。`--json` で `Report` をそのまま出す。

## 実績

実際の改正 22 件（13 法令。2025 年の区分所有法大改正 396 項、建替え円滑化法 766 項を含む）で、溶け込み後の本則が
e-Gov の改正後リビジョンと全項一致する（`fixtures/cases/cases.json`、`cargo test -p lawean-check`）。
