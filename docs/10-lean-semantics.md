# 10. 法令の意味を Lean に載せる — 計画

状態: M1〜M3 済み（`lean/Lawean/Sem.lean`、`SemTheorems.lean`、`Properties.lean`、`Frame.lean`、`FrameExamples.lean`、`Data/Sem_403AC0000000090_hand.lean`）。M4 以降は §6。[ADR-0016](adr/0016-lean-as-semantic-backend.md)（2026-09-20）

## 1. 何を作るか

[07](07-verification.md) で Z3 に落としていた Semantic IR の意味論を、Lean の評価器として書き直し、
実データ（手書き IR → 層 2 の抽出結果）を Lean のデータとして流し込み、性質を定理にする。
[ADR-0011](adr/0011-lean-as-reference-for-consolidation.md) が改正について作った経路と同じ形:

```
Rust                                   Lean（lake build / CI）
────────────────────────────────       ───────────────────────────────────────────────
Source IR ──→ Ident.Revision  ──────→  Data/Rev_*.lean   ┐
改め文    ──→ Ident.AmendUnit ──────→  Data/Unit_*.lean  ├─ Consolidate.lean（溶け込み、実装済み）
Semantic IR ─→ Sem.Model      ──────→  Data/Sem_*.lean   ┘  Properties.lean（内容の性質）
                                                            Frame.lean（改正 × 意味）
```

法令は **Lean のデータ**、意味論は **Lean の評価器 1 つ**（深い埋め込み）。条を Lean のコードにはしない（理由は ADR-0016）。

## 2. Lean 側の型（`lean/Lawean/Sem.lean`）

Rust の `SemanticModel`（[04](04-semantic-ir.md)、`lawean-semantic::ir`）のうち、[07](07-verification.md) が SMT に落とせている部分から始める。同型を保つ。
実装は `Sem.lean` が正で、ここは要旨（述語の引数は Rust の `pred_name` で名前に畳む。`Effect` には `preserve` / `exception` もある）。

```lean
namespace Lawean.Sem

abbrev RuleId := String
abbrev Name   := String            -- 述語名・属性名・変数名（Rust と同じ文字列）

inductive Value
  | int (n : Int)                  -- 期間は月数、金額は円。Rust の Duration / Money を Rust 側で Int に落とす
  | var (x : Name)                 -- 属性 = 変数（「存続期間」）。07 と同じく同じ名前空間
  | ruleValue (r : RuleId)         -- 他 Rule の Set が定める値（「これより長い」）
  | add (a b : Value) | sub (a b : Value)

inductive CmpOp | eq | ne | lt | le | gt | ge

inductive Expr
  | tt | ff
  | and (es : List Expr) | or (es : List Expr) | not (e : Expr)
  | pred (name : Name)                         -- 不透明。引数は名前に畳んである。同じ名前なら同じ Bool 変数
  | cmp (a : Value) (op : CmpOp) (b : Value)
  | ref (r : RuleId)                           -- applies r
  | unknown (key : Name)                       -- 自由な Bool 変数（評価概念・Unparsed）

inductive Effect
  | set (attr : Name) (v : Value)
  | deem (fact : Name)
  | void (target : Name)
  | sameAs (r : RuleId)                        -- 「同様とする」「みなして適用する」: r の効果を再主張
  | mark (kind : Name)                         -- Obligation / Permission / … の印（07 の eff:R）

structure Rule where
  id        : RuleId
  cond      : Expr
  effect    : Effect
  overrides : List RuleId
  source    : String                           -- Provenance.stable_id（改正 × 意味の frame 定理で使う）
  conf      : Nat                              -- 確度（0..100）。定理の前提として読める

structure Model where
  rules : List Rule
```

**世界** = 自由変数への割り当て。Z3 のモデルに当たる。

```lean
structure World where
  bools : Name → Bool               -- 述語・unknown・deem された事実・void
  ints  : Name → Int                -- 変数・属性
```

**評価器**（[07](07-verification.md) の意味論をそのまま）:

```
applies m w r  ⇔  eval (cond r)  ∧  ¬ (∃ x ∈ exceptions m r, applies m w x)
```

`exceptions` は `overrides` の逆引き。再帰は燃料ではなく**層化**で止める（M1 で決めた）: Rust が Rule を「例外 → 原則」「参照先 → 参照元」の順に並べ、
評価器はその順に applies を積む（`run : Model → World → Env`）。順序が守られているかは `Model.stratified` で検査し、上書きの循環（`lawean-space` が検出する異常）があれば層化できないのでその時点で弾く。
燃料と違って構造再帰なので `simp` で素直に展開でき、`decide` も効く。

