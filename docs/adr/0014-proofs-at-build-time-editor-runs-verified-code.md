# 0014. 証明は開発時に一度。エディタ（ブラウザ）は検証済み関数を実行するだけで、証明のためのサーバーは要らない

状態: accepted（2026-09-20）

## 文脈

[ADR-0012](0012-structured-authoring.md) の構造化記述をブラウザ上のエディタとして実装するとき、
「Lean の証明はブラウザ（WASM）で動くのか。動かないならサーバー必須か」という問いが出た（2026-09-20）。

前提の整理:

- Lean の `theorem` は**型検査の時点**で検証され、実行時コードには残らない（証明消去）。`def`（`applyUnit` 等）だけがプログラムとして残る
- Lean 4 本体には Emscripten ビルドがあるが、公式の Web エディタ（live.lean-lang.org）はサーバーで Lean を動かしている。
  `.olean` の大きさとメモリが理由で、ブラウザ内の型検査は実験的な域を出ない
- lawean の定理は「**どんな** `r : Revision`, `u : AmendUnit` に対しても成り立つ」形（`applyUnit_comm` 等）。
  ユーザーがエディタで編集するのは `Op` / `Article` という**データ**であり、定理を書き直す必要はない

## 判断

1. **証明は開発時（`lake build` / CI）に一度だけ行う。** エディタの操作ごとに証明はしない
2. **エディタが実行するのは検証済みの関数。** `applyUnit`（と、正しさの定理を持つ判定関数 `checkUnit : Revision → AmendUnit → Bool` 相当）を
   WASM にして走らせる。証明は既に済んでいるので、ブラウザ側には Lean のエラボレータもカーネルも要らない
3. **証明のためのサーバーは置かない。** サーバーが要るのは「ユーザーが定理そのものを書く」「入力ごとに証明項を生成して配布する」場合だけで、
   lawean の要件ではない
4. [ADR-0011](0011-lean-as-reference-for-consolidation.md) の `consolidates_… := by native_decide`（実データごとの定理）は
   **CI で検査するもの**であり、エディタの実行時とは別の層。エディタで作った改正案を CI に流して定理化する経路は残す
5. WASM 化の経路は 2 つ残す。決めるのは実装時
   - Lean → C（標準バックエンド）→ Emscripten。Lean の定義そのものが動くので Rust との一致テストが不要になる
   - Rust `lawean-amend` → `wasm32`。Lean との一致は [TODO](../TODO.md) の突き合わせテストで担保する
6. 第三者が「Lean の定理が本当に通っているか」を確かめたい場合は `lean4export` + 独立カーネル（nanoda / lean4checker）で行う。
   これも開発成果物に対する検査で、エディタの実行時ではない。必要になったら足す

## 理由

- 証明の対象はプログラムであってデータではない。一度証明した性質は、どの入力にも自動で適用される
- 「検証済みデシジョンプロシージャを実行時に走らせる」のは reflection / `native_decide` と同じ考え方で、Lean の定石
- ブラウザ内で Lean を動かす道は塞がないが（Mathlib 不要なので不可能ではない）、要件に無いものにコストを払わない

## 結果（トレードオフ）

- エディタに出せる保証は「この操作列を `applyUnit` に通した結果」と「一般の定理（可換性・依存・衝突）」まで。
  「この改正案について新しい定理を証明した」形の保証は CI（ADR-0011）で出す
- Lean → C → WASM を採る場合、Lean の C 出力を Emscripten でビルドするツールチェーンが要る。Rust → wasm32 を採る場合、一致テストが本線になる
- Lean 側で `checkUnit` を書くなら「`checkUnit r u = true ↔ applyUnit r u ≠ none ∧ …`」の形の定理を足す

## 関連

[ADR-0011](0011-lean-as-reference-for-consolidation.md)、[ADR-0012](0012-structured-authoring.md)、[ADR-0013](0013-identity-patches.md)、`lean/README.md`
