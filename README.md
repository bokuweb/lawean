# lawean

日本法向け Legal IR と「法令コンパイラ」。

e-Gov 法令 XML を読み込み、原文構造（Source IR）と法的意味（Semantic IR）を別々に持つ中間表現へ変換し、
その上で **改正法（改め文）を patch として適用・検査**し、施行シナリオごとのリビジョンについて参照解決・矛盾検出・性質の証明を行う。
成立する法律の約 8 割は一部改正法であり、審査の負担と誤りはそこに集中している（[docs/08](docs/08-amendment.md) §1）。

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
| [docs/08-amendment.md](docs/08-amendment.md) | **改正**: 法制執務の実態、改正単位 / シナリオ / リビジョン、改め文の語彙、検査、Lean の役割 | 実装（Lean が実データの溶け込みを検査） |
| [docs/09-cross-law-impact.md](docs/09-cross-law-impact.md) | **他法令への波及**: A の改正が A を参照する B に参照切れ・ずれ・意味変化・時期不整合を生むことを、施行時点の法令空間で検出する。実データは高齢者居住安定確保法・借地借家法施行令 | 実装（Lean は未） |
| [docs/10-lean-semantics.md](docs/10-lean-semantics.md) | **法令の意味を Lean に載せる計画**（[ADR-0016](docs/adr/0016-lean-as-semantic-backend.md)）: 法令は Lean のデータ、意味論は評価器 1 つ、性質は定理、Z3 は反例。改正 × 意味の frame 定理。M1〜M5 | 計画 |
| [docs/11-layer2.md](docs/11-layer2.md) | **層 2 の計画**: 規則で述語・引数・値の候補（形態素解析 + 格助詞）→ grande で判定 → 人が昇格 → Lean へ。評価指標つき | 計画 |
| [docs/12-cases.md](docs/12-cases.md) | **検証ケース**: 実際の改正 4 件（通る）と、実際の改め文から作った失敗例 8 件（発射台違い・順序・ハネ漏れ・番号違い・引用ミス・新旧対照表の誤記・衝突・他法令の参照切れ。指定した検査だけが落ちる）。ブラウザの playground | 実装 |
| [docs/playground/](docs/playground/index.html) | **改正案の検査 playground**（WASM）。ケースを選び、改め文を書き換えて検査する | — |
| [docs/TODO.md](docs/TODO.md) | 後回しにしたもの | — |
| [docs/adr/](docs/adr/) | 設計判断の記録 | — |
| [docs/design-notes.html](docs/design-notes.html) | 方針と設計のやさしい解説（HTML）。最小の Lean 例、異常検出の例、identity patch の解説つき | — |
| [docs/playground.html](docs/playground.html) | identity patch をブラウザで試す。改正単位の順序を入れ替えて溶け込み・番号の描画・衝突・依存・全順序の合流を確認（`Ident.lean` と同じ意味論を JS で写したもの） | — |

構想段階のメモ（ツールチェーン比較、改め文の patch 化など）は
`bokuweb/life` の `idea/legal-ir/` にある。本リポジトリの docs はそれを実装に向けて絞り込んだもの。

## crates

