import Lawean.Ident
import Lawean.Check
import Lawean.Refs

/-!
# C から呼ぶ入口（ADR-0014 / ADR-0015 の「Lean → C」）

Rust（と WASM）が呼ぶのは**この定義そのもの**をコンパイルしたもの。Rust 側に `applyUnit` の写しは要らなくなる。
受け渡しは単純な行形式（本文は空白を除いてあるのでタブ・改行を含まない）:

- リビジョン: 1 行 1 項 `id<TAB>art<TAB>text`
- 改正単位: 1 行 1 操作 `replace<TAB>id<TAB>expected<TAB>new` / `insertAfter<TAB>anchor<TAB>newId<TAB>art<TAB>text` / `delete<TAB>id` / `resolve<TAB>id<TAB>text`
- 結果: 1 行目が `ok` / `none`（対象の id が無い）、以下 1 行 1 項 `id<TAB>art<TAB>text<TAB>conflicts`（衝突は `\x1f` 区切り）

`lean_io_mark_end_initialization` の後は純粋な関数なので、スレッドから呼んでも状態を持たない。
-/

namespace Lawean.Ident

def parseRevision (s : String) : Revision :=
  { nodes := (s.splitOn "\n").filterMap fun line =>
      match line.splitOn "\t" with
      | [id, art, text] => some { id, art, text }
      | [id, art, text, _] => some { id, art, text }
      | _ => none }

def parseUnit (s : String) : Option AmendUnit :=
  (s.splitOn "\n").filter (· ≠ "") |>.mapM fun line =>
    match line.splitOn "\t" with
    | ["replace", id, expected, new] => some (.replace id expected new)
    | ["insertAfter", anchor, newId, art, text] => some (.insertAfter anchor newId art text)
    | ["delete", id] => some (.delete id)
    | ["resolve", id, text] => some (.resolve id text)
    | _ => none

def emitRevision (r : Revision) : String :=
  String.intercalate "\n" <| r.nodes.map fun n =>
    n.id ++ "\t" ++ n.art ++ "\t" ++ n.text ++ "\t" ++ String.intercalate "\x1f" n.conflicts

/-- 溶け込み。`applyUnit` そのもの -/
@[export lawean_apply_unit]
def applyUnitFfi (rev unit : String) : String :=
  match parseUnit unit with
  | none => "error\tbad unit"
  | some u =>
    match applyUnit (parseRevision rev) u with
    | none => "none"
    | some r => "ok\n" ++ emitRevision r

/-- `checkUnit`（溶け込めて、id 重複なく、衝突なし）。`checkUnit_iff` が正しさを言う -/
@[export lawean_check_unit]
def checkUnitFfi (rev unit : String) : String :=
  match parseUnit unit with
  | none => "error"
  | some u => if checkUnit (parseRevision rev) u then "true" else "false"

/-- 依存と独立（`dependsOn` / `independentUnits`） -/
@[export lawean_relation]
def relationFfi (a b : String) : String :=
  match parseUnit a, parseUnit b with
  | some ua, some ub =>
    if independentUnits ua ub then "independent"
    else if dependsOn ub ua then "b_depends_on_a"
    else if dependsOn ua ub then "a_depends_on_b"
    else "overlap"
  | _, _ => "error"

end Lawean.Ident
