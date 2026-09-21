//! Lean への出力（ADR-0011）。Rust は XML → Source IR → 改め文の束縛までを担い、
//! その結果を `lean/Lawean/Data/*.lean` のデータとして出す。溶け込みの検査は Lean の `Ident.applyUnit` で行う。
//!
//! 出すもの:
//! - `def rev_… : Revision` — e-Gov のリビジョンの本則を id ベースの項の列にしたもの（`ident::from_document`）
//! - `def unit_… : AmendUnit` — 改め文を発射台に束縛した id ベースの操作列（`ident::bind`）
//! - `def sem_… : Model` — Semantic IR を層化した Rule の列にしたもの（`sem::emit_model`、docs/10）
//!
//! 本文の正規化は Rust の `para_text`（空白を除いた平文）だけで、Lean 側では何もしない。
//! 定理（`lean/Lawean/Consolidate.lean`）は手で書く。ここは定理の中で使う名前を決めるだけ。

pub mod sem;

use lawean_amend::ident::{IdentOp, IdentRevision};
pub use sem::{emit_model, emit_models, stratify, SemError};
use std::fmt::Write;

/// Lean の文字列リテラル。空白は `para_text` で除いてあるので、逃がすのは `\` と `"` だけ
pub fn lean_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c if c.is_control() => write!(out, "\\u{{{:04x}}}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn header(doc: &str) -> String {
    format!(
        "import Lawean.Ident\n\n/-!\n自動生成: `cargo run -p lawean-lean --example gen`。手で編集しない。\n\n{doc}\n-/\n\nnamespace Lawean.Data\nopen Lawean.Ident\n\n"
    )
}

/// `def <name> : Revision`。id と本文は `IdentRevision` のまま
pub fn emit_revision(doc: &str, name: &str, rev: &IdentRevision) -> String {
    let mut s = header(doc);
    writeln!(s, "def {name} : Revision :=").unwrap();
    writeln!(s, "  {{ nodes :=").unwrap();
    for (i, n) in rev.nodes.iter().enumerate() {
        let open = if i == 0 { "[" } else { " " };
        let close = if i + 1 == rev.nodes.len() {
            " ] }"
        } else {
            ","
        };
        writeln!(
            s,
            "      {open} {{ id := {}, art := {}, text := {} }}{close}",
            lean_string(&n.id),
            lean_string(&n.art),
            lean_string(&n.text)
        )
        .unwrap();
    }
    if rev.nodes.is_empty() {
        writeln!(s, "      [] }}").unwrap();
    }
    s.push_str("\nend Lawean.Data\n");
    s
}

/// `def <name> : AmendUnit`
pub fn emit_unit(doc: &str, name: &str, ops: &[IdentOp]) -> String {
    let mut s = header(doc);
    writeln!(s, "def {name} : AmendUnit :=").unwrap();
    for (i, op) in ops.iter().enumerate() {
        let open = if i == 0 { "[" } else { " " };
        let close = if i + 1 == ops.len() { " ]" } else { "," };
        let body = match op {
            IdentOp::Replace { id, expected, new } => format!(
                ".replace {} {} {}",
                lean_string(id),
                lean_string(expected),
                lean_string(new)
            ),
            IdentOp::InsertAfter {
                anchor,
                new_id,
                art,
                text,
            } => format!(
                ".insertAfter {} {} {} {}",
                lean_string(anchor),
                lean_string(new_id),
                lean_string(art),
                lean_string(text)
            ),
            IdentOp::Delete { id } => format!(".delete {}", lean_string(id)),
            IdentOp::Resolve { id, text } => {
                format!(".resolve {} {}", lean_string(id), lean_string(text))
            }
            IdentOp::Renumber { id, art } => {
                format!(".renumber {} {}", lean_string(id), lean_string(art))
            }
        };
        writeln!(s, "  {open} {body}{close}").unwrap();
    }
    if ops.is_empty() {
        writeln!(s, "  []").unwrap();
    }
    s.push_str("\nend Lawean.Data\n");
    s
}