| crate | 内容 |
|---|---|
| [lawean-source](crates/lawean-source) | Source IR。e-Gov 法令 XML の lossless なパース・出力。`fixtures/` の 2 法令で往復テスト済み |
| [lawean-semantic](crates/lawean-semantic) | Semantic IR の型、手書き用の構築子、Source IR に対する参照整合性の検査。借地借家法 8 条分の手書きデータ入り |
| [lawean-extract](crates/lawean-extract) | 層 1 の規則ベース抽出（[ADR-0008](docs/adr/0008-extraction-strategy.md)）。文末の効果種別（20 種）、条件節、「〜の規定にかかわらず」→ overrides、「契約の条件にかかわらず」→ Contract、譲歩、参照を文ごとに認識し、条件の中身が `Unknown` の骨組み Rule を作る。借地借家法の平叙文 211 のうち 97% を分類 |
| [lawean-llm](crates/lawean-llm) | 層 2 の Claude 版（[docs/06](docs/06-llm-extraction.md)）。**中核からは外した**（[ADR-0009](docs/adr/0009-layer2-decisions-via-grande.md): 判定は grande で行う）。残余・レビュー補助用に残す。実 API は未実行 |
| [lawean-amend](crates/lawean-amend) | **改正**（[docs/08](docs/08-amendment.md)）。改め文パーサ（閉じた語彙 11 種）、Source IR への apply、発射台・順序・ハネの検査。実際の改正 3 件で e-Gov の改正後リビジョンと一致。`ident`: 番号ベースの `Op` を発射台の stable_id に束縛して identity patch にする（繰り下げは消え、字句置換は本文全体の `replace` に、全部改正は `delete` + `insertAfter`）。Lean の `Ident.applyUnit` の写しで前検査 |
| [lean/](lean/) | patch 代数（`Revision` / `Op` / `applyOp`）とメタ定理。触る条が違う 2 操作の可換性 `applyOp_comm` を証明。**identity patch**（[ADR-0013](docs/adr/0013-identity-patches.md)）: 対象を stable_id で指し、独立な 2 改正単位の可換性 `applyUnit_comm`、衝突は値、依存は半順序（`scheduleOk`）。**`Consolidate.lean`**: 実データ（`Data/`）を `Ident.applyUnit` に通し、e-Gov の改正後リビジョンとの一致・依存・独立・衝突を `native_decide` で検査 |
| [lawean-lean](crates/lawean-lean) | **Lean への出力**（[ADR-0011](docs/adr/0011-lean-as-reference-for-consolidation.md)、[ADR-0016](docs/adr/0016-lean-as-semantic-backend.md)）。e-Gov のリビジョンを `def rev_… : Revision`、発射台に束縛した改め文を `def unit_… : AmendUnit`、Semantic IR を層化して `def sem_… : Model` として `lean/Lawean/Data/` に出す。番号 → stable_id の束縛は `lawean-amend::ident::bind` |
| [lawean-space](crates/lawean-space) | **他法令への波及**（[docs/09](docs/09-cross-law-impact.md)）。法令空間、法令をまたぐ参照の解決、改正による条・項の移動と本文変化の追跡、参照切れ・ずれ・意味変化・時期不整合の報告、上書きの循環検出。テスト計画 1〜5 が実データ + 自作改正案で通る |
| [lawean-render](crates/lawean-render) | **逆変換**（[ADR-0012](docs/adr/0012-structured-authoring.md)）。`Op` → 改め文（実改正 2 件で parse ∘ render = id）、Semantic IR → 日本語（原文と並べてレビュー） |
| [lawean-verify](crates/lawean-verify) | Verification IR（[docs/07](docs/07-verification.md)）。Semantic IR を SMT-LIB に落とし z3 で性質を証明・反証。手書き IR で 第3・4・9・22条の性質 6 件（証明 5、意図した反例 1）。**無矛盾検査** `conflicts`（相反する効果が同時に適用される世界を探す）。**期間の計算** `temporal`（民法 140〜143 条を暦の式に。初日不算入・応当日・月末）と**効力の区間** `validity`（参照元が効力を持つのに参照先が違う日を反例に。公職選挙法の 3 年の空白）。手書き IR のバグを 2 件検出（第4条の「これ」、第4条の括弧書きとただし書きの優先） |
| [lawean-check](crates/lawean-check) | **検査 API**（サービスが呼ぶもの、[docs/12](docs/12-cases.md)）。発射台・順序・衝突・ハネ・溶け込み・改正後との一致・新旧対照表・他法令の 9 検査を `Report` に。`cargo run -p lawean-check --example check -- --case <id>` |
| [lawean-leanrt](crates/lawean-leanrt) | **証明した定義そのものを呼ぶ**（[ADR-0017](docs/adr/0017-lean-to-c-is-the-runtime.md)）。`lean/Lawean/Ffi.lean` の `@[export]` → `lake build Lawean:static` → C の皮 → Rust。`lawean-check` は Lean がリンクされていればこちらで溶け込む（`Report.engine = "lean"`）。公職選挙法 1167 項で 6 ms |
| [lawean-wasm](crates/lawean-wasm) | `lawean-check` を wasm32 に（playground 用）。溶け込みは `docs/playground/lean/` の Lean WASM を JS 越しに呼ぶ |
| [lawean-resolve](crates/lawean-resolve) | Resolved IR。条項参照（前項・同条・第N条第M項・附則第N条・他法令）の認識と解決、overrides の逆引き、scope → Rule 集合、定義語の有効 scope。借地借家法の参照 233 件を未解決 0 で解決 |