**世界が法令の模型である**こと（Z3 で全 Rule の含意を assert していたのに当たる）:

```lean
def consistent (m : Model) (w : World) : Bool :=
  m.rules.all fun r => !(applies m w r.id) || holds w r.effect
-- holds w (set a v) = (w.ints a == evalV w v), holds w (deem f) = w.bools f, …
```

**性質**は「模型なら P」:

```lean
theorem art3_ge_30 : ∀ w, consistent model w → 360 ≤ w.ints "存続期間"
```

## 3. 証明の仕方

| 形 | 手段 | 例 |
|---|---|---|
| 有限の Bool 変数だけ | `decide`（変数を列挙した有界の ∀ に書き換える） | 第22条 × 第9条 |
| Int を含む線形算術 | `simp [applies, consistent, …]` で評価器を展開 → `split_ifs` → `omega` | 第3条・第4条の期間 |
| 補題つき | 補題を仮定に足す（07 と同じ。Provenance を持たない検証入力） | 第9条 + 「30 年未満は不利」 |
| 反例 | **Z3**（`lawean-verify`、sat モデル）。または有界域で `decide` の否定 | 第3条 = 30 年（偽） |
| 非線形（利息） | 当面 Z3 か補題。Lean は `nlinarith` 相当が core に無い | 第32条 |

`∀ w : World` は関数上の量化のまま扱える（M2 で確かめた。Z3 と同じ強さ）。ただし **Model 全体を `simp` で展開してはいけない**: 20 Rule でも `run` の展開は Rule 数の 2 乗で膨らみ、400 万 heartbeats でも終わらなかった。
代わりにメタ定理で局所化する（`SemTheorems.lean`）:

- `applies_spec`: wf な Model では `applies m w r.id = (evalE … r.cond && r.exceptions.all (!applies m w ·))`。層化された順に積むと各 Rule の値は最終表に対する `applies1` と一致する、を `run` の不変量で証明
- `consistent_rule`: `consistent m w → r ∈ m.rules → applies m w r.id → holds m w r.effect`

性質の証明は、関係する Rule だけを `consistent_rule` で取り出し、`applies_spec` で開き、`simp only [評価器の定義]` で線形算術に落として `omega`。
6 性質すべて 1 秒以内、公理は `propext` / `Classical.choice` / `Quot.sound` のみ（`M.wf` も `decide` で済む）。
反例は **Z3（または人）が見つけ、Lean は証人として検証する**（`⟨w, by native_decide, by decide⟩`）。有界域の探索を Lean でやる必要は無かった。

同じ性質を Z3 にも出す（`lawean-verify::Property` から Lean の `theorem` 文を生成）。両方が同じ結論なら実装ミスの検出になる。

## 4. 改正 × 意味（`lean/Lawean/Frame.lean`、M3 済み）

改め文が意味に与える影響を、再抽出せずに言えるところまで言う。実装した形は計画より単純で、改正単位そのものは定理に出てこない:

- 性質ごとに依存する Rule の集合 `S` を宣言する（`Properties.lean` の `S3` / `S4` / `S9`）。性質は `consistentOn m S w`（S の Rule だけの consistent）から証明する
- `closed S`（`decide`）: S の各 Rule の依存先 — 例外（`exceptions`）、条件の `ref`、`ruleValue`、効果の `sameAs` — が S の中にある
- `Sub S m`（要素ごとに `rfl`）: S の Rule が m に**同じレコードで**入っている。Rust は同じレコードの Rule を Model をまたいで同じ `def` にするので、`rfl` で決まる
- **`transfer`**: `wf m → wf m' → Sub S m → Sub S m' → closed S → (∀ w, consistentOn m S w → P w) → ∀ w, consistent m' w → P w`。
  中身は `applies_agree`（閉じた S の applies は両 Model で一致。層化順の帰納法）と `holds_congr`
- 「改正が S の Rule を変えた」は `Sub S m'` が壊れることで現れる: 本文が変わって層 2 が抽出し直した（レコードが違う）か、新しい Rule がそれを上書きするようになった（`exceptions` が違う）か。
  どちらも再検証。「〜にかかわらず」の追加は後者
- 改正単位との結びつきは Rust が計算する `modified_…`（本文を変えた・作った項の e-Gov id）で人が読む。`untouched S modified`（`decide`）は説明で、定理の前提ではない