/// 生成するファイルの一覧: (`lean/Lawean/Data/` からの相対パス, 内容)。
/// 借地借家法の実際の改正 3 件を、e-Gov のリビジョンの系列に沿って束縛する:
/// 20210519 →(令3-37 第35条)→ 20220518 →(令4-48 第73条)→ 20230220 →(同 第74条)→ 20260521（現行）。
/// 20220525 / 20230614 の版は本則が直前の版と同じなので出さない
pub fn generate(fixtures: &std::path::Path) -> Vec<(String, String)> {
    use lawean_amend::ident::{bind, from_document};
    use lawean_amend::parse_units;
    use lawean_source::parse_response;

    let read = |rel: &str| std::fs::read_to_string(fixtures.join(rel)).expect(rel);
    let rev = |id: &str| parse_response(&read(&format!("revisions/{id}.xml"))).unwrap();
    let r0 = rev("403AC0000000090_20210519_503AC0000000037");
    let r1 = rev("403AC0000000090_20220518_503AC0000000037");
    let r2 = rev("403AC0000000090_20230220_504AC0000000048");
    let r3 = parse_response(&read("403AC0000000090.xml")).unwrap();
    let r4 = rev("403AC0000000090_20280613_505AC0000000053");

    let u35 = parse_units(&read("amendments/503AC0000000037_art35.txt"))
        .unwrap()
        .remove(0);
    let mut u = parse_units(&read("amendments/504AC0000000048_art73-74.txt")).unwrap();
    let u74 = u.remove(1);
    let u73 = u.remove(0);

    let b35 = bind(&r0, &u35, "503AC0000000037/art35").unwrap();
    // 失敗例（fixtures/cases、docs/12）: 束縛はできるが溶け込み後が違う / 衝突するもの
    let u35_hane = parse_units(&read(
        "amendments/failures/503AC0000000037_art35_hane_missing.txt",
    ))
    .unwrap()
    .remove(0);
    let b35_hane = bind(&r0, &u35_hane, "case-hane-missing").unwrap();
    let mut uc = parse_units(&read("amendments/failures/conflict-22.txt")).unwrap();
    let ucb = uc.remove(1);
    let uca = uc.remove(0);
    let bca = bind(&r0, &uca, "case-conflict-22/A").unwrap();
    let bcb = bind(&r0, &ucb, "case-conflict-22/B").unwrap();
    // 他法令への波及（docs/09）の自作改正案: 第38条への項の挿入（現行に対して）、第28条の削除（2022-05-18 版に対して）
    let u_ins = parse_units(&read("amendments/drafts/insert-38-4.txt"))
        .unwrap()
        .remove(0);
    let b_ins = bind(&r3, &u_ins, "draft/insert-38-4").unwrap();
    let u_del = parse_units(&read("amendments/drafts/delete-28.txt"))
        .unwrap()
        .remove(0);
    let b_del = bind(&r1, &u_del, "draft/delete-28").unwrap();
    let b73 = bind(&r1, &u73, "504AC0000000048/art73").unwrap();
    // 第74条は第73条が作った id を触るので、e-Gov の版ではなく第73条を束縛した文書に対して束縛する
    let b74 = bind(&b73.doc, &u74, "504AC0000000048/art74").unwrap();

    // 令和5年法律第53号 第125条（民事関係手続のデジタル化）: 現行に当てると 2028-06-13 版になる。条ずれ（renumber）を含む
    let u125 = parse_units(&read("amendments/505AC0000000053_art125.txt"))
        .unwrap()
        .remove(0);
    let b125 = bind(&r3, &u125, "505AC0000000053/art125").unwrap();

    // 公職選挙法（実際に起きた改正漏れ、docs/12 §6）: 平成30年法律第75号の発射台と、令和3年法律第51号（誤りの訂正）の前後
    let k0 = rev("325AC1000000100_20180620_430AC0000000059");
    let k1 = rev("325AC1000000100_20201212_502AC1000000045");
    let k2 = rev("325AC1000000100_20210602_503AC0000000051");
    let uk75 = parse_units(&read("amendments/430AC0100000075.txt"))
        .unwrap()
        .remove(0);
    let bk75 = bind(&k0, &uk75, "430AC0100000075").unwrap();
    let uk51 = parse_units(&read("amendments/503AC0000000051.txt"))
        .unwrap()
        .remove(0);
    let bk51 = bind(&k1, &uk51, "503AC0000000051").unwrap();

    let revs = [
        ("Rev_325AC1000000100_20180620", "rev_325AC1000000100_20180620", "公職選挙法 `325AC1000000100_20180620_430AC0000000059`（平成30年法律第75号の発射台。本則 1164 項）", from_document(&k0)),
        ("Rev_325AC1000000100_20201212", "rev_325AC1000000100_20201212", "公職選挙法 `325AC1000000100_20201212_502AC1000000045`（令和3年法律第51号の発射台）", from_document(&k1)),
        ("Rev_325AC1000000100_20210602", "rev_325AC1000000100_20210602", "公職選挙法 `325AC1000000100_20210602_503AC0000000051`（令和3年法律第51号の施行後）", from_document(&k2)),
        ("Rev_403AC0000000090_20210519", "rev_403AC0000000090_20210519", "借地借家法 `403AC0000000090_20210519_503AC0000000037`（令和3年法律第37号 第35条の発射台）", from_document(&r0)),
        ("Rev_403AC0000000090_20220518", "rev_403AC0000000090_20220518", "借地借家法 `403AC0000000090_20220518_503AC0000000037`（令和3年法律第37号 第35条の施行後 = 令和4年法律第48号 第73条の発射台。20220525 版と本則は同じ）", from_document(&r1)),
        ("Rev_403AC0000000090_20230220", "rev_403AC0000000090_20230220", "借地借家法 `403AC0000000090_20230220_504AC0000000048`（令和4年法律第48号 第73条の施行後。20230614 版と本則は同じ）", from_document(&r2)),
        ("Rev_403AC0000000090_20260521", "rev_403AC0000000090_20260521", "借地借家法 `403AC0000000090_20260521_504AC0000000048`（現行。令和4年法律第48号 第74条の施行後）", from_document(&r3)),
        ("Rev_403AC0000000090_20280613", "rev_403AC0000000090_20280613", "借地借家法 `403AC0000000090_20280613_505AC0000000053`（令和5年法律第53号 第125条の施行後。未施行）", from_document(&r4)),
    ];
    let units = [
        ("Unit_430AC0100000075", "unit_430AC0100000075", "公職選挙法の一部を改正する法律（平成30年法律第75号、参議院の特定枠）を `rev_325AC1000000100_20180620` に束縛したもの（34 文のうち 32 文。入れ子の読替え規定の書き換えと別表を除く）。\n第142条の4に第4項を挿入し第6項を第7項に繰り下げたが、第244条第1項第2号の2の「第百四十二条の四第六項」を改めておらず、罰則が消えた（実際に起きた改正漏れ。docs/12 §6）", bk75.ops),
        ("Unit_503AC0000000051", "unit_503AC0000000051", "公職選挙法の一部を改正する法律（令和3年法律第51号）を `rev_325AC1000000100_20201212` に束縛したもの。上の誤りを「第百四十二条の四第六項」→「第百四十二条の四第七項」で正す", bk51.ops),
        ("Unit_503AC0000000037_art35", "unit_503AC0000000037_art35", "デジタル社会形成整備法（令和3年法律第37号）第35条（`fixtures/amendments/503AC0000000037_art35.txt`）を `rev_403AC0000000090_20210519` に束縛したもの。\n繰り下げ・「第P項を第Q項とし」は id の世界では操作にならないので消え、「前項」の手当ては本文全体の `replace` になる", b35.ops.clone()),
        ("Unit_504AC0000000048_art73", "unit_504AC0000000048_art73", "民事訴訟法等改正法（令和4年法律第48号）第73条（`fixtures/amendments/504AC0000000048_art73-74.txt`）を `rev_403AC0000000090_20220518` に束縛したもの。目次・第42条第1項・第61条の新設", b73.ops),
        ("Unit_504AC0000000048_art74", "unit_504AC0000000048_art74", "同 第74条を、第73条を当てた後の状態に束縛したもの。第61条の全部改正 = 第73条が作った id に anchor した `insertAfter` と、その id の `delete`。\n第73条が作った id を触るので第73条に依存する（`dependsOn`）", b74.ops),
        ("Unit_505AC0000000053_art125", "unit_505AC0000000053_art125", "民事関係手続等における情報通信技術の活用等の推進を図るための関係法律の整備に関する法律（令和5年法律第53号）第125条（`fixtures/amendments/505AC0000000053_art125.txt`）を現行 `rev_403AC0000000090_20260521` に束縛したもの。\n第47条〜第61条を第49条〜第64条に繰り下げる条ずれ（`renumber`）と、第47・48・51条の新設", b125.ops),
        ("Unit_case_hane_missing", "unit_case_hane_missing", "失敗例 `hane-missing`（fixtures/cases）: 令和3年 第35条から「同条第三項中「前項」を「第三項」に改め」を落としたもの。`rev_403AC0000000090_20210519` に束縛。溶け込みはするが e-Gov の改正後と一致しない", b35_hane.ops),
        ("Unit_case_conflict_22_A", "unit_case_conflict_22_A", "失敗例 `conflict-22`（fixtures/cases）の第一条: 第22条第1項「書面によって」を「書面又は電磁的記録によって」に。`rev_403AC0000000090_20210519` に束縛", bca.ops),
        ("Unit_draft_insert_38_4", "unit_draft_insert_38_4", "自作の改正案 `fixtures/amendments/drafts/insert-38-4.txt`（docs/09 計画 3）: 第38条第3項の次に 1 項を挿入し、以降を繰り下げる。現行 `rev_403AC0000000090_20260521` に束縛。施行令の「第三十八条第四項」がずれる", b_ins.ops),
        ("Unit_draft_delete_28", "unit_draft_delete_28", "自作の改正案 `fixtures/amendments/drafts/delete-28.txt`（docs/09 計画 5）: 第28条を削る。`rev_403AC0000000090_20220518` に束縛。高齢者居住安定確保法第58条の「借地借家法第二十八条」が参照切れになる", b_del.ops),
        ("Unit_case_conflict_22_B", "unit_case_conflict_22_B", "同 第二条: 「書面によって」を「書面（電磁的記録を含む。）によって」に。同じ発射台に束縛。A の後に当てると期待した本文と違うので衝突として残る", bcb.ops),
    ];
    let mut out = Vec::new();
    for (file, name, doc, r) in &revs {
        out.push((format!("{file}.lean"), emit_revision(doc, name, r)));
    }
    for (file, name, doc, ops) in &units {
        out.push((format!("{file}.lean"), emit_unit(doc, name, ops)));
    }
    // 手書き Semantic IR（docs/03-examples の 8 条、現行 = 令3-37 の施行後）。層 2 の出力も同じ経路に乗せる。
    // frame 定理（docs/10 §4）のために、令3-37 の発射台（2021-05-19 版）に存在する項の Rule だけを残した Model と、
    // 令3-37 が触った項（e-Gov の id）も出す
    let hand = lawean_semantic::examples::shakuchi_shakuya::model();
    let before_ids: Vec<String> = from_document(&r0)
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .collect();
    let para_of = |src: &str| src.split("/sent:").next().unwrap_or(src).to_string();
    let mut hand_before = hand.clone();
    hand_before
        .rules
        .retain(|r| before_ids.contains(&para_of(&r.provenance.source.0)));
    let touched =
        lawean_amend::ident::touched_egov_ids(&from_document(&r0), &b35.ops, &from_document(&r1))
            .expect("令3-37 は 2021-05-19 版に当たる");
    let modified =
        lawean_amend::ident::modified_egov_ids(&from_document(&r0), &b35.ops, &from_document(&r1))
            .unwrap();
    out.push((
        "Sem_403AC0000000090_hand.lean".into(),
        emit_models(
            "借地借家法の手書き Semantic IR（`lawean-semantic/src/examples/shakuchi_shakuya.rs`、docs/03-examples の第2〜6・9・22・26条）。Rule は層化された順（例外 → 原則、参照先 → 参照元）。\n\n- `sem_403AC0000000090_hand`: 現行（`403AC0000000090_20260521_504AC0000000048`）\n- `sem_403AC0000000090_20210519_hand`: 令和3年法律第37号 第35条の発射台（2021-05-19 版）に存在する項の Rule だけ（第22条第2項が無い）\n- `touched_503AC0000000037_art35`: 同条が触った項（anchor を含む。e-Gov の 2022-05-18 版の id）\n- `modified_503AC0000000037_art35`: 同条が本文を変えた・作った項（anchor を除く）。frame 定理（`Frame.lean`）で「触らない Rule の性質は保たれる」を言うのに使う",
            &[
                ("sem_403AC0000000090_hand", &hand),
                ("sem_403AC0000000090_20210519_hand", &hand_before),
            ],
            &[
                ("touched_503AC0000000037_art35", &touched),
                ("modified_503AC0000000037_art35", &modified),
            ],
        )
        .unwrap(),
    ));
    out
}