```sh
cargo test
cargo run -p lawean-source --example dump -- fixtures/403AC0000000090.xml 3    # 条文を stable_id 付きで表示
cargo run -p lawean-resolve --example refs -- fixtures/403AC0000000090.xml 41  # 条文中の参照を解決して表示
cargo run -p lawean-extract --example skeleton -- fixtures/403AC0000000090.xml 22  # 層 1 の抽出結果（骨組み）
cargo run -p lawean-verify --example smt -- R3 R4   # 手書き IR の SMT-LIB（要 z3 で cargo test）
cargo run -p lawean-render --example review -- fixtures/403AC0000000090.xml  # 手書き IR を原文と並べて日本語で表示
cargo run -p lawean-amend --example amend -- fixtures/amendments/503AC0000000037_art35.txt fixtures/revisions/403AC0000000090_20210519_503AC0000000037.xml fixtures/revisions/403AC0000000090_20220518_503AC0000000037.xml  # 改め文の適用と突き合わせ
cargo run -p lawean-check --example check -- --case hane-missing   # 検証ケースの報告（fixtures/cases/cases.json）
cargo run -p lawean-lean --example gen   # 実リビジョンと束縛した改め文を lean/Lawean/Data/ に出力（テストが最新かを検査する）
(cd lean && lake build)                  # 定理の検査。Consolidate.lean が実データの溶け込みを native_decide で確かめる
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
9. **改正**（[ADR-0010](docs/adr/0010-amendment-first.md)、[docs/08](docs/08-amendment.md) §6）← いまここ
   1. ~~改め文パーサ（令和3年法律第37号 第35条、令和4年法律第48号 第73・74条）~~
   2. ~~apply と e-Gov リビジョンとの一致~~ 3 件とも一致
   3. ~~発射台・ハネの検査~~
   4. ~~Lean: patch 代数と可換性の十分条件~~ [lean/](lean/)。`applyOp_comm` を証明
   5. ~~3 段施行のシナリオ検査~~ 順序依存と合流を実データで検出
   6. ~~リビジョンへの意味層の適用~~ Semantic IR はリビジョンに束縛される（改正前では validate が落ちる）
10. ~~逆変換~~（[ADR-0012](docs/adr/0012-structured-authoring.md)）: 既存法令は一回だけ構造化、改正は構造化記述で書いて改め文を生成する方針。`Op` → 改め文と Semantic IR → 日本語を実装
11. ~~Lean を溶け込みの正にする~~（[ADR-0011](docs/adr/0011-lean-as-reference-for-consolidation.md)）: 実リビジョン 4 版と束縛した改め文 3 件を Lean に出力し、`consolidates_…` を `Ident.applyUnit` + `native_decide` で。第73→74条の依存、第35条と第73条の独立（`applyUnit_comm` の実データ版）、発射台のずれが衝突として残ることも定理に
12. ~~他法令への波及~~（[docs/09](docs/09-cross-law-impact.md)）: 借地借家法の改正案が高齢者居住安定確保法・施行令に生む参照ずれ・時期の区間・上書きの循環を検出、Z3 で反例
13. ~~identity patch~~（[ADR-0013](docs/adr/0013-identity-patches.md)）: 割り込み（未確定施行日・整備法）で発射台がずれても同じ項に当たるよう、改正単位を stable_id で書く。独立なら可換を Lean で証明。Rust の束縛 `ident::bind` も実データ 3 件で通る
14. **法令の意味を Lean に**（[ADR-0016](docs/adr/0016-lean-as-semantic-backend.md)、[docs/10](docs/10-lean-semantics.md)）← いまここ。~~M1: `Sem.lean` の評価器と第3条~~ → ~~M2: 手書き 8 条と 07 の 6 性質を Lean で~~（層化が IR のバグを 1 件検出）→ ~~M3: 改正 × 意味の frame 定理を令3-37 で~~（5 性質を再証明なしで移送）→ M4: Lean → C（Rust の写しを消す）
15. **層 2**（[docs/11](docs/11-layer2.md)）: 形態素解析 + 格助詞で候補 → grande で判定。docs/10 の M1〜M3 と並行、M5 で合流
16. ~~検証ケースと playground~~（[docs/12](docs/12-cases.md)）: 実際の改正 4 件が通り、失敗例 8 件が指定した検査だけで落ちる。`lawean-check` + WASM playground
    - ~~ハネの手当てを生成する~~（ADR-0012）: Rust が正しい置換を生成して改め文と突き合わせ、足りなければ改め文の形で提案。Lean `Refs.lean` で「描画が変わるのは参照先の番号が動いたときだけ」（完全性）を証明、令3-37 の実データで生成 = 実際の置換
    - ~~他法令への波及を Lean に~~（docs/09）: `Space.lean` の法令空間と `impact` の分類（健全性・完全性）。施行令の参照のずれと高齢者居住安定確保法の参照切れを実データで
    - ~~M4: Lean → C → Rust / WASM~~（[ADR-0017](docs/adr/0017-lean-to-c-is-the-runtime.md)）: `lawean-leanrt` が証明した `applyUnit` / `checkUnit` を C 経由で呼び `lawean-check` の本線に。playground もブラウザ内で Lean の WASM（Emscripten）が溶け込みを計算する。Rust の写しは代役
    - ~~実際に起きた改正漏れの再現~~（docs/12 §5）: 公職選挙法 平成30年法律第75号（罰則の引用の改め忘れ）を実物の改め文と e-Gov の発射台で再現。唯一の未手当てがその箇所で、生成した手当ては 3 年後の訂正法と一字違わず同じ。Lean `Koshoku.lean`
17. `Ident` に条の挿入（条ずれ、令和5年法律第53号）と参照の id 化。`checkUnit`（[ADR-0014](docs/adr/0014-proofs-at-build-time-editor-runs-verified-code.md)）
18. その他: [TODO](docs/TODO.md)