実データ（`FrameExamples.lean`）: 令和3年法律第37号 第35条の前後（現行の手書き Model と、2021-05-19 版に存在する項だけの Model）で、
第3・4・9条の 5 性質を `transfer` で移送した。再証明なし、公理は `propext` / `Classical.choice` / `Quot.sound`。
計画では「第22条第2項の追加は第22条 × 第9条の閉包に入るので再検証」と見ていたが、実際は違った: 第2項は第1項の**後ろに加わった**だけで、
第1項（R22-1a）のレコードは変わらず、第2項の Rule（R22-2）は何も上書きしないので S9 に入らない。`touches`（anchor を含む）とは交わるが `modified` とは交わらない。
触る改正の例は架空のもの（第3条ただし書きを「二十年より長い」に書き換える）: `Sub S3` が壊れ、再検証すると 25 年の反例で破れる。第4条は同じ改正後でもそのまま移送できる。

## 5. 生成（`lawean-lean` の拡張）

- `emit_model(name, &SemanticModel) -> String`: `def sem_403AC0000000090_20260521 : Model := …`。Rule ごとに 1 `def` にして条単位のファイルに分ける（変わった条だけ再ビルド）
- `emit_property(&Property) -> String`: `theorem … : ∀ (w : W_…), consistent … → … := by …`。証明本体は最初は人が埋める
- 手書き IR（`lawean-semantic/src/examples`）から始め、層 2 の出力（[11](11-layer2.md)）も同じ経路に乗せる。`conf` と `unknown` がデータに残るので、性質は「確度 ≥ c の Rule だけを使って」「Unknown を仮定して」の形で書ける

## 6. 着手順

| # | やること | 完了の印 |
|---|---|---|
| ~~M1~~ | `Sem.lean`（型・評価器・`consistent`）と第3条の手書きデータ。性質 2 件 | 済み。≥ 30 年は `simp` + `omega`、= 30 年の反例は証人を `native_decide`。Z3 と同じ結論 |
| ~~M2~~ | `lawean-lean::sem` が手書き 8 条を層化して出力。[07](07-verification.md) の 6 性質を `Properties.lean` に | 済み。6 件が Lean と Z3 で一致。公理は `propext` / `Classical.choice` / `Quot.sound`（`ofReduceBool` は反例の証人だけ）。**層化が手書き IR のバグを 1 件検出**（§8） |
| ~~M3~~ | `Frame.lean`（`applies_agree` / `consistentOn_agree` / `transfer`）。令3-37 第35条の前後で 5 性質を移送。触る改正（架空）で `Sub` が壊れて反例 | 済み。`drafts/widen-30.txt` は第30条の手書き IR が無いので未。層 2 が出せるようになったら |
| M4 | Lean → C。`Ident.applyUnit` と `Sem.applies` を C に出して Rust / WASM から呼ぶ | `ident::apply_unit`（Rust の写し）を削除。三者一致テストが二者に減る |
| M5 | 層 2 の出力を同じ経路に（[11](11-layer2.md)）。確度と Unknown を前提に持つ性質 | 手書きでない IR で M2 の性質が通る（または Unknown が前提に出る） |

M1〜M3 は Rust 側の変更が小さい。M4 はツールチェーン（Lean の C 出力 + `leanc` / Emscripten）の作業。

## 7. 検証が見つけた IR のバグ

[07](07-verification.md) の第4条に続いて 2 件目。第26条第1項のただし書き `R26-1-proviso` は「本文で更新された契約の期間は定めが無いものとする」で、
手書き IR は `condition: Ref(R26-1)` と `overrides: [R26-1]` の**両方**を付けていた（[art-26.md](03-examples/art-26.md)「部分的な上書き」のつもり）。
07 の意味論では `applies(R26-1) = cond ∧ ¬applies(proviso)`、`applies(proviso) = applies(R26-1)` となり、cond が真の世界が存在しない（第26条第1項が成り立つ模型が無い）。
Z3 は第26条を触る性質が無かったので気づかず、Lean 出力の層化（上書き・参照グラフのトポロジカルソート）が循環として検出した。
修正: `overrides` を外す（ただし書きは例外ではなく、本文が適用された上での追加規定）。`lawean-semantic/tests/examples.rs` の「ただし書きは必ず overrides」も、「overrides か Ref のどちらか一方」に改めた。

## 8. やらないこと

- 条を Lean のコードにする（浅い埋め込み）。ADR-0016
- 手続法の状態遷移。[00](00-overview.md) の非目的のまま
- Z3 の廃止。反例探索は Z3 のまま。Lean から Z3 を呼ぶ（`lean-smt`）のは必要になったら
- 民法 140〜143 条の暦計算の Lean 化。期間は月数の Int のまま（07 と同じ制限）。日付が要る性質が出た時点で `Time` を足す
