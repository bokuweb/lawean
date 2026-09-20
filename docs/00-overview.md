# 00. Overview

## 何を作るか

**日本法の machine-readable semantic layer**。法令を「コードの AST」にするのではなく、
原文と対応づいた意味グラフ（Legal IR）にし、その上に検証・解析ツールを載せる。

```
e-Gov 法令 XML
   │
   ▼
① Source IR        条・項・号・文など原文構造。lossless。意味解析しない
   │  semantic parser（人手 + LLM 補助）
   ▼
② Semantic IR      Rule / Definition / Effect / Exception / Temporal / Unknown / Provenance
   │  reference / definition / temporal resolver
   ▼
③ Resolved IR      参照・定義語・特則・時間を解決したグラフ
   │
   ├──▶ ④ Verification IR ──▶ Lean（性質の証明） / Z3・cvc5（矛盾検出、数量・日付）
   ├──▶ Rust evaluator（条件充足の判定）
   └──▶ 依存グラフ（参照グラフ、改正の波及解析）
```

中心は **② Semantic IR**。Lean / SMT / Rust はすべて IR のバックエンドであり、差し替え可能。

## 何を作らないか

- 法令の「唯一の正しい解釈」を決めるもの。IR は複数解釈と `Unknown` を許す（[ADR-0005](adr/0005-no-single-interpretation.md)）
- 全法令の完全形式化。**部分形式化**で段階的に取り込む
- 自然言語からの全自動変換。LLM は候補生成に使うが、IR には必ず Provenance と確度を付ける
- 手続法（申請 → 審査 → 許可 → 取消）の状態遷移モデル。v0.1 では対象外。必要になったら別モデル（Dafny / TLA+ 向け）

## v0.1 のスコープ

- 法律 **1 本**（[01-target-law.md](01-target-law.md)）
- Source IR は **100%**（附則・別表含めて lossless）
- Semantic IR は **60〜80%**。残りは構造化された `Unknown`
- Semantic IR のノードは **8 種類**に限定

  ```
  Rule / Definition / Condition / Obligation / Prohibition / Permission / Exception / Unknown
  ```

- 扱う対象: **人物・日付・期間・金額・条件・義務・禁止・許可・例外・条項参照**
- 検証は「数値・期間・参照だけ Z3」「重要な 5〜10 個の性質だけ Lean」
- Resolved IR / Verification IR / 改正 patch は **v0.1 に含めない**。ただし ID 設計（`stable_id + version_id`）だけは最初から入れる

## 設計原則（ADR に切り出したもの）

| ADR | 原則 |
|---|---|
| [0001](adr/0001-unknown-not-opaque.md) | `Opaque` ではなく `Unknown`。「理解できなかった」と「意図的に曖昧」を区別する |
| [0002](adr/0002-deem-vs-presume.md) | 「みなす」と「推定する」を分ける |
| [0003](adr/0003-permission-vs-power.md) | 「できる」を Permission と Power で分ける |
| [0004](adr/0004-exception-not-negation.md) | 例外・特則を `Not(condition)` に潰さない |
| [0005](adr/0005-no-single-interpretation.md) | IR に「唯一の正しい解釈」を持たせない |
| [0006](adr/0006-source-and-semantic-separated.md) | 文章構造と意味構造を別々に持つ |
| [0007](adr/0007-stable-id-and-version-id.md) | ID は `stable_id + version_id`。改正 lineage を持つ |

その他、ADR にするほどではないが守ること:

- 参照を文字列にしない。必ず構造化された `Reference`
- 時間（施行日・適用期間・経過措置）を最初から一級市民にする
- 全ノードに Provenance（原文位置・変換者・確度）を付ける

## 最終形

改正法（改め文）を入力すると、参照切れ・ハネ改正漏れ・意味変更・証明失敗（反例付き）を出す「法令コンパイラ」。
法令パーサではなく、HIR → MIR → backend というコンパイラの構成に近い。
