# 0017. 動くコードは Lean の C 出力。Rust の写しは消す。構成の比較（Verus / Dafny / Rocq）

状態: accepted（2026-09-21）。ネイティブは実装済み（`crates/lawean-leanrt`）、WASM は未

## 文脈

「Lean より適した構成はあるか」（2026-09-21）。ここまでの構成は:

- Lean: 改正の代数（`Ident.applyUnit`）、意味の評価器、ハネ・他法令の定理。実データは `native_decide`
- Rust: 全部の実行。`ident::apply_unit` は Lean の定義の**手書きの写し**で、一致は三者一致テストだけ
- サービスの WASM は Rust の写しを wasm32 にしたもの

弱点は証明支援系の選択ではなく、**証明した定義と動く定義が別物**であること。写しのバグは定理が守らない（ADR-0010 の「意味論を二重に持たない」に反する）。

## 比較

| 構成 | 動くコード = 証明したコード | メタ定理 | 実行環境 | 移植 |
|---|---|---|---|---|
| **A. Lean → C → Rust / WASM**（ADR-0014 の経路 1） | **はい** | 今のまま（可換性・frame・完全性は Lean で済んでいる） | ネイティブは Lean のランタイムを静的リンク。WASM は Emscripten | 無し。`@[export]` を足すだけ |
| B. Rust を Verus で検証 | はい | 帰納法の証明は書けるが Lean より手間。日本語文字列（`Seq<char>`）の扱いが弱い | wasm32 は今のまま | `ident` / `Sem` / `Refs` / `Space` の全面移植 |
| C. Dafny → Rust 出力 | はい | 自動証明が強く手間は少ない | 生成 Rust は読めず、相互運用が面倒 | 全面移植 |
| D. Rocq + 抽出（OCaml） | はい | Lean と同等 | wasm_of_ocaml | 全面移植 |
| 今（Lean は仕様、Rust は写し） | いいえ | — | 楽 | — |

法令の**内容**の証明（Semantic IR の性質、Z3 との併用）は、どの構成でも Lean（か Rocq）が要る。B/C は改正の代数だけを単一言語にできるが、内容の側は別の証明器が残るので、言語は 2 つのまま。

## 判断

1. **A を採る。** `lean/Lawean/Ffi.lean` に `@[export]` の入口（`lawean_apply_unit` / `lawean_check_unit` / `lawean_relation`）を置き、
   `lake build Lawean:static` の静的ライブラリを Rust（`crates/lawean-leanrt`）から C の皮（`csrc/shim.c`）越しに呼ぶ。
   ランタイム（`libleanrt` + `libInit` + gmp + uv）は静的リンク。スレッドごとに `lean_initialize_thread`
2. **`lawean-check` は Lean がリンクされていればそれを使う**（`Report.engine = "lean"`）。無ければ Rust の写し（`"rust"`）。
   写しは「Lean が無い環境の代役」と「一致テストの相手」に格下げし、WASM が Lean 経路になった時点で消す
3. **入出力は行形式**（`id<TAB>art<TAB>text`）。`para_text` が空白を除くので本文にタブ・改行は無い。JSON にして `Lean` パッケージに依存しない
4. **WASM は Emscripten で Lean の C 出力をビルドする**のが次（ADR-0015 §表）。それまでの playground は Rust の写し（`Report.engine = "rust"`）
5. Verus は「Rust に残る部分（束縛 `bind`、パーサ）の検証」に将来使う候補として残す。Lean で書くには IO 寄りすぎる部分

## 理由

- 一致テストは「テストしたケースで一致」しか言わない。A なら `applyUnit_comm` / `checkUnit_iff` / `applyUnit_none_iff` が**動いているコードについて**言える
- 速い: 公職選挙法（本則 1167 項、2MB）の `applyUnit` が Lean の C で 6 ms。サービスの 1 キー入力ごとに足りる
- 移植ゼロ。Lean 側は 60 行、Rust 側は 200 行、C は 50 行

## 結果（トレードオフ）

- ビルドに Lean が要る。`build.rs` が `lake build Lawean:static` を呼ぶ。無ければ `no_lean` で組み、`lawean-leanrt` は `Err(Unavailable)` を返す。CI の rust ジョブにも Lean を入れた
- 静的ライブラリには `Data/`（実リビジョン）も入る（4.5MB）。`Lawean.Ffi` は `Ident` / `Check` / `Refs` しか import しないので初期化はそこまで
- Lean のランタイムは Rust のスレッドを知らない。呼ぶスレッドごとに `lean_initialize_thread` が要る（やらないと落ちる。実際に落ちた）
- WASM は未。Emscripten で `libleanrt` を組む作業が要る。それまで playground と Lean は別のコード

## 関連

[ADR-0010](0010-amendment-first.md)、[ADR-0014](0014-proofs-at-build-time-editor-runs-verified-code.md)、[ADR-0015](0015-service-architecture.md)、[ADR-0016](0016-lean-as-semantic-backend.md)、[docs/10](../10-lean-semantics.md) M4、[docs/12](../12-cases.md) §4
