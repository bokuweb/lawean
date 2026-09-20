/-!
# Revision と改正単位（docs/08-amendment.md §3, §5）

Rust の Source IR を、patch 代数の議論に必要な分だけに単純化したもの。
条 = 番号 + 項の列、項 = 番号 + 本文。参照索引はここでは持たず、定理では番号の写像だけを扱う。
-/

namespace Lawean

structure Para where
  num  : Nat
  text : String
deriving Repr, DecidableEq, Inhabited

structure Article where
  num   : Nat
  paras : List Para
deriving Repr, DecidableEq, Inhabited

/-- 法令の 1 リビジョン（本則のみ） -/
structure Revision where
  articles : List Article
deriving Repr, DecidableEq, Inhabited

/-- 改め文の操作。番号は「この操作の直前」の番号で解釈する -/
inductive Op where
  /-- 第N条[第M項]中「A」を「B」に改める -/
  | replace (art : Nat) (para : Option Nat) (from_ to : String)
  /-- 第N条第M項の次に次の一項を加える（後続の項は 1 つ繰り下がる） -/
  | insertParaAfter (art : Nat) (after : Nat) (text : String)
  /-- 第N条に次の一項を加える -/
  | appendPara (art : Nat) (text : String)
  /-- 第N条第M項を削る（後続の項は 1 つ繰り上がる） -/
  | deletePara (art : Nat) (para : Nat)
deriving Repr, DecidableEq

/-- 改正単位 = 施行期日を共有する操作の列 -/
abbrev AmendUnit := List Op

/-- 操作が触る条 -/
def Op.article : Op → Nat
  | .replace a _ _ _ => a
  | .insertParaAfter a _ _ => a
  | .appendPara a _ => a
  | .deletePara a _ => a

end Lawean
