# 0011. 溶け込みの正は Lean の `applyUnit`。Rust は実データを Lean に出力する前処理

状態: accepted（2026-09-20）。実装は未（[TODO](../TODO.md)）。**位置づけを [ADR-0015](0015-service-architecture.md) で明確化**（2026-09-20）

> **位置づけ（2026-09-20 追記）**: 本 ADR の `consolidates_… := by native_decide` は**開発時の回帰テスト**である。
> Lean の `applyUnit` が e-Gov の過去の改正を本則・目次まで再現することを確かめ、モデルが現実に合っている裏付けにする。
> 省庁向けエディタ（サービス）の経路には出てこない。担当者の改正案ごとに定理を作ることも、サービスのサーバーで Lean を動かすこともしない。
> 製品の経路は [ADR-0014](0014-proofs-at-build-time-editor-runs-verified-code.md) / [ADR-0015](0015-service-architecture.md):
> ブラウザが同じ Lean ソースから作った WASM の `applyUnit` を実行し、証跡は決定性による再現記録で出す。
> 以下の「CI で `lake build` が実データの定理を検査する」は、この開発時の層についての記述である。

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

- Lean の `Revision` に目次と条の挿入・繰り下げ（条ずれ）を足す必要がある
- 文字列の照合は Lean と Rust で同じでなければならない（空白・全角数字の正規化を出力側で揃える）
- CI で `lake build` が実データの定理を検査する。Lean のビルド時間が増える

## 関連

[08-amendment.md](../08-amendment.md) §5、[09-cross-law-impact.md](../09-cross-law-impact.md)、`lean/README.md`
