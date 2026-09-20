# ADR — 設計判断の記録

形式は `NNNN-slug.md`。状態は `proposed / accepted / superseded by NNNN`。
0001〜0007 は構想段階（`bokuweb/life` の `idea/legal-ir/`）で出た原則を判断として書き直したもの。
03-examples で反例が出たら状態を変える。

| # | 判断 | 状態 |
|---|---|---|
| [0001](0001-unknown-not-opaque.md) | `Opaque` ではなく `Unknown`。種類を持つ | accepted |
| [0002](0002-deem-vs-presume.md) | 「みなす」と「推定する」を分ける | accepted |
| [0003](0003-permission-vs-power.md) | 「できる」を Permission と Power で分ける | accepted |
| [0004](0004-exception-not-negation.md) | 例外・特則を `Not(condition)` に潰さない | accepted |
| [0005](0005-no-single-interpretation.md) | IR に「唯一の正しい解釈」を持たせない | accepted |
| [0006](0006-source-and-semantic-separated.md) | 文章構造と意味構造を別ツリーで持つ | accepted |
| [0007](0007-stable-id-and-version-id.md) | ID は `stable_id + version_id` | accepted |
| [0008](0008-extraction-strategy.md) | 自然文 → IR の抽出は 3 層。定型は規則、評価概念は人 | accepted（層 2 の手段は 0009 で変更） |
| [0009](0009-layer2-decisions-via-grande.md) | 層 2 の判定は grande（Gemma 4 E4B）。生成 LLM は中核から外す | accepted、実装は後回し |
| [0010](0010-amendment-first.md) | 主役は改正。patch 代数を Lean で定義し、Z3 は反例オラクル | accepted |
| [0011](0011-lean-as-reference-for-consolidation.md) | 溶け込みの正は Lean の `applyUnit`。Rust は実データを Lean に出力する前処理 | accepted、実装は未 |
| [0012](0012-structured-authoring.md) | 既存法令は一回だけ構造化（frontier model 可）、改正は構造化記述で書いて改め文と条文を生成。逆変換を一級に | accepted |
| [0013](0013-identity-patches.md) | 改正単位は identity で書く。独立なら可換（`applyUnit_comm`）、衝突は値、依存は半順序。CRDT は中核に採らない | accepted、Rust の束縛は未 |
| [0014](0014-proofs-at-build-time-editor-runs-verified-code.md) | 証明は開発時に一度。エディタ（ブラウザ）は検証済み `applyUnit` を WASM で実行するだけ。証明用サーバーは置かない | accepted |
| [0015](0015-service-architecture.md) | 省庁向けエディタはサービス。証明はブラウザで走る検証済み関数、サーバーはデータ・保存・協調編集のみ。証跡は決定性による再現記録。0011 の `consolidates` は開発時の回帰テスト | accepted |
| [0016](0016-lean-as-semantic-backend.md) | 法令の意味も Lean に載せる。法令は Lean のデータ（深い埋め込み）、意味論は Lean の評価器 1 つ。Z3 は反例探索に併用。改正 × 意味の frame 定理を最初の目標に | accepted、M1・M2 済み |

## テンプレート

```markdown
# NNNN. タイトル

状態: proposed

## 文脈
## 判断
## 理由
## 結果（トレードオフ）
## 関連
```
