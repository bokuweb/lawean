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

Rust 側（`crates/lawean-amend`）はこの定義の「実装が豊かな版」（本文の文分割・XML との往復・ハネ検出）。意味論の対応は
`Op` の種類と「番号は直前の状態で解釈する」規約で揃えている。Rust の apply を Lean の定義から生成／検証する段階には至っていない（[TODO](../docs/TODO.md)）。
