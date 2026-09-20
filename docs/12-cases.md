# 12. 検証ケース — 実際の改正の成功例と、実際の改め文から作った失敗例

状態: 実装（`crates/lawean-check`、`fixtures/cases/cases.json`、`lean/Lawean/Cases.lean`、`docs/playground/`）。2026-09-21。

## 1. 目的

ここまで作った検査（発射台・溶け込み・ハネ・順序・衝突・新旧対照表・他法令）が、**実際に成立した改正は通し、実際に起きる型の失敗は正しく落とす**ことを、
固定したケース集で確かめる。ケースは `cargo test -p lawean-check` が全部走らせ、ブラウザの playground で改め文を書き換えながら試せる。

## 2. 検査 API（`lawean-check`）

`run_texts(発射台 XML, 改め文, 改正後 XML?, 新旧対照表?, 他法令 XML[], 施行日?) -> Report`。中身は既存の crate を順に呼ぶだけで、意味論はここに無い。

| 検査 | 何を見るか | 呼ぶもの |
|---|---|---|
| Parse | 改め文が閉じた語彙で読めるか | `lawean-amend::parse_units` |
| Base（発射台） | 指す条・項・字句が発射台にあるか | `ident::bind` |
| Order（施行順序） | 当たらない単位が、他の単位の後なら当たるか（依存）。`schedule_ok` | `ident::bind` の再試行、`depends_on` |
| Conflict（衝突） | 発射台には当たるが先行する改正の後では本文が違う | `ident::apply_unit` の `conflicts` |
| Hane（ハネ） | 項の繰り下げでずれる参照に、**正しい番号への**手当てがあるか。無ければ**手当てを生成して改め文の形で出す**（`suggested_fixes`） | `hane_candidates`（`render_fix` で置換先を生成し、改め文の置換と突き合わせる） |
| Consolidate | id の重複なし | `IdentRevision::wf` |
| Expected | e-Gov の改正後リビジョンと本則が一致するか | `render` の比較 |
| Taisho（新旧対照表） | 「新」欄が溶け込み後、「旧」欄が改正前の本文と一致するか | 位置 → 本文の突き合わせ |
| CrossLaw（他法令） | 他法令からの参照切れ・ずれ。ずれには他法令側の手当て（「第三十八条第四項」→「第三十八条第五項」）を添える | `lawean-space::impact`（Lean `Space.lean` の `impact` と同じ分類） |

Order / Conflict / Consolidate は Lean の `Ident.applyUnit` の Rust 写しで判定している。同じデータを Lean にも出し、`Consolidate.lean` / `Cases.lean` が同じ結論を `native_decide` で確かめる。
Hane の生成規則は Lean の `Refs.lean`（`renderRef`）と同じで、`RefsExamples.lean` が「生成した手当て = 令3-37 の実際の置換」を確かめる。

## 3. ケース

### 成功例（実際に成立・施行された改正。すべて通る）

| id | 改正 | 発射台 → 改正後 | Lean |
|---|---|---|---|
| `r3-37-art35` | 令和3年法律第37号 第35条（デジタル社会形成整備法）。第22・38・39条に電磁的記録の項、第38条の繰り下げとハネ 2 箇所の手当て | 2021-05-19 → 2022-05-18 | `consolidates_503AC0000000037_art35` |
| `r4-48-art73` | 令和4年法律第48号 第73条（民事訴訟法等改正法、2 段目）。目次・第42条・第61条の新設 | 2022-05-18 → 2023-02-20 | `consolidates_504AC0000000048_art73` |
| `r4-48-art73-74` | 同 第73条 → 第74条（3 段目、第61条の全部改正） | 2022-05-18 → 2026-05-21（現行） | `consolidates_504AC0000000048_art73_74`、`art74_depends_on_art73` |
| `r3-37-then-r4-48-art73` | 令3-37 第35条と令4-48 第73条を 1 つの入力に。独立なので順序不問 | 2021-05-19 → 2023-02-20 | `independent_art35_art73` → `applyUnit_comm` |

`r3-37-art35` には新旧対照表（本則の変わった項を手で起こしたもの）と他法令（高齢者居住安定確保法・施行令）も付けてあり、Taisho と CrossLaw も通る。

### 失敗例（実際の改め文から作ったもの。指定した検査だけが落ちる）

