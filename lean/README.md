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
| `Lawean/IdentExamples.lean` | 令和3年 第35条・令和4年 第73/74条の形で、割り込み（発射台がずれても同じ項に当たる）、独立なら可換（定理を `decide` で適用）、依存と施行順序の違反、同じ項への 2 改正の衝突と調整規定による解消を `native_decide` で検査 |

番号ベース（`Basic` / `Apply`）は改め文の表層構文として残す。ADR-0013 以降の本線は `Ident` で、番号 → id の束縛は Rust 側の仕事（未実装）。

Rust 側（`crates/lawean-amend`）は番号ベースの定義の「実装が豊かな版」（本文の文分割・XML との往復・ハネ検出）。意味論の対応は
`Op` の種類と「番号は直前の状態で解釈する」規約で揃えている。Rust の apply を Lean の定義から生成／検証する段階には至っていない（[TODO](../docs/TODO.md)）。

証明は `lake build` で一度だけ検査する。エディタ（ブラウザ）側は検証済みの `applyUnit` を WASM で実行するだけで、実行時に Lean を動かす必要はない（[ADR-0014](../docs/adr/0014-proofs-at-build-time-editor-runs-verified-code.md)）。
