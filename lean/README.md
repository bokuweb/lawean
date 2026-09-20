# lean/ — patch 代数とメタ定理

[ADR-0010](../docs/adr/0010-amendment-first.md)、[docs/08](../docs/08-amendment.md) §5。Lean 4（core のみ、Mathlib 不要）。

```sh
cd lean && lake build
```

| ファイル | 内容 |
|---|---|
| `Lawean/Basic.lean` | `Revision`（条 → 項 → 本文）、`Op`（改め文の操作）、`AmendUnit` |
| `Lawean/Apply.lean` | `applyOp : Revision → Op → Option Revision`（対象が無ければ `none` = 発射台の不一致）、`applyUnit` |
| `Lawean/Theorems.lean` | `updateArticle_comm`: 番号の違う条への書き換えは条番号を変えない限り可換。`articleUpdate_keepsNum`: 改め文の操作は条番号を変えない。**`applyOp_comm`: 触る条が違う 2 操作は可換**（改正単位の可換性の十分条件）。`shiftAfter_gt` / `shiftAfter_le`: 項の挿入は挿入点より後ろだけを 1 つ繰り下げる |
| `Lawean/Examples.lean` | 第38条の項の挿入を `applyUnit` で実行（`native_decide`）、可換性の具体例、発射台不一致の例。`#print axioms` は `propext` のみ |
| `Lawean/Ident.lean` | **identity patch**（[ADR-0013](../docs/adr/0013-identity-patches.md)）。対象を stable_id で指す `Op`（`replace` は期待本文つき / `insertAfter` / `delete` / `resolve`）、`editAt` による apply、衝突を値として持つ `Node.conflicts`、`paraNum`（番号は描画時に計算）、`dependsOn` / `scheduleOk`（依存の半順序と施行順序）。**`applyOp_comm` / `applyUnit_comm`: 触る id が交わらない 2 操作 / 2 改正単位は可換** |
| `Lawean/Consolidate.lean` | **実データ**（ADR-0011）。`Data/` の 4 リビジョン・3 改正単位について、`consolidates_…`（束縛した改め文を発射台に当てると e-Gov の改正後リビジョンと `render` が一致）、`wf`（id の重複・衝突なし）、第73→74条の `dependsOn` / `scheduleOk` と逆順の `= none`、第35条と第73条の `IndependentUnits`（→ `applyUnit_comm` で全発射台について可換）、改正済みの版に当て直すと衝突が 2 つ残ること。すべて `native_decide` |
| `Lawean/Data/*.lean` | **自動生成**（`cargo run -p lawean-lean --example gen`、手で編集しない）。`rev_403AC0000000090_YYYYMMDD : Revision`（借地借家法の本則 131〜133 項 + 目次）、`unit_<改正法ID>_artN : AmendUnit`（発射台に束縛した改め文）、`unit_case_… : AmendUnit`（失敗例）、`unit_draft_… : AmendUnit`（docs/09 の自作改正案）、`sem_403AC0000000090_hand : Model` と Rule ごとの `def «R3-1» : Rule`（層化済みの Semantic IR）。Rust のテストが最新かを検査する |
| `Lawean/Sem.lean` | **法令の意味**（ADR-0016、docs/10）。Semantic IR のデータ型（`Value` / `Expr` / `Effect` / `Rule` / `Model`）、世界 `World`（自由変数の割り当て）、評価器 `applies`（層化された順に例外 → 原則で積む。燃料なし）、`consistent`（世界が法令の模型か）、`Model.stratified` / `wf` |
| `Lawean/SemTheorems.lean` | 評価器のメタ定理。`applies_spec`（wf な Model では applies はその Rule の条件と例外だけで決まる。`run` の不変量で証明）、`consistent_rule`。性質の証明を Model 全体の展開なしに局所化する |
| `Lawean/Properties.lean` | **法令の内容の性質**（docs/10 M2）。`Data/Sem_403AC0000000090_hand.lean`（Rust が出した手書き 8 条）について docs/07 の 6 性質: 第3条 ≥ 30 年、= 30 年の反例、第4条 ≥ 10 年 / 最初の更新 ≥ 20 年、第22条 × 第9条、第9条 + 補題。すべて `omega`、公理は `propext` / `Classical.choice` / `Quot.sound` |
| `Lawean/Frame.lean` | **改正 × 意味の frame 定理**（docs/10 §4）。`closed S`（依存で閉じた Rule 集合）、`Sub S m`（同じレコードで入っている）、`applies_agree`（閉じた S の applies は両 Model で一致）、`consistentOn_agree`、`transfer`（S だけから証明した性質は、S を同じレコードで含む別の Model に再証明なしで移る） |
| `Lawean/FrameExamples.lean` | 令3-37 第35条の前後で第3・4・9条の 5 性質を `transfer` で移送。`modified_…`（Rust が計算した本文の変わった項）と S が交わらないことを `decide`。架空の第3条改正で `Sub` が壊れ、再検証すると破れる例 |
| `Lawean/SemExamples.lean` | 第3条だけの小さな Model で、評価器を全部展開する素朴な証明と反例の検証 |
| `Lawean/Refs.lean` | **参照を id で持つ本文とハネの完全性**（ADR-0012 / 0013）。`Body`（平文の断片 + id 参照）、`renderBody`（番号は `paraNum` から描画。相対形は距離が同じならそのまま、1 なら「前項」、それ以外は絶対形）、`renderBody_congr`（描画は参照の番号にしか依存しない）、`hane_complete`（本文が同じなのに描画が違えば参照の番号が動いている）、`haneFixes` / `haneFixes_sound`（手当ての生成と健全性）。公理は `propext` / `Classical.choice` / `Quot.sound` |
| `Lawean/RefsExamples.lean` | 令3-37 第35条の第38条第2・3項を `Body` で書き、生成した手当てが実際の改め文の `replace` と一字違わず一致することを `native_decide` で |
| `Lawean/Space.lean` | **法令空間と他法令への波及**（docs/09）。`LawSpace`（法令 id → リビジョン）、`XPiece.xref`（他法令の項への id 参照）、`renderXBody`、`impact`（`dangling` / `shifted fix` / `semanticChange` の分類）。`ximpact_complete`（他法令の描画が変わるなら参照先の番号が動いている）、`classify_dangling` / `classify_shifted` / `classify_semantic`（健全性） |
| `Lawean/SpaceExamples.lean` | 施行令の「借地借家法第三十八条第四項」2 箇所と高齢者居住安定確保法第58条の「借地借家法第二十八条」を `XBody` で書き、第38条への項の挿入で `shifted`（手当て「第三十八条第五項」）、第28条の削除で `dangling` を `native_decide` で |
| `Lawean/Cases.lean` | **失敗例**（docs/12、`fixtures/cases`）。`hane-missing`: 溶け込むが e-Gov と第38条の 1 項だけ違う。`conflict-22`: どちらの順でも第22条第1項が衝突として残り、`resolve` で解消できる |
| `Lawean/IdentExamples.lean` | 令和3年 第35条・令和4年 第73/74条の形で、割り込み（発射台がずれても同じ項に当たる）、独立なら可換（定理を `decide` で適用）、依存と施行順序の違反、同じ項への 2 改正の衝突と調整規定による解消を `native_decide` で検査 |