| id | 失敗の型 | 作り方 | 落ちる検査 | 現実の対応物 |
|---|---|---|---|---|
| `wrong-base` | **発射台の取り違え** | 令3-37 第35条を施行済みの 2022-05-18 版に当てる | Base | 他法が先に同じ条を改正した「改正の競合」。「前項」がもう無い |
| `r4-48-reversed` | **施行順序の違反** | 令4-48 第74条を第73条より先に | Order | 3 段施行で施行日が未確定なら調整規定が要る（[08](08-amendment.md) §6） |
| `hane-missing` | **ハネの手当て漏れ** | 第35条から「同条第三項中「前項」を「第三項」に改め」を落とす | Hane, Expected | 繰り下げに伴う参照の改正漏れ。溶け込みは成功するので、本文の突き合わせが無いと気づかない |
| `hane-wrong-number` | **ハネの番号違い** | 同じ箇所を「第四項」に | Hane, Expected | 手当てはあるが指す先が違う |
| `wrong-ref` | **引用条項の誤り** | 「第三十八条第一項の次に」を「第十一項の次に」 | Base | 2021 年のデジタル改革関連法案で報告された参照条項の誤り（45 箇所、[時事](https://www.jiji.com/jc/article?k=2021030900903&g=pol)）の型 |
| `taisho-wrong` | **新旧対照表の誤記** | 改め文は正しく、添付の新旧対照表の「新」欄 2 行を誤らせる | Taisho | 同上。2021 年の誤りは主に新旧対照表・参照条文にあった |
| `conflict-22` | **衝突** | 2 つの改正法が第22条第1項「書面によって」を別々に改める | Conflict | 同じ項への 2 改正。黙って片方を勝たせず値として残す（[ADR-0013](adr/0013-identity-patches.md)） |
| `delete-28-dangling` | **他法令の参照切れ** | 第28条を削る改正案 + 高齢者居住安定確保法 | CrossLaw | 借地借家法の中だけ見れば溶け込む（[09](09-cross-law-impact.md) 計画 5） |

Lean 側: `hane-missing` は `Cases.lean` の `hane_missing_consolidates`（溶け込む）と `hane_missing_differs_only_at_38_5`（違うのは第38条の 1 項だけ）、
`conflict-22` は `conflict_22_has_conflict`（どちらの順でも第22条第1項が衝突）と `conflict_22_resolved`（調整規定で解消）。
`wrong-base` / `r4-48-reversed` は `Consolidate.lean` の `art35_on_wrong_base_conflicts` / `art74_before_art73_fails`。
束縛の段階で止まる失敗（`wrong-ref`）と資料の検査（`taisho-wrong`）、他法令（`delete-28-dangling`）は Rust だけ。

## 4. playground

`docs/playground/index.html`。ケースを選ぶと発射台・改め文・新旧対照表・他法令が入り、「検査する」で報告（各検査の ✓✗、明細、発射台からの差分、束縛した id 操作、対応する Lean の定理名）が出る。
改め文を書き換えて再検査できる（例: ハネの「第三項」を「第四項」にすると Expected と Taisho が落ちる）。

判定はブラウザ内の WASM（`crates/lawean-wasm` = `lawean-check` を wasm32 にしたもの、1.4MB）で、サーバーは静的ファイルだけ。1 ケース数十 ms。

```sh
wasm-pack build crates/lawean-wasm --target web --out-dir ../../docs/playground/pkg --no-typescript --release
python3 -m http.server 8765   # リポジトリのルートで
open http://127.0.0.1:8765/docs/playground/index.html
```

[ADR-0015](adr/0015-service-architecture.md) の経路（Lean → C → WASM）ではなく、Rust の写し（`ident::apply_unit`）を wasm32 にしたもの。Lean との一致は `Consolidate.lean` / `Cases.lean` で担保している。

## 5. 分かったこと・限界

- 成功例 4 件・失敗例 8 件が意図どおりの判定になる。失敗例はすべて**指定した検査だけ**が落ちる（他の検査は巻き添えにならない）
- ハネの検査は「置換があるか」では足りず、**置換先の番号が改正後の番号と合うか**まで見る必要があった（`hane-wrong-number`）
- 「発射台に無い」の分類: 発射台そのものに無い（Base）か、先行する単位の後でだけ無い（Order / Conflict）かは、発射台に対して束縛し直して区別する
- 失敗例の改め文は自作（実際の改め文の書き換え）である。実際に誤った改め文の原文は入手していない（2021 年の誤りは資料側で、改め文本体ではない）。実際に誤った改正法の改め文を fixture にできれば、ここに足す
- 新旧対照表の形式は独自（1 行 1 項）。実物の 2 段組みからの変換は未
- 条ずれ（令和5年法律第53号、2028 施行）は `Ident` が条の繰り下げを持たないので対象外のまま
