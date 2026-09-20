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
