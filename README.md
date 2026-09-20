# lawean

日本法向け Legal IR と「法令コンパイラ」。

e-Gov 法令 XML を読み込み、原文構造（Source IR）と法的意味（Semantic IR）を別々に持つ中間表現へ変換し、
その上で参照解決・矛盾検出（SMT）・性質の証明（Lean）・改正の波及解析を行うことを目指す。

## ドキュメント

| ファイル | 内容 | 状態 |
|---|---|---|
| [docs/00-overview.md](docs/00-overview.md) | 目的・非目的・v0.1 のスコープ・全体像 | draft |
| [docs/01-target-law.md](docs/01-target-law.md) | 最初の対象法律の選定 | 確定（借地借家法） |
| [docs/02-source-ir.md](docs/02-source-ir.md) | e-Gov 法令 XML → Source IR の写像 | 実装済み |
| [docs/03-examples/](docs/03-examples/) | 対象法の条文と、手書きの期待 Semantic IR（IR 設計のテストコーパス） | 15 例 |
| [docs/04-semantic-ir.md](docs/04-semantic-ir.md) | Semantic IR の仕様 | v0.1 型を実装 |
| [docs/05-glossary.md](docs/05-glossary.md) | 法令用語 ↔ IR 用語 | 育成中 |
| [docs/06-llm-extraction.md](docs/06-llm-extraction.md) | 層 2 の Claude 版。契約と棄却条件は grande 版でも流用 | 保留（ADR-0009） |
| [docs/07-verification.md](docs/07-verification.md) | Verification IR: overrides の意味論、SMT への写像、最初の証明と検証が見つけたバグ | 実装 |
| [docs/TODO.md](docs/TODO.md) | 後回しにしたもの | — |
| [docs/adr/](docs/adr/) | 設計判断の記録 | — |

構想段階のメモ（ツールチェーン比較、改め文の patch 化など）は
`bokuweb/life` の `idea/legal-ir/` にある。本リポジトリの docs はそれを実装に向けて絞り込んだもの。

## crates

| crate | 内容 |
|---|---|
| [lawean-source](crates/lawean-source) | Source IR。e-Gov 法令 XML の lossless なパース・出力。`fixtures/` の 2 法令で往復テスト済み |
| [lawean-semantic](crates/lawean-semantic) | Semantic IR の型、手書き用の構築子、Source IR に対する参照整合性の検査。借地借家法 8 条分の手書きデータ入り |
| [lawean-extract](crates/lawean-extract) | 層 1 の規則ベース抽出（[ADR-0008](docs/adr/0008-extraction-strategy.md)）。文末の効果種別（20 種）、条件節、「〜の規定にかかわらず」→ overrides、「契約の条件にかかわらず」→ Contract、譲歩、参照を文ごとに認識し、条件の中身が `Unknown` の骨組み Rule を作る。借地借家法の平叙文 211 のうち 97% を分類 |
| [lawean-llm](crates/lawean-llm) | 層 2 の Claude 版（[docs/06](docs/06-llm-extraction.md)）。**中核からは外した**（[ADR-0009](docs/adr/0009-layer2-decisions-via-grande.md): 判定は grande で行う）。残余・レビュー補助用に残す。実 API は未実行 |
| [lawean-verify](crates/lawean-verify) | Verification IR（[docs/07](docs/07-verification.md)）。Semantic IR を SMT-LIB に落とし z3 で性質を証明・反証。手書き IR で 第3・4・9・22条の性質 6 件（証明 5、意図した反例 1）。**手書き IR のバグを 1 件検出**（第4条ただし書きの「これ」） |
| [lawean-resolve](crates/lawean-resolve) | Resolved IR。条項参照（前項・同条・第N条第M項・附則第N条・他法令）の認識と解決、overrides の逆引き、scope → Rule 集合、定義語の有効 scope。借地借家法の参照 233 件を未解決 0 で解決 |

```sh
cargo test
cargo run -p lawean-source --example dump -- fixtures/403AC0000000090.xml 3    # 条文を stable_id 付きで表示
cargo run -p lawean-resolve --example refs -- fixtures/403AC0000000090.xml 41  # 条文中の参照を解決して表示
cargo run -p lawean-extract --example skeleton -- fixtures/403AC0000000090.xml 22  # 層 1 の抽出結果（骨組み）
cargo run -p lawean-verify --example smt -- R3 R4   # 手書き IR の SMT-LIB（要 z3 で cargo test）
```

## 進め方

1. ~~対象法律を 1 つ決める（01）~~ 借地借家法
2. ~~e-Gov XML → Source IR のパーサを書く。意味解析はしない（02）~~
3. ~~対象法から条文を 10〜20 抜き、期待する Semantic IR を手書きする（03）~~ 15 例
4. ~~03 で裏付けられた分だけ Semantic IR を仕様化し、実装する（04）~~ 型と手書きデータまで
5. ~~Resolved IR: 相対参照（前項・前条）の解決、`overrides` の逆引き、定義語 scope の解決~~
6. ~~層 1: 文末の効果表現の分類、「〜にかかわらず」3 分類、節境界、骨組み Rule（[ADR-0008](docs/adr/0008-extraction-strategy.md)）~~
7. 層 2: 規則で述語・引数の候補 → grande（Gemma 4 E4B）で判定（[ADR-0009](docs/adr/0009-layer2-decisions-via-grande.md)）— **後回し**（[TODO](docs/TODO.md)）
8. ~~手書きデータを Z3 に落とし、性質を証明する（Verification IR）~~ 6 性質。IR のバグを 1 件検出
9. Verification IR の拡張: 主体、時間（起算点・暦計算）、第26条の時間窓、Lean へのバックエンド ← 次
10. 改正 patch（過去版の取得から）