番号ベース（`Basic` / `Apply`）は改め文の表層構文として残す。ADR-0013 以降の本線は `Ident` で、番号 → id の束縛は Rust 側（`lawean-amend::ident::bind`）が行い、結果を `Data/` に出す。

## Lean の入力（`Data/`）の形

```lean
def rev_403AC0000000090_20210519 : Revision :=
  { nodes :=
      [ { id := "toc", art := "toc", text := "目次第一章総則（第一条―第二条）…" },
        { id := "403AC0000000090/main/chap:1/art:1/para:1", art := "1", text := "この法律は、建物の所有を目的とする…" },
        …
        { id := "403AC0000000090/main/chap:3/sec:3/art:38/para:3", art := "38", text := "建物の賃貸人が前項の規定による説明をしなかったときは、…" },
        … ] }

def unit_503AC0000000037_art35 : AmendUnit :=
  [ .insertAfter "…/art:22/para:1" "503AC0000000037/art35/art:22/new:1" "22" "前項前段の特約が…",
    .replace "…/art:38/para:3" "建物の賃貸人が前項の規定による…" "建物の賃貸人が第三項の規定による…",
    … ]
```

- リビジョン = 文書順の項の列。項は `id`（e-Gov の stable_id）・`art`（条番号の文字列。枝番は `"42_2"`、目次は `"toc"`）・`text`（文 + 号の平文、空白除去）。番号は持たない
- 改正単位 = id で対象を指す操作の列。改め文の「第N条第M項」は Rust が発射台の id に束縛する。繰り下げ・「第P項を第Q項とし」は番号だけの話なので消える。「「A」を「B」に改める」は当たった項ごとに本文全体の `replace`（`expected` が Pijul の context）。新しい項の id は `<改正法ID>/art<条>/art:<条>/new:<連番>`
- 一致は `Revision.render`（id を捨てた (条, 本文) の列）で見る。e-Gov は改正後に stable_id を振り直すので id までは一致しない

Rust 側（`crates/lawean-amend`）は番号ベースの定義の「実装が豊かな版」（本文の文分割・XML との往復・ハネ検出）。意味論の対応は
`Op` の種類と「番号は直前の状態で解釈する」規約で揃えている。Rust の apply を Lean の定義から生成／検証する段階には至っていない（[TODO](../docs/TODO.md)）。

法令の**意味**も同じ形で載せる（[ADR-0016](../docs/adr/0016-lean-as-semantic-backend.md)、[docs/10](../docs/10-lean-semantics.md)）: `Sem.lean` / `Properties.lean` / `Frame.lean` まで済み。次は Lean → C（M4）。

証明は `lake build` で一度だけ検査する。エディタ（ブラウザ）側は検証済みの `applyUnit` を WASM で実行するだけで、実行時に Lean を動かす必要はない（[ADR-0014](../docs/adr/0014-proofs-at-build-time-editor-runs-verified-code.md)）。
Lean が走るのはサービスを作る側のビルドだけで、省庁向けエディタのサーバーにも担当者の手元にも置かない。ADR-0011 の `consolidates` 定理は開発時の回帰テスト（[ADR-0015](../docs/adr/0015-service-architecture.md)）。
