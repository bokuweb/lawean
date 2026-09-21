# 0016. 法令の意味も Lean に載せる。法令は Lean の「データ」、意味論は Lean の評価器 1 つ。Z3 は反例探索に併用

状態: accepted（2026-09-20）。実装は [docs/10](../10-lean-semantics.md) の計画に沿って進める

## 文脈

現状、道具ごとに検証している対象が違う（2026-09-20 の整理）:

| 道具 | 対象 |
|---|---|
| Lean（`lean/`） | **改正の代数**。独立な改正単位の可換性（全法令）、実データの溶け込み（`Consolidate.lean`） |
| Z3（`lawean-verify`） | **法令の内容**。手書き Semantic IR の性質 6 件（証明 5、反例 1） |
| Rust | 動くコード全部。Lean の `applyUnit` の手書きの写しを含む |

このままだと 2 つの穴が残る。

1. **改正と意味をまたぐ検査ができない。** 「この改正案を当てても第3条の『存続期間 ≥ 30 年』は保たれるか」は、改正の側が Z3 に無く、意味の側が Lean に無いので、どちらでも書けない
2. **「Lean が正」が改正の代数に限られる。** [ADR-0011](0011-lean-as-reference-for-consolidation.md) の 5 で Semantic IR の Lean 化を後回しにした理由（Z3 の自動反例、IR の形が動く）は、Lean を使い倒す方針とは相性が悪い

問い: 法令を Lean に変換して実行する形になるのか、それは筋が悪くないか。

## 判断

1. **深い埋め込み（deep embedding）にする。** 法令は Lean の**データ**（`Rule` / `Expr` / `Effect` の値）であり、Lean の**コード**（条ごとの `def` / `theorem`）ではない。
   意味論は `applies : Model → World → RuleId → Bool` という**評価器 1 つ**を Lean で書き、性質はその評価器についての定理にする。
   [ADR-0011](0011-lean-as-reference-for-consolidation.md) が `Revision` でやったことを `SemanticModel` でもやる
2. **Rust は Semantic IR を Lean のデータとして出力する**（`lawean-lean` を拡張）。手書き IR も層 2 の抽出結果も同じ経路。Provenance（stable_id・確度）はデータに残す
3. **性質は人が Lean で書く**（`lean/Lawean/Properties.lean`）。証明は評価器を `simp` で展開して `decide` / `omega`。
   自由変数（評価概念・Unknown）を含む反例探索は **Z3 を残して併用**する。Lean は「証明と実行」、Z3 は「反例」
4. **改正 × 意味の定理を最初の目標にする**: 改正単位が触る id と、性質が依存する Rule の閉包が交わらなければ性質は保たれる（frame 定理）。
   交われば再検証（新しいデータに対する `native_decide` / `omega`）。`lawean-space` の「意味変化」報告がその引き金
5. **評価器は実行時にも使う。** 「条件充足の判定」（[00](../00-overview.md) の Rust evaluator）は Lean の評価器を Lean → C で動かす。Rust に写しを書かない（[ADR-0014](0014-proofs-at-build-time-editor-runs-verified-code.md) の経路 1、[ADR-0015](0015-service-architecture.md) の表）。
   性質の**定理**（`Properties.lean`）は [ADR-0015](0015-service-architecture.md) §6 の `consolidates` と同じく開発時の検証で、担当者の改正案ごとに作るものではない。担当者に返るのは評価器と Z3 の値

## 位置づけ（2026-09-21 追記）

内容の性質の定理は「法令が正しい」ことの証明ではない（[00](../00-overview.md)「何を「証明」と呼ぶか」）。性質は人が書き、形式化は解釈であり、結果は起案者向けの検討資料である。本線は改正の検証で、内容の側は改正 × 意味の frame 定理（影響分析）に価値がある。

## 理由

なぜ浅い埋め込み（条を Lean のコードにする、Catala → Coq の流儀）にしないか:

- **抽出器が Lean 構文を吐くことになる。** 層 2（規則 + grande）の出力は IR の値であるべきで、Lean のソースを生成してエラボレータに通すのは監査もテストも難しい
- **Provenance と Unknown が消える。** 条をコードにすると「この述語はどの文から、確度いくつで」がコードのコメントにしかならない。ADR-0001 / 0005 / 0008 の「IR 自身がどこまで正しいかを言う」が壊れる
- **改正がコードの diff になる。** 改正の代数は `Revision`（データ）に対して定義してある。意味もデータなら「改正単位が触る id」と「Rule の provenance」が同じ id 空間で交わり、frame 定理が書ける。コードでは書けない
- 浅い埋め込みの利点（条ごとに自由な述語論理が書ける、Lean の全表現力）は、法令の要件が開放的で全自動で書けないという現実（ADR-0008）の前では活きない。表現力が要る場所は**補題**（人が書く Lean の定理）で足す。補題は Provenance を持たない検証入力である（[07](../07-verification.md)）

なぜ Z3 を捨てないか:

- 「評価概念に何を入れれば性質が破れるか」は ∃ の探索で、Lean の `decide` は有界域でしか答えない。Z3 の sat モデルはそのまま法的に読める反例になっている（[07](../07-verification.md) の第4条のバグ発見）
- 同じ性質を Lean と Z3 の両方に出せば、片方の実装ミスをもう片方が拾う（`Consolidate.lean` と `ident_binding.rs` の三者一致と同じ構図）

## 結果（トレードオフ）

- Semantic IR を**決定可能な形**に絞る必要がある: 期間は月数の Int、述語は名前 + 引数の Bool 変数（不透明）、`overrides` の再帰は燃料つき（上書きの循環が無ければ燃料に依らないことを定理にする。循環検出は `lawean-space` にある）。[07](../07-verification.md) の SMT 写像と同じ制限から始める
- 「∀ 世界」の定理は、性質が触る変数だけを量化する形にする。無限域の Int は `omega`（線形算術のみ）。非線形（第32条の利息）は当面 Z3 か補題
- Lean の `Model` と Rust の `SemanticModel` は同型でなければならない。Rust に Lean の評価器の写しは**書かない**（ADR-0010 の「意味論を二重に持たない」）。Rust 側の検証は Z3 経路だけ
- `lake build` の時間が増える。データは条ごとに分割し、変わった条だけ再ビルドされるようにする
- Lean → C を本線にするので、`ident::apply_unit`（Rust の写し）は C 呼び出しができた時点で消す

## 関連

[docs/10-lean-semantics.md](../10-lean-semantics.md)（計画）、[docs/11-layer2.md](../11-layer2.md)（入力側の計画）、
[ADR-0011](0011-lean-as-reference-for-consolidation.md)、[ADR-0013](0013-identity-patches.md)、[ADR-0014](0014-proofs-at-build-time-editor-runs-verified-code.md)、[ADR-0015](0015-service-architecture.md)、[07](../07-verification.md)
