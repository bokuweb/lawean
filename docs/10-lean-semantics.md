# 10. 法令の意味を Lean に載せる — 計画

状態: 計画（[ADR-0015](adr/0015-lean-as-semantic-backend.md)、2026-09-20）。着手順は §6。

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

法令は **Lean のデータ**、意味論は **Lean の評価器 1 つ**（深い埋め込み）。条を Lean のコードにはしない（理由は ADR-0015）。

## 2. Lean 側の型（`lean/Lawean/Sem.lean`）

Rust の `SemanticModel`（[04](04-semantic-ir.md)、`lawean-semantic::ir`）のうち、[07](07-verification.md) が SMT に落とせている部分から始める。同型を保つ。

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
  | pred (name : Name) (args : List Name)      -- 不透明。同じ名前・引数なら同じ Bool 変数
  | cmp (a : Value) (op : CmpOp) (b : Value)
  | ref (r : RuleId)                           -- applies r
  | unknown (key : Name)                       -- 自由な Bool 変数（評価概念・Unparsed）

inductive Effect
  | set (attr : Name) (v : Value)
  | deem (fact : Name)
  | void (target : Name)
  | sameAs (r : RuleId)
  | deemAndApply (fact : Name) (r : RuleId)
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

`exceptions` は `overrides` の逆引き。再帰は**燃料**（`fuel = m.rules.length`）で止め、
定理 `applies_fuel_stable`: 上書きグラフが非循環なら燃料に依らない、を証明する（循環の検出は `lawean-space::override_cycles` にあり、Lean でも `Bool` で判定する）。

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

`∀ w : World` は関数上の量化なので `decide` できない。性質が触る名前を Rust 側で列挙し（`Property::names()`）、
`World` をその名前だけの構造体（`W3 := { 存続期間 : Int, 契約で定めた期間 : Int, 定めがある : Bool }`）に特殊化した版を生成する。
汎用の `World` は評価器と meta 定理用、特殊化した `W*` は個々の性質用。ここは M1 で試して決める。

同じ性質を Z3 にも出す（`lawean-verify::Property` から Lean の `theorem` 文を生成）。両方が同じ結論なら実装ミスの検出になる。

## 4. 改正 × 意味（`lean/Lawean/Frame.lean`）

最初の目標。改め文が意味に与える影響を、再抽出せずに言えるところまで言う。

- `Rule.source` は項の stable_id。改正単位 `u : Ident.AmendUnit` が触る id は `u.touches`
- 性質 `P` が依存する Rule の閉包 `dep m P`: `P` が参照する変数・述語を効果に持つ Rule、その条件が参照する Rule、それらを上書きする Rule、の推移閉包
- **frame 定理**: `(dep m P).all (·.source ∉ u.touches) → (∀ w, consistent m w → P w) → (∀ w, consistent m' w → P w)`
  ただし `m'` は「`u.touches` に source を持つ Rule だけを差し替え、それ以外は `m` と同じ」モデル。
  差し替えた Rule が閉包内の Rule を上書きしない（`overrides` が閉包に入らない）ことも前提に入る — 「〜にかかわらず」の追加は閉包を破るので再検証になる
- 交わる場合は `m'` の該当 Rule を層 2 で抽出し直し（または人が書き）、性質を新しいデータで再証明する。`lawean-space` の「意味変化」報告がこの引き金

実データでの最初の定理: 令和3年法律第37号 第35条（第22・38・39条を触る）は第3・4・9条の性質を壊さない（frame）。
第22条第2項の追加（電磁的記録のみなし）は第22条 × 第9条の性質の閉包に入るので再検証になる — これは正しい挙動で、再検証しても成立する。

## 5. 生成（`lawean-lean` の拡張）

- `emit_model(name, &SemanticModel) -> String`: `def sem_403AC0000000090_20260521 : Model := …`。Rule ごとに 1 `def` にして条単位のファイルに分ける（変わった条だけ再ビルド）
- `emit_property(&Property) -> String`: `theorem … : ∀ (w : W_…), consistent … → … := by …`。証明本体は最初は人が埋める
- 手書き IR（`lawean-semantic/src/examples`）から始め、層 2 の出力（[11](11-layer2.md)）も同じ経路に乗せる。`conf` と `unknown` がデータに残るので、性質は「確度 ≥ c の Rule だけを使って」「Unknown を仮定して」の形で書ける

## 6. 着手順

| # | やること | 完了の印 |
|---|---|---|
| M1 | `Sem.lean`（型・評価器・`consistent`）と第3条の手書きデータ。性質 2 件（≥ 30 年の証明、= 30 年の反例を有界 `decide` で） | Z3 と同じ結論。証明の手段（§3）が決まる |
| M2 | `lawean-lean` で手書き 8 条を出力。[07](07-verification.md) の 6 性質を `Properties.lean` に。`applies_fuel_stable` | 6 件が Lean と Z3 で一致。`#print axioms` が `propext` / `ofReduceBool` のみ |
| M3 | `Frame.lean`。令3-37 第35条で frame 定理を実データに当てる。`drafts/widen-30.txt`（第30条を広げる案）で閉包に入る例 | 「触らない条の性質は保たれる」が定理に。触る案は再検証が要ると Lean が言う |
| M4 | Lean → C。`Ident.applyUnit` と `Sem.applies` を C に出して Rust / WASM から呼ぶ | `ident::apply_unit`（Rust の写し）を削除。三者一致テストが二者に減る |
| M5 | 層 2 の出力を同じ経路に（[11](11-layer2.md)）。確度と Unknown を前提に持つ性質 | 手書きでない IR で M2 の性質が通る（または Unknown が前提に出る） |

M1〜M3 は Rust 側の変更が小さい。M4 はツールチェーン（Lean の C 出力 + `leanc` / Emscripten）の作業。

## 7. やらないこと

- 条を Lean のコードにする（浅い埋め込み）。ADR-0015
- 手続法の状態遷移。[00](00-overview.md) の非目的のまま
- Z3 の廃止。反例探索は Z3 のまま。Lean から Z3 を呼ぶ（`lean-smt`）のは必要になったら
- 民法 140〜143 条の暦計算の Lean 化。期間は月数の Int のまま（07 と同じ制限）。日付が要る性質が出た時点で `Time` を足す
