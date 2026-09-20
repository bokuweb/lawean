# 0011. 溶け込みの正は Lean の `applyUnit`。Rust は実データを Lean に出力する前処理

状態: accepted（2026-09-20）。実装済み（2026-09-20、`crates/lawean-lean`、`lean/Lawean/Consolidate.lean`、`lean/Lawean/Data/`）。
判断 1 の `applyUnit` は番号ベースではなく [ADR-0013](0013-identity-patches.md) の `Ident.applyUnit` で行い、Rust の束縛（`lawean-amend::ident::bind`）が番号を id に落とす。
一致は `Revision.render`（id を捨てた (条, 本文) の列）で見る。e-Gov は改正後に stable_id を振り直すので id までは一致しない

## 文脈

[ADR-0010](0010-amendment-first.md) は「patch 代数を Lean で定義し、意味論を二重に持たない」とした。しかし現状は:

| | Rust | Lean |
|---|---|---|
| 適用元・先のリビジョン | e-Gov XML → Source IR | 無い（玩具データ） |
| 改め文 | パーサで `Op` 列に | 無い |
| apply | `lawean-amend`（文分割・XML 往復・ハネ検出つき） | `applyOp`（簡略版） |
| 定理 | — | Lean 版 `applyOp` について |

定理は Lean の中の抽象モデルについて成り立ち、Rust が走らせている apply がそのモデルと同じである保証は「同じ規約で書いた」という言明だけ。二重になっている。

## 判断

1. **溶け込みの検証は Lean の `applyUnit` で行う。** Rust は XML → Source IR → 改め文パースまでを担い、その結果を Lean のデータ（`def rev_… : Revision`、`def unit_… : AmendUnit`）として出力する（`lawean-lean` crate）
2. 個々の改正の検査は `theorem consolidates : applyUnit rev_before unit = some rev_after := by native_decide` の形。`rev_after` も e-Gov のリビジョンから出力する。これで `applyOp_comm` 等の定理が**その実データに載っている `applyUnit`** について言える
3. 施行順序の依存（3 段施行で「第74条が先だと失敗」）も `applyUnit … = none` の定理にする
4. Rust の `apply_unit` は残すが、位置づけは「Lean に出す前の高速な前検査とハネ検出」。Lean と Rust の結果が食い違ったらテストで検出する
5. Semantic IR の Lean 化（`applies_R` を Bool 関数、期間を Int、性質を `omega` / `decide`）は**後回し**。Z3 で反例つきの同じ検査ができており、Semantic IR の形が層 2 で動く可能性があるため

## 理由

- 「Lean が正」を看板で終わらせない。定理と実装の間の穴を、実データを Lean に通すことで塞ぐ
- `Revision` は「条 → 項 → 本文」で足りる（改め文の操作は字句置換と項の出し入れなので文の構造は要らない）。目次だけ足す
- `native_decide` はコンパイル実行なので 100KB 級の法令でも実用になる

## 結果

- Lean の `Revision` に目次を足した（`toc` ノード）。条の挿入は `insertAfter` で足りる。条の繰り下げ（条ずれ）は未
- 文字列の照合は Lean と Rust で同じでなければならない（空白・全角数字の正規化を出力側で揃える）
- CI で `lake build` が実データの定理を検査する。借地借家法（131 項 × 4 版）で `Consolidate.lean` のビルドは数秒

## 関連

[08-amendment.md](../08-amendment.md) §5、[09-cross-law-impact.md](../09-cross-law-impact.md)、`lean/README.md`
