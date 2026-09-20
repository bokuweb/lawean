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
| [docs/03-examples/](docs/03-examples/) | 対象法の条文と、手書きの期待 Semantic IR（IR 設計のテストコーパス） | 12 例 |
| [docs/04-semantic-ir.md](docs/04-semantic-ir.md) | Semantic IR の仕様 | 03 から逆算中 |
| [docs/05-glossary.md](docs/05-glossary.md) | 法令用語 ↔ IR 用語 | 育成中 |
| [docs/adr/](docs/adr/) | 設計判断の記録 | — |

構想段階のメモ（ツールチェーン比較、改め文の patch 化など）は
`bokuweb/life` の `idea/legal-ir/` にある。本リポジトリの docs はそれを実装に向けて絞り込んだもの。

## crates

| crate | 内容 |
|---|---|
| [lawean-source](crates/lawean-source) | Source IR。e-Gov 法令 XML の lossless なパース・出力。`fixtures/` の 2 法令で往復テスト済み |

```sh
cargo test
cargo run -p lawean-source --example dump -- fixtures/403AC0000000090.xml 3   # 条文を stable_id 付きで表示
```

## 進め方

1. ~~対象法律を 1 つ決める（01）~~ 借地借家法
2. ~~e-Gov XML → Source IR のパーサを書く。意味解析はしない（02）~~
3. ~~対象法から条文を 10〜20 抜き、期待する Semantic IR を手書きする（03）~~ 12 例。残り第10・13・32条
4. 03 で裏付けられた分だけ Semantic IR を仕様化し、実装する（04）← いまここ
5. Resolved IR / Verification IR / 改正 patch はその後
